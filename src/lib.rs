#[allow(unused_extern_crates)]
extern crate nota_next as nota;

pub mod actors;
pub mod command;
pub mod configuration;
pub mod daemon;
pub mod envelope;
pub mod error;
#[cfg(feature = "eval-fixture-prepopulation")]
pub mod eval_fixture {
    use signal_mind::AcceptedKnowledge;

    use crate::tables::{GraphSubscriptionPublisher, MindTables};
    use crate::{Result, StoreLocation};

    pub struct AcceptedKnowledgeFixturePrepopulation {
        store: StoreLocation,
        record: AcceptedKnowledge,
    }

    impl AcceptedKnowledgeFixturePrepopulation {
        pub fn new(store: StoreLocation, record: AcceptedKnowledge) -> Self {
            Self { store, record }
        }

        pub fn prepopulate(self) -> Result<AcceptedKnowledge> {
            let tables = MindTables::open(&self.store, GraphSubscriptionPublisher::Disabled)?;
            tables.assert_accepted_knowledge(self.record)
        }
    }
}
pub(crate) mod frame_bytes;
pub mod graph;
pub mod knowledge;
pub mod memory;
pub mod meta;
pub mod supervision;
pub mod tables;
pub mod technical_seed;
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
pub use configuration::{
    ConfigurationError, MindDaemonConfiguration, MindJudgeRequestResponseLog,
    MindKnowledgeJudgeAgentConfiguration, MindKnowledgeJudgeConfiguration,
    MindKnowledgeJudgeTrainingSource,
};
pub use daemon::{MindDaemonError, MindEngine, MindProcessDaemon};
pub use envelope::MindEnvelope;
pub use error::{Error, Result};
pub use kameo::actor::ActorRef;
pub use knowledge::{
    AgentKnowledgeJudge, FixtureKnowledgeJudge, KnowledgeJudge, KnowledgeJudgePort,
    KnowledgeJudgeRequest,
};
pub(crate) use memory::MemoryGraph;
pub use memory::{MemoryState, StoreLocation};
pub use meta::{MetaMindClient, MetaMindEndpoint, MetaMindFrameCodec};
#[cfg(feature = "nota-text")]
pub use meta::{MetaMindCommand, MetaMindCommandEnvironment};
pub use supervision::{
    SupervisionFrameCodec, SupervisionListener, SupervisionProfile, SupervisionSocketMode,
};
/// Mind's durable table owner.
///
/// Normal builds do not expose the eval fixture accepted-knowledge bypass as a
/// `MindTables` API:
///
/// ```compile_fail
/// use mind::MindTables;
///
/// let _bypass = MindTables::eval_fixture_prepopulate_accepted_knowledge;
/// ```
pub use tables::MindTables;
pub use technical_seed::TechnicalSeedDataset;
pub use text::{MindTextReply, MindTextRequest};
pub use transport::{MindClient, MindDaemon, MindDaemonEndpoint, MindFrameCodec, MindSocketMode};
