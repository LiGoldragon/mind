use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use meta_signal_mind::{
    Frame as MetaMindFrame, FrameBody as MetaMindFrameBody, MetaMindReply,
    Operation as MetaMindOperation, Request as MetaMindRequest, RequestUnimplemented,
    UnimplementedReason,
};
use nota_next::{NotaEncode, NotaSource};
use signal_frame::{
    ExchangeIdentifier, ExchangeLane, LaneSequence, NonEmpty, Reply, RequestPayload, SessionEpoch,
    SubReply,
};
use tokio::io::AsyncWriteExt;
use tokio::net::UnixStream;
use triad_runtime::{ComponentArgument, ComponentCommand};

use crate::frame_bytes::LengthPrefixedFrameBytes;
use crate::{Error, Result};

const DEFAULT_META_MIND_SOCKET: &str = "/tmp/meta-mind.sock";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetaMindEndpoint {
    socket: PathBuf,
}

impl MetaMindEndpoint {
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
pub struct MetaMindFrameCodec {
    maximum_frame_bytes: usize,
}

#[derive(Debug)]
pub enum MetaMindFrameDecode {
    NotMeta,
    UnexpectedFrame(&'static str),
    UnexpectedSubReply(String),
}

impl MetaMindFrameCodec {
    pub const fn new(maximum_frame_bytes: usize) -> Self {
        Self {
            maximum_frame_bytes,
        }
    }

    fn synthetic_exchange(&self) -> ExchangeIdentifier {
        let _codec_configuration = self.maximum_frame_bytes;
        ExchangeIdentifier::new(
            SessionEpoch::new(0),
            ExchangeLane::Connector,
            LaneSequence::first(),
        )
    }

    pub async fn read_frame_bytes(
        &self,
        stream: &mut UnixStream,
    ) -> Result<LengthPrefixedFrameBytes> {
        LengthPrefixedFrameBytes::read_from_stream(stream, self.maximum_frame_bytes).await
    }

    pub async fn read_frame(&self, stream: &mut UnixStream) -> Result<MetaMindFrame> {
        let bytes = self.read_frame_bytes(stream).await?;
        Ok(MetaMindFrame::decode_length_prefixed(bytes.as_slice())?)
    }

    pub async fn write_frame(&self, stream: &mut UnixStream, frame: &MetaMindFrame) -> Result<()> {
        let bytes = frame.encode_length_prefixed()?;
        stream.write_all(bytes.as_slice()).await?;
        stream.flush().await?;
        Ok(())
    }

    pub fn request_frame(&self, operation: MetaMindOperation) -> MetaMindFrame {
        MetaMindFrame::new(MetaMindFrameBody::Request {
            exchange: self.synthetic_exchange(),
            request: operation.into_request(),
        })
    }

    pub fn reply_frame(&self, exchange: ExchangeIdentifier, reply: MetaMindReply) -> MetaMindFrame {
        MetaMindFrame::new(MetaMindFrameBody::Reply {
            exchange,
            reply: Reply::committed(NonEmpty::single(SubReply::Ok(reply))),
        })
    }

    pub fn decode_request_frame(
        &self,
        bytes: &LengthPrefixedFrameBytes,
    ) -> std::result::Result<(ExchangeIdentifier, MetaMindOperation), MetaMindFrameDecode> {
        let frame = MetaMindFrame::decode_length_prefixed(bytes.as_slice())
            .map_err(|_error| MetaMindFrameDecode::NotMeta)?;
        match frame.into_body() {
            MetaMindFrameBody::Request { exchange, request } => {
                let mut operations = request.payloads.into_vec();
                if operations.len() != 1 {
                    return Err(MetaMindFrameDecode::UnexpectedFrame(
                        "expected one meta mind operation",
                    ));
                }
                Ok((exchange, operations.remove(0)))
            }
            _ => Err(MetaMindFrameDecode::UnexpectedFrame(
                "expected meta mind request operation",
            )),
        }
    }

    pub async fn write_unimplemented_reply(
        &self,
        stream: &mut UnixStream,
        exchange: ExchangeIdentifier,
        operation: MetaMindOperation,
    ) -> Result<MetaMindReply> {
        let reply = MetaMindReply::RequestUnimplemented(RequestUnimplemented {
            operation: operation.kind(),
            reason: UnimplementedReason::NotBuiltYet,
        });
        let frame = self.reply_frame(exchange, reply.clone());
        self.write_frame(stream, &frame).await?;
        Ok(reply)
    }

    pub fn reply_from_frame(&self, frame: MetaMindFrame) -> Result<MetaMindReply> {
        match frame.into_body() {
            MetaMindFrameBody::Reply { reply, .. } => match reply {
                Reply::Accepted { per_operation, .. } => match per_operation.into_head() {
                    SubReply::Ok(payload) => Ok(payload),
                    other => Err(Error::UnexpectedSubReply(format!("{other:?}"))),
                },
                Reply::Rejected { reason } => Err(Error::FrameReplyRejected(reason)),
            },
            _ => Err(Error::UnexpectedFrame("expected meta mind reply operation")),
        }
    }
}

impl Default for MetaMindFrameCodec {
    fn default() -> Self {
        Self::new(1024 * 1024)
    }
}

pub struct MetaMindClient {
    endpoint: MetaMindEndpoint,
    codec: MetaMindFrameCodec,
}

impl MetaMindClient {
    pub fn new(endpoint: MetaMindEndpoint) -> Self {
        Self {
            endpoint,
            codec: MetaMindFrameCodec::default(),
        }
    }

    pub async fn submit(&self, operation: MetaMindOperation) -> Result<MetaMindReply> {
        let mut stream = UnixStream::connect(self.endpoint.as_path()).await?;
        let frame = self.codec.request_frame(operation);
        self.codec.write_frame(&mut stream, &frame).await?;
        let reply = self.codec.read_frame(&mut stream).await?;
        self.codec.reply_from_frame(reply)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetaMindCommand {
    command: ComponentCommand,
    environment: MetaMindCommandEnvironment,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetaMindCommandEnvironment {
    socket: String,
}

impl MetaMindCommand {
    pub fn from_env() -> Self {
        Self {
            command: ComponentCommand::from_environment(),
            environment: MetaMindCommandEnvironment::from_process(),
        }
    }

    pub fn from_arguments<Arguments, Argument>(arguments: Arguments) -> Self
    where
        Arguments: IntoIterator<Item = Argument>,
        Argument: Into<String>,
    {
        Self::from_arguments_with_environment(arguments, MetaMindCommandEnvironment::from_process())
    }

    pub fn from_arguments_with_environment<Arguments, Argument>(
        arguments: Arguments,
        environment: MetaMindCommandEnvironment,
    ) -> Self
    where
        Arguments: IntoIterator<Item = Argument>,
        Argument: Into<String>,
    {
        Self {
            command: ComponentCommand::from_arguments(arguments),
            environment,
        }
    }

    pub async fn run(self, mut output: impl Write) -> Result<()> {
        let operation = MetaMindOperationSource::from_command(self.command)?.into_operation()?;
        let reply = MetaMindClient::new(self.environment.endpoint())
            .submit(operation)
            .await?;
        writeln!(output, "{}", reply.to_nota())?;
        Ok(())
    }
}

impl MetaMindCommandEnvironment {
    pub fn new(socket: impl Into<String>) -> Self {
        Self {
            socket: socket.into(),
        }
    }

    fn from_process() -> Self {
        Self {
            socket: std::env::var("MIND_META_SOCKET")
                .unwrap_or_else(|_| String::from(DEFAULT_META_MIND_SOCKET)),
        }
    }

    fn endpoint(&self) -> MetaMindEndpoint {
        MetaMindEndpoint::new(PathBuf::from(&self.socket))
    }
}

struct MetaMindOperationSource {
    text: String,
}

impl MetaMindOperationSource {
    fn from_command(command: ComponentCommand) -> Result<Self> {
        match command.nota_argument()? {
            ComponentArgument::InlineNota(argument) => Ok(Self::new(argument.into_string())),
            ComponentArgument::NotaFile(file) => {
                let path = file.into_path();
                fs::read_to_string(&path)
                    .map(Self::new)
                    .map_err(|source| Error::ReadNotaFile { path, source })
            }
            ComponentArgument::SignalFile(file) => {
                let path = file.into_path();
                fs::read_to_string(&path)
                    .map(Self::new)
                    .map_err(|source| Error::ReadNotaFile { path, source })
            }
        }
    }

    fn new(text: String) -> Self {
        Self { text }
    }

    fn into_operation(self) -> Result<MetaMindOperation> {
        let request = NotaSource::new(&self.text).parse::<MetaMindRequest>()?;
        Ok(request.payloads().head().clone())
    }
}
