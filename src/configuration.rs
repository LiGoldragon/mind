//! The binary rkyv startup configuration the `mind` daemon accepts as its
//! single argument.
//!
//! Per the daemon-binary-only override, the daemon never parses NOTA — it reads
//! exactly one pre-generated rkyv configuration file. A deploy/bootstrap tool
//! encodes typed NOTA into this archive before it reaches the daemon. The
//! configuration names the two listener sockets (working `MindFrame` ingress and
//! the owner-only engine-management meta socket) plus the durable store path.

use std::path::Path;

use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};
use signal_mind::WirePath;
use triad_runtime::{BindingSurface, RequestConcurrencyLimit, SocketMode};

const OWNER_ONLY_SOCKET_MODE: u32 = 0o600;
const MAXIMUM_CONCURRENT_REQUESTS: usize = 64;

#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq, Eq)]
pub struct MindDaemonConfiguration {
    pub store_path: WirePath,
    pub socket_path: WirePath,
    pub meta_socket_path: WirePath,
    pub knowledge_judge: MindKnowledgeJudgeConfiguration,
}

impl MindDaemonConfiguration {
    pub fn new(store_path: WirePath, socket_path: WirePath, meta_socket_path: WirePath) -> Self {
        Self {
            store_path,
            socket_path,
            meta_socket_path,
            knowledge_judge: MindKnowledgeJudgeConfiguration::Fixture,
        }
    }

    pub fn with_mind_judge(
        mut self,
        knowledge_judge: MindKnowledgeJudgeSocketConfiguration,
    ) -> Self {
        self.knowledge_judge = MindKnowledgeJudgeConfiguration::MindJudge(knowledge_judge);
        self
    }

    /// Encode the configuration to the binary rkyv form the daemon accepts as
    /// its single startup argument (daemons never parse NOTA — hard override).
    pub fn to_signal_bytes(&self) -> Result<Vec<u8>, ConfigurationError> {
        rkyv::to_bytes::<rkyv::rancor::Error>(self)
            .map(|bytes| bytes.to_vec())
            .map_err(|_| ConfigurationError::ArchiveEncode)
    }

    /// Decode the configuration from the binary rkyv startup bytes.
    pub fn from_signal_bytes(bytes: &[u8]) -> Result<Self, ConfigurationError> {
        let configuration = rkyv::from_bytes::<Self, rkyv::rancor::Error>(bytes)
            .map_err(|_| ConfigurationError::ArchiveDecode)?;
        configuration.validate()?;
        Ok(configuration)
    }

    /// Read and decode the binary rkyv configuration from the daemon's single
    /// startup-argument file path.
    pub fn from_signal_file(path: &Path) -> Result<Self, ConfigurationError> {
        let bytes = std::fs::read(path).map_err(ConfigurationError::Read)?;
        Self::from_signal_bytes(&bytes)
    }

    pub fn validate(&self) -> Result<(), ConfigurationError> {
        Ok(())
    }
}

impl BindingSurface for MindDaemonConfiguration {
    fn socket_path(&self) -> &Path {
        Path::new(self.socket_path.as_str())
    }

    fn socket_mode(&self) -> Option<SocketMode> {
        Some(SocketMode::new(OWNER_ONLY_SOCKET_MODE))
    }

    fn request_concurrency_limit(&self) -> RequestConcurrencyLimit {
        RequestConcurrencyLimit::new(MAXIMUM_CONCURRENT_REQUESTS)
    }

    fn meta_socket_path(&self) -> Option<&Path> {
        Some(Path::new(self.meta_socket_path.as_str()))
    }

    fn meta_socket_mode(&self) -> Option<SocketMode> {
        Some(SocketMode::new(OWNER_ONLY_SOCKET_MODE))
    }

    fn database_path(&self) -> &Path {
        Path::new(self.store_path.as_str())
    }
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq, Eq)]
pub enum MindKnowledgeJudgeConfiguration {
    Fixture,
    MindJudge(MindKnowledgeJudgeSocketConfiguration),
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq, Eq)]
pub struct MindKnowledgeJudgeSocketConfiguration {
    pub socket_path: WirePath,
    pub timeout_milliseconds: u64,
}

impl MindKnowledgeJudgeSocketConfiguration {
    pub const DEFAULT_TIMEOUT_MILLISECONDS: u64 = 180_000;

    pub fn new(socket_path: WirePath, timeout_milliseconds: u64) -> Self {
        Self {
            socket_path,
            timeout_milliseconds,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigurationError {
    #[error("read daemon configuration file: {0}")]
    Read(std::io::Error),

    #[error("daemon configuration rkyv encode failed")]
    ArchiveEncode,

    #[error("daemon configuration rkyv decode failed")]
    ArchiveDecode,
}
