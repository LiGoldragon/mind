//! Mind's daemon hooks — the only daemon code mind hand-writes.
//!
//! The uniform daemon skeleton (argv parsing, async task-backed multi-listener
//! binding, request gating, peer credentials, lifecycle, and the `ExitReport`
//! entry) is emitted into `src/schema/daemon.rs` by schema-rust-next's daemon
//! emitter. Mind's working tier is *component-decoded*: the ordinary socket
//! speaks the hand-written `signal-mind` `MindFrame` contract, which is not a
//! schema-derived root, so the component owns the per-connection frame
//! decode/encode and drives the existing `MindRoot` kameo actor tree. The
//! emitted shell owns everything else.
//!
//! `MindEngine` is the `Daemon::Engine`. It opens the durable store path and
//! lazily starts the `MindRoot` actor on the first served connection (inside the
//! daemon's Tokio runtime), exactly like router's `RouterEngine`. The working
//! hook reuses `MindFrameCodec`; the meta hook reuses `SupervisionFrameCodec` to
//! answer the owner-only engine-management (supervision) protocol.

use thiserror::Error;
use tokio::sync::OnceCell;
use triad_runtime::{AcceptedConnection, FrameError};

use crate::schema::daemon::ComponentDaemon;
use crate::{
    ActorRef, Error as MindError, MindFrameCodec, MindRoot, MindRootArguments, StoreLocation,
    SupervisionFrameCodec,
};

/// The type-level selector for mind's emitted daemon. It carries no runtime
/// data — it is the marker the emitted `DaemonCommand<MindProcessDaemon>` and the
/// generated runtime dispatch on, selecting mind's `Configuration` / `Engine` /
/// `Error` types through the `ComponentDaemon` associated types.
#[derive(Debug)]
pub struct MindProcessDaemon;

/// Mind's engine: the durable store location plus the lazily-started root actor.
/// `build_runtime` runs before the daemon's Tokio runtime serves, so the actor
/// is spawned on first connection inside that runtime rather than at construction
/// time.
pub struct MindEngine {
    store: StoreLocation,
    root: OnceCell<ActorRef<MindRoot>>,
    working_codec: MindFrameCodec,
    supervision_codec: SupervisionFrameCodec,
}

/// Mind's daemon error: the engine-facing variants the emitted spine needs
/// (`From<FrameError>` for the working/meta wire) plus mind's domain error. The
/// emitted `DaemonError<MindProcessDaemon>` wraps this under its `Component` arm.
#[derive(Debug, Error)]
pub enum MindDaemonError {
    #[error("daemon frame error: {0}")]
    Frame(#[from] FrameError),

    #[error("mind engine error: {0}")]
    Engine(#[from] MindError),
}

impl MindEngine {
    pub fn open(store: StoreLocation) -> Self {
        Self {
            store,
            root: OnceCell::new(),
            working_codec: MindFrameCodec::default(),
            supervision_codec: SupervisionFrameCodec::new(1024 * 1024),
        }
    }

    /// The started root actor, spawned on first use inside the daemon runtime.
    async fn root(&self) -> Result<&ActorRef<MindRoot>, MindDaemonError> {
        self.root
            .get_or_try_init(|| async {
                Ok(MindRoot::start(MindRootArguments::new(self.store.clone())).await?)
            })
            .await
    }

    async fn handle_working_connection(
        &self,
        mut connection: AcceptedConnection,
    ) -> Result<(), MindDaemonError> {
        let root = self.root().await?;
        self.working_codec
            .serve_request(connection.stream_mut(), root)
            .await?;
        Ok(())
    }

    async fn handle_meta_connection(
        &self,
        mut connection: AcceptedConnection,
    ) -> Result<(), MindDaemonError> {
        let root = self.root().await?;
        self.supervision_codec
            .serve_connection(connection.stream_mut(), root)
            .await?;
        Ok(())
    }
}

impl ComponentDaemon for MindProcessDaemon {
    type Configuration = crate::MindDaemonConfiguration;
    type ConfigurationError = crate::ConfigurationError;
    type Engine = MindEngine;
    type Error = MindDaemonError;

    const PROCESS_NAME: &'static str = "mind-daemon";

    fn load_configuration(
        path: &std::path::Path,
    ) -> Result<Self::Configuration, Self::ConfigurationError> {
        crate::MindDaemonConfiguration::from_signal_file(path)
    }

    fn build_runtime(configuration: &Self::Configuration) -> Result<Self::Engine, Self::Error> {
        Ok(MindEngine::open(StoreLocation::new(
            configuration.store_path.as_str(),
        )))
    }

    async fn handle_working_connection(
        engine: &Self::Engine,
        connection: AcceptedConnection,
    ) -> Result<(), Self::Error> {
        engine.handle_working_connection(connection).await
    }

    /// The owner-only meta socket carries the engine-management (supervision)
    /// protocol: announce / readiness / health / stop. Routing it through the
    /// same `MindRoot` actor keeps supervision serialized with working state.
    async fn handle_meta_connection(
        engine: &Self::Engine,
        connection: AcceptedConnection,
    ) -> Result<(), Self::Error> {
        engine.handle_meta_connection(connection).await
    }
}
