use std::collections::{HashSet, VecDeque};
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use nota_next::{NotaEncode, NotaSource};
use signal_agent::{
    ChatMessage, ChatTranscript, CompletionText, Input as AgentInput, MaximumOutputTokens,
    ModelName, Output as AgentOutput, OutputMode, Prompt, PromptOptions, ProviderName,
    ReasoningEffort, SystemText, TemperatureMilli, ThinkingMode,
};
use signal_mind::{
    AcceptedKnowledge, AcceptedKnowledgeDraft, AcceptedKnowledgeView, ActorName, CandidateSummary,
    CurrentView, KnowledgeAccepted, KnowledgeCandidate, KnowledgeDomain, KnowledgeDomainCandidate,
    KnowledgeDomainKey, KnowledgeDomainSelector, KnowledgeEndpointSelector, KnowledgeEntity,
    KnowledgeFixturePolicy, KnowledgeIdentifier, KnowledgeJudgePacket, KnowledgeJudgeVerdict,
    KnowledgeList, KnowledgeQuery, KnowledgeRecordDraft, KnowledgeRecordHeader, KnowledgeRejection,
    KnowledgeRejectionReason, KnowledgeRelation, KnowledgeRelationDraft, KnowledgeRelationEndpoint,
    KnowledgeRelationKind, KnowledgeRelationRule, KnowledgeSource, KnowledgeStableKey,
    KnowledgeStatement, MindReply, MindRequest, QueryLimit, RelationSelector, RetryHint,
    StructuralRejection, StructuralRejectionReason, TextBody,
};
use triad_runtime::{FrameBody, LengthPrefixedCodec};

use crate::{MindEnvelope, MindKnowledgeJudgeAgentConfiguration, MindTables, Result};

const KNOWLEDGE_JUDGE_MALFORMED_RETRY_HINT: &str =
    "retry with one valid KnowledgeJudgeVerdict NOTA value";

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
            .unwrap_or_else(|| {
                KnowledgeJudgeVerdict::Reject(KnowledgeRejection::new_semantic(
                    KnowledgeRejectionReason::MeaningUnclear,
                    "fixture judge has no verdict",
                ))
            })
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
}

#[derive(Clone, Debug)]
struct KnowledgeJudgePrompt<'packet> {
    packet: &'packet KnowledgeJudgePacket,
    provider_name: Option<&'packet str>,
    model_name: Option<&'packet str>,
    maximum_output_tokens: Option<u64>,
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

    fn unavailable_verdict(error: AgentKnowledgeJudgeError) -> KnowledgeJudgeVerdict {
        KnowledgeJudgeVerdict::Reject(KnowledgeRejection {
            reason: KnowledgeRejectionReason::MeaningUnclear,
            candidate_summary: CandidateSummary {
                summary: TextBody::new(format!("knowledge judge unavailable: {error}")),
            },
            retry_hint: Some(RetryHint {
                hint: TextBody::new(KNOWLEDGE_JUDGE_MALFORMED_RETRY_HINT),
            }),
        })
    }
}

impl KnowledgeJudge for AgentKnowledgeJudge {
    fn judge(&self, packet: KnowledgeJudgePacket) -> KnowledgeJudgeVerdict {
        let prompt = KnowledgeJudgePrompt::new(
            &packet,
            self.configuration.provider_name.as_deref(),
            self.configuration.model_name.as_deref(),
            self.configuration.maximum_output_tokens,
        )
        .into_agent_prompt();
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
        }
    }

    fn socket_path(&self) -> &Path {
        &self.socket_path
    }
}

impl<'packet> KnowledgeJudgePrompt<'packet> {
    fn new(
        packet: &'packet KnowledgeJudgePacket,
        provider_name: Option<&'packet str>,
        model_name: Option<&'packet str>,
        maximum_output_tokens: Option<u64>,
    ) -> Self {
        Self {
            packet,
            provider_name,
            model_name,
            maximum_output_tokens,
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
            "You are Mind's accepted-knowledge judge.\n\n\
             Judge whether one candidate belongs in Mind's accepted-knowledge store. Semantic \
             judgment belongs to you: whether the candidate is knowledge, meaningful, true enough, \
             in-domain, private or unauthorized, duplicate, conflicting, superseding, supported, or \
             better represented as another accepted-knowledge shape.\n\n\
             Deterministic code already handles typed structure, endpoint preflight, relation \
             domain/range validation, storage, and query views. Do not ask for source or \
             provenance unless the candidate itself makes source part of the knowledge. Source is \
             stored only as accepted knowledge when it appears in your accepted draft. The \
             packet's `FixtureOnly` field is a legacy contract field; when this prompt reaches \
             you through AgentKnowledgeJudge, do not reject solely because that field says \
             FixtureOnly.\n\n\
             Reject tasks, logs, receipts, admission receipts, process chatter, private or \
             unauthorized material, vague prose, unsupported or false content, wrong-domain \
             content, duplicates, conflicts, and supersessions that should not be stored as new \
             knowledge.\n\n\
             Return exactly one KnowledgeJudgeVerdict NOTA value and nothing else: no markdown, no \
             prose around it, no JSON, no code fence. A valid accept is shaped like {accept}. A \
             valid reject is shaped like {reject}. Use only the typed variants in the packet and \
             these grammar examples.",
            accept = Self::accept_example(),
            reject = Self::reject_example(),
        )
    }

    fn user_prompt(&self) -> String {
        format!(
            "KnowledgeJudgePacket under judgment:\n{}\n\n\
             Allowed relation rules are advisory semantic context; structural relation endpoint \
             validation has already run. Relevant neighbors are the only accepted records you may \
             use for duplicate, conflict, support, supersession, and relation decisions.\n\n\
             Return one KnowledgeJudgeVerdict.",
            self.packet.to_nota(),
        )
    }

    fn prompt_options(&self) -> PromptOptions {
        PromptOptions::new(
            self.model_name
                .map(|model| ModelName::new(model.to_owned())),
            self.provider_name
                .map(|provider| ProviderName::new(provider.to_owned())),
            Some(TemperatureMilli::new(0)),
            self.maximum_output_tokens.map(MaximumOutputTokens::new),
            OutputMode::Nota,
            Some(ReasoningEffort::Low),
            Some(ThinkingMode::Disabled),
        )
    }

    fn accept_example() -> String {
        KnowledgeJudgeVerdict::Accept(AcceptedKnowledgeDraft {
            records: vec![KnowledgeRecordDraft::Domain(KnowledgeDomainCandidate {
                domain_key: KnowledgeDomainKey::from_canonical("domain:component")
                    .expect("example domain key is valid"),
                name: TextBody::new("Component"),
                description: None,
            })],
            relations: Vec::new(),
        })
        .to_nota()
    }

    fn reject_example() -> String {
        KnowledgeJudgeVerdict::Reject(KnowledgeRejection {
            reason: KnowledgeRejectionReason::NotKnowledge,
            candidate_summary: CandidateSummary {
                summary: TextBody::new("candidate is a task, not durable knowledge"),
            },
            retry_hint: Some(RetryHint {
                hint: TextBody::new("submit a specific declarative knowledge candidate"),
            }),
        })
        .to_nota()
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
            MindRequest::SubmitKnowledge(submission) => {
                let admission = KnowledgeAdmission::new(
                    self.tables,
                    actor,
                    submission.candidate.clone(),
                    submission.fixture_policy,
                );
                Ok(admission.reply_from_judge(self.judge.as_ref()))
            }
            _ => Ok(Self::unimplemented()),
        }
    }

    pub(crate) fn query(&self, envelope: MindEnvelope) -> Result<MindReply> {
        let MindEnvelope { request, .. } = envelope;
        match request {
            MindRequest::QueryKnowledge(query) => Ok(KnowledgeQueryEngine::new(
                self.tables.accepted_knowledge_records()?,
            )
            .reply(query)),
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
    candidate: KnowledgeCandidate,
    fixture_policy: KnowledgeFixturePolicy,
}

impl<'tables> KnowledgeAdmission<'tables> {
    fn new(
        tables: &'tables MindTables,
        actor: ActorName,
        candidate: KnowledgeCandidate,
        fixture_policy: KnowledgeFixturePolicy,
    ) -> Self {
        Self {
            tables,
            actor,
            candidate,
            fixture_policy,
        }
    }

    fn reply_from_judge(&self, judge: &dyn KnowledgeJudge) -> MindReply {
        if let Some(rejection) = self.preflight_candidate() {
            return MindReply::KnowledgeRejected(rejection);
        }

        let packet = KnowledgeJudgePacket {
            candidate: self.candidate.clone(),
            relevant_neighbors: self.tables.accepted_knowledge_records().unwrap_or_default(),
            allowed_relations: KnowledgeRelationRules::all(),
            fixture_policy: self.fixture_policy.clone(),
        };

        match judge.judge(packet) {
            KnowledgeJudgeVerdict::Accept(draft) => self.apply_draft(draft),
            KnowledgeJudgeVerdict::Reject(rejection) => MindReply::KnowledgeRejected(rejection),
        }
    }

    fn preflight_candidate(&self) -> Option<KnowledgeRejection> {
        match &self.candidate {
            KnowledgeCandidate::Relation(candidate) => {
                let records =
                    match self.tables.accepted_knowledge_records() {
                        Ok(records) => records,
                        Err(_error) => {
                            return Some(self.structural_rejection(
                                StructuralRejectionReason::PersistenceRejected,
                            ));
                        }
                    };
                let accepted = KnowledgeRecords::new(records);
                let source = accepted.resolve(&candidate.source);
                let target = accepted.resolve(&candidate.target);
                match (source, target) {
                    (None, _) => Some(self.structural_rejection(
                        StructuralRejectionReason::MissingEndpoint(candidate.source.clone()),
                    )),
                    (_, None) => Some(self.structural_rejection(
                        StructuralRejectionReason::MissingEndpoint(candidate.target.clone()),
                    )),
                    (Some(source), Some(target)) => candidate
                        .kind
                        .validate_endpoints(&source.endpoint, &target.endpoint)
                        .err()
                        .map(|mismatch| {
                            self.structural_rejection(
                                StructuralRejectionReason::RelationDomainRangeViolation(mismatch),
                            )
                        }),
                }
            }
            _ => None,
        }
    }

    fn apply_draft(&self, draft: AcceptedKnowledgeDraft) -> MindReply {
        match KnowledgeDraftApplication::new(self.tables, self.actor.clone(), draft).accepted() {
            Ok(records) => MindReply::KnowledgeAccepted(KnowledgeAccepted {
                accepted: AcceptedKnowledgeView { records },
            }),
            Err(rejection) => MindReply::KnowledgeRejected(rejection),
        }
    }

    fn structural_rejection(&self, reason: StructuralRejectionReason) -> KnowledgeRejection {
        KnowledgeRejection::new_structural(reason, self.candidate.summary())
    }
}

struct KnowledgeDraftApplication<'tables> {
    tables: &'tables MindTables,
    actor: ActorName,
    draft: AcceptedKnowledgeDraft,
}

impl<'tables> KnowledgeDraftApplication<'tables> {
    fn new(tables: &'tables MindTables, actor: ActorName, draft: AcceptedKnowledgeDraft) -> Self {
        Self {
            tables,
            actor,
            draft,
        }
    }

    fn accepted(self) -> std::result::Result<Vec<AcceptedKnowledge>, KnowledgeRejection> {
        if self.draft.records.is_empty() && self.draft.relations.is_empty() {
            return Err(KnowledgeRejection::new_structural(
                StructuralRejectionReason::EmptyAcceptedDraft,
                "empty accepted knowledge draft",
            ));
        }

        let existing = self.tables.accepted_knowledge_records().map_err(|_| {
            KnowledgeRejection::new_structural(
                StructuralRejectionReason::PersistenceRejected,
                "accepted knowledge store is not ready",
            )
        })?;
        let mut records = KnowledgeRecords::new(existing);
        let mut accepted = Vec::new();
        let mut minted = KnowledgeIdentifierMint::new(records.records.len() as u64);

        for draft in self.draft.records {
            let record = KnowledgeRecordMaterializer::new(self.actor.clone(), &mut minted)
                .materialize(draft)
                .map_err(|reason| {
                    KnowledgeRejection::new_structural(reason, "accepted knowledge draft")
                })?;
            if record
                .stable_key()
                .is_some_and(|stable_key| records.contains_stable_key(stable_key))
            {
                return Err(KnowledgeRejection::new_structural(
                    StructuralRejectionReason::PersistenceRejected,
                    "duplicate accepted knowledge stable key",
                ));
            }
            records.push(record.clone());
            accepted.push(record);
        }

        for draft in self.draft.relations {
            let relation = KnowledgeRelationMaterializer::new(self.actor.clone(), &mut minted)
                .materialize(&records, draft)
                .map_err(|reason| {
                    KnowledgeRejection::new_structural(reason, "accepted knowledge relation draft")
                })?;
            records.push(relation.clone());
            accepted.push(relation);
        }

        for record in accepted.iter().cloned() {
            self.tables.assert_accepted_knowledge(record).map_err(|_| {
                KnowledgeRejection::new_structural(
                    StructuralRejectionReason::PersistenceRejected,
                    "accepted knowledge persistence failed",
                )
            })?;
        }

        Ok(accepted)
    }
}

struct KnowledgeRecordMaterializer<'mint> {
    actor: ActorName,
    mint: &'mint mut KnowledgeIdentifierMint,
}

impl<'mint> KnowledgeRecordMaterializer<'mint> {
    fn new(actor: ActorName, mint: &'mint mut KnowledgeIdentifierMint) -> Self {
        Self { actor, mint }
    }

    fn materialize(
        &mut self,
        draft: KnowledgeRecordDraft,
    ) -> std::result::Result<AcceptedKnowledge, StructuralRejectionReason> {
        match draft {
            KnowledgeRecordDraft::Entity(candidate) => {
                let header = self.header(candidate.stable_key.clone())?;
                Ok(AcceptedKnowledge::Entity(KnowledgeEntity {
                    header,
                    name: candidate.name,
                    description: candidate.description,
                    domains: candidate.domains,
                }))
            }
            KnowledgeRecordDraft::Statement(candidate) => {
                let header = self.header(candidate.stable_key.clone())?;
                Ok(AcceptedKnowledge::Statement(KnowledgeStatement {
                    header,
                    body: candidate.body,
                    about: candidate.about,
                    domains: candidate.domains,
                }))
            }
            KnowledgeRecordDraft::Domain(candidate) => {
                let stable_key = KnowledgeStableKey::from_canonical(candidate.domain_key.as_str())
                    .map_err(StructuralRejectionReason::InvalidStableKey)?;
                let header = self.header(Some(stable_key))?;
                Ok(AcceptedKnowledge::Domain(KnowledgeDomain {
                    header,
                    domain_key: candidate.domain_key,
                    name: candidate.name,
                    description: candidate.description,
                }))
            }
            KnowledgeRecordDraft::Source(candidate) => {
                let header = self.header(candidate.stable_key.clone())?;
                Ok(AcceptedKnowledge::Source(KnowledgeSource {
                    header,
                    locator: candidate.locator,
                    description: candidate.description,
                }))
            }
        }
    }

    fn header(
        &mut self,
        stable_key: Option<KnowledgeStableKey>,
    ) -> std::result::Result<KnowledgeRecordHeader, StructuralRejectionReason> {
        Ok(KnowledgeRecordHeader {
            identifier: self.mint.next_identifier()?,
            stable_key,
            accepted_by: self.actor.clone(),
            accepted_at: self.mint.timestamp()?,
        })
    }
}

struct KnowledgeRelationMaterializer<'mint> {
    actor: ActorName,
    mint: &'mint mut KnowledgeIdentifierMint,
}

impl<'mint> KnowledgeRelationMaterializer<'mint> {
    fn new(actor: ActorName, mint: &'mint mut KnowledgeIdentifierMint) -> Self {
        Self { actor, mint }
    }

    fn materialize(
        &mut self,
        records: &KnowledgeRecords,
        draft: KnowledgeRelationDraft,
    ) -> std::result::Result<AcceptedKnowledge, StructuralRejectionReason> {
        let source = records
            .resolve(&draft.source)
            .ok_or_else(|| StructuralRejectionReason::MissingEndpoint(draft.source.clone()))?;
        let target = records
            .resolve(&draft.target)
            .ok_or_else(|| StructuralRejectionReason::MissingEndpoint(draft.target.clone()))?;
        draft
            .kind
            .validate_endpoints(&source.endpoint, &target.endpoint)
            .map_err(StructuralRejectionReason::RelationDomainRangeViolation)?;

        Ok(AcceptedKnowledge::Relation(KnowledgeRelation {
            header: KnowledgeRecordHeader {
                identifier: self.mint.next_identifier()?,
                stable_key: None,
                accepted_by: self.actor.clone(),
                accepted_at: self.mint.timestamp()?,
            },
            kind: draft.kind,
            source: source.endpoint,
            target: target.endpoint,
            note: draft.note,
        }))
    }
}

struct KnowledgeIdentifierMint {
    sequence: u64,
}

impl KnowledgeIdentifierMint {
    fn new(sequence: u64) -> Self {
        Self { sequence }
    }

    fn next_identifier(
        &mut self,
    ) -> std::result::Result<KnowledgeIdentifier, StructuralRejectionReason> {
        let value = self.timestamp()?.value();
        let sequence = self.sequence;
        self.sequence = self.sequence.saturating_add(1);
        Ok(KnowledgeIdentifier::new(format!("k{value:x}{sequence:x}")))
    }

    fn timestamp(
        &self,
    ) -> std::result::Result<signal_mind::TimestampNanos, StructuralRejectionReason> {
        crate::tables::StoreClock::system()
            .timestamp()
            .map_err(|_| StructuralRejectionReason::PersistenceRejected)
    }
}

struct KnowledgeRecords {
    records: Vec<AcceptedKnowledge>,
}

impl KnowledgeRecords {
    fn new(records: Vec<AcceptedKnowledge>) -> Self {
        Self { records }
    }

    fn push(&mut self, record: AcceptedKnowledge) {
        self.records.push(record);
    }

    fn resolve(&self, selector: &KnowledgeEndpointSelector) -> Option<KnowledgeResolvedEndpoint> {
        self.records
            .iter()
            .find(|record| match selector {
                KnowledgeEndpointSelector::Identifier(identifier) => {
                    record.identifier() == identifier
                }
                KnowledgeEndpointSelector::StableKey(stable_key) => {
                    record.stable_key() == Some(stable_key)
                }
            })
            .map(KnowledgeResolvedEndpoint::new)
    }

    fn contains_stable_key(&self, stable_key: &KnowledgeStableKey) -> bool {
        self.records
            .iter()
            .any(|record| record.stable_key() == Some(stable_key))
    }

    fn superseded_identifiers(&self) -> HashSet<KnowledgeIdentifier> {
        self.records
            .iter()
            .filter_map(|record| match record {
                AcceptedKnowledge::Relation(relation)
                    if relation.kind == KnowledgeRelationKind::Supersedes =>
                {
                    Some(relation.target.identifier.clone())
                }
                _ => None,
            })
            .collect()
    }

    fn domain_identifiers(
        &self,
        selector: KnowledgeDomainSelector,
    ) -> HashSet<KnowledgeIdentifier> {
        let mut domains = HashSet::new();
        match selector {
            KnowledgeDomainSelector::Any => {
                for record in &self.records {
                    if let AcceptedKnowledge::Domain(domain) = record {
                        domains.insert(domain.header.identifier.clone());
                    }
                }
            }
            KnowledgeDomainSelector::Direct(domain_key) => {
                self.insert_domain_identifier(&domain_key, &mut domains);
            }
            KnowledgeDomainSelector::WithDescendants(domain_key) => {
                self.insert_domain_identifier(&domain_key, &mut domains);
                self.insert_descendant_domain_identifiers(&domain_key, &mut domains);
            }
        }
        domains
    }

    fn insert_domain_identifier(
        &self,
        domain_key: &KnowledgeDomainKey,
        domains: &mut HashSet<KnowledgeIdentifier>,
    ) {
        for record in &self.records {
            if let AcceptedKnowledge::Domain(domain) = record
                && &domain.domain_key == domain_key
            {
                domains.insert(domain.header.identifier.clone());
            }
        }
    }

    fn insert_descendant_domain_identifiers(
        &self,
        domain_key: &KnowledgeDomainKey,
        domains: &mut HashSet<KnowledgeIdentifier>,
    ) {
        let mut changed = true;
        while changed {
            changed = false;
            for record in &self.records {
                let AcceptedKnowledge::Relation(relation) = record else {
                    continue;
                };
                match relation.kind {
                    KnowledgeRelationKind::BroaderThan
                        if domains.contains(&relation.source.identifier)
                            && domains.insert(relation.target.identifier.clone()) =>
                    {
                        changed = true;
                    }
                    KnowledgeRelationKind::NarrowerThan
                        if domains.contains(&relation.target.identifier)
                            && domains.insert(relation.source.identifier.clone()) =>
                    {
                        changed = true;
                    }
                    _ => {}
                }
            }
        }
        self.insert_domain_identifier(domain_key, domains);
    }

    fn classified_record_identifiers(
        &self,
        domains: &HashSet<KnowledgeIdentifier>,
    ) -> HashSet<KnowledgeIdentifier> {
        self.records
            .iter()
            .filter_map(|record| match record {
                AcceptedKnowledge::Relation(relation)
                    if relation.kind == KnowledgeRelationKind::ClassifiedAs
                        && domains.contains(&relation.target.identifier) =>
                {
                    Some(relation.source.identifier.clone())
                }
                _ => None,
            })
            .collect()
    }

    fn domain_keys_for_identifiers(
        &self,
        domains: &HashSet<KnowledgeIdentifier>,
    ) -> HashSet<KnowledgeDomainKey> {
        self.records
            .iter()
            .filter_map(|record| match record {
                AcceptedKnowledge::Domain(domain)
                    if domains.contains(&domain.header.identifier) =>
                {
                    Some(domain.domain_key.clone())
                }
                _ => None,
            })
            .collect()
    }
}

struct KnowledgeResolvedEndpoint {
    endpoint: KnowledgeRelationEndpoint,
}

impl KnowledgeResolvedEndpoint {
    fn new(record: &AcceptedKnowledge) -> Self {
        Self {
            endpoint: KnowledgeRelationEndpoint {
                identifier: record.identifier().clone(),
                stable_key: record.stable_key().cloned(),
                kind: record.kind(),
            },
        }
    }
}

struct KnowledgeQueryEngine {
    records: KnowledgeRecords,
}

impl KnowledgeQueryEngine {
    fn new(records: Vec<AcceptedKnowledge>) -> Self {
        Self {
            records: KnowledgeRecords::new(records),
        }
    }

    fn reply(&self, query: KnowledgeQuery) -> MindReply {
        match query {
            KnowledgeQuery::GetByIdentifier(identifier) => self.list(
                self.records
                    .records
                    .iter()
                    .filter(|record| record.identifier() == &identifier),
            ),
            KnowledgeQuery::GetByStableKey(stable_key) => self.list(
                self.records
                    .records
                    .iter()
                    .filter(|record| record.stable_key() == Some(&stable_key)),
            ),
            KnowledgeQuery::ListByKind(kind, current_view) => {
                let superseded = self.records.superseded_identifiers();
                self.list(self.records.records.iter().filter(|record| {
                    record.kind() == kind
                        && CurrentKnowledge::new(current_view, &superseded).accepts(record)
                }))
            }
            KnowledgeQuery::ListByDomain(selector, current_view) => {
                self.list_by_domain(selector, current_view)
            }
            KnowledgeQuery::ListRelations(selector, current_view) => {
                self.list_relations(selector, current_view)
            }
        }
    }

    fn list<'record>(
        &self,
        records: impl Iterator<Item = &'record AcceptedKnowledge>,
    ) -> MindReply {
        MindReply::KnowledgeList(KnowledgeList {
            records: records.cloned().collect(),
            has_more: false,
        })
    }

    fn list_by_domain(
        &self,
        selector: KnowledgeDomainSelector,
        current_view: CurrentView,
    ) -> MindReply {
        let superseded = self.records.superseded_identifiers();
        let current = CurrentKnowledge::new(current_view, &superseded);
        if matches!(selector, KnowledgeDomainSelector::Any) {
            return self.list(
                self.records
                    .records
                    .iter()
                    .filter(|record| current.accepts(record)),
            );
        }

        let domain_identifiers = self.records.domain_identifiers(selector.clone());
        let domain_keys = self
            .records
            .domain_keys_for_identifiers(&domain_identifiers);
        let classified = self
            .records
            .classified_record_identifiers(&domain_identifiers);
        self.list(self.records.records.iter().filter(|record| {
            current.accepts(record)
                && (record
                    .domain_keys()
                    .iter()
                    .any(|domain| domain_keys.contains(*domain))
                    || classified.contains(record.identifier()))
        }))
    }

    fn list_relations(&self, selector: RelationSelector, current_view: CurrentView) -> MindReply {
        let superseded = self.records.superseded_identifiers();
        let current = CurrentKnowledge::new(current_view, &superseded);
        let mut records = self
            .records
            .records
            .iter()
            .filter(|record| {
                let AcceptedKnowledge::Relation(relation) = record else {
                    return false;
                };
                current.accepts(record)
                    && selector.kind.is_none_or(|kind| relation.kind == kind)
                    && selector
                        .source
                        .as_ref()
                        .is_none_or(|source| &relation.source.identifier == source)
                    && selector
                        .target
                        .as_ref()
                        .is_none_or(|target| &relation.target.identifier == target)
            })
            .cloned()
            .collect::<Vec<_>>();
        let limited = KnowledgeLimit::new(selector.limit).apply(&mut records);
        MindReply::KnowledgeList(KnowledgeList {
            records,
            has_more: limited.has_more,
        })
    }
}

struct KnowledgeLimit {
    limit: usize,
}

impl KnowledgeLimit {
    fn new(limit: QueryLimit) -> Self {
        Self {
            limit: usize::from(limit.into_u16()),
        }
    }

    fn apply(&self, records: &mut Vec<AcceptedKnowledge>) -> LimitedKnowledge {
        let has_more = records.len() > self.limit;
        records.truncate(self.limit);
        LimitedKnowledge { has_more }
    }
}

struct LimitedKnowledge {
    has_more: bool,
}

struct CurrentKnowledge<'a> {
    current_view: CurrentView,
    superseded: &'a HashSet<KnowledgeIdentifier>,
}

impl<'a> CurrentKnowledge<'a> {
    fn new(current_view: CurrentView, superseded: &'a HashSet<KnowledgeIdentifier>) -> Self {
        Self {
            current_view,
            superseded,
        }
    }

    fn accepts(&self, record: &AcceptedKnowledge) -> bool {
        match self.current_view {
            CurrentView::IncludeSuperseded => true,
            CurrentView::CurrentOnly => !self.superseded.contains(record.identifier()),
        }
    }
}

struct KnowledgeRelationRules;

impl KnowledgeRelationRules {
    fn all() -> Vec<KnowledgeRelationRule> {
        KnowledgeRelationKind::ALL
            .into_iter()
            .map(|kind| KnowledgeRelationRule {
                kind,
                source_kinds: kind.expected_source_kinds(),
                target_kinds: kind.expected_target_kinds(),
            })
            .collect()
    }
}

trait AcceptedKnowledgeAccess {
    fn identifier(&self) -> &KnowledgeIdentifier;
    fn stable_key(&self) -> Option<&KnowledgeStableKey>;
    fn domain_keys(&self) -> Vec<&KnowledgeDomainKey>;
}

impl AcceptedKnowledgeAccess for AcceptedKnowledge {
    fn identifier(&self) -> &KnowledgeIdentifier {
        match self {
            Self::Entity(record) => &record.header.identifier,
            Self::Statement(record) => &record.header.identifier,
            Self::Relation(record) => &record.header.identifier,
            Self::Domain(record) => &record.header.identifier,
            Self::Source(record) => &record.header.identifier,
        }
    }

    fn stable_key(&self) -> Option<&KnowledgeStableKey> {
        match self {
            Self::Entity(record) => record.header.stable_key.as_ref(),
            Self::Statement(record) => record.header.stable_key.as_ref(),
            Self::Relation(record) => record.header.stable_key.as_ref(),
            Self::Domain(record) => record.header.stable_key.as_ref(),
            Self::Source(record) => record.header.stable_key.as_ref(),
        }
    }

    fn domain_keys(&self) -> Vec<&KnowledgeDomainKey> {
        match self {
            Self::Entity(record) => record.domains.iter().collect(),
            Self::Statement(record) => record.domains.iter().collect(),
            Self::Domain(record) => vec![&record.domain_key],
            Self::Relation(_) | Self::Source(_) => Vec::new(),
        }
    }
}

trait KnowledgeCandidateSummary {
    fn summary(&self) -> String;
}

impl KnowledgeCandidateSummary for KnowledgeCandidate {
    fn summary(&self) -> String {
        match self {
            Self::Entity(candidate) => format!("entity {}", candidate.name.as_str()),
            Self::Statement(candidate) => format!("statement {}", candidate.body.as_str()),
            Self::Relation(candidate) => format!("relation {:?}", candidate.kind),
            Self::Domain(candidate) => format!("domain {}", candidate.domain_key.as_str()),
            Self::Source(candidate) => format!("source {}", candidate.locator.as_str()),
        }
    }
}

trait KnowledgeRejectionConstructors {
    fn new_structural(reason: StructuralRejectionReason, summary: impl Into<String>) -> Self;
    fn new_semantic(reason: KnowledgeRejectionReason, summary: impl Into<String>) -> Self;
}

impl KnowledgeRejectionConstructors for KnowledgeRejection {
    fn new_structural(reason: StructuralRejectionReason, summary: impl Into<String>) -> Self {
        Self {
            reason: KnowledgeRejectionReason::StructuralPreflightFailed(StructuralRejection {
                reason,
            }),
            candidate_summary: CandidateSummary {
                summary: TextBody::new(summary),
            },
            retry_hint: None,
        }
    }

    fn new_semantic(reason: KnowledgeRejectionReason, summary: impl Into<String>) -> Self {
        Self {
            reason,
            candidate_summary: CandidateSummary {
                summary: TextBody::new(summary),
            },
            retry_hint: Some(RetryHint {
                hint: TextBody::new("submit a more specific accepted-knowledge candidate"),
            }),
        }
    }
}
