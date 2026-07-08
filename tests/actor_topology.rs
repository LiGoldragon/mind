use std::io::{Read, Write};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use mind::{
    ActorRef, MindEnvelope, MindJudgeSocketKnowledgeJudge, MindKnowledgeJudgeSocketConfiguration,
    MindRoot, MindRootArguments, MindRootReply, StoreLocation, SubmitEnvelope,
};
use signal_domain::{Domain, EngineeringLeaf, Software, Technology};
use signal_frame::{NonEmpty, Reply, SubReply};
use signal_mind::{
    ActorName, KnowledgeRejectionReason, KnowledgeSubmission, MindReply, MindRequest, TextBody,
    WirePath,
};

const COMPONENT_DOMAIN: Domain = Domain::Technology(Technology::Software(Software::Engineering(
    EngineeringLeaf::Architecture,
)));

struct ActorFixture {
    root: ActorRef<MindRoot>,
    actor: ActorName,
    store: PathBuf,
}

struct FakeMindJudgeSocket {
    socket: PathBuf,
    server: thread::JoinHandle<Vec<signal_mind_judge::KnowledgeJudgePacket>>,
}

impl ActorFixture {
    async fn new(judge_socket: PathBuf) -> Self {
        let store = temporary_path("mind-actor-topology", "sema");
        let judge = MindJudgeSocketKnowledgeJudge::new(MindKnowledgeJudgeSocketConfiguration::new(
            WirePath::from_absolute_path(judge_socket.to_string_lossy().into_owned())
                .expect("judge socket path is absolute"),
            500,
        ));
        Self {
            root: MindRoot::start(
                MindRootArguments::new(StoreLocation::new(store.to_string_lossy().to_string()))
                    .with_knowledge_judge(Arc::new(judge)),
            )
            .await
            .expect("mind root starts"),
            actor: ActorName::new("tester"),
            store,
        }
    }

    async fn submit(&self, request: MindRequest) -> MindRootReply {
        self.root
            .ask(SubmitEnvelope {
                envelope: MindEnvelope::new(self.actor.clone(), request),
            })
            .await
            .expect("actor request succeeds")
    }

    async fn stop(self) {
        MindRoot::stop(self.root).await.expect("mind root stops");
        let _ = std::fs::remove_file(&self.store);
        let _ = std::fs::remove_dir_all(&self.store);
    }
}

impl FakeMindJudgeSocket {
    fn bind(replies: Vec<signal_mind_judge::MindJudgeReply>) -> Self {
        let socket = temporary_path("fake-mind-judge", "sock");
        let _ = std::fs::remove_file(&socket);
        let listener = UnixListener::bind(&socket).expect("fake mind-judge binds");
        let server = thread::spawn(move || {
            let mut packets = Vec::new();
            for reply in replies {
                let (mut stream, _) = listener.accept().expect("accept mind judge client");
                let (exchange, packet) = read_request(&mut stream);
                packets.push(packet);
                let frame = signal_mind_judge::MindJudgeFrame::new(
                    signal_mind_judge::MindJudgeFrameBody::Reply {
                        exchange,
                        reply: Reply::committed(NonEmpty::single(SubReply::Ok(reply))),
                    },
                );
                stream
                    .write_all(&frame.encode_length_prefixed().expect("encode reply"))
                    .expect("write reply");
            }
            packets
        });
        Self { socket, server }
    }

    fn socket(&self) -> PathBuf {
        self.socket.clone()
    }

    fn join(self) -> Vec<signal_mind_judge::KnowledgeJudgePacket> {
        let packets = self.server.join().expect("fake mind-judge thread joins");
        let _ = std::fs::remove_file(self.socket);
        packets
    }
}

fn read_request(
    stream: &mut std::os::unix::net::UnixStream,
) -> (
    signal_frame::ExchangeIdentifier,
    signal_mind_judge::KnowledgeJudgePacket,
) {
    let mut length = [0_u8; 4];
    stream.read_exact(&mut length).expect("read frame length");
    let length = u32::from_be_bytes(length) as usize;
    let mut payload = vec![0_u8; length];
    stream.read_exact(&mut payload).expect("read frame payload");
    let mut frame_bytes = Vec::with_capacity(4 + payload.len());
    frame_bytes.extend_from_slice(&(length as u32).to_be_bytes());
    frame_bytes.extend_from_slice(&payload);
    let frame = signal_mind_judge::MindJudgeFrame::decode_length_prefixed(&frame_bytes)
        .expect("decode request frame");
    let signal_mind_judge::MindJudgeFrameBody::Request { exchange, request } = frame.into_body()
    else {
        panic!("expected request frame");
    };
    let signal_mind_judge::MindJudgeRequest::JudgeKnowledge(packet) = request.payloads.into_head();
    (exchange, packet)
}

fn accepted_reply() -> signal_mind_judge::MindJudgeReply {
    signal_mind_judge::MindJudgeReply::KnowledgeJudged(
        signal_mind_judge::KnowledgeJudgeResponse::new(
            signal_mind_judge::KnowledgeJudgeVerdict::Accept,
            None,
        ),
    )
}

fn operational_rejection_reply() -> signal_mind_judge::MindJudgeReply {
    signal_mind_judge::MindJudgeReply::RequestRejected(
        signal_mind_judge::MindJudgeRequestRejection::new(
            signal_mind_judge::MindJudgeRequestRejectionReason::ProviderUnavailable,
            signal_mind_judge::TextBody::new("provider unavailable").unwrap(),
        ),
    )
}

fn submit(statement: &str) -> MindRequest {
    MindRequest::Submit(KnowledgeSubmission {
        domain: COMPONENT_DOMAIN,
        statement: TextBody::new(statement),
    })
}

fn temporary_path(name: &str, extension: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    std::env::temp_dir().join(format!("{name}-{}-{stamp}.{extension}", std::process::id()))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn accepted_knowledge_submit_uses_typed_mind_judge_socket() {
    let fake_judge = FakeMindJudgeSocket::bind(vec![accepted_reply()]);
    let fixture = ActorFixture::new(fake_judge.socket()).await;

    let accepted = fixture
        .submit(submit(
            "Mind routes accepted-knowledge judgment through a typed socket.",
        ))
        .await;
    let MindReply::Accepted(identity) = accepted.reply().expect("accepted reply exists") else {
        panic!("expected accepted reply, got {:?}", accepted.reply());
    };

    let found = fixture.submit(MindRequest::Get(identity.clone())).await;
    let MindReply::Found(record) = found.reply().expect("found reply exists") else {
        panic!("expected found reply, got {:?}", found.reply());
    };
    assert_eq!(record.identity, *identity);
    assert_eq!(
        record.statement.as_str(),
        "Mind routes accepted-knowledge judgment through a typed socket."
    );

    fixture.stop().await;
    let packets = fake_judge.join();
    assert_eq!(packets.len(), 1);
    assert_eq!(
        packets[0].statement.as_str(),
        "Mind routes accepted-knowledge judgment through a typed socket."
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mind_judge_operational_failure_is_not_meaning_unclear() {
    let fake_judge = FakeMindJudgeSocket::bind(vec![operational_rejection_reply()]);
    let fixture = ActorFixture::new(fake_judge.socket()).await;

    let rejected = fixture
        .submit(submit(
            "Mind should not store this when the judge provider is down.",
        ))
        .await;

    assert!(matches!(
        rejected.reply().expect("rejection reply exists"),
        MindReply::Rejected(KnowledgeRejectionReason::PersistenceRejected)
    ));

    fixture.stop().await;
    let packets = fake_judge.join();
    assert_eq!(packets.len(), 1);
}

#[test]
fn actor_topology_does_not_reintroduce_prompt_prose_into_mind() {
    let prompt = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src/knowledge-judge-prompts/accepted-knowledge.md"),
    )
    .expect("prompt tombstone is readable");

    assert!(prompt.contains("mind-judge-config"));
    assert!(!prompt.contains("Reason Precedence"));
}
