use std::time::{SystemTime, UNIX_EPOCH};

use std::os::unix::fs::PermissionsExt;

use persona_mind::{
    Error, MindClient, MindDaemon, MindDaemonEndpoint, MindFrameCodec, MindSocketMode,
    StoreLocation, SupervisionFrameCodec, SupervisionListener, SupervisionSocketMode,
};
use signal_core::{
    ExchangeIdentifier, ExchangeLane, LaneSequence, NonEmpty, Operation, Request, RequestPayload,
    RequestRejectionReason, SessionEpoch, SignalVerb,
};
use signal_persona::{
    ComponentHealth, ComponentHealthQuery, ComponentHello, ComponentKind, ComponentName,
    ComponentReadinessQuery, SupervisionProtocolVersion, SupervisionReply, SupervisionRequest,
};
use signal_persona_mind::{
    ActiveClaim, ActorName, Alternative, AlternativeId, ByRelationKind, ByThoughtKind,
    ClaimActivity, ClaimBody, ClaimScope, DecisionBody, GoalBody, GoalScope, ItemKind,
    ItemPriority, MindFrame as Frame, MindFrameBody as FrameBody, MindReply, MindRequest,
    NoteToSelf, ObservationBody, ObservationSummary, Opening, PathClaimScope, Query, QueryKind,
    QueryLimit, QueryRelations, QueryThoughts, RelationFilter, RelationKind, RoleClaim, RoleName,
    RoleObservation, ScopeReason, ScopeReference, SubmitRelation, SubmitThought, TextBody,
    ThoughtBody, ThoughtFilter, ThoughtKind, TimestampNanos, Title, WirePath, WorkspaceGoal,
};
use tokio::net::UnixStream;

struct SocketFixture {
    endpoint: MindDaemonEndpoint,
    store: StoreLocation,
}

impl SocketFixture {
    fn new(test_name: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "persona-mind-{test_name}-{}-{stamp}",
            std::process::id()
        ));
        let socket = root.with_extension("sock");
        let store = root.with_extension("redb");
        Self {
            endpoint: MindDaemonEndpoint::new(socket),
            store: StoreLocation::new(store.to_string_lossy().to_string()),
        }
    }

    fn endpoint(&self) -> MindDaemonEndpoint {
        self.endpoint.clone()
    }

    fn store(&self) -> StoreLocation {
        self.store.clone()
    }

    fn request(&self) -> MindRequest {
        MindRequest::Opening(Opening {
            kind: ItemKind::Task,
            priority: ItemPriority::High,
            title: Title::new("Route one request through daemon wire"),
            body: TextBody::new("The daemon receives a Signal frame and replies with one."),
        })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn daemon_round_trip_uses_signal_frames_over_socket() {
    let fixture = SocketFixture::new("round-trip");
    let daemon = MindDaemon::new(fixture.endpoint(), fixture.store())
        .bind()
        .await
        .expect("daemon binds");
    let endpoint = daemon.endpoint().clone();
    let server = tokio::spawn(async move { daemon.serve_one().await });

    let client = MindClient::new(endpoint, ActorName::new("operator"));
    let client_reply = client
        .submit(fixture.request())
        .await
        .expect("client receives reply frame");
    let daemon_reply = server
        .await
        .expect("daemon task joins")
        .expect("daemon serves one request");

    assert_eq!(client_reply, daemon_reply);
    let MindReply::OpeningReceipt(receipt) = client_reply else {
        panic!("expected opening receipt");
    };
    assert_eq!(receipt.event.header.actor, ActorName::new("operator"));
}

#[test]
fn mind_frame_codec_rejects_mismatched_signal_verb() {
    let frame = Frame::new(FrameBody::Request {
        exchange: ExchangeIdentifier::new(
            SessionEpoch::new(0),
            ExchangeLane::Connector,
            LaneSequence::first(),
        ),
        request: Request::from_operations(NonEmpty::single(Operation::new(
            SignalVerb::Assert,
            MindRequest::QueryThoughts(QueryThoughts {
                filter: ThoughtFilter::ByKind(ByThoughtKind { kinds: Vec::new() }),
                limit: 1,
            }),
        ))),
    });
    let error = MindFrameCodec::default()
        .request_from_frame(frame)
        .expect_err("mismatched verb is rejected");

    assert!(matches!(
        error,
        Error::RequestRejected(RequestRejectionReason::VerbPayloadMismatch { index: 0 })
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn constraint_mind_daemon_applies_spawn_envelope_socket_mode() {
    let fixture = SocketFixture::new("socket-mode");
    let daemon = MindDaemon::new(fixture.endpoint(), fixture.store())
        .with_socket_mode(MindSocketMode::new(0o600))
        .bind()
        .await
        .expect("daemon binds with mode");
    let endpoint = daemon.endpoint().clone();
    let mode = std::fs::metadata(endpoint.as_path())
        .expect("socket metadata is readable")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600);

    let server = tokio::spawn(async move { daemon.serve_one().await });
    let client = MindClient::new(endpoint, ActorName::new("operator"));
    client
        .submit(fixture.request())
        .await
        .expect("client receives reply frame");
    server
        .await
        .expect("daemon task joins")
        .expect("daemon serves one request");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mind_daemon_answers_component_supervision_relation() {
    let fixture = SocketFixture::new("supervision");
    let supervision_socket = fixture
        .endpoint
        .as_path()
        .with_extension("supervision.sock");
    let supervision = SupervisionListener::new(
        supervision_socket.clone(),
        SupervisionSocketMode::from_octal(0o600),
    );
    let daemon = MindDaemon::new(fixture.endpoint(), fixture.store())
        .with_supervision_listener(supervision)
        .bind()
        .await
        .expect("daemon binds with supervision relation");
    let endpoint = daemon.endpoint().clone();

    let mode = std::fs::metadata(&supervision_socket)
        .expect("supervision socket metadata is readable")
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600);

    let mut stream = UnixStream::connect(&supervision_socket)
        .await
        .expect("supervision client connects");
    let supervision_codec = SupervisionFrameCodec::new(1024 * 1024);

    supervision_codec
        .write_request(
            &mut stream,
            SupervisionRequest::ComponentHello(ComponentHello {
                expected_component: ComponentName::new("persona-mind"),
                expected_kind: ComponentKind::Mind,
                supervision_protocol_version: SupervisionProtocolVersion::new(1),
            }),
        )
        .await
        .expect("component hello writes");
    assert!(matches!(
        supervision_codec
            .read_reply(&mut stream)
            .await
            .expect("component identity reply"),
        SupervisionReply::ComponentIdentity(identity)
            if identity.name.as_str() == "persona-mind"
                && identity.kind == ComponentKind::Mind
    ));

    supervision_codec
        .write_request(
            &mut stream,
            SupervisionRequest::ComponentReadinessQuery(ComponentReadinessQuery {
                component: ComponentName::new("persona-mind"),
            }),
        )
        .await
        .expect("readiness query writes");
    assert!(matches!(
        supervision_codec
            .read_reply(&mut stream)
            .await
            .expect("readiness reply"),
        SupervisionReply::ComponentReady(_)
    ));

    supervision_codec
        .write_request(
            &mut stream,
            SupervisionRequest::ComponentHealthQuery(ComponentHealthQuery {
                component: ComponentName::new("persona-mind"),
            }),
        )
        .await
        .expect("health query writes");
    assert!(matches!(
        supervision_codec
            .read_reply(&mut stream)
            .await
            .expect("health reply"),
        SupervisionReply::ComponentHealthReport(report)
            if report.health == ComponentHealth::Running
    ));

    let server = tokio::spawn(async move { daemon.serve_one().await });
    let client = MindClient::new(endpoint, ActorName::new("operator"));
    client
        .submit(fixture.request())
        .await
        .expect("client receives reply frame");
    server
        .await
        .expect("daemon task joins")
        .expect("daemon serves one request");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn daemon_stamps_local_operator_actor_at_ingress() {
    let fixture = SocketFixture::new("ingress-identity");
    let daemon = MindDaemon::new(fixture.endpoint(), fixture.store())
        .bind()
        .await
        .expect("daemon binds");
    let endpoint = daemon.endpoint().clone();
    let server = tokio::spawn(async move { daemon.serve_one().await });

    let client = MindClient::new(endpoint, ActorName::new("designer"));
    let client_reply = client
        .submit(fixture.request())
        .await
        .expect("client receives reply frame");
    server
        .await
        .expect("daemon task joins")
        .expect("daemon serves one request");

    let MindReply::OpeningReceipt(receipt) = client_reply else {
        panic!("expected opening receipt");
    };
    assert_eq!(receipt.event.header.actor, ActorName::new("operator"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn daemon_accepts_sender_free_request_frames() {
    let fixture = SocketFixture::new("sender-free");
    let daemon = MindDaemon::new(fixture.endpoint(), fixture.store())
        .bind()
        .await
        .expect("daemon binds");
    let endpoint = daemon.endpoint().clone();
    let server = tokio::spawn(async move { daemon.serve_one().await });

    let codec = MindFrameCodec::default();
    let mut stream = UnixStream::connect(endpoint.as_path())
        .await
        .expect("client connects to daemon");
    let frame = Frame::new(FrameBody::Request {
        exchange: ExchangeIdentifier::new(
            SessionEpoch::new(0),
            ExchangeLane::Connector,
            LaneSequence::first(),
        ),
        request: fixture.request().into_request(),
    });
    codec
        .write_frame(&mut stream, &frame)
        .await
        .expect("client writes frame");

    server
        .await
        .expect("daemon task joins")
        .expect("daemon accepts sender-free signal frame");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn client_cannot_reply_without_daemon_signal_frame() {
    let fixture = SocketFixture::new("no-daemon");
    let client = MindClient::new(fixture.endpoint(), ActorName::new("operator"));
    let error = client
        .submit(fixture.request())
        .await
        .expect_err("missing daemon cannot produce reply");

    assert!(matches!(error, persona_mind::Error::Io(_)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mind_store_survives_process_restart() {
    let fixture = SocketFixture::new("store-restart");

    {
        let daemon = MindDaemon::new(fixture.endpoint(), fixture.store())
            .bind()
            .await
            .expect("first daemon binds");
        let endpoint = daemon.endpoint().clone();
        let server = tokio::spawn(async move { daemon.serve_one().await });

        let client = MindClient::new(endpoint, ActorName::new("operator"));
        client
            .submit(MindRequest::RoleClaim(RoleClaim {
                role: RoleName::Operator,
                scopes: vec![ScopeReference::Path(
                    WirePath::from_absolute_path("/git/github.com/LiGoldragon/persona-mind")
                        .expect("absolute path"),
                )],
                reason: ScopeReason::from_text("durable role claim").expect("scope reason"),
            }))
            .await
            .expect("claim committed");

        server
            .await
            .expect("first daemon joins")
            .expect("first daemon serves claim");
    }

    let daemon = MindDaemon::new(fixture.endpoint(), fixture.store())
        .bind()
        .await
        .expect("second daemon binds");
    let endpoint = daemon.endpoint().clone();
    let server = tokio::spawn(async move { daemon.serve_one().await });

    let client = MindClient::new(endpoint, ActorName::new("operator"));
    let reply = client
        .submit(MindRequest::RoleObservation(RoleObservation))
        .await
        .expect("observation reads durable store");

    server
        .await
        .expect("second daemon joins")
        .expect("second daemon serves observation");

    let MindReply::RoleSnapshot(snapshot) = reply else {
        panic!("expected role snapshot");
    };
    let operator = snapshot
        .roles
        .iter()
        .find(|status| status.role == RoleName::Operator)
        .expect("operator status exists");

    assert_eq!(operator.claims.len(), 1);
    assert_eq!(
        operator.claims[0].scope,
        ScopeReference::Path(
            WirePath::from_absolute_path("/git/github.com/LiGoldragon/persona-mind")
                .expect("absolute path")
        )
    );
    assert_eq!(
        operator.claims[0].reason,
        ScopeReason::from_text("durable role claim").expect("scope reason")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mind_memory_graph_survives_process_restart() {
    let fixture = SocketFixture::new("memory-restart");

    {
        let daemon = MindDaemon::new(fixture.endpoint(), fixture.store())
            .bind()
            .await
            .expect("first daemon binds");
        let endpoint = daemon.endpoint().clone();
        let server = tokio::spawn(async move { daemon.serve_one().await });

        let client = MindClient::new(endpoint, ActorName::new("operator"));
        client
            .submit(MindRequest::Opening(Opening {
                kind: ItemKind::Task,
                priority: ItemPriority::High,
                title: Title::new("Durable mind memory"),
                body: TextBody::new("The work graph survives daemon restart."),
            }))
            .await
            .expect("opening committed");

        server
            .await
            .expect("first daemon joins")
            .expect("first daemon serves opening");
    }

    let daemon = MindDaemon::new(fixture.endpoint(), fixture.store())
        .bind()
        .await
        .expect("second daemon binds");
    let endpoint = daemon.endpoint().clone();
    let server = tokio::spawn(async move { daemon.serve_one().await });

    let client = MindClient::new(endpoint, ActorName::new("operator"));
    let reply = client
        .submit(MindRequest::Query(Query {
            kind: QueryKind::Open,
            limit: QueryLimit::new(10),
        }))
        .await
        .expect("query reads durable graph");

    server
        .await
        .expect("second daemon joins")
        .expect("second daemon serves query");

    let MindReply::View(view) = reply else {
        panic!("expected view reply");
    };

    assert_eq!(view.items.len(), 1);
    assert_eq!(view.items[0].title, Title::new("Durable mind memory"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mind_typed_thought_graph_survives_process_restart() {
    let fixture = SocketFixture::new("typed-thought-restart");
    let record;

    {
        let daemon = MindDaemon::new(fixture.endpoint(), fixture.store())
            .bind()
            .await
            .expect("first daemon binds");
        let endpoint = daemon.endpoint().clone();
        let server = tokio::spawn(async move { daemon.serve_one().await });

        let client = MindClient::new(endpoint, ActorName::new("operator"));
        let reply = client
            .submit(MindRequest::SubmitThought(SubmitThought {
                kind: ThoughtKind::Goal,
                body: ThoughtBody::Goal(GoalBody {
                    description: TextBody::new("Persist typed graph thought"),
                    scope: GoalScope::Workspace(WorkspaceGoal {
                        workspace: TextBody::new("primary"),
                    }),
                }),
            }))
            .await
            .expect("thought committed");

        server
            .await
            .expect("first daemon joins")
            .expect("first daemon serves thought");

        let MindReply::ThoughtCommitted(receipt) = reply else {
            panic!("expected thought commit");
        };
        record = receipt.record;
        assert_eq!(record.as_str().len(), 3);
    }

    let daemon = MindDaemon::new(fixture.endpoint(), fixture.store())
        .bind()
        .await
        .expect("second daemon binds");
    let endpoint = daemon.endpoint().clone();
    let server = tokio::spawn(async move { daemon.serve_one().await });

    let client = MindClient::new(endpoint, ActorName::new("operator"));
    let reply = client
        .submit(MindRequest::QueryThoughts(QueryThoughts {
            filter: ThoughtFilter::ByKind(ByThoughtKind {
                kinds: vec![ThoughtKind::Goal],
            }),
            limit: 10,
        }))
        .await
        .expect("query reads durable typed graph");

    server
        .await
        .expect("second daemon joins")
        .expect("second daemon serves query");

    let MindReply::ThoughtList(list) = reply else {
        panic!("expected thought list");
    };
    assert_eq!(list.thoughts.len(), 1);
    assert_eq!(list.thoughts[0].id, record);
    assert_eq!(list.thoughts[0].kind, ThoughtKind::Goal);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mind_typed_relation_round_trip_uses_committed_thought_ids() {
    let fixture = SocketFixture::new("typed-relation");
    let daemon = MindDaemon::new(fixture.endpoint(), fixture.store())
        .bind()
        .await
        .expect("daemon binds");
    let endpoint = daemon.endpoint().clone();
    let server = tokio::spawn(async move { daemon.serve_count(4).await });
    let client = MindClient::new(endpoint, ActorName::new("operator"));

    let goal = client
        .submit(MindRequest::SubmitThought(SubmitThought {
            kind: ThoughtKind::Goal,
            body: ThoughtBody::Goal(GoalBody {
                description: TextBody::new("Route relation through graph store"),
                scope: GoalScope::Workspace(WorkspaceGoal {
                    workspace: TextBody::new("primary"),
                }),
            }),
        }))
        .await
        .expect("goal committed");
    let prerequisite_goal = client
        .submit(MindRequest::SubmitThought(SubmitThought {
            kind: ThoughtKind::Goal,
            body: ThoughtBody::Goal(GoalBody {
                description: TextBody::new("Required earlier goal"),
                scope: GoalScope::Workspace(WorkspaceGoal {
                    workspace: TextBody::new("primary"),
                }),
            }),
        }))
        .await
        .expect("prerequisite goal committed");

    let MindReply::ThoughtCommitted(goal) = goal else {
        panic!("expected goal commit");
    };
    let MindReply::ThoughtCommitted(prerequisite_goal) = prerequisite_goal else {
        panic!("expected prerequisite goal commit");
    };

    let relation = client
        .submit(MindRequest::SubmitRelation(SubmitRelation {
            kind: RelationKind::Requires,
            source: prerequisite_goal.record.clone(),
            target: goal.record.clone(),
            note: Some(TextBody::new("typed relation witness")),
        }))
        .await
        .expect("relation committed");
    let list = client
        .submit(MindRequest::QueryRelations(QueryRelations {
            filter: RelationFilter::ByKind(ByRelationKind {
                kinds: vec![RelationKind::Requires],
            }),
            limit: 10,
        }))
        .await
        .expect("relations queried");

    server
        .await
        .expect("daemon joins")
        .expect("daemon serves relation sequence");

    let MindReply::RelationCommitted(receipt) = relation else {
        panic!("expected relation commit");
    };
    let MindReply::RelationList(list) = list else {
        panic!("expected relation list");
    };
    assert_eq!(list.relations.len(), 1);
    assert_eq!(list.relations[0].id, receipt.relation);
    assert_eq!(list.relations[0].source, prerequisite_goal.record);
    assert_eq!(list.relations[0].target, goal.record);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mind_typed_graph_handles_goal_claim_observation_decision_scenario() {
    let fixture = SocketFixture::new("typed-graph-scenario");
    let daemon = MindDaemon::new(fixture.endpoint(), fixture.store())
        .bind()
        .await
        .expect("daemon binds");
    let endpoint = daemon.endpoint().clone();
    let server = tokio::spawn(async move { daemon.serve_count(7).await });
    let client = MindClient::new(endpoint, ActorName::new("operator"));

    let goal = client
        .submit(MindRequest::SubmitThought(SubmitThought {
            kind: ThoughtKind::Goal,
            body: ThoughtBody::Goal(GoalBody {
                description: TextBody::new("Replace lock files with persona-mind"),
                scope: GoalScope::Workspace(WorkspaceGoal {
                    workspace: TextBody::new("primary"),
                }),
            }),
        }))
        .await
        .expect("goal committed");
    let claim = client
        .submit(MindRequest::SubmitThought(SubmitThought {
            kind: ThoughtKind::Claim,
            body: ThoughtBody::Claim(ClaimBody {
                claimed_by: ActorName::new("operator"),
                scope: ClaimScope::Paths(PathClaimScope {
                    paths: vec![
                        WirePath::from_absolute_path("/git/github.com/LiGoldragon/persona-mind")
                            .expect("absolute path"),
                    ],
                }),
                role: RoleName::Operator,
                activity: ClaimActivity::Active(ActiveClaim {
                    started_at: TimestampNanos::new(1),
                }),
            }),
        }))
        .await
        .expect("claim committed");
    let observation = client
        .submit(MindRequest::SubmitThought(SubmitThought {
            kind: ThoughtKind::Observation,
            body: ThoughtBody::Observation(ObservationBody {
                summary: ObservationSummary::NoteToSelf(NoteToSelf {
                    body: TextBody::new("Graph scenario crossed the daemon path"),
                }),
                detail: None,
                location: None,
            }),
        }))
        .await
        .expect("observation committed");
    let decision = client
        .submit(MindRequest::SubmitThought(SubmitThought {
            kind: ThoughtKind::Decision,
            body: ThoughtBody::Decision(DecisionBody {
                question: TextBody::new("Where should workspace coordination live?"),
                alternatives: vec![Alternative {
                    id: AlternativeId::new("mind"),
                    description: TextBody::new("Use persona-mind as the central graph"),
                    pros: vec![TextBody::new("typed state")],
                    cons: vec![TextBody::new("prototype still young")],
                }],
                chosen: AlternativeId::new("mind"),
                criteria: vec![TextBody::new("typed daemon state")],
                rationale: TextBody::new("Mind replaces lock files and BEADS over time"),
            }),
        }))
        .await
        .expect("decision committed");

    let MindReply::ThoughtCommitted(goal) = goal else {
        panic!("expected goal commit");
    };
    let MindReply::ThoughtCommitted(claim) = claim else {
        panic!("expected claim commit");
    };
    let MindReply::ThoughtCommitted(observation) = observation else {
        panic!("expected observation commit");
    };
    let MindReply::ThoughtCommitted(decision) = decision else {
        panic!("expected decision commit");
    };

    let _claim_relation = client
        .submit(MindRequest::SubmitRelation(SubmitRelation {
            kind: RelationKind::Realizes,
            source: claim.record.clone(),
            target: goal.record.clone(),
            note: Some(TextBody::new("claim advances the goal")),
        }))
        .await
        .expect("claim relation committed");
    let _decision_relation = client
        .submit(MindRequest::SubmitRelation(SubmitRelation {
            kind: RelationKind::Decides,
            source: decision.record.clone(),
            target: goal.record.clone(),
            note: Some(TextBody::new("decision chooses the goal shape")),
        }))
        .await
        .expect("decision relation committed");

    let thoughts = client
        .submit(MindRequest::QueryThoughts(QueryThoughts {
            filter: ThoughtFilter::ByKind(ByThoughtKind {
                kinds: vec![
                    ThoughtKind::Observation,
                    ThoughtKind::Goal,
                    ThoughtKind::Claim,
                    ThoughtKind::Decision,
                ],
            }),
            limit: 10,
        }))
        .await
        .expect("thoughts queried");

    server
        .await
        .expect("daemon joins")
        .expect("daemon serves scenario");

    let MindReply::ThoughtList(thoughts) = thoughts else {
        panic!("expected thought list");
    };
    let mut kinds = thoughts
        .thoughts
        .into_iter()
        .map(|thought| (thought.id, thought.kind))
        .collect::<Vec<_>>();
    kinds.sort_by_key(|(_id, kind)| *kind as u8);

    assert!(kinds.contains(&(goal.record.clone(), ThoughtKind::Goal)));
    assert!(kinds.contains(&(claim.record.clone(), ThoughtKind::Claim)));
    assert!(kinds.contains(&(observation.record, ThoughtKind::Observation)));
    assert!(kinds.contains(&(decision.record, ThoughtKind::Decision)));
}
