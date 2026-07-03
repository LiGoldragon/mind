use std::collections::{BTreeSet, VecDeque};
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use nota_next::{NotaEncode, NotaSource};
use serde_json::json;
use sha2::{Digest, Sha256};
use signal_agent::{
    ChatMessage, ChatTranscript, CompletionText, Input as AgentInput, MaximumOutputTokens,
    ModelName, Output as AgentOutput, OutputMode, Prompt, PromptOptions, ProviderName,
    ReasoningEffort, SystemText, TemperatureMilli, ThinkingMode,
};
use signal_mind::{
    AcceptedKnowledge, ActorName, KnowledgeIdentity, KnowledgeJudgePacket, KnowledgeJudgeVerdict,
    KnowledgeRejectionReason, KnowledgeSubject, KnowledgeSubmission, MindReply, MindRequest,
};
use triad_runtime::{FrameBody, LengthPrefixedCodec};

use crate::{
    MindEnvelope, MindKnowledgeJudgeAgentConfiguration, MindKnowledgeJudgeTrainingSource,
    MindTables, Result,
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
    fn judge(&self, packet: KnowledgeJudgePacket) -> KnowledgeJudgeVerdict;
}

pub type KnowledgeJudgePort = Arc<dyn KnowledgeJudge>;

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
    fn judge(&self, _packet: KnowledgeJudgePacket) -> KnowledgeJudgeVerdict {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.next_verdict()
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

    fn parse_verdict(
        &self,
        completion: &CompletionText,
    ) -> std::result::Result<KnowledgeJudgeVerdict, AgentKnowledgeJudgeError> {
        NotaSource::new(completion.payload())
            .parse::<KnowledgeJudgeVerdict>()
            .map_err(|error| AgentKnowledgeJudgeError::Malformed(error.to_string()))
    }

    fn unavailable_verdict(_error: AgentKnowledgeJudgeError) -> KnowledgeJudgeVerdict {
        KnowledgeJudgeVerdict::Reject(KnowledgeRejectionReason::MeaningUnclear)
    }
}

impl KnowledgeJudge for AgentKnowledgeJudge {
    fn judge(&self, packet: KnowledgeJudgePacket) -> KnowledgeJudgeVerdict {
        let prompt = KnowledgeJudgePrompt::new(
            &packet,
            self.configuration.provider_name.as_deref(),
            self.configuration.model_name.as_deref(),
            self.configuration.maximum_output_tokens,
            self.configuration.training_source(),
        )
        .into_agent_prompt();
        JudgeDiagnostic::from_environment(&packet, &prompt, self.configuration.training_source())
            .write();
        let output = match self.call_agent(prompt) {
            Ok(output) => output,
            Err(error) => return Self::unavailable_verdict(error),
        };
        let AgentOutput::Completed(completion) = output else {
            return Self::unavailable_verdict(AgentKnowledgeJudgeError::AgentRejected(format!(
                "{output:?}"
            )));
        };
        match self.parse_verdict(&completion.completion_text) {
            Ok(verdict) => verdict,
            Err(error) => Self::unavailable_verdict(error),
        }
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
        }
    }

    fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    fn training_source(&self) -> &MindKnowledgeJudgeTrainingSource {
        &self.training_source
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
             Return exactly one KnowledgeJudgeVerdict NOTA value and nothing else: no markdown, no \
             prose around it, no JSON, no code fence. A valid accept is shaped like {accept}. A \
             valid reject is shaped like {reject}. Duplicate, conflict, vague, and wrong-subject \
             rejects are shaped like {duplicate}, {conflict}, {vague}, and {wrong_subject}.",
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
             Return one KnowledgeJudgeVerdict.",
            self.packet.to_nota(),
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
            Some(TemperatureMilli::new(0)),
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
        KnowledgeJudgeVerdict::Accept.to_nota()
    }

    fn reject_example() -> String {
        KnowledgeJudgeVerdict::Reject(KnowledgeRejectionReason::NotKnowledge).to_nota()
    }

    fn duplicate_example() -> String {
        KnowledgeJudgeVerdict::Reject(KnowledgeRejectionReason::SemanticDuplicate(
            KnowledgeIdentity::new("abcd"),
        ))
        .to_nota()
    }

    fn conflict_example() -> String {
        KnowledgeJudgeVerdict::Reject(KnowledgeRejectionReason::ConflictsAcceptedKnowledge(vec![
            KnowledgeIdentity::new("abcd"),
        ]))
        .to_nota()
    }

    fn vague_example() -> String {
        KnowledgeJudgeVerdict::Reject(KnowledgeRejectionReason::NeedsMoreSpecificShape).to_nota()
    }

    fn wrong_subject_example() -> String {
        KnowledgeJudgeVerdict::Reject(KnowledgeRejectionReason::WrongSubject(
            KnowledgeSubject::Component,
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

struct RedactedAcceptedKnowledge<'record> {
    record: &'record AcceptedKnowledge,
}

impl<'record> RedactedAcceptedKnowledge<'record> {
    fn new(record: &'record AcceptedKnowledge) -> Self {
        Self { record }
    }

    fn to_text(&self) -> String {
        format!(
            "({} {:?} [redacted statement sha256:{}] {} {})",
            self.record.identity.as_str(),
            self.record.subject,
            Sha256Text::new(self.record.statement.as_str()).hex(),
            self.record.accepted_by.as_str(),
            self.record.accepted_at.value()
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

        match judge.judge(packet) {
            KnowledgeJudgeVerdict::Accept => self.apply_acceptance(),
            KnowledgeJudgeVerdict::Reject(reason) => MindReply::Rejected(reason),
        }
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
