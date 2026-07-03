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

    pub fn with_agent_knowledge_judge(
        mut self,
        knowledge_judge: MindKnowledgeJudgeAgentConfiguration,
    ) -> Self {
        self.knowledge_judge = MindKnowledgeJudgeConfiguration::Agent(knowledge_judge);
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
        if let MindKnowledgeJudgeConfiguration::Agent(judge) = &self.knowledge_judge {
            if let MindJudgeRequestResponseLog::JsonLines(path) = &judge.request_response_log {
                if self.judge_request_response_log_conflicts_with_store(path) {
                    return Err(ConfigurationError::JudgeRequestResponseLogPathIsStore {
                        path: path.as_str().to_owned(),
                    });
                }
            }
        }
        Ok(())
    }

    fn judge_request_response_log_conflicts_with_store(&self, path: &WirePath) -> bool {
        let store_path = Path::new(self.store_path.as_str());
        let log_path = Path::new(path.as_str());
        if store_path == log_path {
            return true;
        }
        if log_path
            .symlink_metadata()
            .map(|metadata| metadata.file_type().is_symlink())
            .unwrap_or(false)
        {
            return true;
        }
        match (store_path.canonicalize(), log_path.canonicalize()) {
            (Ok(store), Ok(log)) => store == log,
            _ => false,
        }
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
    Agent(MindKnowledgeJudgeAgentConfiguration),
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq, Eq)]
pub struct MindKnowledgeJudgeAgentConfiguration {
    pub agent_socket_path: WirePath,
    pub provider_name: Option<String>,
    pub model_name: Option<String>,
    pub timeout_milliseconds: u64,
    pub maximum_output_tokens: Option<u64>,
    pub training_source: MindKnowledgeJudgeTrainingSource,
    pub request_response_log: MindJudgeRequestResponseLog,
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq, Eq)]
pub enum MindKnowledgeJudgeTrainingSource {
    CompiledDefault,
    OverrideText(String),
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, PartialEq, Eq)]
pub enum MindJudgeRequestResponseLog {
    Disabled,
    JsonLines(WirePath),
}

impl MindKnowledgeJudgeAgentConfiguration {
    pub const DEEPSEEK_PROVIDER: &'static str = "deepseek";
    pub const DEEPSEEK_FLASH_MODEL: &'static str = "deepseek-v4-flash";
    pub const LOCAL_OPENAI_COMPATIBLE_PROVIDER: &'static str = "local-openai";
    pub const LOCAL_OPENAI_COMPATIBLE_MODEL: &'static str = "gpt-5.5";
    pub const LOCAL_OPENAI_COMPATIBLE_ENDPOINT: &'static str = "http://127.0.0.1:18080/v1";
    pub const DEFAULT_TIMEOUT_MILLISECONDS: u64 = 180_000;
    pub const DEFAULT_MAXIMUM_OUTPUT_TOKENS: u64 = 2048;

    pub fn new(
        agent_socket_path: WirePath,
        provider_name: Option<String>,
        model_name: Option<String>,
        timeout_milliseconds: u64,
        maximum_output_tokens: Option<u64>,
    ) -> Self {
        Self {
            agent_socket_path,
            provider_name,
            model_name,
            timeout_milliseconds,
            maximum_output_tokens,
            training_source: MindKnowledgeJudgeTrainingSource::CompiledDefault,
            request_response_log: MindJudgeRequestResponseLog::Disabled,
        }
    }

    pub fn with_training_source(
        mut self,
        training_source: MindKnowledgeJudgeTrainingSource,
    ) -> Self {
        self.training_source = training_source;
        self
    }

    pub fn with_request_response_log(mut self, log: MindJudgeRequestResponseLog) -> Self {
        self.request_response_log = log;
        self
    }

    pub fn deepseek_flash(agent_socket_path: WirePath) -> Self {
        Self::new(
            agent_socket_path,
            Some(Self::DEEPSEEK_PROVIDER.to_owned()),
            Some(Self::DEEPSEEK_FLASH_MODEL.to_owned()),
            Self::DEFAULT_TIMEOUT_MILLISECONDS,
            Some(Self::DEFAULT_MAXIMUM_OUTPUT_TOKENS),
        )
    }

    pub fn local_openai_compatible(agent_socket_path: WirePath) -> Self {
        Self::new(
            agent_socket_path,
            Some(Self::LOCAL_OPENAI_COMPATIBLE_PROVIDER.to_owned()),
            Some(Self::LOCAL_OPENAI_COMPATIBLE_MODEL.to_owned()),
            Self::DEFAULT_TIMEOUT_MILLISECONDS,
            Some(Self::DEFAULT_MAXIMUM_OUTPUT_TOKENS),
        )
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

    #[error("judge request/response log path must differ from store path: {path}")]
    JudgeRequestResponseLogPathIsStore { path: String },
}
