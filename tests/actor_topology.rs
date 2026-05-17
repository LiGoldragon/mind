use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use persona_mind::actors::{
    ActorManifest, ActorResidency, ReadSubscriptionEvents, TraceAction, TraceNode,
};
use persona_mind::{
    ActorRef, MindEnvelope, MindRoot, MindRootArguments, MindRootReply, StoreLocation,
    SubmitEnvelope,
};
use signal_persona_mind::{
    ActiveClaim, ActivityFilter, ActivityQuery, ActivitySubmission, ActorName, ByRelationKind,
    ByThoughtKind, ClaimActivity, ClaimBody, ClaimScope, FileReference, GoalBody, GoalScope,
    ItemKind, ItemPriority, MindDelta, MindReply, MindRequest, Opening, PathClaimScope, Query,
    QueryKind, QueryLimit, QueryRelations, QueryThoughts, ReferenceBody, ReferenceTarget,
    RelationFilter, RelationKind, RoleClaim, RoleHandoff, RoleName, RoleObservation, RoleRelease,
    ScopeReason, ScopeReference, SubmitRelation, SubmitThought, SubscribeRelations,
    SubscribeThoughts, TextBody, ThoughtBody, ThoughtFilter, ThoughtKind, TimestampNanos, Title,
    WirePath, WorkspaceGoal,
};

struct ActorFixture {
    root: ActorRef<MindRoot>,
    actor: ActorName,
    store: PathBuf,
}

impl ActorFixture {
    async fn new() -> Self {
        let store = Self::store_path();
        Self {
            root: MindRoot::start(MindRootArguments::new(StoreLocation::new(
                store.to_string_lossy().to_string(),
            )))
            .await
            .expect("mind root starts"),
            actor: ActorName::new("operator-assistant"),
            store,
        }
    }

    fn store_path() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "persona-mind-actor-topology-{}-{stamp}.redb",
            std::process::id()
        ))
    }

    fn envelope(&self, request: MindRequest) -> MindEnvelope {
        MindEnvelope::new(self.actor.clone(), request)
    }

    async fn submit(&self, request: MindRequest) -> MindRootReply {
        self.root
            .ask(SubmitEnvelope {
                envelope: self.envelope(request),
            })
            .await
            .expect("actor request succeeds")
    }

    async fn subscription_events(&self) -> Vec<signal_persona_mind::SubscriptionEvent> {
        self.root
            .ask(ReadSubscriptionEvents::all())
            .await
            .expect("subscription event read succeeds")
            .events()
            .to_vec()
    }

    async fn stop(self) {
        MindRoot::stop(self.root).await.expect("mind root stops");
        let _ = std::fs::remove_file(self.store);
    }

    async fn stop_without_removing_store(self) {
        MindRoot::stop(self.root).await.expect("mind root stops");
    }
}

struct ClaimFixture {
    role: RoleName,
    reason: ScopeReason,
}

impl ClaimFixture {
    fn operator() -> Self {
        Self {
            role: RoleName::Operator,
            reason: ScopeReason::from_text("testing persona mind claim flow").expect("reason"),
        }
    }

    fn path(&self, path: &str) -> ScopeReference {
        ScopeReference::Path(WirePath::from_absolute_path(path).expect("absolute path"))
    }

    fn claim(&self, path: &str) -> MindRequest {
        MindRequest::RoleClaim(RoleClaim {
            role: self.role,
            scopes: vec![self.path(path)],
            reason: self.reason.clone(),
        })
    }
}

#[test]
fn topology_manifest_names_required_actor_planes() {
    let manifest = ActorManifest::persona_mind_phase_one();

    for actor in [
        TraceNode::MIND_ROOT,
        TraceNode::INGRESS_PHASE,
        TraceNode::DISPATCH_PHASE,
        TraceNode::DOMAIN_PHASE,
        TraceNode::STORE_SUPERVISOR,
        TraceNode::STORE_KERNEL,
        TraceNode::MEMORY_STORE,
        TraceNode::CLAIM_STORE,
        TraceNode::ACTIVITY_STORE,
        TraceNode::VIEW_PHASE,
        TraceNode::SUBSCRIPTION_SUPERVISOR,
        TraceNode::REPLY_SHAPER,
        TraceNode::SEMA_WRITER,
        TraceNode::SEMA_READER,
        TraceNode::ID_MINT,
        TraceNode::READY_WORK_VIEW,
        TraceNode::NOTA_REPLY_ENCODER,
    ] {
        assert!(manifest.contains(actor), "missing {}", actor.label());
    }

    assert_eq!(manifest.actor_count_for(ActorResidency::Root), 1);
    assert!(manifest.actor_count_for(ActorResidency::LongLived) >= 12);
    assert!(manifest.contains_edge(TraceNode::MIND_ROOT, TraceNode::STORE_SUPERVISOR));
    assert!(manifest.contains_edge(TraceNode::STORE_SUPERVISOR, TraceNode::STORE_KERNEL));
    assert!(manifest.contains_edge(TraceNode::STORE_SUPERVISOR, TraceNode::MEMORY_STORE));
    assert!(manifest.contains_edge(TraceNode::STORE_SUPERVISOR, TraceNode::CLAIM_STORE));
    assert!(manifest.contains_edge(TraceNode::STORE_SUPERVISOR, TraceNode::ACTIVITY_STORE));
    assert!(manifest.contains_edge(TraceNode::REPLY_SHAPER, TraceNode::NOTA_REPLY_ENCODER));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn open_item_runs_through_kameo_write_path() {
    let fixture = ActorFixture::new().await;
    let response = fixture
        .submit(MindRequest::Opening(Opening {
            kind: ItemKind::Task,
            priority: ItemPriority::High,
            title: Title::new("Implement Kameo-backed mind"),
            body: TextBody::new("Phase one actor path"),
        }))
        .await;

    let MindReply::OpeningReceipt(receipt) = response.reply().expect("reply exists") else {
        panic!("expected opened reply");
    };

    assert_eq!(
        receipt.event.header.actor,
        ActorName::new("operator-assistant")
    );
    assert!(response.trace().contains_ordered(&[
        TraceNode::MIND_ROOT,
        TraceNode::INGRESS_PHASE,
        TraceNode::DISPATCH_PHASE,
        TraceNode::MEMORY_FLOW,
        TraceNode::DOMAIN_PHASE,
        TraceNode::ITEM_OPEN,
        TraceNode::STORE_SUPERVISOR,
        TraceNode::MEMORY_STORE,
        TraceNode::SEMA_WRITER,
        TraceNode::COMMIT,
        TraceNode::REPLY_SHAPER,
        TraceNode::MIND_ROOT,
    ]));
    assert!(
        response
            .trace()
            .contains_action(TraceNode::SEMA_WRITER, TraceAction::WriteIntentSent)
    );
    assert!(
        response
            .trace()
            .contains_action(TraceNode::COMMIT, TraceAction::CommitCompleted)
    );

    fixture.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn store_kernel_supervised_thread_restart_reopens_same_database() {
    let first = ActorFixture::new().await;
    let store = first.store.clone();
    let claim = ClaimFixture::operator();
    let claim_scope = claim.path("/git/github.com/LiGoldragon/persona-mind");

    let response = first
        .submit(claim.claim("/git/github.com/LiGoldragon/persona-mind"))
        .await;
    assert!(matches!(
        response.reply().expect("claim reply exists"),
        MindReply::ClaimAcceptance(_)
    ));
    first.stop_without_removing_store().await;

    let second = ActorFixture {
        root: MindRoot::start(MindRootArguments::new(StoreLocation::new(
            store.to_string_lossy().to_string(),
        )))
        .await
        .expect("mind root restarts on same store"),
        actor: ActorName::new("operator-assistant"),
        store,
    };
    let response = second
        .submit(MindRequest::RoleObservation(RoleObservation))
        .await;
    let MindReply::RoleSnapshot(snapshot) = response.reply().expect("observation reply exists")
    else {
        panic!("expected role snapshot");
    };
    let operator_status = snapshot
        .roles
        .iter()
        .find(|status| status.role == RoleName::Operator)
        .expect("operator role status exists");

    assert!(
        operator_status
            .claims
            .iter()
            .any(|active_claim| active_claim.scope == claim_scope),
        "second StoreKernel opens the same redb after the first state drops"
    );
    second.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn query_path_uses_read_actor_without_writer() {
    let fixture = ActorFixture::new().await;
    let _opened = fixture
        .submit(MindRequest::Opening(Opening {
            kind: ItemKind::Task,
            priority: ItemPriority::Normal,
            title: Title::new("Query actor path"),
            body: TextBody::new("Read path witness"),
        }))
        .await;

    let response = fixture
        .submit(MindRequest::Query(Query {
            kind: QueryKind::Ready,
            limit: QueryLimit::new(10),
        }))
        .await;

    let MindReply::View(view) = response.reply().expect("reply exists") else {
        panic!("expected view reply");
    };

    assert_eq!(view.items.len(), 1);
    assert!(response.trace().contains_ordered(&[
        TraceNode::MIND_ROOT,
        TraceNode::INGRESS_PHASE,
        TraceNode::DISPATCH_PHASE,
        TraceNode::QUERY_FLOW,
        TraceNode::VIEW_PHASE,
        TraceNode::READY_WORK_VIEW,
        TraceNode::STORE_SUPERVISOR,
        TraceNode::MEMORY_STORE,
        TraceNode::SEMA_READER,
        TraceNode::QUERY_RESULT_SHAPER,
        TraceNode::REPLY_SHAPER,
    ]));
    assert!(response.trace().contains(TraceNode::SEMA_READER));
    assert!(!response.trace().contains(TraceNode::SEMA_WRITER));

    fixture.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn role_claim_reaches_claim_flow_and_commits() {
    let fixture = ActorFixture::new().await;
    let claim = ClaimFixture::operator();
    let response = fixture
        .submit(claim.claim("/git/github.com/LiGoldragon/persona-mind"))
        .await;

    let MindReply::ClaimAcceptance(acceptance) = response.reply().expect("reply exists") else {
        panic!("expected claim acceptance");
    };

    assert_eq!(acceptance.role, RoleName::Operator);
    assert_eq!(acceptance.scopes.len(), 1);
    assert!(response.trace().contains_ordered(&[
        TraceNode::MIND_ROOT,
        TraceNode::INGRESS_PHASE,
        TraceNode::DISPATCH_PHASE,
        TraceNode::CLAIM_FLOW,
        TraceNode::DOMAIN_PHASE,
        TraceNode::CLAIM_SUPERVISOR,
        TraceNode::STORE_SUPERVISOR,
        TraceNode::CLAIM_STORE,
        TraceNode::SEMA_WRITER,
        TraceNode::COMMIT,
        TraceNode::REPLY_SHAPER,
    ]));

    fixture.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn conflicting_claim_returns_typed_rejection() {
    let fixture = ActorFixture::new().await;
    let claim = ClaimFixture::operator();
    let _accepted = fixture
        .submit(claim.claim("/git/github.com/LiGoldragon/persona"))
        .await;

    let response = fixture
        .submit(MindRequest::RoleClaim(RoleClaim {
            role: RoleName::Designer,
            scopes: vec![claim.path("/git/github.com/LiGoldragon/persona/src")],
            reason: ScopeReason::from_text("designer conflict probe").expect("reason"),
        }))
        .await;

    let MindReply::ClaimRejection(rejection) = response.reply().expect("reply exists") else {
        panic!("expected claim rejection");
    };

    assert_eq!(rejection.role, RoleName::Designer);
    assert_eq!(rejection.conflicts.len(), 1);
    assert_eq!(rejection.conflicts[0].held_by, RoleName::Operator);
    assert!(response.trace().contains(TraceNode::CLAIM_FLOW));
    assert!(response.trace().contains(TraceNode::COMMIT));

    fixture.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn role_observation_reads_claims_without_writer() {
    let fixture = ActorFixture::new().await;
    let claim = ClaimFixture::operator();
    let _accepted = fixture
        .submit(claim.claim("/git/github.com/LiGoldragon/persona-mind"))
        .await;

    let response = fixture
        .submit(MindRequest::RoleObservation(RoleObservation))
        .await;

    let MindReply::RoleSnapshot(snapshot) = response.reply().expect("reply exists") else {
        panic!("expected role snapshot");
    };
    let operator = snapshot
        .roles
        .iter()
        .find(|status| status.role == RoleName::Operator)
        .expect("operator status exists");

    assert_eq!(operator.claims.len(), 1);
    assert!(response.trace().contains(TraceNode::ROLE_SNAPSHOT_VIEW));
    assert!(response.trace().contains(TraceNode::SEMA_READER));
    assert!(!response.trace().contains(TraceNode::SEMA_WRITER));

    fixture.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn role_release_removes_claims_from_observation() {
    let fixture = ActorFixture::new().await;
    let claim = ClaimFixture::operator();
    let _accepted = fixture
        .submit(claim.claim("/git/github.com/LiGoldragon/persona-mind"))
        .await;

    let released = fixture
        .submit(MindRequest::RoleRelease(RoleRelease {
            role: RoleName::Operator,
        }))
        .await;
    let MindReply::ReleaseAcknowledgment(acknowledgment) = released.reply().expect("reply exists")
    else {
        panic!("expected release acknowledgment");
    };
    assert_eq!(acknowledgment.released_scopes.len(), 1);

    let observed = fixture
        .submit(MindRequest::RoleObservation(RoleObservation))
        .await;
    let MindReply::RoleSnapshot(snapshot) = observed.reply().expect("reply exists") else {
        panic!("expected role snapshot");
    };
    let operator = snapshot
        .roles
        .iter()
        .find(|status| status.role == RoleName::Operator)
        .expect("operator status exists");

    assert!(operator.claims.is_empty());

    fixture.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn role_handoff_moves_claim_between_roles() {
    let fixture = ActorFixture::new().await;
    let claim = ClaimFixture::operator();
    let scope = claim.path("/git/github.com/LiGoldragon/persona-mind");
    let _accepted = fixture
        .submit(claim.claim("/git/github.com/LiGoldragon/persona-mind"))
        .await;

    let response = fixture
        .submit(MindRequest::RoleHandoff(RoleHandoff {
            from: RoleName::Operator,
            to: RoleName::Designer,
            scopes: vec![scope.clone()],
            reason: ScopeReason::from_text("handoff to designer").expect("reason"),
        }))
        .await;

    let MindReply::HandoffAcceptance(acceptance) = response.reply().expect("reply exists") else {
        panic!("expected handoff acceptance");
    };

    assert_eq!(acceptance.from, RoleName::Operator);
    assert_eq!(acceptance.to, RoleName::Designer);
    assert_eq!(acceptance.scopes, vec![scope.clone()]);
    assert!(response.trace().contains_ordered(&[
        TraceNode::MIND_ROOT,
        TraceNode::INGRESS_PHASE,
        TraceNode::DISPATCH_PHASE,
        TraceNode::HANDOFF_FLOW,
        TraceNode::DOMAIN_PHASE,
        TraceNode::CLAIM_SUPERVISOR,
        TraceNode::STORE_SUPERVISOR,
        TraceNode::CLAIM_STORE,
        TraceNode::SEMA_WRITER,
        TraceNode::COMMIT,
        TraceNode::REPLY_SHAPER,
    ]));

    let observed = fixture
        .submit(MindRequest::RoleObservation(RoleObservation))
        .await;
    let MindReply::RoleSnapshot(snapshot) = observed.reply().expect("reply exists") else {
        panic!("expected role snapshot");
    };
    let operator = snapshot
        .roles
        .iter()
        .find(|status| status.role == RoleName::Operator)
        .expect("operator status exists");
    let designer = snapshot
        .roles
        .iter()
        .find(|status| status.role == RoleName::Designer)
        .expect("designer status exists");

    assert!(operator.claims.is_empty());
    assert_eq!(designer.claims.len(), 1);
    assert_eq!(designer.claims[0].scope, scope);

    fixture.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn handoff_without_source_claim_returns_typed_rejection() {
    let fixture = ActorFixture::new().await;
    let claim = ClaimFixture::operator();
    let scope = claim.path("/git/github.com/LiGoldragon/persona-mind");
    let response = fixture
        .submit(MindRequest::RoleHandoff(RoleHandoff {
            from: RoleName::Operator,
            to: RoleName::Designer,
            scopes: vec![scope],
            reason: ScopeReason::from_text("missing source probe").expect("reason"),
        }))
        .await;

    let MindReply::HandoffRejection(rejection) = response.reply().expect("reply exists") else {
        panic!("expected handoff rejection");
    };

    assert_eq!(rejection.from, RoleName::Operator);
    assert_eq!(rejection.to, RoleName::Designer);
    assert!(response.trace().contains(TraceNode::HANDOFF_FLOW));
    assert!(response.trace().contains(TraceNode::COMMIT));

    fixture.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn activity_submission_reaches_activity_flow_and_store_mints_time() {
    let fixture = ActorFixture::new().await;
    let response = fixture
        .submit(MindRequest::ActivitySubmission(ActivitySubmission {
            role: RoleName::Operator,
            scope: ScopeReference::Path(
                WirePath::from_absolute_path("/git/github.com/LiGoldragon/persona-mind")
                    .expect("absolute path"),
            ),
            reason: ScopeReason::from_text("record durable activity").expect("reason"),
        }))
        .await;

    let MindReply::ActivityAcknowledgment(acknowledgment) = response.reply().expect("reply exists")
    else {
        panic!("expected activity acknowledgment");
    };

    assert_eq!(acknowledgment.slot, 0);
    assert!(response.trace().contains_ordered(&[
        TraceNode::MIND_ROOT,
        TraceNode::INGRESS_PHASE,
        TraceNode::DISPATCH_PHASE,
        TraceNode::ACTIVITY_FLOW,
        TraceNode::DOMAIN_PHASE,
        TraceNode::STORE_SUPERVISOR,
        TraceNode::ACTIVITY_STORE,
        TraceNode::CLOCK,
        TraceNode::SEMA_WRITER,
        TraceNode::COMMIT,
        TraceNode::REPLY_SHAPER,
    ]));

    fixture.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn role_observation_includes_recent_activity() {
    let fixture = ActorFixture::new().await;
    let scope = ScopeReference::Path(
        WirePath::from_absolute_path("/git/github.com/LiGoldragon/persona-mind")
            .expect("absolute path"),
    );
    let _activity = fixture
        .submit(MindRequest::ActivitySubmission(ActivitySubmission {
            role: RoleName::Operator,
            scope: scope.clone(),
            reason: ScopeReason::from_text("activity before observe").expect("reason"),
        }))
        .await;

    let response = fixture
        .submit(MindRequest::RoleObservation(RoleObservation))
        .await;

    let MindReply::RoleSnapshot(snapshot) = response.reply().expect("reply exists") else {
        panic!("expected role snapshot");
    };

    assert_eq!(snapshot.recent_activity.len(), 1);
    assert_eq!(snapshot.recent_activity[0].role, RoleName::Operator);
    assert_eq!(snapshot.recent_activity[0].scope, scope);
    assert_eq!(
        snapshot.recent_activity[0].reason,
        ScopeReason::from_text("activity before observe").expect("reason")
    );
    assert!(snapshot.recent_activity[0].stamped_at.value() > 0);

    fixture.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn activity_query_reads_recent_activity_without_writer() {
    let fixture = ActorFixture::new().await;
    let first_scope = ScopeReference::Path(
        WirePath::from_absolute_path("/git/github.com/LiGoldragon/persona-mind")
            .expect("absolute path"),
    );
    let second_scope = ScopeReference::Path(
        WirePath::from_absolute_path("/git/github.com/LiGoldragon/persona-router")
            .expect("absolute path"),
    );
    let _first = fixture
        .submit(MindRequest::ActivitySubmission(ActivitySubmission {
            role: RoleName::Operator,
            scope: first_scope,
            reason: ScopeReason::from_text("first activity").expect("reason"),
        }))
        .await;
    let _second = fixture
        .submit(MindRequest::ActivitySubmission(ActivitySubmission {
            role: RoleName::Designer,
            scope: second_scope.clone(),
            reason: ScopeReason::from_text("second activity").expect("reason"),
        }))
        .await;

    let response = fixture
        .submit(MindRequest::ActivityQuery(ActivityQuery {
            limit: 10,
            filters: vec![ActivityFilter::PathPrefix(
                WirePath::from_absolute_path("/git/github.com/LiGoldragon/persona-router")
                    .expect("absolute path"),
            )],
        }))
        .await;

    let MindReply::ActivityList(list) = response.reply().expect("reply exists") else {
        panic!("expected activity list");
    };

    assert_eq!(list.records.len(), 1);
    assert_eq!(list.records[0].role, RoleName::Designer);
    assert_eq!(list.records[0].scope, second_scope);
    assert_eq!(
        list.records[0].reason,
        ScopeReason::from_text("second activity").expect("reason")
    );
    assert!(list.records[0].stamped_at.value() > 0);
    assert!(response.trace().contains(TraceNode::RECENT_ACTIVITY_VIEW));
    assert!(response.trace().contains(TraceNode::SEMA_READER));
    assert!(!response.trace().contains(TraceNode::SEMA_WRITER));

    fixture.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn typed_thought_runs_through_graph_actor_lane_and_store_mints_id() {
    let fixture = ActorFixture::new().await;
    let response = fixture
        .submit(MindRequest::SubmitThought(SubmitThought {
            kind: ThoughtKind::Goal,
            body: ThoughtBody::Goal(GoalBody {
                description: TextBody::new("Make persona-mind replace lock files"),
                scope: GoalScope::Workspace(WorkspaceGoal {
                    workspace: TextBody::new("primary"),
                }),
            }),
        }))
        .await;

    let MindReply::ThoughtCommitted(receipt) = response.reply().expect("reply exists") else {
        panic!("expected thought commit");
    };

    assert_eq!(receipt.record.as_str().len(), 3);
    assert_eq!(receipt.display.as_str(), receipt.record.as_str());
    assert!(!receipt.record.as_str().starts_with("item-"));
    assert!(receipt.occurred_at.value() > 0);
    assert!(response.trace().contains_ordered(&[
        TraceNode::MIND_ROOT,
        TraceNode::INGRESS_PHASE,
        TraceNode::DISPATCH_PHASE,
        TraceNode::GRAPH_FLOW,
        TraceNode::DOMAIN_PHASE,
        TraceNode::MIND_GRAPH_SUPERVISOR,
        TraceNode::THOUGHT_COMMIT,
        TraceNode::STORE_SUPERVISOR,
        TraceNode::GRAPH_STORE,
        TraceNode::ID_MINT,
        TraceNode::CLOCK,
        TraceNode::SEMA_WRITER,
        TraceNode::COMMIT,
        TraceNode::REPLY_SHAPER,
    ]));

    fixture.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn typed_thought_query_uses_reader_without_writer() {
    let fixture = ActorFixture::new().await;
    let _written = fixture
        .submit(MindRequest::SubmitThought(SubmitThought {
            kind: ThoughtKind::Goal,
            body: ThoughtBody::Goal(GoalBody {
                description: TextBody::new("Query typed mind graph"),
                scope: GoalScope::Workspace(WorkspaceGoal {
                    workspace: TextBody::new("primary"),
                }),
            }),
        }))
        .await;

    let response = fixture
        .submit(MindRequest::QueryThoughts(QueryThoughts {
            filter: ThoughtFilter::ByKind(ByThoughtKind {
                kinds: vec![ThoughtKind::Goal],
            }),
            limit: 10,
        }))
        .await;

    let MindReply::ThoughtList(list) = response.reply().expect("reply exists") else {
        panic!("expected thought list");
    };

    assert_eq!(list.thoughts.len(), 1);
    assert_eq!(list.thoughts[0].kind, ThoughtKind::Goal);
    assert_eq!(
        list.thoughts[0].author,
        ActorName::new("operator-assistant")
    );
    assert!(!list.has_more);
    assert!(response.trace().contains_ordered(&[
        TraceNode::MIND_ROOT,
        TraceNode::INGRESS_PHASE,
        TraceNode::DISPATCH_PHASE,
        TraceNode::GRAPH_QUERY_FLOW,
        TraceNode::VIEW_PHASE,
        TraceNode::QUERY_SUPERVISOR,
        TraceNode::THOUGHT_QUERY,
        TraceNode::STORE_SUPERVISOR,
        TraceNode::GRAPH_STORE,
        TraceNode::SEMA_READER,
        TraceNode::QUERY_RESULT_SHAPER,
        TraceNode::REPLY_SHAPER,
    ]));
    assert!(!response.trace().contains(TraceNode::SEMA_WRITER));

    fixture.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn typed_thought_subscription_registers_and_returns_initial_snapshot() {
    let fixture = ActorFixture::new().await;
    let _written = fixture
        .submit(MindRequest::SubmitThought(SubmitThought {
            kind: ThoughtKind::Goal,
            body: ThoughtBody::Goal(GoalBody {
                description: TextBody::new("Subscribe to typed goals"),
                scope: GoalScope::Workspace(WorkspaceGoal {
                    workspace: TextBody::new("primary"),
                }),
            }),
        }))
        .await;

    let response = fixture
        .submit(MindRequest::SubscribeThoughts(SubscribeThoughts {
            filter: ThoughtFilter::ByKind(ByThoughtKind {
                kinds: vec![ThoughtKind::Goal],
            }),
        }))
        .await;

    let MindReply::SubscriptionAccepted(subscription) = response.reply().expect("reply exists")
    else {
        panic!("expected subscription accepted");
    };

    assert_eq!(subscription.subscription.as_str().len(), 3);
    assert_eq!(subscription.initial_snapshot.len(), 1);
    assert!(response.trace().contains_ordered(&[
        TraceNode::MIND_ROOT,
        TraceNode::INGRESS_PHASE,
        TraceNode::DISPATCH_PHASE,
        TraceNode::GRAPH_QUERY_FLOW,
        TraceNode::VIEW_PHASE,
        TraceNode::SUBSCRIPTION_SUPERVISOR,
        TraceNode::STORE_SUPERVISOR,
        TraceNode::GRAPH_STORE,
        TraceNode::ID_MINT,
        TraceNode::SEMA_READER,
        TraceNode::SEMA_WRITER,
        TraceNode::COMMIT,
        TraceNode::REPLY_SHAPER,
    ]));

    fixture.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn typed_thought_subscription_delivers_live_delta_through_subscription_actor() {
    let fixture = ActorFixture::new().await;
    let subscription_response = fixture
        .submit(MindRequest::SubscribeThoughts(SubscribeThoughts {
            filter: ThoughtFilter::ByKind(ByThoughtKind {
                kinds: vec![ThoughtKind::Goal],
            }),
        }))
        .await;

    let MindReply::SubscriptionAccepted(subscription) =
        subscription_response.reply().expect("reply exists")
    else {
        panic!("expected subscription accepted");
    };

    let commit_response = fixture
        .submit(MindRequest::SubmitThought(SubmitThought {
            kind: ThoughtKind::Goal,
            body: ThoughtBody::Goal(GoalBody {
                description: TextBody::new("Deliver a live thought delta"),
                scope: GoalScope::Workspace(WorkspaceGoal {
                    workspace: TextBody::new("primary"),
                }),
            }),
        }))
        .await;
    let events = fixture.subscription_events().await;

    let MindReply::ThoughtCommitted(receipt) = commit_response.reply().expect("reply exists")
    else {
        panic!("expected thought commit");
    };
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].subscription, subscription.subscription);
    let MindDelta::ThoughtCommitted(thought) = &events[0].delta else {
        panic!("expected thought delta");
    };
    assert_eq!(thought.id, receipt.record);
    assert_eq!(thought.kind, ThoughtKind::Goal);

    fixture.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn typed_thought_subscription_filters_live_nonmatching_delta() {
    let fixture = ActorFixture::new().await;
    let _subscription = fixture
        .submit(MindRequest::SubscribeThoughts(SubscribeThoughts {
            filter: ThoughtFilter::ByKind(ByThoughtKind {
                kinds: vec![ThoughtKind::Decision],
            }),
        }))
        .await;

    let _commit = fixture
        .submit(MindRequest::SubmitThought(SubmitThought {
            kind: ThoughtKind::Goal,
            body: ThoughtBody::Goal(GoalBody {
                description: TextBody::new("This is not a decision"),
                scope: GoalScope::Workspace(WorkspaceGoal {
                    workspace: TextBody::new("primary"),
                }),
            }),
        }))
        .await;
    let events = fixture.subscription_events().await;

    assert!(events.is_empty());

    fixture.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn typed_relation_subscription_registers_and_returns_initial_snapshot() {
    let fixture = ActorFixture::new().await;
    let goal = fixture
        .submit(MindRequest::SubmitThought(SubmitThought {
            kind: ThoughtKind::Goal,
            body: ThoughtBody::Goal(GoalBody {
                description: TextBody::new("Relate subscription target"),
                scope: GoalScope::Workspace(WorkspaceGoal {
                    workspace: TextBody::new("primary"),
                }),
            }),
        }))
        .await;
    let claim = fixture
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
        .await;

    let MindReply::ThoughtCommitted(goal) = goal.reply().expect("goal reply exists") else {
        panic!("expected goal commit");
    };
    let MindReply::ThoughtCommitted(claim) = claim.reply().expect("claim reply exists") else {
        panic!("expected claim commit");
    };

    let _relation = fixture
        .submit(MindRequest::SubmitRelation(SubmitRelation {
            kind: RelationKind::Implements,
            source: claim.record.clone(),
            target: goal.record.clone(),
            note: None,
        }))
        .await;
    let response = fixture
        .submit(MindRequest::SubscribeRelations(SubscribeRelations {
            filter: RelationFilter::ByKind(ByRelationKind {
                kinds: vec![RelationKind::Implements],
            }),
        }))
        .await;

    let MindReply::SubscriptionAccepted(subscription) = response.reply().expect("reply exists")
    else {
        panic!("expected subscription accepted");
    };

    assert_eq!(subscription.subscription.as_str().len(), 3);
    assert_eq!(subscription.initial_snapshot.len(), 1);
    assert!(response.trace().contains_ordered(&[
        TraceNode::MIND_ROOT,
        TraceNode::INGRESS_PHASE,
        TraceNode::DISPATCH_PHASE,
        TraceNode::GRAPH_QUERY_FLOW,
        TraceNode::VIEW_PHASE,
        TraceNode::SUBSCRIPTION_SUPERVISOR,
        TraceNode::STORE_SUPERVISOR,
        TraceNode::GRAPH_STORE,
        TraceNode::ID_MINT,
        TraceNode::SEMA_READER,
        TraceNode::SEMA_WRITER,
        TraceNode::COMMIT,
        TraceNode::REPLY_SHAPER,
    ]));

    fixture.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn typed_relation_subscription_delivers_live_delta_through_subscription_actor() {
    let fixture = ActorFixture::new().await;
    let goal = fixture
        .submit(MindRequest::SubmitThought(SubmitThought {
            kind: ThoughtKind::Goal,
            body: ThoughtBody::Goal(GoalBody {
                description: TextBody::new("Relation live delta target"),
                scope: GoalScope::Workspace(WorkspaceGoal {
                    workspace: TextBody::new("primary"),
                }),
            }),
        }))
        .await;
    let claim = fixture
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
        .await;

    let MindReply::ThoughtCommitted(goal) = goal.reply().expect("goal reply exists") else {
        panic!("expected goal commit");
    };
    let MindReply::ThoughtCommitted(claim) = claim.reply().expect("claim reply exists") else {
        panic!("expected claim commit");
    };

    let subscription_response = fixture
        .submit(MindRequest::SubscribeRelations(SubscribeRelations {
            filter: RelationFilter::ByKind(ByRelationKind {
                kinds: vec![RelationKind::Implements],
            }),
        }))
        .await;
    let MindReply::SubscriptionAccepted(subscription) =
        subscription_response.reply().expect("reply exists")
    else {
        panic!("expected subscription accepted");
    };

    let commit_response = fixture
        .submit(MindRequest::SubmitRelation(SubmitRelation {
            kind: RelationKind::Implements,
            source: claim.record.clone(),
            target: goal.record.clone(),
            note: None,
        }))
        .await;
    let events = fixture.subscription_events().await;

    let MindReply::RelationCommitted(receipt) = commit_response.reply().expect("reply exists")
    else {
        panic!("expected relation commit");
    };
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].subscription, subscription.subscription);
    let MindDelta::RelationCommitted(relation) = &events[0].delta else {
        panic!("expected relation delta");
    };
    assert_eq!(relation.id, receipt.relation);
    assert_eq!(relation.kind, RelationKind::Implements);

    fixture.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn superseded_thought_excluded_from_current_query() {
    let fixture = ActorFixture::new().await;
    let old = fixture
        .submit(MindRequest::SubmitThought(SubmitThought {
            kind: ThoughtKind::Goal,
            body: ThoughtBody::Goal(GoalBody {
                description: TextBody::new("Old correction target"),
                scope: GoalScope::Workspace(WorkspaceGoal {
                    workspace: TextBody::new("primary"),
                }),
            }),
        }))
        .await;
    let new = fixture
        .submit(MindRequest::SubmitThought(SubmitThought {
            kind: ThoughtKind::Goal,
            body: ThoughtBody::Goal(GoalBody {
                description: TextBody::new("New correction source"),
                scope: GoalScope::Workspace(WorkspaceGoal {
                    workspace: TextBody::new("primary"),
                }),
            }),
        }))
        .await;

    let MindReply::ThoughtCommitted(old) = old.reply().expect("old reply exists") else {
        panic!("expected old thought commit");
    };
    let MindReply::ThoughtCommitted(new) = new.reply().expect("new reply exists") else {
        panic!("expected new thought commit");
    };

    let relation = fixture
        .submit(MindRequest::SubmitRelation(SubmitRelation {
            kind: RelationKind::Supersedes,
            source: new.record.clone(),
            target: old.record.clone(),
            note: Some(TextBody::new("correction witness")),
        }))
        .await;
    let query = fixture
        .submit(MindRequest::QueryThoughts(QueryThoughts {
            filter: ThoughtFilter::ByKind(ByThoughtKind {
                kinds: vec![ThoughtKind::Goal],
            }),
            limit: 10,
        }))
        .await;

    let MindReply::RelationCommitted(_) = relation.reply().expect("relation reply exists") else {
        panic!("expected supersedes relation commit");
    };
    let MindReply::ThoughtList(list) = query.reply().expect("query reply exists") else {
        panic!("expected thought list");
    };

    assert_eq!(list.thoughts.len(), 1);
    assert_eq!(list.thoughts[0].id, new.record);
    assert_ne!(list.thoughts[0].id, old.record);

    fixture.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn supersedes_relation_rejects_different_thought_kinds() {
    let fixture = ActorFixture::new().await;
    let goal = fixture
        .submit(MindRequest::SubmitThought(SubmitThought {
            kind: ThoughtKind::Goal,
            body: ThoughtBody::Goal(GoalBody {
                description: TextBody::new("Correction target kind"),
                scope: GoalScope::Workspace(WorkspaceGoal {
                    workspace: TextBody::new("primary"),
                }),
            }),
        }))
        .await;
    let claim = fixture
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
        .await;

    let MindReply::ThoughtCommitted(goal) = goal.reply().expect("goal reply exists") else {
        panic!("expected goal commit");
    };
    let MindReply::ThoughtCommitted(claim) = claim.reply().expect("claim reply exists") else {
        panic!("expected claim commit");
    };

    let rejected = fixture
        .submit(MindRequest::SubmitRelation(SubmitRelation {
            kind: RelationKind::Supersedes,
            source: claim.record.clone(),
            target: goal.record.clone(),
            note: None,
        }))
        .await;
    let relations = fixture
        .submit(MindRequest::QueryRelations(QueryRelations {
            filter: RelationFilter::ByKind(ByRelationKind {
                kinds: vec![RelationKind::Supersedes],
            }),
            limit: 10,
        }))
        .await;

    let MindReply::Rejection(_) = rejected.reply().expect("rejection reply exists") else {
        panic!("expected typed rejection");
    };
    let MindReply::RelationList(list) = relations.reply().expect("relations reply exists") else {
        panic!("expected relation list");
    };

    assert!(list.relations.is_empty());

    fixture.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn typed_relation_rejects_missing_thought_endpoint() {
    let fixture = ActorFixture::new().await;
    let response = fixture
        .submit(MindRequest::SubmitRelation(SubmitRelation {
            kind: signal_persona_mind::RelationKind::Supports,
            source: signal_persona_mind::RecordId::new("missing-source"),
            target: signal_persona_mind::RecordId::new("missing-target"),
            note: None,
        }))
        .await;

    let MindReply::Rejection(_) = response.reply().expect("reply exists") else {
        panic!("expected typed rejection");
    };
    assert!(response.trace().contains(TraceNode::GRAPH_FLOW));
    assert!(response.trace().contains(TraceNode::GRAPH_STORE));

    fixture.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn relation_kind_rejects_wrong_domain() {
    let fixture = ActorFixture::new().await;
    let goal = fixture
        .submit(MindRequest::SubmitThought(SubmitThought {
            kind: ThoughtKind::Goal,
            body: ThoughtBody::Goal(GoalBody {
                description: TextBody::new("Wrong relation source"),
                scope: GoalScope::Workspace(WorkspaceGoal {
                    workspace: TextBody::new("primary"),
                }),
            }),
        }))
        .await;
    let claim = fixture
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
        .await;

    let MindReply::ThoughtCommitted(goal) = goal.reply().expect("goal reply exists") else {
        panic!("expected goal commit");
    };
    let MindReply::ThoughtCommitted(claim) = claim.reply().expect("claim reply exists") else {
        panic!("expected claim commit");
    };

    let rejected = fixture
        .submit(MindRequest::SubmitRelation(SubmitRelation {
            kind: RelationKind::Implements,
            source: goal.record.clone(),
            target: claim.record.clone(),
            note: None,
        }))
        .await;
    let relations = fixture
        .submit(MindRequest::QueryRelations(QueryRelations {
            filter: RelationFilter::ByKind(ByRelationKind {
                kinds: vec![RelationKind::Implements],
            }),
            limit: 10,
        }))
        .await;

    let MindReply::Rejection(_) = rejected.reply().expect("rejection reply exists") else {
        panic!("expected typed rejection");
    };
    let MindReply::RelationList(list) = relations.reply().expect("relations reply exists") else {
        panic!("expected relation list");
    };

    assert!(list.relations.is_empty());

    fixture.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn authored_relation_rejects_non_identity_reference_source() {
    let fixture = ActorFixture::new().await;
    let source = fixture
        .submit(MindRequest::SubmitThought(SubmitThought {
            kind: ThoughtKind::Reference,
            body: ThoughtBody::Reference(ReferenceBody {
                target: ReferenceTarget::File(FileReference {
                    path: WirePath::from_absolute_path(
                        "/git/github.com/LiGoldragon/persona-mind/ARCHITECTURE.md",
                    )
                    .expect("absolute path"),
                }),
                sense: Some(TextBody::new("a file reference cannot author a thought")),
            }),
        }))
        .await;
    let target = fixture
        .submit(MindRequest::SubmitThought(SubmitThought {
            kind: ThoughtKind::Goal,
            body: ThoughtBody::Goal(GoalBody {
                description: TextBody::new("Only identities can author graph thoughts"),
                scope: GoalScope::Workspace(WorkspaceGoal {
                    workspace: TextBody::new("primary"),
                }),
            }),
        }))
        .await;

    let MindReply::ThoughtCommitted(source) = source.reply().expect("source reply exists") else {
        panic!("expected source reference commit");
    };
    let MindReply::ThoughtCommitted(target) = target.reply().expect("target reply exists") else {
        panic!("expected target goal commit");
    };

    let rejected = fixture
        .submit(MindRequest::SubmitRelation(SubmitRelation {
            kind: RelationKind::Authored,
            source: source.record.clone(),
            target: target.record.clone(),
            note: None,
        }))
        .await;
    let relations = fixture
        .submit(MindRequest::QueryRelations(QueryRelations {
            filter: RelationFilter::ByKind(ByRelationKind {
                kinds: vec![RelationKind::Authored],
            }),
            limit: 10,
        }))
        .await;

    let MindReply::Rejection(_) = rejected.reply().expect("rejection reply exists") else {
        panic!("expected typed rejection");
    };
    let MindReply::RelationList(list) = relations.reply().expect("relations reply exists") else {
        panic!("expected relation list");
    };

    assert!(list.relations.is_empty());

    fixture.stop().await;
}
