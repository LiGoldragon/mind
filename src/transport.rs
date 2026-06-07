use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use signal_frame::{
    ExchangeIdentifier, ExchangeLane, LaneSequence, NonEmpty, Reply, RequestPayload, SessionEpoch,
    SubReply,
};
use signal_mind::{ActorName, MindFrame, MindFrameBody, MindReply, MindRequest};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};

use crate::{
    Error, MindEnvelope, MindRoot, MindRootArguments, Result, StoreLocation, SubmitEnvelope,
    supervision::{SupervisionHandle, SupervisionListener, SupervisionProfile},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MindDaemonEndpoint {
    socket: PathBuf,
}

impl MindDaemonEndpoint {
    pub fn new(socket: impl Into<PathBuf>) -> Self {
        Self {
            socket: socket.into(),
        }
    }

    pub fn as_path(&self) -> &Path {
        &self.socket
    }

    fn bind_listener(&self, mode: Option<MindSocketMode>) -> Result<UnixListener> {
        match fs::remove_file(&self.socket) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        let listener = UnixListener::bind(&self.socket)?;
        if let Some(mode) = mode {
            fs::set_permissions(&self.socket, fs::Permissions::from_mode(mode.as_octal()))?;
        }
        Ok(listener)
    }

    fn remove_socket(&self) -> Result<()> {
        match fs::remove_file(&self.socket) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MindSocketMode(u32);

impl MindSocketMode {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub fn from_environment() -> Option<Self> {
        std::env::var("PERSONA_SOCKET_MODE")
            .ok()
            .and_then(|value| u32::from_str_radix(value.as_str(), 8).ok())
            .map(Self::new)
    }

    pub const fn as_octal(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MindFrameCodec {
    maximum_frame_bytes: usize,
}

impl MindFrameCodec {
    pub const fn new(maximum_frame_bytes: usize) -> Self {
        Self {
            maximum_frame_bytes,
        }
    }

    /// Synchronous request/reply transport uses a degenerate exchange
    /// identifier until async multiplexing owns real lane tracking.
    fn synthetic_exchange(&self) -> ExchangeIdentifier {
        let _codec_configuration = self.maximum_frame_bytes;
        ExchangeIdentifier::new(
            SessionEpoch::new(0),
            ExchangeLane::Connector,
            LaneSequence::first(),
        )
    }

    pub async fn read_frame(&self, stream: &mut UnixStream) -> Result<MindFrame> {
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
        Ok(MindFrame::decode_length_prefixed(&bytes)?)
    }

    pub async fn write_frame(&self, stream: &mut UnixStream, frame: &MindFrame) -> Result<()> {
        let bytes = frame.encode_length_prefixed()?;
        stream.write_all(&bytes).await?;
        stream.flush().await?;
        Ok(())
    }

    pub fn request_frame(&self, actor: &ActorName, request: MindRequest) -> MindFrame {
        let _ingress_scaffold = actor;
        MindFrame::new(MindFrameBody::Request {
            exchange: self.synthetic_exchange(),
            request: request.into_request(),
        })
    }

    pub fn reply_frame(&self, reply: MindReply) -> MindFrame {
        MindFrame::new(MindFrameBody::Reply {
            exchange: self.synthetic_exchange(),
            reply: Reply::committed(NonEmpty::single(SubReply::Ok(reply))),
        })
    }

    pub fn request_from_frame(&self, frame: MindFrame) -> Result<MindRequest> {
        match frame.into_body() {
            MindFrameBody::Request { request, .. } => Ok(request.payloads.into_head()),
            _ => Err(Error::UnexpectedFrame("expected mind request operation")),
        }
    }

    pub fn reply_from_frame(&self, frame: MindFrame) -> Result<MindReply> {
        match frame.into_body() {
            MindFrameBody::Reply { reply, .. } => match reply {
                Reply::Accepted { per_operation, .. } => match per_operation.into_head() {
                    SubReply::Ok(payload) => Ok(payload),
                    other => Err(Error::UnexpectedSubReply(format!("{other:?}"))),
                },
                Reply::Rejected { reason } => Err(Error::FrameReplyRejected(reason)),
            },
            _ => Err(Error::UnexpectedFrame("expected mind reply operation")),
        }
    }
}

impl Default for MindFrameCodec {
    fn default() -> Self {
        Self::new(1024 * 1024)
    }
}

pub struct MindClient {
    endpoint: MindDaemonEndpoint,
    actor: ActorName,
    codec: MindFrameCodec,
}

impl MindClient {
    pub fn new(endpoint: MindDaemonEndpoint, actor: ActorName) -> Self {
        Self {
            endpoint,
            actor,
            codec: MindFrameCodec::default(),
        }
    }

    pub async fn submit(&self, request: MindRequest) -> Result<MindReply> {
        let mut stream = UnixStream::connect(self.endpoint.as_path()).await?;
        let frame = self.codec.request_frame(&self.actor, request);
        self.codec.write_frame(&mut stream, &frame).await?;
        let reply = self.codec.read_frame(&mut stream).await?;
        self.codec.reply_from_frame(reply)
    }
}

pub struct MindDaemon {
    endpoint: MindDaemonEndpoint,
    store: StoreLocation,
    socket_mode: Option<MindSocketMode>,
    supervision: Option<SupervisionListener>,
    codec: MindFrameCodec,
}

impl MindDaemon {
    pub fn new(endpoint: MindDaemonEndpoint, store: StoreLocation) -> Self {
        Self {
            endpoint,
            store,
            socket_mode: MindSocketMode::from_environment(),
            supervision: SupervisionListener::from_environment(SupervisionProfile::mind()),
            codec: MindFrameCodec::default(),
        }
    }

    pub fn with_socket_mode(mut self, socket_mode: MindSocketMode) -> Self {
        self.socket_mode = Some(socket_mode);
        self
    }

    pub fn with_supervision_listener(mut self, supervision: SupervisionListener) -> Self {
        self.supervision = Some(supervision);
        self
    }

    pub async fn bind(self) -> Result<BoundMindDaemon> {
        let listener = self.endpoint.bind_listener(self.socket_mode)?;
        let root = MindRoot::start(MindRootArguments::new(self.store)).await?;
        let supervision = match self.supervision {
            Some(listener) => Some(listener.spawn(root.clone())?),
            None => None,
        };
        Ok(BoundMindDaemon {
            endpoint: self.endpoint,
            codec: self.codec,
            listener,
            root,
            _supervision: supervision,
        })
    }
}

pub struct BoundMindDaemon {
    endpoint: MindDaemonEndpoint,
    codec: MindFrameCodec,
    listener: UnixListener,
    root: crate::ActorRef<MindRoot>,
    _supervision: Option<SupervisionHandle>,
}

impl BoundMindDaemon {
    pub fn endpoint(&self) -> &MindDaemonEndpoint {
        &self.endpoint
    }

    pub async fn serve_one(self) -> Result<MindReply> {
        let reply = self.serve_next().await;
        MindRoot::stop(self.root).await?;
        self.endpoint.remove_socket()?;
        reply
    }

    pub async fn serve_count(self, count: usize) -> Result<Vec<MindReply>> {
        let mut replies = Vec::with_capacity(count);
        let result = async {
            for _ in 0..count {
                replies.push(self.serve_next().await?);
            }
            Ok(replies)
        }
        .await;
        MindRoot::stop(self.root).await?;
        self.endpoint.remove_socket()?;
        result
    }

    pub async fn serve_forever(self) -> Result<()> {
        loop {
            if let Err(error) = self.serve_next().await {
                eprintln!("mind daemon client error: {error}");
            }
        }
    }

    async fn serve_next(&self) -> Result<MindReply> {
        let (mut stream, _address) = self.listener.accept().await?;
        let frame = self.codec.read_frame(&mut stream).await?;
        let actor = ActorName::new("operator");
        let request = self.codec.request_from_frame(frame)?;
        let envelope = MindEnvelope::new(actor, request);
        let root_reply = self
            .root
            .ask(SubmitEnvelope { envelope })
            .await
            .map_err(|error| Error::ActorCall(error.to_string()))?;
        let reply = root_reply
            .reply()
            .cloned()
            .ok_or(Error::UnexpectedFrame("mind root returned no reply"))?;
        let frame = self.codec.reply_frame(reply.clone());
        self.codec.write_frame(&mut stream, &frame).await?;
        Ok(reply)
    }
}
