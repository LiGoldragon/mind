use std::path::PathBuf;

use signal_mind::{RelationKindMismatch, ThoughtKind};
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("actor call: {0}")]
    ActorCall(String),

    #[error("actor spawn: {0}")]
    ActorSpawn(String),

    #[error("actor join: {0}")]
    ActorJoin(String),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("system time: {0}")]
    SystemTime(#[from] std::time::SystemTimeError),

    #[error("component argument error: {0}")]
    Argument(#[from] triad_runtime::ArgumentError),

    #[error("failed to read NOTA file {}: {source}", path.display())]
    ReadNotaFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("signal-frame: {0}")]
    SignalFrameLayer(#[from] signal_frame::FrameError),

    #[error("frame-layer request rejected: {0}")]
    FrameRequestRejected(signal_frame::RequestRejectionReason),

    #[error("unexpected sub-reply for single-op request: {0}")]
    UnexpectedSubReply(String),

    #[error("frame-layer reply rejected before execution: {0}")]
    FrameReplyRejected(signal_frame::RequestRejectionReason),

    #[error("signal persona mind: {0}")]
    SignalPersonaMind(#[from] signal_mind::Error),

    #[error("nota: {0}")]
    Nota(#[from] nota_next::NotaDecodeError),

    #[error("storage kernel: {0}")]
    StorageKernel(#[from] sema_engine::StorageKernelError),

    #[error("sema engine: {0}")]
    SemaEngine(#[from] sema_engine::Error),

    #[error("unexpected signal frame: {0}")]
    UnexpectedFrame(&'static str),

    #[error("frame is larger than configured limit: {found} > {limit}")]
    FrameTooLarge { found: usize, limit: usize },

    #[error("missing required --store path")]
    MissingStorePath,

    #[error("mind graph thought kind mismatch: declared {declared:?}, body {actual:?}")]
    MindGraphThoughtKindMismatch {
        declared: ThoughtKind,
        actual: ThoughtKind,
    },

    #[error("mind graph relation references missing thought: {record}")]
    MindGraphMissingRecord { record: String },

    #[error("mind graph relation kind mismatch: {mismatch:?}")]
    MindGraphRelationKindMismatch { mismatch: RelationKindMismatch },
}
