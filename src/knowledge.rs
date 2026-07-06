use std::collections::{BTreeSet, VecDeque};
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use nota::{NotaEncode, NotaSource};
use serde_json::json;
use sha2::{Digest, Sha256};
use signal_agent::{
    ChatMessage, ChatTranscript, CompletionText, Input as AgentInput, MaximumOutputTokens,
    ModelName, Output as AgentOutput, OutputMode, Prompt, PromptOptions, ProviderName,
    ReasoningEffort, SystemText, TemperatureMilli, ThinkingMode,
};
use signal_mind::{
    AcceptedKnowledge, ActorName, KnowledgeIdentity, KnowledgeJudgePacket, KnowledgeJudgeResponse,
    KnowledgeJudgeVerdict, KnowledgeRecord, KnowledgeRejectionReason, KnowledgeSubject,
    KnowledgeSubmission, MindReply, MindRequest, TextBody,
};
use triad_runtime::{FrameBody, LengthPrefixedCodec};

use crate::{
    MindEnvelope, MindJudgeRequestResponseLog, MindKnowledgeJudgeAgentConfiguration,
    MindKnowledgeJudgeTrainingSource, MindTables, Result,
};

const KNOWLEDGE_IDENTITY_MINIMUM_CODE_LENGTH: usize = 4;
const KNOWLEDGE_IDENTITY_MAXIMUM_CODE_LENGTH: usize = 7;
const KNOWLEDGE_IDENTITY_CODE_RADIX: u64 = 36;
const RANDOM_IDENTITY_ATTEMPTS_PER_LENGTH: usize = 128;
const ACCEPTED_KNOWLEDGE_JUDGE_TRAINING: &str =
    include_str!("knowledge-judge-prompts/accepted-knowledge.md");
const JUDGE_DIAGNOSTIC_PATH_ENVIRONMENT: &str = "MIND_JUDGE_DIAGNOSTIC_PATH";
const JUDGE_DIAGNOSTIC_TEXT_ENVIRONMENT: &str = "MIND_JUDGE_DIAGNOSTIC_TEXT";

pub trait KnowledgeJudge: Send + Sync {
    fn judge(&self, request: KnowledgeJudgeRequest) -> KnowledgeJudgeDecision;

    fn record_applied_decision(&self, _decision: KnowledgeJudgeAppliedDecision) {}
}

pub type KnowledgeJudgePort = Arc<dyn KnowledgeJudge>;

#[derive(Clone, Debug)]
pub struct KnowledgeJudgeRequest {
    client_request: MindRequest,
    packet: KnowledgeJudgePacket,
}

impl KnowledgeJudgeRequest {
    fn new(client_request: MindRequest, packet: KnowledgeJudgePacket) -> Self {
        Self {
            client_request,
            packet,
        }
    }

    fn client_request(&self) -> &MindRequest {
        &self.client_request
    }

    fn packet(&self) -> &KnowledgeJudgePacket {
        &self.packet
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KnowledgeJudgeDecision {
    verdict: KnowledgeJudgeVerdict,
    diagnostic_message: Option<TextBody>,
    parse_status: KnowledgeJudgeParseStatus,
}

impl KnowledgeJudgeDecision {
    fn new(verdict: KnowledgeJudgeVerdict) -> Self {
        Self {
            verdict,
            diagnostic_message: None,
            parse_status: KnowledgeJudgeParseStatus::ParsedKnowledgeJudgeResponse,
        }
    }

    fn from_response(response: KnowledgeJudgeResponse) -> Self {
        Self {
            verdict: response.verdict,
            diagnostic_message: response.diagnostic_message,
            parse_status: KnowledgeJudgeParseStatus::ParsedKnowledgeJudgeResponse,
        }
    }

    fn verdict(&self) -> &KnowledgeJudgeVerdict {
        &self.verdict
    }

    fn diagnostic_message(&self) -> Option<&TextBody> {
        self.diagnostic_message.as_ref()
    }

    fn parse_status(&self) -> &KnowledgeJudgeParseStatus {
        &self.parse_status
    }

    fn format_failure(error: String) -> Self {
        Self {
            verdict: KnowledgeJudgeVerdict::Reject(KnowledgeRejectionReason::MeaningUnclear),
            diagnostic_message: None,
            parse_status: KnowledgeJudgeParseStatus::JudgeFormatFailure { error },
        }
    }

    fn agent_unavailable(error: String) -> Self {
        Self {
            verdict: KnowledgeJudgeVerdict::Reject(KnowledgeRejectionReason::MeaningUnclear),
            diagnostic_message: None,
            parse_status: KnowledgeJudgeParseStatus::AgentUnavailable { error },
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum KnowledgeJudgeParseStatus {
    ParsedKnowledgeJudgeResponse,
    JudgeFormatFailure { error: String },
    AgentUnavailable { error: String },
}

impl KnowledgeJudgeParseStatus {
    fn as_str(&self) -> &'static str {
        match self {
            Self::ParsedKnowledgeJudgeResponse => "parsed_knowledge_judge_response",
            Self::JudgeFormatFailure { .. } => "judge_format_failure",
            Self::AgentUnavailable { .. } => "agent_unavailable",
        }
    }

    fn parsed_completed_response(&self) -> bool {
        matches!(self, Self::ParsedKnowledgeJudgeResponse)
    }

    fn error(&self) -> Option<&str> {
        match self {
            Self::ParsedKnowledgeJudgeResponse => None,
            Self::JudgeFormatFailure { error } | Self::AgentUnavailable { error } => Some(error),
        }
    }
}

#[derive(Clone, Debug)]
pub struct KnowledgeJudgeAppliedDecision {
    client_request: MindRequest,
    verdict: KnowledgeJudgeVerdict,
    diagnostic_message: Option<TextBody>,
    parse_status: KnowledgeJudgeParseStatus,
    reply: MindReply,
}

impl KnowledgeJudgeAppliedDecision {
    fn new(
        client_request: MindRequest,
        decision: KnowledgeJudgeDecision,
        reply: MindReply,
    ) -> Self {
        Self {
            client_request,
            verdict: decision.verdict,
            diagnostic_message: decision.diagnostic_message,
            parse_status: decision.parse_status,
            reply,
        }
    }
}

pub struct FixtureKnowledgeJudge {
    verdicts: Mutex<VecDeque<KnowledgeJudgeVerdict>>,
    calls: AtomicUsize,
}

impl FixtureKnowledgeJudge {
    pub fn new(verdicts: Vec<KnowledgeJudgeVerdict>) -> Self {
        Self {
            verdicts: Mutex::new(verdicts.into()),
            calls: AtomicUsize::new(0),
        }
    }

    pub fn empty() -> Self {
        Self::new(Vec::new())
    }

    pub fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }

    fn next_verdict(&self) -> KnowledgeJudgeVerdict {
        self.verdicts
            .lock()
            .expect("fixture judge lock is not poisoned")
            .pop_front()
            .unwrap_or(KnowledgeJudgeVerdict::Reject(
                KnowledgeRejectionReason::MeaningUnclear,
            ))
    }
}

impl Default for FixtureKnowledgeJudge {
    fn default() -> Self {
        Self::empty()
    }
}

impl KnowledgeJudge for FixtureKnowledgeJudge {
    fn judge(&self, _request: KnowledgeJudgeRequest) -> KnowledgeJudgeDecision {
        self.calls.fetch_add(1, Ordering::SeqCst);
        KnowledgeJudgeDecision::new(self.next_verdict())
    }
}

#[derive(Clone, Debug)]
pub struct AgentKnowledgeJudge {
    configuration: AgentKnowledgeJudgeConfiguration,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AgentKnowledgeJudgeConfiguration {
    socket_path: PathBuf,
    provider_name: Option<String>,
    model_name: Option<String>,
    timeout: Duration,
    maximum_output_tokens: Option<u64>,
    training_source: MindKnowledgeJudgeTrainingSource,
    request_response_log: JudgeRequestResponseLog,
}

#[derive(Clone, Debug)]
struct KnowledgeJudgePrompt<'packet> {
    packet: &'packet KnowledgeJudgePacket,
    provider_name: Option<&'packet str>,
    model_name: Option<&'packet str>,
    maximum_output_tokens: Option<u64>,
    training_source: &'packet MindKnowledgeJudgeTrainingSource,
}

#[derive(Debug, thiserror::Error)]
enum AgentKnowledgeJudgeError {
    #[error("knowledge judge agent socket unavailable: {0}")]
    Socket(std::io::Error),

    #[error("knowledge judge agent frame failed: {0}")]
    Frame(String),

    #[error("knowledge judge agent rejected the call: {0}")]
    AgentRejected(String),

    #[error("knowledge judge agent returned malformed verdict: {0}")]
    Malformed(String),
}

impl AgentKnowledgeJudge {
    pub fn new(configuration: MindKnowledgeJudgeAgentConfiguration) -> Self {
        Self {
            configuration: AgentKnowledgeJudgeConfiguration::from_contract(configuration),
        }
    }

    fn call_agent(
        &self,
        prompt: Prompt,
    ) -> std::result::Result<AgentOutput, AgentKnowledgeJudgeError> {
        let mut stream = UnixStream::connect(self.configuration.socket_path())
            .map_err(AgentKnowledgeJudgeError::Socket)?;
        stream
            .set_read_timeout(Some(self.configuration.timeout))
            .map_err(AgentKnowledgeJudgeError::Socket)?;
        stream
            .set_write_timeout(Some(self.configuration.timeout))
            .map_err(AgentKnowledgeJudgeError::Socket)?;
        let input = AgentInput::call(prompt);
        let codec = LengthPrefixedCodec::default();
        codec
            .write_body(
                &mut stream,
                &FrameBody::new(
                    input
                        .encode_signal_frame()
                        .map_err(|error| AgentKnowledgeJudgeError::Frame(error.to_string()))?,
                ),
            )
            .map_err(|error| AgentKnowledgeJudgeError::Frame(error.to_string()))?;
        stream.flush().map_err(AgentKnowledgeJudgeError::Socket)?;
        let reply = codec
            .read_body(&mut stream)
            .map_err(|error| AgentKnowledgeJudgeError::Frame(error.to_string()))?;
        AgentOutput::decode_signal_frame(&reply.into_bytes())
            .map(|(_route, output)| output)
            .map_err(|error| AgentKnowledgeJudgeError::Frame(error.to_string()))
    }

    fn parse_decision(
        &self,
        completion: &CompletionText,
    ) -> std::result::Result<KnowledgeJudgeDecision, AgentKnowledgeJudgeError> {
        let source = NotaSource::new(completion.payload());
        source
            .parse::<KnowledgeJudgeResponse>()
            .map(KnowledgeJudgeDecision::from_response)
            .map_err(|response_error| {
                AgentKnowledgeJudgeError::Malformed(format!(
                    "KnowledgeJudgeResponse: {response_error}"
                ))
            })
    }

    fn unavailable_decision(error: AgentKnowledgeJudgeError) -> KnowledgeJudgeDecision {
        match error {
            AgentKnowledgeJudgeError::Malformed(message) => {
                KnowledgeJudgeDecision::format_failure(message)
            }
            other => KnowledgeJudgeDecision::agent_unavailable(other.to_string()),
        }
    }
}

impl KnowledgeJudge for AgentKnowledgeJudge {
    fn judge(&self, request: KnowledgeJudgeRequest) -> KnowledgeJudgeDecision {
        let prompt = KnowledgeJudgePrompt::new(
            request.packet(),
            self.configuration.provider_name.as_deref(),
            self.configuration.model_name.as_deref(),
            self.configuration.maximum_output_tokens,
            self.configuration.training_source(),
        )
        .into_agent_prompt();
        JudgeDiagnostic::from_environment(
            request.packet(),
            &prompt,
            self.configuration.training_source(),
        )
        .write();
        let output = match self.call_agent(prompt) {
            Ok(output) => output,
            Err(error) => return Self::unavailable_decision(error),
        };
        let AgentOutput::Completed(completion) = output else {
            self.configuration
                .request_response_log
                .write_agent_output(request.client_request(), &output);
            return Self::unavailable_decision(AgentKnowledgeJudgeError::AgentRejected(format!(
                "{output:?}"
            )));
        };
        match self.parse_decision(&completion.completion_text) {
            Ok(decision) => {
                self.configuration.request_response_log.write_completed(
                    request.client_request(),
                    completion.completion_text.payload(),
                    Some(&decision),
                );
                decision
            }
            Err(error) => {
                let decision = Self::unavailable_decision(error);
                self.configuration.request_response_log.write_completed(
                    request.client_request(),
                    completion.completion_text.payload(),
                    Some(&decision),
                );
                decision
            }
        }
    }

    fn record_applied_decision(&self, decision: KnowledgeJudgeAppliedDecision) {
        self.configuration
            .request_response_log
            .write_applied_decision(&decision);
    }
}

impl AgentKnowledgeJudgeConfiguration {
    fn from_contract(configuration: MindKnowledgeJudgeAgentConfiguration) -> Self {
        Self {
            socket_path: PathBuf::from(configuration.agent_socket_path.as_str()),
            provider_name: configuration.provider_name,
            model_name: configuration.model_name,
            timeout: Duration::from_millis(configuration.timeout_milliseconds),
            maximum_output_tokens: configuration.maximum_output_tokens,
            training_source: configuration.training_source,
            request_response_log: JudgeRequestResponseLog::from_contract(
                configuration.request_response_log,
            ),
        }
    }

    fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    fn training_source(&self) -> &MindKnowledgeJudgeTrainingSource {
        &self.training_source
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct JudgeRequestResponseLog {
    destination: JudgeRequestResponseLogDestination,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum JudgeRequestResponseLogDestination {
    Disabled,
    JsonLines(PathBuf),
}

impl JudgeRequestResponseLog {
    fn from_contract(configuration: MindJudgeRequestResponseLog) -> Self {
        match configuration {
            MindJudgeRequestResponseLog::Disabled => Self {
                destination: JudgeRequestResponseLogDestination::Disabled,
            },
            MindJudgeRequestResponseLog::JsonLines(path) => Self {
                destination: JudgeRequestResponseLogDestination::JsonLines(PathBuf::from(
                    path.as_str(),
                )),
            },
        }
    }

    fn write_completed(
        &self,
        client_request: &MindRequest,
        raw_response: &str,
        decision: Option<&KnowledgeJudgeDecision>,
    ) {
        let mut record = json!({
            "kind": "completed_response",
            "timestamp_unix_millis": JudgeRequestResponseLogClock::now_unix_millis(),
            "request": client_request.to_nota(),
            "raw_response": raw_response,
        });
        if let Some(decision) = decision {
            record["judge_response_parse_status"] = json!(decision.parse_status().as_str());
            record["parsed_completed_response"] =
                json!(decision.parse_status().parsed_completed_response());
            if let Some(error) = decision.parse_status().error() {
                record["judge_response_parse_error"] = json!(error);
            }
            record["parsed_verdict"] = json!(decision.verdict().to_nota());
            record["diagnostic_message"] = json!(
                decision
                    .diagnostic_message()
                    .map(|message| message.as_str())
            );
        }
        self.write_record(record);
    }

    fn write_agent_output(&self, client_request: &MindRequest, output: &AgentOutput) {
        self.write_record(json!({
            "kind": "agent_output",
            "timestamp_unix_millis": JudgeRequestResponseLogClock::now_unix_millis(),
            "request": client_request.to_nota(),
            "agent_output": format!("{output:?}"),
        }));
    }

    fn write_applied_decision(&self, decision: &KnowledgeJudgeAppliedDecision) {
        self.write_record(json!({
            "kind": "applied_decision",
            "timestamp_unix_millis": JudgeRequestResponseLogClock::now_unix_millis(),
            "request": decision.client_request.to_nota(),
            "parsed_verdict": decision.verdict.to_nota(),
            "diagnostic_message": decision.diagnostic_message.as_ref().map(|message| message.as_str()),
            "judge_response_parse_status": decision.parse_status.as_str(),
            "parsed_completed_response": decision.parse_status.parsed_completed_response(),
            "judge_response_parse_error": decision.parse_status.error(),
            "reply": decision.reply.to_nota(),
            "accepted_identity": match &decision.reply {
                MindReply::Accepted(identity) => Some(identity.as_str()),
                _ => None,
            },
        }));
    }

    fn write_record(&self, record: serde_json::Value) {
        let JudgeRequestResponseLogDestination::JsonLines(path) = &self.destination else {
            return;
        };
        let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        else {
            return;
        };
        let _ = writeln!(file, "{record}");
    }
}

struct JudgeRequestResponseLogClock;

impl JudgeRequestResponseLogClock {
    fn now_unix_millis() -> u128 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or_default()
    }
}

impl MindKnowledgeJudgeTrainingSource {
    fn prompt_text(&self) -> &str {
        match self {
            Self::CompiledDefault => ACCEPTED_KNOWLEDGE_JUDGE_TRAINING,
            Self::OverrideText(text) => text.as_str(),
        }
    }
}

impl<'packet> KnowledgeJudgePrompt<'packet> {
    fn new(
        packet: &'packet KnowledgeJudgePacket,
        provider_name: Option<&'packet str>,
        model_name: Option<&'packet str>,
        maximum_output_tokens: Option<u64>,
        training_source: &'packet MindKnowledgeJudgeTrainingSource,
    ) -> Self {
        Self {
            packet,
            provider_name,
            model_name,
            maximum_output_tokens,
            training_source,
        }
    }

    fn into_agent_prompt(self) -> Prompt {
        Prompt::new(
            Some(SystemText::new(self.system_prompt())),
            ChatTranscript::new(vec![ChatMessage::user(self.user_prompt())]),
            self.prompt_options(),
        )
    }

    fn system_prompt(&self) -> String {
        format!(
            "{training}\n\n\
             Return exactly one KnowledgeJudgeResponse NOTA value and nothing else: no markdown, no \
             prose around it, no JSON, no code fence. Its first field is the load-bearing \
             KnowledgeJudgeVerdict. Its optional diagnostic_message field is debug-only and \
             non-load-bearing. The encoded value is positional; do not prefix it with \
             KnowledgeJudgeResponse. A valid accept response is shaped like {accept}. A valid reject \
             response is shaped like {reject}. Duplicate, conflict, vague, and wrong-subject \
             reject responses are shaped like {duplicate}, {conflict}, {vague}, and \
             {wrong_subject}. Payload-bearing reject reasons must be one nested reason object \
             inside Reject: the reason name and its payload stay inside the same inner \
             parentheses. Never flatten a payload-bearing reason into separate siblings after \
             Reject. Before sending, check the first field: WrongSubject Component starts \
             ((Reject (WrongSubject Component)); SemanticDuplicate p001 starts \
             ((Reject (SemanticDuplicate p001)); ConflictsAcceptedKnowledge p001 starts \
             ((Reject (ConflictsAcceptedKnowledge [p001])). If you are not certain you can emit \
             a valid nested payload shape, choose a no-payload rejection reason instead of \
             malformed NOTA. WrongSubject always requires a subject payload; if you cannot \
             include it, choose a no-payload rejection reason instead of \
             malformed NOTA. Never return (Verdict accepted); that is malformed output.",
            training = self.training_source.prompt_text().trim(),
            accept = Self::accept_example(),
            reject = Self::reject_example(),
            duplicate = Self::duplicate_example(),
            conflict = Self::conflict_example(),
            vague = Self::vague_example(),
            wrong_subject = Self::wrong_subject_example(),
        )
    }

    fn user_prompt(&self) -> String {
        format!(
            "KnowledgeJudgePacket under judgment:\n{}\n\n\
             Return one KnowledgeJudgeResponse.",
            ModelVisibleKnowledgeJudgePacket::from_packet(self.packet).to_nota(),
        )
    }

    fn prompt_options(&self) -> PromptOptions {
        let local_openai_compatible = self.provider_name
            == Some(MindKnowledgeJudgeAgentConfiguration::LOCAL_OPENAI_COMPATIBLE_PROVIDER);
        PromptOptions::new(
            self.model_name
                .map(|model| ModelName::new(model.to_owned())),
            self.provider_name
                .map(|provider| ProviderName::new(provider.to_owned())),
            if local_openai_compatible {
                None
            } else {
                Some(TemperatureMilli::new(0))
            },
            self.maximum_output_tokens.map(MaximumOutputTokens::new),
            OutputMode::Nota,
            if local_openai_compatible {
                None
            } else {
                Some(ReasoningEffort::Low)
            },
            if local_openai_compatible {
                None
            } else {
                Some(ThinkingMode::Disabled)
            },
        )
    }

    fn accept_example() -> String {
        KnowledgeJudgeResponse::new(KnowledgeJudgeVerdict::Accept).to_nota()
    }

    fn reject_example() -> String {
        KnowledgeJudgeResponse::new(KnowledgeJudgeVerdict::Reject(
            KnowledgeRejectionReason::NotKnowledge,
        ))
        .to_nota()
    }

    fn duplicate_example() -> String {
        KnowledgeJudgeResponse::new(KnowledgeJudgeVerdict::Reject(
            KnowledgeRejectionReason::SemanticDuplicate(KnowledgeIdentity::new("abcd")),
        ))
        .to_nota()
    }

    fn conflict_example() -> String {
        KnowledgeJudgeResponse::new(KnowledgeJudgeVerdict::Reject(
            KnowledgeRejectionReason::ConflictsAcceptedKnowledge(vec![KnowledgeIdentity::new(
                "abcd",
            )]),
        ))
        .to_nota()
    }

    fn vague_example() -> String {
        KnowledgeJudgeResponse::new(KnowledgeJudgeVerdict::Reject(
            KnowledgeRejectionReason::NeedsMoreSpecificShape,
        ))
        .to_nota()
    }

    fn wrong_subject_example() -> String {
        KnowledgeJudgeResponse::new(KnowledgeJudgeVerdict::Reject(
            KnowledgeRejectionReason::WrongSubject(KnowledgeSubject::Component),
        ))
        .to_nota()
    }
}

struct JudgeDiagnostic<'packet> {
    packet: &'packet KnowledgeJudgePacket,
    prompt: &'packet Prompt,
    training_source: &'packet MindKnowledgeJudgeTrainingSource,
    path: Option<PathBuf>,
    text_mode: JudgeDiagnosticTextMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum JudgeDiagnosticTextMode {
    HashesOnly,
    RedactedStructure,
}

impl<'packet> JudgeDiagnostic<'packet> {
    fn from_environment(
        packet: &'packet KnowledgeJudgePacket,
        prompt: &'packet Prompt,
        training_source: &'packet MindKnowledgeJudgeTrainingSource,
    ) -> Self {
        let path = std::env::var_os(JUDGE_DIAGNOSTIC_PATH_ENVIRONMENT).map(PathBuf::from);
        let text_mode = JudgeDiagnosticTextMode::from_environment();
        Self {
            packet,
            prompt,
            training_source,
            path,
            text_mode,
        }
    }

    fn write(&self) {
        let Some(path) = &self.path else {
            return;
        };
        let mut record = json!({
            "packet_sha256": Sha256Text::new(&self.packet.to_nota()).hex(),
            "prompt_sha256": Sha256Text::new(&self.prompt_text()).hex(),
            "training_sha256": Sha256Text::new(self.training_source.prompt_text()).hex(),
            "diagnostic_text_mode": self.text_mode.as_str(),
        });
        if self.text_mode == JudgeDiagnosticTextMode::RedactedStructure {
            record["packet_redacted_structure"] =
                json!(RedactedKnowledgeJudgePacket::new(self.packet).to_text());
            record["prompt_redacted_text"] = json!(self.redacted_prompt_text());
            record["training_text"] = json!(self.training_source.prompt_text());
        }
        let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        else {
            return;
        };
        let _ = writeln!(file, "{record}");
    }

    fn prompt_text(&self) -> String {
        let system = self
            .prompt
            .system()
            .map(|system| system.payload().as_str())
            .unwrap_or("");
        let transcript = self
            .prompt
            .chat_transcript()
            .payload()
            .iter()
            .map(|message| message.text.payload().as_str())
            .collect::<Vec<_>>()
            .join("\n");
        format!("{system}\n{transcript}")
    }

    fn redacted_prompt_text(&self) -> String {
        let system = self
            .prompt
            .system()
            .map(|system| system.payload().as_str())
            .unwrap_or("");
        let packet = RedactedKnowledgeJudgePacket::new(self.packet).to_text();
        format!(
            "{system}\nKnowledgeJudgePacket under judgment:\n{packet}\n\nReturn one KnowledgeJudgeResponse."
        )
    }
}

impl JudgeDiagnosticTextMode {
    fn from_environment() -> Self {
        match std::env::var(JUDGE_DIAGNOSTIC_TEXT_ENVIRONMENT).as_deref() {
            Ok("redacted") => Self::RedactedStructure,
            _ => Self::HashesOnly,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::HashesOnly => "hashes_only",
            Self::RedactedStructure => "redacted_structure",
        }
    }
}

struct RedactedKnowledgeJudgePacket<'packet> {
    packet: &'packet KnowledgeJudgePacket,
}

impl<'packet> RedactedKnowledgeJudgePacket<'packet> {
    fn new(packet: &'packet KnowledgeJudgePacket) -> Self {
        Self { packet }
    }

    fn to_text(&self) -> String {
        let neighbors = self
            .packet
            .relevant_neighbors
            .iter()
            .map(RedactedAcceptedKnowledge::new)
            .map(|neighbor| neighbor.to_text())
            .collect::<Vec<_>>()
            .join(" ");
        format!(
            "({:?} [redacted statement sha256:{}] [{}])",
            self.packet.subject,
            Sha256Text::new(self.packet.statement.as_str()).hex(),
            neighbors
        )
    }
}

#[derive(NotaEncode)]
struct ModelVisibleKnowledgeJudgePacket {
    subject: KnowledgeSubject,
    statement: TextBody,
    relevant_neighbors: Vec<KnowledgeRecord>,
}

impl ModelVisibleKnowledgeJudgePacket {
    fn from_packet(packet: &KnowledgeJudgePacket) -> Self {
        Self {
            subject: packet.subject,
            statement: packet.statement.clone(),
            relevant_neighbors: packet
                .relevant_neighbors
                .iter()
                .map(AcceptedKnowledge::public_record)
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use serde_json::Value;
    use signal_mind::TimestampNanos;

    use super::*;

    #[test]
    fn accepted_knowledge_judge_training_contains_packet_only_curriculum() {
        let training = ACCEPTED_KNOWLEDGE_JUDGE_TRAINING;

        assert!(training.contains("The `KnowledgeJudgePacket` is the only evidence"));
        assert!(training.contains("Training examples are examples of judgment, not facts"));
        assert!(training.contains("No extra provenance fields exist in the live packet"));
        assert!(training.contains("## Response Shape Drill"));
        assert!(training.contains("((Reject (SemanticDuplicate abcd)) None)"));
        assert!(training.contains("((Reject (ConflictsAcceptedKnowledge [abcd])) None)"));
        assert!(training.contains("((Reject (WrongSubject Interface)) None)"));
        assert!(training.contains("((Reject (WrongSubject Source)) None)"));
        assert!(training.contains("never emit `((Reject WrongSubject) None)`"));
        assert!(training.contains("the reason payload is always nested inside the `Reject` value"));
        assert!(training.contains("## Reason Precedence"));
        let task_precedence_index = training
            .find("imperative, request, task")
            .expect("task-like rejection should be trained");
        let malformed_precedence_index = training
            .find("malformed, uninterpretable")
            .expect("malformed rejection should be trained");
        assert!(task_precedence_index < malformed_precedence_index);
        assert!(training.contains("Duplicate outranks conflict"));
        assert!(training.contains("## Narrow Accept Rule"));
        assert!(training.contains("## Semantic Duplicate Curriculum"));
        assert!(training.contains(
            "The accepted-knowledge protocol answers with Accepted or Rejected for Submit and Found or NotFound for Get."
        ));
        assert!(training.contains(
            "Callers submit a subject and statement for accepted knowledge, not their own compact id."
        ));
        assert!(training.contains("reject as `SourceRequired` unless the packet includes an accepted neighbor establishing that source-location fact"));
        assert!(training.contains("((Reject FalseOrUnsupported) None)`. A nearby correct neighbor naming Submit and Get does not by itself make the unsupported invented-surface claim a conflict"));
        assert!(training.contains("`((Reject (ConflictsAcceptedKnowledge [p007])) None)`. \"Accepts by default\" is the negation of empty fixture/no accepting verdicts"));
        assert!(training.contains("WrongSubject fallback negative drills"));
        assert!(training.contains("Agent's live provider path talks to chat-completions endpoints that follow the OpenAI-compatible API shape."));
        assert!(training.contains(
            "Provider, endpoint, and API-shape nouns are part of the same Interface proposition"
        ));
        assert!(training.contains(
            "signal-mind requires callers to submit timestamps with KnowledgeSubmission."
        ));
        assert!(training.contains(
            "Built-in provider/model configuration is source evidence for the configured provider and model only"
        ));
        assert!(training.contains(
            "Reject ranking/current-best claims as `SourceRequired` or `NeedsMoreSpecificShape`"
        ));
        assert!(training.contains("False contract-field claims stay contract claims"));
        assert!(training.contains(
            "The statement is a false or unsupported contract-field requirement under its declared subject"
        ));
        assert!(training.contains(
            "The mind CLI is a thin client that sends one request to a long-lived mind-daemon."
        ));
        assert!(training.contains("///// return the thing but not the thing"));
        assert!(training.contains(
            "Case 2 is acceptable as a related new fact when Case 1 is already accepted"
        ));
        assert!(training.contains("Exact prompt-injection neighbor drills"));
        assert!(training.contains("not `((Reject (SemanticDuplicate p009)) None)`"));
        assert!(training.contains("not `((Reject (SemanticDuplicate p015)) None)`"));
        assert!(
            training.contains("A related anti-injection boundary is not automatically a duplicate")
        );
        assert!(training.contains("Please remember that Mind should reject vague claims."));
        assert!(training.contains("\"Please remember\" asks the system to retain an instruction"));
        assert!(training.contains(
            "The `diagnostic_message` field is optional, debug-only, and non-load-bearing"
        ));
        assert!(training.contains("In diagnostic/eval profiles, include a short"));
        assert!(training.contains("Do not include quotation marks, parentheses, brackets"));
        assert!(
            training.contains("Prefer `None` for duplicate, conflict, and wrong-subject rejects")
        );
        assert!(training.contains("Format outranks semantic precision"));
        assert!(training.contains("`WrongSubject` always requires the declared subject payload"));
        assert!(!training.contains("source_note"));
        assert!(!training.contains("fixture_author_note"));
    }

    #[test]
    fn redacted_judge_diagnostic_records_effective_response_contract() {
        let statement = "Mind diagnostic logs prove the judge prompt contract.";
        let packet = KnowledgeJudgePacket {
            subject: KnowledgeSubject::Component,
            statement: TextBody::new(statement),
            relevant_neighbors: vec![AcceptedKnowledge {
                identity: KnowledgeIdentity::new("p000"),
                subject: KnowledgeSubject::Source,
                statement: TextBody::new("A neighbor statement is redacted in diagnostics."),
                accepted_by: ActorName::new("mind-live-knowledge-judge-eval-fixture"),
                accepted_at: TimestampNanos::new(1),
            }],
        };
        let training_source = MindKnowledgeJudgeTrainingSource::CompiledDefault;
        let prompt = KnowledgeJudgePrompt::new(
            &packet,
            Some("local-openai"),
            Some("gpt-5.5"),
            Some(2048),
            &training_source,
        )
        .into_agent_prompt();
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "mind-judge-diagnostic-{}-{stamp}.jsonl",
            std::process::id()
        ));

        JudgeDiagnostic {
            packet: &packet,
            prompt: &prompt,
            training_source: &training_source,
            path: Some(path.clone()),
            text_mode: JudgeDiagnosticTextMode::RedactedStructure,
        }
        .write();

        let text = std::fs::read_to_string(&path).expect("read diagnostic artifact");
        let record: Value = serde_json::from_str(text.trim()).expect("diagnostic json");
        let prompt_text = record["prompt_redacted_text"]
            .as_str()
            .expect("prompt text is recorded");
        let training_text = record["training_text"]
            .as_str()
            .expect("training text is recorded");

        assert!(prompt_text.contains("KnowledgeJudgeResponse"));
        assert!(prompt_text.contains("diagnostic_message field is debug-only"));
        assert!(prompt_text.contains("A valid accept response is shaped like (Accept None)"));
        assert!(prompt_text.contains("Never return (Verdict accepted)"));
        assert!(prompt_text.contains("KnowledgeJudgePacket under judgment:"));
        assert!(prompt_text.contains("[redacted statement sha256:"));
        assert!(prompt_text.contains("p000"));
        assert!(
            !prompt_text.contains(statement),
            "redacted diagnostic prompt must not include the raw statement"
        );
        assert!(!prompt_text.contains("mind-live-knowledge-judge-eval-fixture"));
        assert!(!prompt_text.contains("source_note"));
        assert!(!prompt_text.contains("fixture_author_note"));
        assert!(!prompt_text.contains("accepted_by"));
        assert!(!prompt_text.contains("accepted_at"));
        assert!(training_text.contains("# Mind accepted-knowledge judge training"));
        assert!(training_text.contains("Do not emit a bare verdict"));
        assert!(training_text.contains("The `KnowledgeJudgePacket` is the only evidence"));
        assert!(training_text.contains("## Response Shape Drill"));
        assert!(training_text.contains("## Semantic Duplicate Curriculum"));

        let _ = std::fs::remove_file(path);
    }
}

struct RedactedAcceptedKnowledge<'record> {
    record: &'record AcceptedKnowledge,
}

impl<'record> RedactedAcceptedKnowledge<'record> {
    fn new(record: &'record AcceptedKnowledge) -> Self {
        Self { record }
    }

    fn to_text(&self) -> String {
        format!(
            "({} {:?} [redacted statement sha256:{}])",
            self.record.identity.as_str(),
            self.record.subject,
            Sha256Text::new(self.record.statement.as_str()).hex()
        )
    }
}

struct Sha256Text<'text> {
    text: &'text str,
}

impl<'text> Sha256Text<'text> {
    fn new(text: &'text str) -> Self {
        Self { text }
    }

    fn hex(&self) -> String {
        Sha256::digest(self.text.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }
}

pub(crate) struct AcceptedKnowledgeLedger<'tables> {
    tables: &'tables MindTables,
    judge: KnowledgeJudgePort,
}

impl<'tables> AcceptedKnowledgeLedger<'tables> {
    pub(crate) fn new(tables: &'tables MindTables, judge: KnowledgeJudgePort) -> Self {
        Self { tables, judge }
    }

    pub(crate) fn submit(&self, envelope: MindEnvelope) -> Result<MindReply> {
        let actor = envelope.actor().clone();
        let MindEnvelope { request, .. } = envelope;
        match request {
            MindRequest::Submit(submission) => {
                Ok(KnowledgeAdmission::new(self.tables, actor, submission)
                    .reply_from_judge(self.judge.as_ref()))
            }
            _ => Ok(Self::unimplemented()),
        }
    }

    pub(crate) fn query(&self, envelope: MindEnvelope) -> Result<MindReply> {
        let MindEnvelope { request, .. } = envelope;
        match request {
            MindRequest::Get(identity) => Ok(KnowledgeQueryEngine::new(
                self.tables.accepted_knowledge_records()?,
            )
            .reply(identity)),
            _ => Ok(Self::unimplemented()),
        }
    }

    fn unimplemented() -> MindReply {
        MindReply::MindRequestUnimplemented(signal_mind::MindRequestUnimplemented {
            reason: signal_mind::MindUnimplementedReason::NotInPrototypeScope,
        })
    }
}

struct KnowledgeAdmission<'tables> {
    tables: &'tables MindTables,
    actor: ActorName,
    submission: KnowledgeSubmission,
}

impl<'tables> KnowledgeAdmission<'tables> {
    fn new(tables: &'tables MindTables, actor: ActorName, submission: KnowledgeSubmission) -> Self {
        Self {
            tables,
            actor,
            submission,
        }
    }

    fn reply_from_judge(&self, judge: &dyn KnowledgeJudge) -> MindReply {
        let accepted = match self.tables.accepted_knowledge_records() {
            Ok(records) => records,
            Err(_) => {
                return MindReply::Rejected(KnowledgeRejectionReason::PersistenceRejected);
            }
        };
        if let Some(identity) =
            ExactKnowledgeDuplicate::new(&self.submission, &accepted).accepted_identity()
        {
            return MindReply::Rejected(KnowledgeRejectionReason::SemanticDuplicate(identity));
        }
        let packet = KnowledgeJudgePacket {
            subject: self.submission.subject,
            statement: self.submission.statement.clone(),
            relevant_neighbors: accepted,
        };

        let request =
            KnowledgeJudgeRequest::new(MindRequest::Submit(self.submission.clone()), packet);
        let decision = judge.judge(request);
        let reply = match decision.verdict() {
            KnowledgeJudgeVerdict::Accept => self.apply_acceptance(),
            KnowledgeJudgeVerdict::Reject(reason) => MindReply::Rejected(reason.clone()),
        };
        judge.record_applied_decision(KnowledgeJudgeAppliedDecision::new(
            MindRequest::Submit(self.submission.clone()),
            decision,
            reply.clone(),
        ));
        reply
    }

    fn apply_acceptance(&self) -> MindReply {
        match KnowledgeAcceptanceApplication::new(
            self.tables,
            self.actor.clone(),
            self.submission.clone(),
        )
        .accepted()
        {
            Ok(identity) => MindReply::Accepted(identity),
            Err(reason) => MindReply::Rejected(reason),
        }
    }
}

struct ExactKnowledgeDuplicate<'submission, 'records> {
    submission: &'submission KnowledgeSubmission,
    records: &'records [AcceptedKnowledge],
}

impl<'submission, 'records> ExactKnowledgeDuplicate<'submission, 'records> {
    fn new(
        submission: &'submission KnowledgeSubmission,
        records: &'records [AcceptedKnowledge],
    ) -> Self {
        Self {
            submission,
            records,
        }
    }

    fn accepted_identity(&self) -> Option<KnowledgeIdentity> {
        self.records
            .iter()
            .find(|record| {
                record.subject == self.submission.subject
                    && record.statement == self.submission.statement
            })
            .map(|record| record.identity.clone())
    }
}

struct KnowledgeAcceptanceApplication<'tables> {
    tables: &'tables MindTables,
    actor: ActorName,
    submission: KnowledgeSubmission,
}

impl<'tables> KnowledgeAcceptanceApplication<'tables> {
    fn new(tables: &'tables MindTables, actor: ActorName, submission: KnowledgeSubmission) -> Self {
        Self {
            tables,
            actor,
            submission,
        }
    }

    fn accepted(self) -> std::result::Result<KnowledgeIdentity, KnowledgeRejectionReason> {
        let existing = self
            .tables
            .accepted_knowledge_records()
            .map_err(|_| KnowledgeRejectionReason::PersistenceRejected)?;
        let identity = KnowledgeIdentityMint::from_records(&existing).next_identity()?;
        let accepted_at = crate::tables::StoreClock::system()
            .timestamp()
            .map_err(|_| KnowledgeRejectionReason::PersistenceRejected)?;
        let record = AcceptedKnowledge {
            identity: identity.clone(),
            subject: self.submission.subject,
            statement: self.submission.statement,
            accepted_by: self.actor,
            accepted_at,
        };
        self.tables
            .assert_accepted_knowledge(record)
            .map_err(|_| KnowledgeRejectionReason::PersistenceRejected)?;
        Ok(identity)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct KnowledgeIdentityMint {
    used_identities: BTreeSet<String>,
}

impl KnowledgeIdentityMint {
    fn from_records(records: &[AcceptedKnowledge]) -> Self {
        Self {
            used_identities: records
                .iter()
                .map(|record| record.identity.as_str().to_owned())
                .collect(),
        }
    }

    fn next_identity(&self) -> std::result::Result<KnowledgeIdentity, KnowledgeRejectionReason> {
        for code_length in
            KNOWLEDGE_IDENTITY_MINIMUM_CODE_LENGTH..=KNOWLEDGE_IDENTITY_MAXIMUM_CODE_LENGTH
        {
            if let Some(identity) = self.identity_for_code_length(code_length)? {
                return Ok(identity);
            }
        }
        Err(KnowledgeRejectionReason::PersistenceRejected)
    }

    fn identity_for_code_length(
        &self,
        code_length: usize,
    ) -> std::result::Result<Option<KnowledgeIdentity>, KnowledgeRejectionReason> {
        let range = KnowledgeIdentityCodeRange::new(code_length);
        for _ in 0..RANDOM_IDENTITY_ATTEMPTS_PER_LENGTH {
            let identity = range.random_identity()?;
            if !self.used_identities.contains(identity.as_str()) {
                return Ok(Some(identity));
            }
        }
        Ok(range.first_available_identity(&self.used_identities))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct KnowledgeIdentityCodeRange {
    first_value: u64,
    value_count: u64,
}

impl KnowledgeIdentityCodeRange {
    fn new(code_length: usize) -> Self {
        let first_value = if code_length == KNOWLEDGE_IDENTITY_MINIMUM_CODE_LENGTH {
            0
        } else {
            Self::radix_power(code_length - 1)
        };
        let next_length_first_value = Self::radix_power(code_length);
        Self {
            first_value,
            value_count: next_length_first_value - first_value,
        }
    }

    fn random_identity(&self) -> std::result::Result<KnowledgeIdentity, KnowledgeRejectionReason> {
        let mut bytes = [0_u8; 8];
        getrandom::fill(&mut bytes).map_err(|_| KnowledgeRejectionReason::PersistenceRejected)?;
        let offset = u64::from_be_bytes(bytes) % self.value_count;
        Ok(KnowledgeIdentity::new(Self::code_from_value(
            self.first_value + offset,
        )))
    }

    fn first_available_identity(
        &self,
        used_identities: &BTreeSet<String>,
    ) -> Option<KnowledgeIdentity> {
        let last_value = self.first_value + self.value_count;
        (self.first_value..last_value)
            .map(Self::code_from_value)
            .find(|identity| !used_identities.contains(identity))
            .map(KnowledgeIdentity::new)
    }

    fn code_from_value(mut value: u64) -> String {
        let mut digits = Vec::new();
        while value > 0 {
            let digit = (value % KNOWLEDGE_IDENTITY_CODE_RADIX) as u8;
            digits.push(Self::digit_character(digit));
            value /= KNOWLEDGE_IDENTITY_CODE_RADIX;
        }
        while digits.len() < KNOWLEDGE_IDENTITY_MINIMUM_CODE_LENGTH {
            digits.push('0');
        }
        digits.iter().rev().collect()
    }

    fn digit_character(digit: u8) -> char {
        match digit {
            0..=9 => char::from(b'0' + digit),
            10..=35 => char::from(b'a' + digit - 10),
            _ => unreachable!("base36 digit is constrained by modulo"),
        }
    }

    fn radix_power(exponent: usize) -> u64 {
        (0..exponent).fold(1, |value, _| value * KNOWLEDGE_IDENTITY_CODE_RADIX)
    }
}

struct KnowledgeQueryEngine {
    records: Vec<AcceptedKnowledge>,
}

impl KnowledgeQueryEngine {
    fn new(records: Vec<AcceptedKnowledge>) -> Self {
        Self { records }
    }

    fn reply(&self, identity: KnowledgeIdentity) -> MindReply {
        self.records
            .iter()
            .find(|record| record.identity == identity)
            .map(AcceptedKnowledge::public_record)
            .map(MindReply::Found)
            .unwrap_or(MindReply::NotFound)
    }
}
