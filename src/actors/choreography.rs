use std::path::{Path, PathBuf};

use kameo::actor::{Actor, ActorRef};
use kameo::error::Infallible;
use kameo::message::{Context, Message};
pub use meta_signal_orchestrate::{CreateRoleOrder, RetireRoleOrder};
use meta_signal_orchestrate::{
    Frame as MetaOrchestrateFrame, FrameBody as MetaOrchestrateFrameBody, MetaOrchestrateReply,
    MetaOrchestrateRequest, RefreshRepositoryIndexOrder, Retirement,
};
use signal_frame::{
    ExchangeIdentifier, ExchangeLane, LaneSequence, Reply, RequestPayload, SessionEpoch, SubReply,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

use crate::{Error, Result};

use super::trace::{ActorTrace, TraceAction, TraceNode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetaEndpoint {
    socket: PathBuf,
}

impl MetaEndpoint {
    pub fn new(socket: impl Into<PathBuf>) -> Self {
        Self {
            socket: socket.into(),
        }
    }

    pub fn as_path(&self) -> &Path {
        &self.socket
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetaFrameCodec {
    maximum_frame_bytes: usize,
}

impl MetaFrameCodec {
    pub const fn new(maximum_frame_bytes: usize) -> Self {
        Self {
            maximum_frame_bytes,
        }
    }

    async fn read_frame(&self, stream: &mut UnixStream) -> Result<MetaOrchestrateFrame> {
        let mut prefix = [0_u8; 4];
        stream.read_exact(&mut prefix).await?;
        let length = u32::from_be_bytes(prefix) as usize;
        if length > self.maximum_frame_bytes {
            return Err(Error::FrameTooLarge {
                found: length,
                limit: self.maximum_frame_bytes,
            });
        }

        let mut bytes = Vec::with_capacity(4 + length);
        bytes.extend_from_slice(&prefix);
        bytes.resize(4 + length, 0);
        stream.read_exact(&mut bytes[4..]).await?;
        Ok(MetaOrchestrateFrame::decode_length_prefixed(&bytes)?)
    }

    async fn write_frame(
        &self,
        stream: &mut UnixStream,
        frame: &MetaOrchestrateFrame,
    ) -> Result<()> {
        let bytes = frame.encode_length_prefixed()?;
        stream.write_all(&bytes).await?;
        stream.flush().await?;
        Ok(())
    }

    fn request_frame(&self, request: MetaOrchestrateRequest) -> MetaOrchestrateFrame {
        MetaOrchestrateFrame::new(MetaOrchestrateFrameBody::Request {
            exchange: exchange(),
            request: request.into_request(),
        })
    }

    fn reply_from_frame(&self, frame: MetaOrchestrateFrame) -> Result<MetaOrchestrateReply> {
        match frame.into_body() {
            MetaOrchestrateFrameBody::Reply { reply, .. } => match reply {
                Reply::Accepted { per_operation, .. } => match per_operation.into_head() {
                    SubReply::Ok(payload) => Ok(payload),
                    other => Err(Error::UnexpectedSubReply(format!("{other:?}"))),
                },
                Reply::Rejected { reason } => Err(Error::FrameReplyRejected(reason)),
            },
            _ => Err(Error::UnexpectedFrame(
                "expected meta orchestrate reply operation",
            )),
        }
    }
}

impl Default for MetaFrameCodec {
    fn default() -> Self {
        Self::new(1024 * 1024)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetaClient {
    endpoint: MetaEndpoint,
    codec: MetaFrameCodec,
}

impl MetaClient {
    pub fn new(endpoint: MetaEndpoint) -> Self {
        Self {
            endpoint,
            codec: MetaFrameCodec::default(),
        }
    }

    async fn submit(&self, request: MetaOrchestrateRequest) -> Result<MetaOrchestrateReply> {
        let mut stream = UnixStream::connect(self.endpoint.as_path()).await?;
        let frame = self.codec.request_frame(request);
        self.codec.write_frame(&mut stream, &frame).await?;
        let reply = self.codec.read_frame(&mut stream).await?;
        self.codec.reply_from_frame(reply)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrchestrateDecision {
    Create(CreateRoleOrder),
    Retire(Retirement),
    Refresh(RefreshRepositoryIndexOrder),
}

impl OrchestrateDecision {
    fn into_request(self) -> MetaOrchestrateRequest {
        match self {
            Self::Create(order) => MetaOrchestrateRequest::Create(order),
            Self::Retire(retirement) => MetaOrchestrateRequest::Retire(retirement),
            Self::Refresh(order) => MetaOrchestrateRequest::Refresh(order),
        }
    }
}

pub struct CallOrchestrate {
    pub request: MetaOrchestrateRequest,
    pub trace: ActorTrace,
}

pub struct ApplyDecision {
    pub decision: OrchestrateDecision,
    pub trace: ActorTrace,
}

#[derive(Debug, Clone, PartialEq, Eq, kameo::Reply)]
pub struct ApplicationResult {
    reply: Option<MetaOrchestrateReply>,
    error: Option<String>,
    trace: ActorTrace,
}

impl ApplicationResult {
    fn replied(reply: MetaOrchestrateReply, trace: ActorTrace) -> Self {
        Self {
            reply: Some(reply),
            error: None,
            trace,
        }
    }

    fn failed(error: impl Into<String>, trace: ActorTrace) -> Self {
        Self {
            reply: None,
            error: Some(error.into()),
            trace,
        }
    }

    pub fn reply(&self) -> Option<&MetaOrchestrateReply> {
        self.reply.as_ref()
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn trace(&self) -> &ActorTrace {
        &self.trace
    }
}

#[derive(Clone)]
pub struct CallerArguments {
    pub endpoint: MetaEndpoint,
}

impl CallerArguments {
    pub fn new(endpoint: MetaEndpoint) -> Self {
        Self { endpoint }
    }
}

pub struct MindOrchestrateCaller {
    client: MetaClient,
}

impl MindOrchestrateCaller {
    fn new(client: MetaClient) -> Self {
        Self { client }
    }

    async fn call(
        &self,
        request: MetaOrchestrateRequest,
        mut trace: ActorTrace,
    ) -> ApplicationResult {
        trace.record(
            TraceNode::MIND_ORCHESTRATE_CALLER,
            TraceAction::MessageReceived,
        );
        trace.record(
            TraceNode::MIND_ORCHESTRATE_CALLER,
            TraceAction::WriteIntentSent,
        );
        match self.client.submit(request).await {
            Ok(reply) => {
                trace.record(
                    TraceNode::MIND_ORCHESTRATE_CALLER,
                    TraceAction::MessageReplied,
                );
                ApplicationResult::replied(reply, trace)
            }
            Err(error) => ApplicationResult::failed(error.to_string(), trace),
        }
    }
}

impl Actor for MindOrchestrateCaller {
    type Args = CallerArguments;
    type Error = Infallible;

    async fn on_start(
        arguments: Self::Args,
        _actor_reference: ActorRef<Self>,
    ) -> std::result::Result<Self, Self::Error> {
        Ok(Self::new(MetaClient::new(arguments.endpoint)))
    }
}

impl Message<CallOrchestrate> for MindOrchestrateCaller {
    type Reply = ApplicationResult;

    async fn handle(
        &mut self,
        message: CallOrchestrate,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.call(message.request, message.trace).await
    }
}

#[derive(Clone)]
pub struct AdjudicatorArguments {
    pub caller: ActorRef<MindOrchestrateCaller>,
}

impl AdjudicatorArguments {
    pub fn new(caller: ActorRef<MindOrchestrateCaller>) -> Self {
        Self { caller }
    }
}

pub struct ChoreographyAdjudicator {
    caller: ActorRef<MindOrchestrateCaller>,
}

impl ChoreographyAdjudicator {
    fn new(caller: ActorRef<MindOrchestrateCaller>) -> Self {
        Self { caller }
    }

    async fn apply_decision(
        &self,
        decision: OrchestrateDecision,
        mut trace: ActorTrace,
    ) -> ApplicationResult {
        trace.record(
            TraceNode::CHOREOGRAPHY_ADJUDICATOR,
            TraceAction::MessageReceived,
        );
        let mut result = self
            .caller
            .ask(CallOrchestrate {
                request: decision.into_request(),
                trace,
            })
            .await
            .unwrap_or_else(|error| {
                ApplicationResult::failed(error.to_string(), ActorTrace::new())
            });
        result.trace.record(
            TraceNode::CHOREOGRAPHY_ADJUDICATOR,
            TraceAction::MessageReplied,
        );
        result
    }
}

impl Actor for ChoreographyAdjudicator {
    type Args = AdjudicatorArguments;
    type Error = Infallible;

    async fn on_start(
        arguments: Self::Args,
        _actor_reference: ActorRef<Self>,
    ) -> std::result::Result<Self, Self::Error> {
        Ok(Self::new(arguments.caller))
    }
}

impl Message<ApplyDecision> for ChoreographyAdjudicator {
    type Reply = ApplicationResult;

    async fn handle(
        &mut self,
        message: ApplyDecision,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.apply_decision(message.decision, message.trace).await
    }
}

fn exchange() -> ExchangeIdentifier {
    ExchangeIdentifier::new(
        SessionEpoch::new(0),
        ExchangeLane::Connector,
        LaneSequence::first(),
    )
}
