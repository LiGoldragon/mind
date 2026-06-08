use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use kameo::actor::{Actor, ActorRef};
use kameo::error::Infallible;
use kameo::message::{Context, Message};
use signal_engine_management::{
    ComponentHealth, ComponentHealthReport, ComponentIdentity, ComponentKind, ComponentName,
    ComponentReady, EngineManagementProtocolVersion, Frame as SupervisionFrame, FrameBody,
    Operation as SupervisionRequest, Presence, Query as SupervisionQuery,
    Reply as SupervisionReply, StopAcknowledgement,
};
use signal_frame::{
    ExchangeIdentifier, ExchangeLane, LaneSequence, NonEmpty, Reply, Request, SessionEpoch,
    SubReply,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::task::JoinHandle;

use crate::{MindRoot, Result};

/// Same wave-1 placeholder as [`crate::transport::synthetic_exchange`]
/// — supervision uses synchronous request/reply, so the
/// `ExchangeIdentifier` is degenerate until handshake/lane tracking
/// lands.
fn supervision_synthetic_exchange() -> ExchangeIdentifier {
    ExchangeIdentifier::new(
        SessionEpoch::new(0),
        ExchangeLane::Connector,
        LaneSequence::first(),
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupervisionProfile {
    name: ComponentName,
    kind: ComponentKind,
    health: ComponentHealth,
}

impl SupervisionProfile {
    pub fn mind() -> Self {
        Self {
            name: ComponentName::new("mind"),
            kind: ComponentKind::Mind,
            health: ComponentHealth::Running,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SupervisionSocketMode(u32);

impl SupervisionSocketMode {
    pub const fn from_octal(value: u32) -> Self {
        Self(value)
    }

    pub const fn as_octal(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupervisionListener {
    socket: PathBuf,
    mode: SupervisionSocketMode,
}

impl SupervisionListener {
    pub fn new(socket: impl Into<PathBuf>, mode: SupervisionSocketMode) -> Self {
        Self {
            socket: socket.into(),
            mode,
        }
    }

    pub fn from_environment(_profile: SupervisionProfile) -> Option<Self> {
        let socket = std::env::var_os("PERSONA_SUPERVISION_SOCKET_PATH")?;
        let mode = std::env::var("PERSONA_SUPERVISION_SOCKET_MODE")
            .ok()
            .and_then(|value| u32::from_str_radix(value.as_str(), 8).ok())
            .map(SupervisionSocketMode::from_octal)
            .unwrap_or_else(|| SupervisionSocketMode::from_octal(0o600));
        Some(Self::new(PathBuf::from(socket), mode))
    }

    pub fn socket(&self) -> &Path {
        self.socket.as_path()
    }

    pub fn spawn(self, root: ActorRef<MindRoot>) -> Result<SupervisionHandle> {
        if let Some(parent) = self.socket.parent() {
            fs::create_dir_all(parent)?;
        }
        match fs::remove_file(&self.socket) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        let listener = UnixListener::bind(&self.socket)?;
        fs::set_permissions(
            &self.socket,
            fs::Permissions::from_mode(self.mode.as_octal()),
        )?;
        let socket = self.socket;
        let task = tokio::spawn(async move {
            SupervisionServer::new(listener, root).run().await;
        });
        Ok(SupervisionHandle { socket, task })
    }
}

pub struct SupervisionHandle {
    socket: PathBuf,
    task: JoinHandle<()>,
}

impl Drop for SupervisionHandle {
    fn drop(&mut self) {
        self.task.abort();
        let _ = fs::remove_file(&self.socket);
    }
}

#[derive(Clone)]
pub struct SupervisionArguments {
    profile: SupervisionProfile,
}

impl SupervisionArguments {
    pub fn new(profile: SupervisionProfile) -> Self {
        Self { profile }
    }
}

pub struct SupervisionPhase {
    profile: SupervisionProfile,
    request_count: u64,
}

impl SupervisionPhase {
    fn new(profile: SupervisionProfile) -> Self {
        Self {
            profile,
            request_count: 0,
        }
    }

    fn reply(&mut self, request: SupervisionRequest) -> SupervisionReply {
        self.request_count = self.request_count.saturating_add(1);
        match request {
            SupervisionRequest::Announce(Presence { .. }) => {
                SupervisionReply::Identified(ComponentIdentity {
                    name: self.profile.name.clone(),
                    kind: self.profile.kind,
                    engine_management_protocol_version: EngineManagementProtocolVersion::new(1),
                    last_fatal_startup_error: None,
                })
            }
            SupervisionRequest::Query(SupervisionQuery::ReadinessStatus(_)) => {
                SupervisionReply::Ready(ComponentReady {
                    component_started_at: None,
                })
            }
            SupervisionRequest::Query(SupervisionQuery::HealthStatus(_)) => {
                SupervisionReply::HealthReport(ComponentHealthReport {
                    health: self.profile.health,
                })
            }
            SupervisionRequest::Stop(_) => {
                SupervisionReply::StopAcknowledged(StopAcknowledgement {
                    drain_completed_at: None,
                })
            }
        }
    }
}

impl Actor for SupervisionPhase {
    type Args = SupervisionArguments;
    type Error = Infallible;

    async fn on_start(
        arguments: Self::Args,
        _actor_reference: ActorRef<Self>,
    ) -> std::result::Result<Self, Self::Error> {
        Ok(Self::new(arguments.profile))
    }
}

#[derive(Debug, Clone)]
pub struct HandleSupervisionRequest {
    request: SupervisionRequest,
}

impl HandleSupervisionRequest {
    pub fn new(request: SupervisionRequest) -> Self {
        Self { request }
    }
}

#[derive(Debug, Clone, kameo::Reply)]
pub struct SupervisionPhaseReply {
    reply: SupervisionReply,
}

impl SupervisionPhaseReply {
    fn new(reply: SupervisionReply) -> Self {
        Self { reply }
    }

    pub fn unavailable() -> Self {
        Self {
            reply: SupervisionReply::HealthReport(ComponentHealthReport {
                health: ComponentHealth::Failed,
            }),
        }
    }

    pub(crate) fn into_reply(self) -> SupervisionReply {
        self.reply
    }
}

impl Message<HandleSupervisionRequest> for SupervisionPhase {
    type Reply = SupervisionPhaseReply;

    async fn handle(
        &mut self,
        message: HandleSupervisionRequest,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        SupervisionPhaseReply::new(self.reply(message.request))
    }
}

struct SupervisionServer {
    listener: UnixListener,
    root: ActorRef<MindRoot>,
    codec: SupervisionFrameCodec,
}

impl SupervisionServer {
    fn new(listener: UnixListener, root: ActorRef<MindRoot>) -> Self {
        Self {
            listener,
            root,
            codec: SupervisionFrameCodec::new(1024 * 1024),
        }
    }

    async fn run(self) {
        loop {
            let Ok((stream, _address)) = self.listener.accept().await else {
                continue;
            };
            let root = self.root.clone();
            let codec = self.codec;
            tokio::spawn(async move {
                let _ = Self::serve_connection(root, codec, stream).await;
            });
        }
    }

    async fn serve_connection(
        root: ActorRef<MindRoot>,
        codec: SupervisionFrameCodec,
        mut stream: UnixStream,
    ) -> Result<()> {
        codec.serve_connection(&mut stream, &root).await
    }
}

#[derive(Clone, Copy)]
pub struct SupervisionFrameCodec {
    maximum_frame_bytes: usize,
}

impl SupervisionFrameCodec {
    pub const fn new(maximum_frame_bytes: usize) -> Self {
        Self {
            maximum_frame_bytes,
        }
    }

    /// Drive the owner-only engine-management protocol over one accepted
    /// stream: read each supervision request, route it through the mind root
    /// actor, and write the typed reply until the peer closes the connection.
    /// The daemon shell's meta hook and the in-process supervision server share
    /// this one body.
    pub async fn serve_connection(
        &self,
        stream: &mut UnixStream,
        root: &ActorRef<MindRoot>,
    ) -> Result<()> {
        while let Ok((exchange, request)) = self.read_request(stream).await {
            let reply = root
                .ask(HandleSupervisionRequest::new(request))
                .await
                .map_err(|error| crate::Error::ActorCall(error.to_string()))?
                .into_reply();
            self.write_reply(stream, exchange, reply).await?;
        }
        Ok(())
    }

    pub async fn read_reply(&self, stream: &mut UnixStream) -> Result<SupervisionReply> {
        let frame = self.read_frame(stream).await?;
        match frame.into_body() {
            FrameBody::Reply { reply, .. } => match reply {
                Reply::Accepted { per_operation, .. } => match per_operation.into_head() {
                    SubReply::Ok(payload) => Ok(payload),
                    other => Err(crate::Error::UnexpectedSubReply(format!("{other:?}"))),
                },
                Reply::Rejected { reason } => Err(crate::Error::FrameReplyRejected(reason)),
            },
            _ => Err(crate::Error::UnexpectedFrame(
                "expected supervision reply operation",
            )),
        }
    }

    pub async fn write_request(
        &self,
        stream: &mut UnixStream,
        request: SupervisionRequest,
    ) -> Result<()> {
        let frame = SupervisionFrame::new(FrameBody::Request {
            exchange: supervision_synthetic_exchange(),
            request: Request::from_payload(request),
        });
        self.write_frame(stream, &frame).await
    }

    async fn read_request(
        &self,
        stream: &mut UnixStream,
    ) -> Result<(ExchangeIdentifier, SupervisionRequest)> {
        let frame = self.read_frame(stream).await?;
        match frame.into_body() {
            FrameBody::Request { exchange, request } => {
                let mut operations = request.payloads.into_vec();
                if operations.len() != 1 {
                    return Err(crate::Error::UnexpectedFrame(
                        "expected one supervision operation",
                    ));
                }
                Ok((exchange, operations.remove(0)))
            }
            _ => Err(crate::Error::UnexpectedFrame(
                "expected supervision request operation",
            )),
        }
    }

    async fn write_reply(
        &self,
        stream: &mut UnixStream,
        exchange: ExchangeIdentifier,
        reply: SupervisionReply,
    ) -> Result<()> {
        let frame = SupervisionFrame::new(FrameBody::Reply {
            exchange,
            reply: Reply::committed(NonEmpty::single(SubReply::Ok(reply))),
        });
        self.write_frame(stream, &frame).await
    }

    async fn read_frame(&self, stream: &mut UnixStream) -> Result<SupervisionFrame> {
        let mut prefix = [0_u8; 4];
        stream.read_exact(&mut prefix).await?;
        let length = u32::from_be_bytes(prefix) as usize;
        if length > self.maximum_frame_bytes {
            return Err(crate::Error::FrameTooLarge {
                found: length,
                limit: self.maximum_frame_bytes,
            });
        }

        let mut bytes = Vec::with_capacity(4 + length);
        bytes.extend_from_slice(&prefix);
        bytes.resize(4 + length, 0);
        stream.read_exact(&mut bytes[4..]).await?;
        Ok(SupervisionFrame::decode_length_prefixed(&bytes)?)
    }

    async fn write_frame(&self, stream: &mut UnixStream, frame: &SupervisionFrame) -> Result<()> {
        let bytes = frame.encode_length_prefixed()?;
        stream.write_all(bytes.as_slice()).await?;
        stream.flush().await?;
        Ok(())
    }
}
