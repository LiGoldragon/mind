use std::collections::{BTreeSet, VecDeque};
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use signal_mind::{
    AcceptedKnowledge, ActorName, KnowledgeIdentity, KnowledgeJudgePacket, KnowledgeJudgeResponse,
    KnowledgeJudgeVerdict, KnowledgeRejectionReason, KnowledgeSubmission, MindReply, MindRequest,
    TextBody,
};

use crate::{MindEnvelope, MindKnowledgeJudgeSocketConfiguration, MindTables, Result};

const KNOWLEDGE_IDENTITY_MINIMUM_CODE_LENGTH: usize = 4;
const KNOWLEDGE_IDENTITY_MAXIMUM_CODE_LENGTH: usize = 7;
const KNOWLEDGE_IDENTITY_CODE_RADIX: u64 = 36;
const RANDOM_IDENTITY_ATTEMPTS_PER_LENGTH: usize = 128;

pub trait KnowledgeJudge: Send + Sync {
    fn judge(&self, request: KnowledgeJudgeRequest) -> KnowledgeJudgeDecision;

    fn record_applied_decision(&self, _decision: KnowledgeJudgeAppliedDecision) {}
}

pub type KnowledgeJudgePort = Arc<dyn KnowledgeJudge>;

#[derive(Clone, Debug)]
pub struct KnowledgeJudgeRequest {
    packet: KnowledgeJudgePacket,
}

impl KnowledgeJudgeRequest {
    fn new(_client_request: MindRequest, packet: KnowledgeJudgePacket) -> Self {
        Self { packet }
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
}

#[allow(dead_code)]
#[derive(Clone, Debug, Eq, PartialEq)]
enum KnowledgeJudgeParseStatus {
    ParsedKnowledgeJudgeResponse,
    MindJudgeRequestRejected { reason: String, message: String },
    MindJudgeUnavailable { error: String },
}

#[allow(dead_code)]
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
pub struct MindJudgeSocketKnowledgeJudge {
    configuration: MindJudgeSocketKnowledgeJudgeConfiguration,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MindJudgeSocketKnowledgeJudgeConfiguration {
    socket_path: PathBuf,
    timeout: Duration,
}

#[derive(Debug, thiserror::Error)]
enum MindJudgeSocketKnowledgeJudgeError {
    #[error("mind judge socket unavailable: {0}")]
    Socket(std::io::Error),

    #[error("mind judge frame failed: {0}")]
    Frame(String),
}

impl MindJudgeSocketKnowledgeJudge {
    pub fn new(configuration: MindKnowledgeJudgeSocketConfiguration) -> Self {
        Self {
            configuration: MindJudgeSocketKnowledgeJudgeConfiguration::from_contract(configuration),
        }
    }

    fn call_mind_judge(
        &self,
        packet: signal_mind_judge::KnowledgeJudgePacket,
    ) -> std::result::Result<signal_mind_judge::MindJudgeReply, MindJudgeSocketKnowledgeJudgeError>
    {
        let mut stream = UnixStream::connect(self.configuration.socket_path())
            .map_err(MindJudgeSocketKnowledgeJudgeError::Socket)?;
        stream
            .set_read_timeout(Some(self.configuration.timeout))
            .map_err(MindJudgeSocketKnowledgeJudgeError::Socket)?;
        stream
            .set_write_timeout(Some(self.configuration.timeout))
            .map_err(MindJudgeSocketKnowledgeJudgeError::Socket)?;
        let codec = signal_mind_judge::MindJudgeFrameCodec::default();
        codec
            .write_request(
                &mut stream,
                signal_mind_judge::MindJudgeRequest::JudgeKnowledge(packet),
            )
            .map_err(|error| MindJudgeSocketKnowledgeJudgeError::Frame(error.to_string()))?;
        stream
            .flush()
            .map_err(MindJudgeSocketKnowledgeJudgeError::Socket)?;
        codec
            .read_reply(&mut stream)
            .map_err(|error| MindJudgeSocketKnowledgeJudgeError::Frame(error.to_string()))
    }

    fn unavailable_decision(error: MindJudgeSocketKnowledgeJudgeError) -> KnowledgeJudgeDecision {
        KnowledgeJudgeDecision::judge_unavailable(error.to_string())
    }
}

impl KnowledgeJudge for MindJudgeSocketKnowledgeJudge {
    fn judge(&self, request: KnowledgeJudgeRequest) -> KnowledgeJudgeDecision {
        let packet = MindJudgeContractPacket::new(request.packet()).to_contract();
        let reply = match self.call_mind_judge(packet) {
            Ok(reply) => reply,
            Err(error) => return Self::unavailable_decision(error),
        };
        KnowledgeJudgeDecision::from_mind_judge_reply(reply)
    }
}

impl MindJudgeSocketKnowledgeJudgeConfiguration {
    fn from_contract(configuration: MindKnowledgeJudgeSocketConfiguration) -> Self {
        Self {
            socket_path: PathBuf::from(configuration.socket_path.as_str()),
            timeout: Duration::from_millis(configuration.timeout_milliseconds),
        }
    }

    fn socket_path(&self) -> &Path {
        &self.socket_path
    }
}

impl KnowledgeJudgeDecision {
    fn from_mind_judge_reply(reply: signal_mind_judge::MindJudgeReply) -> Self {
        match reply {
            signal_mind_judge::MindJudgeReply::KnowledgeJudged(response) => {
                Self::from_response(MindJudgeContractResponse::new(response).into_mind())
            }
            signal_mind_judge::MindJudgeReply::RequestRejected(rejection) => {
                Self::request_rejected(rejection)
            }
        }
    }

    fn request_rejected(rejection: signal_mind_judge::MindJudgeRequestRejection) -> Self {
        Self {
            verdict: KnowledgeJudgeVerdict::Reject(KnowledgeRejectionReason::PersistenceRejected),
            diagnostic_message: Some(TextBody::new(format!(
                "mind judge operational rejection: {:?}: {}",
                rejection.reason,
                rejection.message.as_str()
            ))),
            parse_status: KnowledgeJudgeParseStatus::MindJudgeRequestRejected {
                reason: format!("{:?}", rejection.reason),
                message: rejection.message.as_str().to_owned(),
            },
        }
    }

    fn judge_unavailable(error: String) -> Self {
        Self {
            verdict: KnowledgeJudgeVerdict::Reject(KnowledgeRejectionReason::PersistenceRejected),
            diagnostic_message: Some(TextBody::new(format!("mind judge unavailable: {error}"))),
            parse_status: KnowledgeJudgeParseStatus::MindJudgeUnavailable { error },
        }
    }
}

struct MindJudgeContractPacket<'packet> {
    packet: &'packet KnowledgeJudgePacket,
}

impl<'packet> MindJudgeContractPacket<'packet> {
    fn new(packet: &'packet KnowledgeJudgePacket) -> Self {
        Self { packet }
    }

    fn to_contract(&self) -> signal_mind_judge::KnowledgeJudgePacket {
        signal_mind_judge::KnowledgeJudgePacket {
            domain: self.packet.domain.clone(),
            statement: MindJudgeContractText::new(&self.packet.statement).to_contract(),
            relevant_neighbors: self
                .packet
                .relevant_neighbors
                .iter()
                .map(|record| MindJudgeContractRecord::new(record).to_contract())
                .collect(),
        }
    }
}

struct MindJudgeContractRecord<'record> {
    record: &'record AcceptedKnowledge,
}

impl<'record> MindJudgeContractRecord<'record> {
    fn new(record: &'record AcceptedKnowledge) -> Self {
        Self { record }
    }

    fn to_contract(&self) -> signal_mind_judge::KnowledgeRecord {
        signal_mind_judge::KnowledgeRecord {
            identity: MindJudgeContractIdentity::new(&self.record.identity).to_contract(),
            domain: self.record.domain.clone(),
            statement: MindJudgeContractText::new(&self.record.statement).to_contract(),
        }
    }
}

struct MindJudgeContractText<'text> {
    text: &'text TextBody,
}

impl<'text> MindJudgeContractText<'text> {
    fn new(text: &'text TextBody) -> Self {
        Self { text }
    }

    fn to_contract(&self) -> signal_mind_judge::TextBody {
        signal_mind_judge::TextBody::new(self.text.as_str())
            .expect("signal-mind text bodies are non-empty")
    }
}

struct MindJudgeContractIdentity<'identity> {
    identity: &'identity KnowledgeIdentity,
}

impl<'identity> MindJudgeContractIdentity<'identity> {
    fn new(identity: &'identity KnowledgeIdentity) -> Self {
        Self { identity }
    }

    fn to_contract(&self) -> signal_mind_judge::KnowledgeIdentity {
        signal_mind_judge::KnowledgeIdentity::new(self.identity.as_str())
            .expect("signal-mind knowledge identities are non-empty")
    }
}

struct MindJudgeContractResponse {
    response: signal_mind_judge::KnowledgeJudgeResponse,
}

impl MindJudgeContractResponse {
    fn new(response: signal_mind_judge::KnowledgeJudgeResponse) -> Self {
        Self { response }
    }

    fn into_mind(self) -> KnowledgeJudgeResponse {
        KnowledgeJudgeResponse {
            verdict: MindJudgeContractVerdict::new(self.response.verdict).into_mind(),
            diagnostic_message: self
                .response
                .diagnostic_message
                .map(|message| TextBody::new(message.as_str())),
        }
    }
}

struct MindJudgeContractVerdict {
    verdict: signal_mind_judge::KnowledgeJudgeVerdict,
}

impl MindJudgeContractVerdict {
    fn new(verdict: signal_mind_judge::KnowledgeJudgeVerdict) -> Self {
        Self { verdict }
    }

    fn into_mind(self) -> KnowledgeJudgeVerdict {
        match self.verdict {
            signal_mind_judge::KnowledgeJudgeVerdict::Accept => KnowledgeJudgeVerdict::Accept,
            signal_mind_judge::KnowledgeJudgeVerdict::Reject(reason) => {
                KnowledgeJudgeVerdict::Reject(MindJudgeContractRejection::new(reason).into_mind())
            }
        }
    }
}

struct MindJudgeContractRejection {
    reason: signal_mind_judge::KnowledgeRejectionReason,
}

impl MindJudgeContractRejection {
    fn new(reason: signal_mind_judge::KnowledgeRejectionReason) -> Self {
        Self { reason }
    }

    fn into_mind(self) -> KnowledgeRejectionReason {
        match self.reason {
            signal_mind_judge::KnowledgeRejectionReason::NotKnowledge => {
                KnowledgeRejectionReason::NotKnowledge
            }
            signal_mind_judge::KnowledgeRejectionReason::PrivateOrUnauthorized => {
                KnowledgeRejectionReason::PrivateOrUnauthorized
            }
            signal_mind_judge::KnowledgeRejectionReason::MeaningUnclear => {
                KnowledgeRejectionReason::MeaningUnclear
            }
            signal_mind_judge::KnowledgeRejectionReason::SemanticDuplicate(identity) => {
                KnowledgeRejectionReason::SemanticDuplicate(KnowledgeIdentity::new(
                    identity.as_str(),
                ))
            }
            signal_mind_judge::KnowledgeRejectionReason::ConflictsAcceptedKnowledge(identities) => {
                KnowledgeRejectionReason::ConflictsAcceptedKnowledge(
                    identities
                        .into_iter()
                        .map(|identity| KnowledgeIdentity::new(identity.as_str()))
                        .collect(),
                )
            }
            signal_mind_judge::KnowledgeRejectionReason::WrongDomain(domain) => {
                KnowledgeRejectionReason::WrongDomain(domain)
            }
            signal_mind_judge::KnowledgeRejectionReason::NeedsMoreSpecificShape => {
                KnowledgeRejectionReason::NeedsMoreSpecificShape
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::net::UnixListener;
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    use signal_domain::{Domain, EngineeringLeaf, Software, Technology};

    use super::*;

    #[test]
    fn mind_judge_socket_judge_sends_typed_packet_fields() {
        let socket_path = std::env::temp_dir().join(format!(
            "mind-judge-socket-test-{}-{}.sock",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time")
                .as_nanos()
        ));
        let _ = std::fs::remove_file(&socket_path);
        let listener = UnixListener::bind(&socket_path).expect("bind fake mind judge");
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept mind judge client");
            let codec = signal_mind_judge::MindJudgeFrameCodec::default();
            let received = codec.read_request(&mut stream).expect("read judge request");
            let signal_mind_judge::MindJudgeRequest::JudgeKnowledge(packet) = received.request();
            assert_eq!(packet.statement.as_str(), "Mind sends typed judge packets.");
            assert_eq!(packet.relevant_neighbors.len(), 1);
            assert_eq!(
                packet.relevant_neighbors[0].statement.as_str(),
                "Existing accepted neighbor."
            );
            let reply = signal_mind_judge::MindJudgeReply::KnowledgeJudged(
                signal_mind_judge::KnowledgeJudgeResponse::new(
                    signal_mind_judge::KnowledgeJudgeVerdict::Accept,
                    None,
                ),
            );
            codec
                .write_reply(&mut stream, received.exchange(), reply)
                .expect("write reply");
        });
        let judge = MindJudgeSocketKnowledgeJudge::new(MindKnowledgeJudgeSocketConfiguration::new(
            signal_mind::WirePath::from_absolute_path(socket_path.to_string_lossy().into_owned())
                .expect("socket path"),
            5_000,
        ));
        let packet = KnowledgeJudgePacket {
            domain: Domain::Technology(Technology::Software(Software::Engineering(
                EngineeringLeaf::Architecture,
            ))),
            statement: TextBody::new("Mind sends typed judge packets."),
            relevant_neighbors: vec![AcceptedKnowledge {
                identity: KnowledgeIdentity::new("abcd"),
                domain: Domain::Technology(Technology::Software(Software::Engineering(
                    EngineeringLeaf::Architecture,
                ))),
                statement: TextBody::new("Existing accepted neighbor."),
                accepted_by: ActorName::new("tester"),
                accepted_at: signal_mind::TimestampNanos::new(1),
            }],
        };

        let decision = judge.judge(KnowledgeJudgeRequest::new(
            MindRequest::Submit(KnowledgeSubmission {
                domain: packet.domain.clone(),
                statement: packet.statement.clone(),
            }),
            packet,
        ));

        server.join().expect("fake mind judge server");
        let _ = std::fs::remove_file(socket_path);
        assert_eq!(decision.verdict(), &KnowledgeJudgeVerdict::Accept);
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
            domain: self.submission.domain.clone(),
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
                record.domain == self.submission.domain
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
            domain: self.submission.domain,
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
