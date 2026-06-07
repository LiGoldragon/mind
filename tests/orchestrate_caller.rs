use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use kameo::actor::Spawn;
use meta_signal_orchestrate::{
    CreateRoleOrder, Frame as MetaOrchestrateFrame, FrameBody as MetaOrchestrateFrameBody,
    HarnessKind, MetaOrchestrateReply, MetaOrchestrateRequest, MetaOrchestrateRequestUnimplemented,
    MetaOrchestrateUnimplementedReason, RefreshRepositoryIndexOrder, RetireRoleOrder, Retirement,
    RoleCreated, RoleIdentifier, RoleRetired, WirePath,
};
use mind::actors::choreography::{
    AdjudicatorArguments, ApplyDecision, CallerArguments, ChoreographyAdjudicator, MetaEndpoint,
    MindOrchestrateCaller, OrchestrateDecision,
};
use mind::actors::{ActorTrace, TraceNode};
use signal_frame::{NonEmpty, Reply, SubReply};
struct MetaSocketFixture {
    root: PathBuf,
    git_index: PathBuf,
    meta_socket: PathBuf,
}

impl MetaSocketFixture {
    fn new(test_name: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "mind-orchestrate-caller-{test_name}-{}-{stamp}",
            std::process::id()
        ));
        let git_index = root.join("git-index");
        std::fs::create_dir_all(&git_index).expect("git index directory");

        Self {
            meta_socket: root.join("meta.sock"),
            root,
            git_index,
        }
    }

    fn serve_one(&self) -> thread::JoinHandle<MetaOrchestrateRequest> {
        if self.meta_socket.exists() {
            std::fs::remove_file(&self.meta_socket).expect("remove stale meta socket");
        }
        let listener = UnixListener::bind(&self.meta_socket).expect("meta socket binds");
        let responder = MetaResponder::new(self.root.clone(), self.git_index.clone());
        thread::spawn(move || {
            let (mut stream, _address) = listener.accept().expect("meta socket accept");
            let (request, response) = handle_meta_stream(&mut stream, &responder);
            stream.write_all(&response).expect("meta reply write");
            request
        })
    }
}

impl Drop for MetaSocketFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn choreography_create_decision_calls_orchestrate_meta_create() {
    let fixture = MetaSocketFixture::new("create");
    let server = fixture.serve_one();
    let role = role("primary-mind-orchestrate-create-zxq9");

    let result = apply_decision(
        fixture.meta_socket.clone(),
        OrchestrateDecision::Create(CreateRoleOrder {
            role: role.clone(),
            harness: HarnessKind::Codex,
        }),
    )
    .await;
    let request = server.join().expect("meta server joins");

    let Some(MetaOrchestrateReply::RoleCreated(created)) = result.reply() else {
        panic!("expected role created, got {result:?}");
    };
    assert_eq!(created.role, role);
    assert_eq!(
        request,
        MetaOrchestrateRequest::Create(CreateRoleOrder {
            role: role.clone(),
            harness: HarnessKind::Codex,
        })
    );
    assert!(result.trace().contains_ordered(&[
        TraceNode::CHOREOGRAPHY_ADJUDICATOR,
        TraceNode::MIND_ORCHESTRATE_CALLER,
        TraceNode::MIND_ORCHESTRATE_CALLER,
        TraceNode::CHOREOGRAPHY_ADJUDICATOR,
    ]));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn choreography_retire_decision_calls_orchestrate_meta_retire() {
    let fixture = MetaSocketFixture::new("retire");
    let role = role("primary-mind-orchestrate-retire-zxq9");
    let server = fixture.serve_one();

    let result = apply_decision(
        fixture.meta_socket.clone(),
        OrchestrateDecision::Retire(Retirement::Role(RetireRoleOrder { role: role.clone() })),
    )
    .await;
    let request = server.join().expect("meta server joins");

    let Some(MetaOrchestrateReply::RoleRetired(retired)) = result.reply() else {
        panic!("expected role retired, got {result:?}");
    };
    assert_eq!(retired.role, role);
    assert_eq!(
        request,
        MetaOrchestrateRequest::Retire(Retirement::Role(RetireRoleOrder { role }))
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn choreography_refresh_decision_calls_orchestrate_meta_refresh() {
    let fixture = MetaSocketFixture::new("refresh");
    let repository_name = "primary-mind-orchestrate-refresh-zxq9";
    std::fs::create_dir_all(fixture.git_index.join(repository_name)).expect("repository");
    let server = fixture.serve_one();

    let result = apply_decision(
        fixture.meta_socket.clone(),
        OrchestrateDecision::Refresh(RefreshRepositoryIndexOrder {}),
    )
    .await;
    let request = server.join().expect("meta server joins");

    let Some(MetaOrchestrateReply::RepositoryIndexRefreshed(refreshed)) = result.reply() else {
        panic!("expected repository refresh, got {result:?}");
    };
    assert_eq!(refreshed.repositories, 1);
    assert_eq!(
        request,
        MetaOrchestrateRequest::Refresh(RefreshRepositoryIndexOrder {})
    );
}

async fn apply_decision(
    meta_socket: PathBuf,
    decision: OrchestrateDecision,
) -> mind::actors::choreography::ApplicationResult {
    let caller = MindOrchestrateCaller::spawn(CallerArguments::new(MetaEndpoint::new(meta_socket)));
    caller.wait_for_startup().await;
    let adjudicator = ChoreographyAdjudicator::spawn(AdjudicatorArguments::new(caller.clone()));
    adjudicator.wait_for_startup().await;

    let result = adjudicator
        .ask(ApplyDecision {
            decision,
            trace: ActorTrace::new(),
        })
        .await
        .expect("decision application succeeds");

    adjudicator
        .stop_gracefully()
        .await
        .expect("adjudicator stops");
    adjudicator.wait_for_shutdown().await;
    caller.stop_gracefully().await.expect("caller stops");
    caller.wait_for_shutdown().await;

    assert!(
        result.error().is_none(),
        "caller failed: {:?}",
        result.error()
    );
    result
}

struct MetaResponder {
    root: PathBuf,
    git_index: PathBuf,
}

impl MetaResponder {
    fn new(root: PathBuf, git_index: PathBuf) -> Self {
        Self { root, git_index }
    }

    fn reply(&self, request: &MetaOrchestrateRequest) -> MetaOrchestrateReply {
        match request {
            MetaOrchestrateRequest::Create(order) => {
                MetaOrchestrateReply::RoleCreated(RoleCreated {
                    role: order.role.clone(),
                    harness: order.harness,
                    report_repository_path: wire_path(self.root.join("reports")),
                    report_lane_path: wire_path(
                        self.root.join("reports").join(order.role.as_wire_token()),
                    ),
                })
            }
            MetaOrchestrateRequest::Retire(Retirement::Role(order)) => {
                MetaOrchestrateReply::RoleRetired(RoleRetired {
                    role: order.role.clone(),
                })
            }
            MetaOrchestrateRequest::Refresh(_) => {
                let repositories = self
                    .git_index
                    .read_dir()
                    .expect("git index readable")
                    .filter(|entry| entry.as_ref().is_ok_and(|entry| entry.path().is_dir()))
                    .count()
                    .try_into()
                    .expect("repository count fits u32");
                MetaOrchestrateReply::RepositoryIndexRefreshed(
                    meta_signal_orchestrate::RepositoryIndexRefreshed { repositories },
                )
            }
            request => MetaOrchestrateReply::MetaOrchestrateRequestUnimplemented(
                MetaOrchestrateRequestUnimplemented {
                    operation: request.kind(),
                    reason: MetaOrchestrateUnimplementedReason::NotBuiltYet,
                },
            ),
        }
    }
}

fn handle_meta_stream(
    stream: &mut UnixStream,
    responder: &MetaResponder,
) -> (MetaOrchestrateRequest, Vec<u8>) {
    let bytes = read_length_prefixed(stream);
    let frame = MetaOrchestrateFrame::decode_length_prefixed(&bytes).expect("meta frame decodes");
    let MetaOrchestrateFrameBody::Request { exchange, request } = frame.into_body() else {
        panic!("expected meta request frame");
    };
    let operation = request.payloads().head().clone();
    let reply = Reply::committed(NonEmpty::single(SubReply::Ok(responder.reply(&operation))));
    let response = MetaOrchestrateFrame::new(MetaOrchestrateFrameBody::Reply { exchange, reply })
        .encode_length_prefixed()
        .expect("meta reply encodes");
    (operation, response)
}

fn read_length_prefixed(stream: &mut UnixStream) -> Vec<u8> {
    let mut prefix = [0_u8; 4];
    stream.read_exact(&mut prefix).expect("frame prefix");
    let length = u32::from_be_bytes(prefix) as usize;
    let mut payload = vec![0_u8; length];
    stream.read_exact(&mut payload).expect("frame payload");
    let mut bytes = Vec::with_capacity(4 + length);
    bytes.extend_from_slice(&prefix);
    bytes.extend_from_slice(&payload);
    bytes
}

fn role(value: &str) -> RoleIdentifier {
    RoleIdentifier::from_wire_token(value).expect("role")
}

fn wire_path(path: PathBuf) -> WirePath {
    WirePath::from_absolute_path(path.to_string_lossy().into_owned()).expect("wire path")
}
