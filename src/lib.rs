pub mod actors;
pub mod command;
pub mod configuration;
pub mod daemon;
pub mod envelope;
pub mod error;
pub(crate) mod frame_bytes;
pub mod graph;
pub mod memory;
pub mod meta;
pub mod supervision;
pub mod tables;
pub mod text;
pub mod transport;

pub mod schema {
    #[rustfmt::skip]
    pub mod signal;
    #[rustfmt::skip]
    pub mod sema;
    #[rustfmt::skip]
    pub mod nexus;
    #[rustfmt::skip]
    pub mod daemon;
}

pub use actors::root::{
    Arguments as MindRootArguments, MindRoot, RootReply as MindRootReply, SubmitEnvelope,
};
pub use command::{MindCommand, MindCommandEnvironment};
pub use configuration::{ConfigurationError, MindDaemonConfiguration};
pub use daemon::{MindDaemonError, MindEngine, MindProcessDaemon};
pub use envelope::MindEnvelope;
pub use error::{Error, Result};
pub use kameo::actor::ActorRef;
pub(crate) use memory::MemoryGraph;
pub use memory::{MemoryState, StoreLocation};
pub use meta::{
    MetaMindClient, MetaMindCommand, MetaMindCommandEnvironment, MetaMindEndpoint,
    MetaMindFrameCodec,
};
pub use supervision::{
    SupervisionFrameCodec, SupervisionListener, SupervisionProfile, SupervisionSocketMode,
};
pub use tables::MindTables;
pub use text::{MindTextReply, MindTextRequest};
pub use transport::{MindClient, MindDaemon, MindDaemonEndpoint, MindFrameCodec, MindSocketMode};
