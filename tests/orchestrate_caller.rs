use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use kameo::actor::Spawn;
use mind::actors::choreography::{
    AdjudicatorArguments, ApplyDecision, CallerArguments, ChoreographyAdjudicator,
    MindOrchestrateCaller, OrchestrateDecision, OwnerEndpoint,
};
use mind::actors::{ActorTrace, TraceNode};
use owner_signal_persona_orchestrate::{
    CreateRoleOrder, Frame as OwnerOrchestrateFrame, FrameBody as OwnerOrchestrateFrameBody,
    HarnessKind, OwnerOrchestrateReply, RefreshRepositoryIndexOrder, RetireRoleOrder, Retirement,
    RoleIdentifier,
};
use persona_orchestrate::{
    Observation, OrchestrateLayout, OrchestrateReply, OrchestrateRequest, OrchestrateService,
    StoreLocation,
};

struct OwnerSocketFixture {
    root: PathBuf,
    workspace: PathBuf,
    git_index: PathBuf,
    owner_socket: PathBuf,
    service: Arc<OrchestrateService>,
}

impl OwnerSocketFixture {
    fn new(test_name: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "mind-orchestrate-caller-{test_name}-{}-{stamp}",
            std::process::id()
        ));
        let workspace = root.join("workspace");
        let git_index = root.join("git-index");
        std::fs::create_dir_all(workspace.join("reports")).expect("reports directory");
        std::fs::create_dir_all(workspace.join("repos")).expect("repos directory");
        std::fs::create_dir_all(&git_index).expect("git index directory");
        let store = StoreLocation::new(
            root.join("persona-orchestrate.redb")
                .to_string_lossy()
                .into_owned(),
        );
        let service = Arc::new(
            OrchestrateService::open_with_layout(
                &store,
                OrchestrateLayout::new(workspace.clone(), git_index.clone()),
            )
            .expect("orchestrate service opens"),
        );

        Self {
            owner_socket: root.join("owner.sock"),
            root,
            workspace,
            git_index,
            service,
        }
    }

    fn serve_one(&self) -> thread::JoinHandle<()> {
        if self.owner_socket.exists() {
            std::fs::remove_file(&self.owner_socket).expect("remove stale owner socket");
        }
        let listener = UnixListener::bind(&self.owner_socket).expect("owner socket binds");
        let service = Arc::clone(&self.service);
        thread::spawn(move || {
            let (mut stream, _address) = listener.accept().expect("owner socket accept");
            let response = handle_owner_stream(&mut stream, &service);
            stream.write_all(&response).expect("owner reply write");
        })
    }

    fn role_exists(&self, role: &RoleIdentifier) -> bool {
        let reply = self
            .service
            .handle(OrchestrateRequest::Observe(Observation::Roles))
            .expect("observe roles");
        let OrchestrateReply::RoleSnapshot(snapshot) = reply else {
            panic!("expected role snapshot");
        };
        snapshot.roles.iter().any(|status| status.role == *role)
    }
}

impl Drop for OwnerSocketFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn choreography_create_decision_calls_orchestrate_owner_create() {
    let fixture = OwnerSocketFixture::new("create");
    let server = fixture.serve_one();
    let role = role("primary-mind-orchestrate-create-zxq9");

    let result = apply_decision(
        fixture.owner_socket.clone(),
        OrchestrateDecision::Create(CreateRoleOrder {
            role: role.clone(),
            harness: HarnessKind::Codex,
        }),
    )
    .await;
    server.join().expect("owner server joins");

    let Some(OwnerOrchestrateReply::RoleCreated(created)) = result.reply() else {
        panic!("expected role created, got {result:?}");
    };
    assert_eq!(created.role, role);
    assert!(fixture.role_exists(&role));
    assert!(result.trace().contains_ordered(&[
        TraceNode::CHOREOGRAPHY_ADJUDICATOR,
        TraceNode::MIND_ORCHESTRATE_CALLER,
        TraceNode::MIND_ORCHESTRATE_CALLER,
        TraceNode::CHOREOGRAPHY_ADJUDICATOR,
    ]));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn choreography_retire_decision_calls_orchestrate_owner_retire() {
    let fixture = OwnerSocketFixture::new("retire");
    let role = role("primary-mind-orchestrate-retire-zxq9");
    fixture
        .service
        .handle_owner(
            owner_signal_persona_orchestrate::OwnerOrchestrateRequest::Create(CreateRoleOrder {
                role: role.clone(),
                harness: HarnessKind::Codex,
            }),
        )
        .expect("seed role");
    assert!(fixture.role_exists(&role));
    let server = fixture.serve_one();

    let result = apply_decision(
        fixture.owner_socket.clone(),
        OrchestrateDecision::Retire(Retirement::Role(RetireRoleOrder { role: role.clone() })),
    )
    .await;
    server.join().expect("owner server joins");

    let Some(OwnerOrchestrateReply::RoleRetired(retired)) = result.reply() else {
        panic!("expected role retired, got {result:?}");
    };
    assert_eq!(retired.role, role);
    assert!(!fixture.role_exists(&role));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn choreography_refresh_decision_calls_orchestrate_owner_refresh() {
    let fixture = OwnerSocketFixture::new("refresh");
    let repository_name = "primary-mind-orchestrate-refresh-zxq9";
    std::fs::create_dir_all(fixture.git_index.join(repository_name)).expect("repository");
    let server = fixture.serve_one();

    let result = apply_decision(
        fixture.owner_socket.clone(),
        OrchestrateDecision::Refresh(RefreshRepositoryIndexOrder {}),
    )
    .await;
    server.join().expect("owner server joins");

    let Some(OwnerOrchestrateReply::RepositoryIndexRefreshed(refreshed)) = result.reply() else {
        panic!("expected repository refresh, got {result:?}");
    };
    assert_eq!(refreshed.repositories, 1);
    let repositories = fixture.service.repositories().expect("repositories");
    assert_eq!(repositories.len(), 1);
    assert_eq!(repositories[0].name, repository_name);
    assert!(
        fixture
            .workspace
            .join("repos")
            .join(repository_name)
            .exists()
    );
}

async fn apply_decision(
    owner_socket: PathBuf,
    decision: OrchestrateDecision,
) -> mind::actors::choreography::ApplicationResult {
    let caller =
        MindOrchestrateCaller::spawn(CallerArguments::new(OwnerEndpoint::new(owner_socket)));
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

fn handle_owner_stream(stream: &mut UnixStream, service: &OrchestrateService) -> Vec<u8> {
    let bytes = read_length_prefixed(stream);
    let frame = OwnerOrchestrateFrame::decode_length_prefixed(&bytes).expect("owner frame decodes");
    let OwnerOrchestrateFrameBody::Request { exchange, request } = frame.into_body() else {
        panic!("expected owner request frame");
    };
    let reply = service.handle_owner_request(request);
    OwnerOrchestrateFrame::new(OwnerOrchestrateFrameBody::Reply { exchange, reply })
        .encode_length_prefixed()
        .expect("owner reply encodes")
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
