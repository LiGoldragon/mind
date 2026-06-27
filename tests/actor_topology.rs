use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use mind::actors::{ActorManifest, ActorResidency, ReadSubscriptionEvents, TraceAction, TraceNode};
use mind::{
    ActorRef, MindEnvelope, MindRoot, MindRootArguments, MindRootReply, StoreLocation,
    SubmitEnvelope, TechnicalSeedDataset,
};
use signal_mind::{
    AboutTechnicalNode, AcceptedSubscriptionStream, ActiveClaim, ActorName, ByRelationKind,
    ByTechnicalNodeStableKey, ByTechnicalRelationSource, ByThoughtKind, ClaimActivity, ClaimBody,
    ClaimScope, FileReference, GoalBody, GoalScope, ItemKind, Magnitude, MindReply, MindRequest,
    Opening, PathClaimScope, Query, QueryKind, QueryLimit, QueryRelations, QueryTechnicalNodes,
    QueryTechnicalRelations, QueryThoughts, ReferenceBody, ReferenceTarget, RelationFilter,
    RelationKind, RoleName, SubmitRelation, SubmitTechnicalNode, SubmitTechnicalRelation,
    SubmitThought, SubscribeRelations, SubscribeTechnicalNodes, SubscribeTechnicalRelations,
    SubscribeThoughts, SubscriptionCursor, SubscriptionDemand, SubscriptionDemandCredit,
    SubscriptionStreamEvent, SubscriptionStreamKind, TaskToken, TechnicalDependencyClosureQuery,
    TechnicalNodeBody, TechnicalNodeFilter, TechnicalNodeKey, TechnicalNodeKind,
    TechnicalNodeQuery, TechnicalNodeRejectionReason, TechnicalProvenanceChainQuery,
    TechnicalRelationFilter, TechnicalRelationKind, TechnicalRelationNeighborhoodDirection,
    TechnicalRelationNeighborhoodQuery, TechnicalRelationRejectionReason, TechnicalSourceLocator,
    TextBody, ThoughtBody, ThoughtFilter, ThoughtKind, TimestampNanos, Title, WirePath,
    WorkspaceGoal,
};
use signal_persona::ComponentName;

static ACTOR_FIXTURE_LOCK: Mutex<()> = Mutex::new(());

fn technical_key(value: &str) -> TechnicalNodeKey {
    TechnicalNodeKey::from_canonical(value).expect("test technical key is canonical")
}

fn initial_demand(count: u16) -> SubscriptionDemandCredit {
    SubscriptionDemandCredit::new(count)
}

fn technical_component(stable_key: &str, component: &str) -> SubmitTechnicalNode {
    SubmitTechnicalNode {
        stable_key: technical_key(stable_key),
        kind: TechnicalNodeKind::Component,
        body: TechnicalNodeBody::Component(signal_mind::ComponentNode {
            component: ComponentName::new(component),
            summary: None,
        }),
    }
}

fn technical_repository(stable_key: &str, path: &str) -> SubmitTechnicalNode {
    SubmitTechnicalNode {
        stable_key: technical_key(stable_key),
        kind: TechnicalNodeKind::Repository,
        body: TechnicalNodeBody::Repository(signal_mind::RepositoryNode {
            path: WirePath::from_absolute_path(path).expect("test repository path is absolute"),
            remote: None,
        }),
    }
}

fn technical_crate(stable_key: &str, name: &str, repository: &str) -> SubmitTechnicalNode {
    SubmitTechnicalNode {
        stable_key: technical_key(stable_key),
        kind: TechnicalNodeKind::Crate,
        body: TechnicalNodeBody::Crate(signal_mind::CrateNode {
            name: TextBody::new(name),
            repository: technical_key(repository),
        }),
    }
}

fn technical_contract(
    stable_key: &str,
    name: &str,
    surface: signal_mind::ContractSurface,
) -> SubmitTechnicalNode {
    SubmitTechnicalNode {
        stable_key: technical_key(stable_key),
        kind: TechnicalNodeKind::Contract,
        body: TechnicalNodeBody::Contract(signal_mind::ContractNode {
            name: TextBody::new(name),
            surface,
        }),
    }
}

fn technical_claim(stable_key: &str, claim: &str) -> SubmitTechnicalNode {
    SubmitTechnicalNode {
        stable_key: technical_key(stable_key),
        kind: TechnicalNodeKind::TechnicalClaim,
        body: TechnicalNodeBody::TechnicalClaim(signal_mind::TechnicalClaimNode {
            claim: TextBody::new(claim),
        }),
    }
}

fn technical_work_item(stable_key: &str, task: &str, title: &str) -> SubmitTechnicalNode {
    SubmitTechnicalNode {
        stable_key: technical_key(stable_key),
        kind: TechnicalNodeKind::WorkItem,
        body: TechnicalNodeBody::WorkItem(signal_mind::WorkItemNode {
            task: TaskToken::try_new(task.to_string()).expect("test task token is valid"),
            title: TextBody::new(title),
        }),
    }
}

fn technical_source_artifact(
    stable_key: &str,
    locator: TechnicalSourceLocator,
) -> SubmitTechnicalNode {
    SubmitTechnicalNode {
        stable_key: technical_key(stable_key),
        kind: TechnicalNodeKind::SourceArtifact,
        body: TechnicalNodeBody::SourceArtifact(signal_mind::SourceArtifactNode {
            locator,
            summary: None,
        }),
    }
}

fn technical_report(stable_key: &str, path: &str, summary: &str) -> SubmitTechnicalNode {
    SubmitTechnicalNode {
        stable_key: technical_key(stable_key),
        kind: TechnicalNodeKind::Report,
        body: TechnicalNodeBody::Report(signal_mind::ReportNode {
            path: WirePath::from_absolute_path(path).expect("test report path is absolute"),
            summary: Some(TextBody::new(summary)),
        }),
    }
}

fn technical_witness(stable_key: &str, summary: &str) -> SubmitTechnicalNode {
    SubmitTechnicalNode {
        stable_key: technical_key(stable_key),
        kind: TechnicalNodeKind::Witness,
        body: TechnicalNodeBody::Witness(signal_mind::WitnessNode {
            summary: TextBody::new(summary),
            locator: None,
        }),
    }
}

fn technical_relation(
    kind: TechnicalRelationKind,
    source: &str,
    target: &str,
) -> SubmitTechnicalRelation {
    SubmitTechnicalRelation {
        kind,
        source: technical_key(source),
        target: technical_key(target),
        note: None,
    }
}

struct ActorFixture {
    root: ActorRef<MindRoot>,
    actor: ActorName,
    store: PathBuf,
    _guard: MutexGuard<'static, ()>,
}

impl ActorFixture {
    #[allow(clippy::await_holding_lock)]
    async fn new() -> Self {
        let guard = ACTOR_FIXTURE_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let store = Self::store_path();
        Self::from_store_with_guard(store, guard).await
    }

    #[allow(clippy::await_holding_lock)]
    async fn from_store(store: PathBuf) -> Self {
        let guard = ACTOR_FIXTURE_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        Self::from_store_with_guard(store, guard).await
    }

    async fn from_store_with_guard(store: PathBuf, guard: MutexGuard<'static, ()>) -> Self {
        Self {
            root: MindRoot::start(MindRootArguments::new(StoreLocation::new(
                store.to_string_lossy().to_string(),
            )))
            .await
            .expect("mind root starts"),
            actor: ActorName::new("operator-assistant"),
            store,
            _guard: guard,
        }
    }

    fn store_path() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "mind-actor-topology-{}-{stamp}.sema",
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

    async fn subscription_events(&self) -> Vec<signal_mind::SubscriptionEvent> {
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

#[test]
fn topology_manifest_names_required_actor_planes() {
    let manifest = ActorManifest::mind_phase_one();

    for actor in [
        TraceNode::MIND_ROOT,
        TraceNode::INGRESS_PHASE,
        TraceNode::DISPATCH_PHASE,
        TraceNode::DOMAIN_PHASE,
        TraceNode::STORE_SUPERVISOR,
        TraceNode::STORE_KERNEL,
        TraceNode::MEMORY_STORE,
        TraceNode::GRAPH_STORE,
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
    assert!(manifest.actor_count_for(ActorResidency::LongLived) >= 10);
    assert!(manifest.contains_edge(TraceNode::MIND_ROOT, TraceNode::STORE_SUPERVISOR));
    assert!(manifest.contains_edge(TraceNode::STORE_SUPERVISOR, TraceNode::STORE_KERNEL));
    assert!(manifest.contains_edge(TraceNode::STORE_SUPERVISOR, TraceNode::MEMORY_STORE));
    assert!(manifest.contains_edge(TraceNode::STORE_SUPERVISOR, TraceNode::GRAPH_STORE));
    assert!(manifest.contains_edge(TraceNode::REPLY_SHAPER, TraceNode::NOTA_REPLY_ENCODER));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn open_item_runs_through_kameo_write_path() {
    let fixture = ActorFixture::new().await;
    let response = fixture
        .submit(MindRequest::Opening(Opening {
            kind: ItemKind::Task,
            priority: Magnitude::High,
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

    let response = first
        .submit(MindRequest::Opening(Opening {
            kind: ItemKind::Task,
            priority: Magnitude::High,
            title: Title::new("Durable actor work"),
            body: TextBody::new("The reopened StoreKernel sees committed memory."),
        }))
        .await;
    let MindReply::OpeningReceipt(opening) = response.reply().expect("opening reply exists") else {
        panic!("expected opening receipt");
    };
    first.stop_without_removing_store().await;

    let second = ActorFixture {
        root: MindRoot::start(MindRootArguments::new(StoreLocation::new(
            store.to_string_lossy().to_string(),
        )))
        .await
        .expect("mind root restarts on same store"),
        actor: ActorName::new("operator-assistant"),
        store,
        _guard: ACTOR_FIXTURE_LOCK
            .lock()
            .expect("actor fixture lock is available"),
    };
    let response = second
        .submit(MindRequest::Query(Query {
            kind: QueryKind::Ready,
            limit: QueryLimit::new(10),
        }))
        .await;
    let MindReply::View(view) = response.reply().expect("query reply exists") else {
        panic!("expected view reply");
    };

    assert!(
        view.items
            .iter()
            .any(|item| item.id == opening.event.item.id
                && item.title == Title::new("Durable actor work")),
        "second StoreKernel opens the same sema store after the first state drops"
    );
    second.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn query_path_uses_read_actor_without_writer() {
    let fixture = ActorFixture::new().await;
    let _opened = fixture
        .submit(MindRequest::Opening(Opening {
            kind: ItemKind::Task,
            priority: Magnitude::Medium,
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
async fn typed_thought_runs_through_graph_actor_lane_and_store_mints_id() {
    let fixture = ActorFixture::new().await;
    let response = fixture
        .submit(MindRequest::SubmitThought(SubmitThought {
            kind: ThoughtKind::Goal,
            body: ThoughtBody::Goal(GoalBody {
                description: TextBody::new("Make mind replace lock files"),
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
            limit: QueryLimit::new(10),
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
async fn technical_node_and_relation_append_query_through_actor_lane() {
    let fixture = ActorFixture::new().await;
    let component_key = technical_key("component:mind");
    let repository_key = technical_key("repo:mind");

    let component = fixture
        .submit(MindRequest::SubmitTechnicalNode(SubmitTechnicalNode {
            stable_key: component_key.clone(),
            kind: TechnicalNodeKind::Component,
            body: TechnicalNodeBody::Component(signal_mind::ComponentNode {
                component: ComponentName::new("mind"),
                summary: Some(TextBody::new("mind daemon")),
            }),
        }))
        .await;
    let repository = fixture
        .submit(MindRequest::SubmitTechnicalNode(SubmitTechnicalNode {
            stable_key: repository_key.clone(),
            kind: TechnicalNodeKind::Repository,
            body: TechnicalNodeBody::Repository(signal_mind::RepositoryNode {
                path: WirePath::from_absolute_path("/git/github.com/LiGoldragon/mind")
                    .expect("absolute path"),
                remote: None,
            }),
        }))
        .await;
    let relation = fixture
        .submit(MindRequest::SubmitTechnicalRelation(
            SubmitTechnicalRelation {
                kind: TechnicalRelationKind::OwnsRepository,
                source: component_key.clone(),
                target: repository_key.clone(),
                note: Some(TextBody::new(
                    "component owns its implementation repository",
                )),
            },
        ))
        .await;
    let nodes = fixture
        .submit(MindRequest::QueryTechnicalNodes(QueryTechnicalNodes {
            query: TechnicalNodeQuery::Filter(TechnicalNodeFilter::ByStableKey(
                ByTechnicalNodeStableKey {
                    stable_key: component_key.clone(),
                },
            )),
            limit: QueryLimit::new(10),
        }))
        .await;
    let relations = fixture
        .submit(MindRequest::QueryTechnicalRelations(
            QueryTechnicalRelations {
                filter: TechnicalRelationFilter::BySource(ByTechnicalRelationSource {
                    source: component_key.clone(),
                }),
                limit: QueryLimit::new(10),
            },
        ))
        .await;

    let MindReply::TechnicalNodeCommitted(component) =
        component.reply().expect("component reply exists")
    else {
        panic!("expected technical component commit");
    };
    let MindReply::TechnicalNodeCommitted(repository) =
        repository.reply().expect("repository reply exists")
    else {
        panic!("expected technical repository commit");
    };
    let MindReply::TechnicalRelationCommitted(relation) =
        relation.reply().expect("relation reply exists")
    else {
        panic!("expected technical relation commit");
    };
    let MindReply::TechnicalNodeList(nodes) = nodes.reply().expect("node query reply exists")
    else {
        panic!("expected technical node list");
    };
    let MindReply::TechnicalRelationList(relations) =
        relations.reply().expect("relation query reply exists")
    else {
        panic!("expected technical relation list");
    };

    assert_eq!(component.node.identifier.as_str(), "aaa");
    assert_eq!(repository.node.identifier.as_str(), "aab");
    assert_eq!(relation.relation.identifier.as_str(), "aac");
    assert_eq!(
        relation.relation.source.identifier,
        component.node.identifier
    );
    assert_eq!(
        relation.relation.target.identifier,
        repository.node.identifier
    );
    assert_eq!(nodes.nodes.len(), 1);
    assert_eq!(nodes.nodes[0].stable_key, component_key);
    assert!(!nodes.has_more);
    assert_eq!(relations.relations.len(), 1);
    assert_eq!(
        relations.relations[0].source.stable_key,
        nodes.nodes[0].stable_key
    );
    assert!(!relations.has_more);

    fixture.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn technical_append_rejects_invalid_records() {
    let fixture = ActorFixture::new().await;
    let component_key = technical_key("component:mind");
    let repository_key = technical_key("repo:mind");

    let _component = fixture
        .submit(MindRequest::SubmitTechnicalNode(SubmitTechnicalNode {
            stable_key: component_key.clone(),
            kind: TechnicalNodeKind::Component,
            body: TechnicalNodeBody::Component(signal_mind::ComponentNode {
                component: ComponentName::new("mind"),
                summary: None,
            }),
        }))
        .await;
    let _repository = fixture
        .submit(MindRequest::SubmitTechnicalNode(SubmitTechnicalNode {
            stable_key: repository_key.clone(),
            kind: TechnicalNodeKind::Repository,
            body: TechnicalNodeBody::Repository(signal_mind::RepositoryNode {
                path: WirePath::from_absolute_path("/git/github.com/LiGoldragon/mind")
                    .expect("absolute path"),
                remote: None,
            }),
        }))
        .await;

    let duplicate_node = fixture
        .submit(MindRequest::SubmitTechnicalNode(SubmitTechnicalNode {
            stable_key: component_key.clone(),
            kind: TechnicalNodeKind::Component,
            body: TechnicalNodeBody::Component(signal_mind::ComponentNode {
                component: ComponentName::new("mind-duplicate"),
                summary: None,
            }),
        }))
        .await;
    let missing_endpoint = fixture
        .submit(MindRequest::SubmitTechnicalRelation(
            SubmitTechnicalRelation {
                kind: TechnicalRelationKind::OwnsRepository,
                source: technical_key("component:missing"),
                target: repository_key.clone(),
                note: None,
            },
        ))
        .await;
    let first_relation = fixture
        .submit(MindRequest::SubmitTechnicalRelation(
            SubmitTechnicalRelation {
                kind: TechnicalRelationKind::OwnsRepository,
                source: component_key.clone(),
                target: repository_key.clone(),
                note: None,
            },
        ))
        .await;
    let duplicate_relation = fixture
        .submit(MindRequest::SubmitTechnicalRelation(
            SubmitTechnicalRelation {
                kind: TechnicalRelationKind::OwnsRepository,
                source: component_key.clone(),
                target: repository_key.clone(),
                note: Some(TextBody::new("duplicate note still rejects")),
            },
        ))
        .await;
    let wrong_domain = fixture
        .submit(MindRequest::SubmitTechnicalRelation(
            SubmitTechnicalRelation {
                kind: TechnicalRelationKind::OwnsRepository,
                source: repository_key.clone(),
                target: component_key.clone(),
                note: None,
            },
        ))
        .await;

    assert!(matches!(
        duplicate_node.reply().expect("duplicate node reply exists"),
        MindReply::TechnicalNodeRejected(rejection)
            if rejection.reason
                == TechnicalNodeRejectionReason::DuplicateStableNodeKey(component_key.clone())
    ));
    assert!(matches!(
        missing_endpoint
            .reply()
            .expect("missing endpoint reply exists"),
        MindReply::TechnicalRelationRejected(rejection)
            if rejection.reason
                == TechnicalRelationRejectionReason::MissingEndpoint(
                    technical_key("component:missing")
                )
    ));
    assert!(matches!(
        first_relation.reply().expect("first relation reply exists"),
        MindReply::TechnicalRelationCommitted(_)
    ));
    assert!(matches!(
        duplicate_relation
            .reply()
            .expect("duplicate relation reply exists"),
        MindReply::TechnicalRelationRejected(rejection)
            if rejection.reason == TechnicalRelationRejectionReason::DuplicateRelation
    ));
    assert!(matches!(
        wrong_domain.reply().expect("wrong domain reply exists"),
        MindReply::TechnicalRelationRejected(rejection)
            if matches!(
                rejection.reason,
                TechnicalRelationRejectionReason::DomainRangeViolation(_)
            )
    ));

    fixture.stop().await;
}

#[test]
fn technical_node_key_validation_rejects_invalid_shapes() {
    let invalid_keys = [
        "mind",
        "repository:mind",
        "component:Mind",
        "component:",
        "contract:signal-mind",
        "storage:mind",
        "schema:mind:",
        "table:mind:technical nodes",
    ];

    for key in invalid_keys {
        assert!(
            TechnicalNodeKey::from_canonical(key).is_err(),
            "{key} should reject before it can enter a MindRequest"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn technical_storage_schema_and_table_facts_round_trip_through_actor_lane() {
    let fixture = ActorFixture::new().await;
    let storage_key = technical_key("storage:mind:mind.sema");
    let schema_key = technical_key("schema:mind:technical-v2");
    let table_key = technical_key("table:mind:technical_nodes");

    let nodes = [
        technical_component("component:mind", "mind"),
        SubmitTechnicalNode {
            stable_key: storage_key.clone(),
            kind: TechnicalNodeKind::StorageResource,
            body: TechnicalNodeBody::StorageResource(signal_mind::StorageResourceNode {
                owner: technical_key("component:mind"),
                name: TextBody::new("mind.sema"),
                path: Some(
                    WirePath::from_absolute_path("/home/li/.local/state/mind/mind.sema")
                        .expect("absolute path"),
                ),
            }),
        },
        SubmitTechnicalNode {
            stable_key: schema_key.clone(),
            kind: TechnicalNodeKind::SchemaFamily,
            body: TechnicalNodeBody::SchemaFamily(signal_mind::SchemaFamilyNode {
                owner: technical_key("component:mind"),
                name: TextBody::new("technical-v2"),
                version: Some(TextBody::new("2")),
            }),
        },
        SubmitTechnicalNode {
            stable_key: table_key.clone(),
            kind: TechnicalNodeKind::Table,
            body: TechnicalNodeBody::Table(signal_mind::TableNode {
                storage: storage_key.clone(),
                name: TextBody::new("technical_nodes"),
                schema_family: Some(schema_key.clone()),
            }),
        },
    ];

    for node in nodes {
        assert!(matches!(
            fixture
                .submit(MindRequest::SubmitTechnicalNode(node))
                .await
                .reply()
                .expect("node reply exists"),
            MindReply::TechnicalNodeCommitted(_)
        ));
    }

    for relation in [
        technical_relation(
            TechnicalRelationKind::StorageDependency,
            "component:mind",
            "storage:mind:mind.sema",
        ),
        technical_relation(
            TechnicalRelationKind::StorageDependency,
            "storage:mind:mind.sema",
            "schema:mind:technical-v2",
        ),
        technical_relation(
            TechnicalRelationKind::StorageDependency,
            "schema:mind:technical-v2",
            "table:mind:technical_nodes",
        ),
    ] {
        assert!(matches!(
            fixture
                .submit(MindRequest::SubmitTechnicalRelation(relation))
                .await
                .reply()
                .expect("relation reply exists"),
            MindReply::TechnicalRelationCommitted(_)
        ));
    }

    let storage_nodes = fixture
        .submit(MindRequest::QueryTechnicalNodes(QueryTechnicalNodes {
            query: TechnicalNodeQuery::Filter(TechnicalNodeFilter::ByKind(
                signal_mind::ByTechnicalNodeKind {
                    kinds: vec![
                        TechnicalNodeKind::StorageResource,
                        TechnicalNodeKind::SchemaFamily,
                        TechnicalNodeKind::Table,
                    ],
                },
            )),
            limit: QueryLimit::new(10),
        }))
        .await;
    let storage_relations = fixture
        .submit(MindRequest::QueryTechnicalRelations(
            QueryTechnicalRelations {
                filter: TechnicalRelationFilter::ByKind(signal_mind::ByTechnicalRelationKind {
                    kinds: vec![TechnicalRelationKind::StorageDependency],
                }),
                limit: QueryLimit::new(10),
            },
        ))
        .await;

    let MindReply::TechnicalNodeList(storage_nodes) = storage_nodes
        .reply()
        .expect("storage node query reply exists")
    else {
        panic!("expected storage node list");
    };
    let MindReply::TechnicalRelationList(storage_relations) = storage_relations
        .reply()
        .expect("storage relation query reply exists")
    else {
        panic!("expected storage relation list");
    };

    assert_eq!(storage_nodes.nodes.len(), 3);
    assert!(
        storage_nodes
            .nodes
            .iter()
            .any(|node| node.stable_key == storage_key)
    );
    assert!(
        storage_nodes
            .nodes
            .iter()
            .any(|node| node.stable_key == schema_key)
    );
    assert!(
        storage_nodes
            .nodes
            .iter()
            .any(|node| node.stable_key == table_key)
    );
    assert_eq!(storage_relations.relations.len(), 3);

    fixture.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn technical_graph_neighborhood_closure_and_provenance_queries_use_scan_reader() {
    let fixture = ActorFixture::new().await;

    for node in [
        technical_component("component:mind", "mind"),
        technical_repository("repo:mind", "/git/github.com/LiGoldragon/mind"),
        technical_crate("crate:mind", "mind", "repo:mind"),
        technical_contract(
            "contract:signal-mind:ordinary",
            "signal-mind",
            signal_mind::ContractSurface::Ordinary,
        ),
        SubmitTechnicalNode {
            stable_key: technical_key("storage:mind:mind.sema"),
            kind: TechnicalNodeKind::StorageResource,
            body: TechnicalNodeBody::StorageResource(signal_mind::StorageResourceNode {
                owner: technical_key("component:mind"),
                name: TextBody::new("mind.sema"),
                path: None,
            }),
        },
        SubmitTechnicalNode {
            stable_key: technical_key("schema:mind:technical-v2"),
            kind: TechnicalNodeKind::SchemaFamily,
            body: TechnicalNodeBody::SchemaFamily(signal_mind::SchemaFamilyNode {
                owner: technical_key("component:mind"),
                name: TextBody::new("technical-v2"),
                version: Some(TextBody::new("2")),
            }),
        },
        SubmitTechnicalNode {
            stable_key: technical_key("table:mind:technical_nodes"),
            kind: TechnicalNodeKind::Table,
            body: TechnicalNodeBody::Table(signal_mind::TableNode {
                storage: technical_key("storage:mind:mind.sema"),
                name: TextBody::new("technical_nodes"),
                schema_family: Some(technical_key("schema:mind:technical-v2")),
            }),
        },
        technical_work_item("task:primary-pm7l.8", "primary-pm7l.8", "graph queries"),
        technical_work_item("task:primary-pm7l.9", "primary-pm7l.9", "seed expansion"),
        technical_source_artifact(
            "artifact:mind-graph-query-code",
            TechnicalSourceLocator::Path(
                WirePath::from_absolute_path("/git/github.com/LiGoldragon/mind/src/graph.rs")
                    .expect("test artifact path is absolute"),
            ),
        ),
        technical_report(
            "report:technical-query-design",
            "/home/li/primary/reports/designer/technical-query-design.md",
            "technical query design",
        ),
        technical_claim(
            "claim:technical-queries-are-scan-based",
            "technical graph queries are scan-based",
        ),
        technical_claim(
            "claim:old-technical-query-shape",
            "technical graph queries require client-side traversal",
        ),
        technical_witness(
            "witness:technical-query-check",
            "focused actor topology check",
        ),
    ] {
        assert!(matches!(
            fixture
                .submit(MindRequest::SubmitTechnicalNode(node))
                .await
                .reply()
                .expect("technical node reply exists"),
            MindReply::TechnicalNodeCommitted(_)
        ));
    }

    for relation in [
        technical_relation(
            TechnicalRelationKind::DefinesCrate,
            "repo:mind",
            "crate:mind",
        ),
        technical_relation(
            TechnicalRelationKind::DefinesContract,
            "crate:mind",
            "contract:signal-mind:ordinary",
        ),
        technical_relation(
            TechnicalRelationKind::BuildDependency,
            "crate:mind",
            "contract:signal-mind:ordinary",
        ),
        technical_relation(
            TechnicalRelationKind::RuntimeDependency,
            "component:mind",
            "storage:mind:mind.sema",
        ),
        technical_relation(
            TechnicalRelationKind::WireDependency,
            "component:mind",
            "contract:signal-mind:ordinary",
        ),
        technical_relation(
            TechnicalRelationKind::StorageDependency,
            "storage:mind:mind.sema",
            "schema:mind:technical-v2",
        ),
        technical_relation(
            TechnicalRelationKind::StorageDependency,
            "schema:mind:technical-v2",
            "table:mind:technical_nodes",
        ),
        technical_relation(
            TechnicalRelationKind::TaskDependency,
            "task:primary-pm7l.8",
            "task:primary-pm7l.9",
        ),
        technical_relation(
            TechnicalRelationKind::ProvenanceDependency,
            "claim:technical-queries-are-scan-based",
            "report:technical-query-design",
        ),
        technical_relation(
            TechnicalRelationKind::ProvenanceDependency,
            "task:primary-pm7l.8",
            "artifact:mind-graph-query-code",
        ),
        technical_relation(
            TechnicalRelationKind::ProvenBy,
            "claim:technical-queries-are-scan-based",
            "witness:technical-query-check",
        ),
        technical_relation(
            TechnicalRelationKind::Supersedes,
            "claim:technical-queries-are-scan-based",
            "claim:old-technical-query-shape",
        ),
        technical_relation(
            TechnicalRelationKind::Documents,
            "report:technical-query-design",
            "component:mind",
        ),
    ] {
        assert!(matches!(
            fixture
                .submit(MindRequest::SubmitTechnicalRelation(relation))
                .await
                .reply()
                .expect("technical relation reply exists"),
            MindReply::TechnicalRelationCommitted(_)
        ));
    }

    let about = fixture
        .submit(MindRequest::QueryTechnicalNodes(QueryTechnicalNodes {
            query: TechnicalNodeQuery::About(AboutTechnicalNode {
                stable_key: technical_key("component:mind"),
            }),
            limit: QueryLimit::new(25),
        }))
        .await;
    let MindReply::TechnicalNodeNeighborhood(about) = about.reply().expect("about reply exists")
    else {
        panic!("expected technical node neighborhood reply");
    };
    assert_eq!(
        about
            .center
            .as_ref()
            .expect("about center exists")
            .stable_key,
        technical_key("component:mind")
    );
    assert!(
        about
            .incoming
            .iter()
            .any(|relation| relation.kind == TechnicalRelationKind::Documents)
    );
    assert!(
        about
            .outgoing
            .iter()
            .any(|relation| relation.kind == TechnicalRelationKind::RuntimeDependency)
    );
    assert!(
        about
            .outgoing
            .iter()
            .any(|relation| relation.kind == TechnicalRelationKind::WireDependency)
    );
    assert!(!about.has_more);

    let outgoing = fixture
        .submit(MindRequest::QueryTechnicalNodes(QueryTechnicalNodes {
            query: TechnicalNodeQuery::RelationNeighborhood(TechnicalRelationNeighborhoodQuery {
                stable_key: technical_key("component:mind"),
                direction: TechnicalRelationNeighborhoodDirection::Outgoing,
                kinds: vec![
                    TechnicalRelationKind::RuntimeDependency,
                    TechnicalRelationKind::WireDependency,
                ],
            }),
            limit: QueryLimit::new(25),
        }))
        .await;
    let MindReply::TechnicalNodeNeighborhood(outgoing) =
        outgoing.reply().expect("outgoing reply exists")
    else {
        panic!("expected technical node neighborhood reply");
    };
    assert!(outgoing.incoming.is_empty());
    assert_eq!(outgoing.outgoing.len(), 2);

    let incoming = fixture
        .submit(MindRequest::QueryTechnicalNodes(QueryTechnicalNodes {
            query: TechnicalNodeQuery::RelationNeighborhood(TechnicalRelationNeighborhoodQuery {
                stable_key: technical_key("component:mind"),
                direction: TechnicalRelationNeighborhoodDirection::Incoming,
                kinds: Vec::new(),
            }),
            limit: QueryLimit::new(25),
        }))
        .await;
    let MindReply::TechnicalNodeNeighborhood(incoming) =
        incoming.reply().expect("incoming reply exists")
    else {
        panic!("expected technical node neighborhood reply");
    };
    assert_eq!(incoming.incoming.len(), 1);
    assert!(incoming.outgoing.is_empty());

    let closure = fixture
        .submit(MindRequest::QueryTechnicalNodes(QueryTechnicalNodes {
            query: TechnicalNodeQuery::DependencyClosure(TechnicalDependencyClosureQuery {
                stable_key: technical_key("component:mind"),
                kinds: Vec::new(),
            }),
            limit: QueryLimit::new(25),
        }))
        .await;
    let MindReply::TechnicalDependencyClosure(closure) =
        closure.reply().expect("closure reply exists")
    else {
        panic!("expected technical dependency closure reply");
    };
    let closure_keys = closure
        .nodes
        .iter()
        .map(|node| node.stable_key.clone())
        .collect::<HashSet<_>>();
    assert!(closure_keys.contains(&technical_key("contract:signal-mind:ordinary")));
    assert!(closure_keys.contains(&technical_key("storage:mind:mind.sema")));
    assert!(closure_keys.contains(&technical_key("schema:mind:technical-v2")));
    assert!(closure_keys.contains(&technical_key("table:mind:technical_nodes")));
    assert!(closure.relations.iter().all(|relation| matches!(
        relation.kind,
        TechnicalRelationKind::RuntimeDependency
            | TechnicalRelationKind::WireDependency
            | TechnicalRelationKind::StorageDependency
    )));
    assert!(!closure.has_more);

    let build_closure = fixture
        .submit(MindRequest::QueryTechnicalNodes(QueryTechnicalNodes {
            query: TechnicalNodeQuery::DependencyClosure(TechnicalDependencyClosureQuery {
                stable_key: technical_key("crate:mind"),
                kinds: vec![TechnicalRelationKind::BuildDependency],
            }),
            limit: QueryLimit::new(25),
        }))
        .await;
    let MindReply::TechnicalDependencyClosure(build_closure) =
        build_closure.reply().expect("build closure reply exists")
    else {
        panic!("expected technical dependency closure reply");
    };
    assert!(
        build_closure
            .relations
            .iter()
            .any(|relation| relation.kind == TechnicalRelationKind::BuildDependency)
    );

    let task_closure = fixture
        .submit(MindRequest::QueryTechnicalNodes(QueryTechnicalNodes {
            query: TechnicalNodeQuery::DependencyClosure(TechnicalDependencyClosureQuery {
                stable_key: technical_key("task:primary-pm7l.8"),
                kinds: vec![TechnicalRelationKind::TaskDependency],
            }),
            limit: QueryLimit::new(25),
        }))
        .await;
    let MindReply::TechnicalDependencyClosure(task_closure) =
        task_closure.reply().expect("task closure reply exists")
    else {
        panic!("expected technical dependency closure reply");
    };
    assert!(
        task_closure
            .nodes
            .iter()
            .any(|node| node.stable_key == technical_key("task:primary-pm7l.9"))
    );

    let provenance = fixture
        .submit(MindRequest::QueryTechnicalNodes(QueryTechnicalNodes {
            query: TechnicalNodeQuery::ProvenanceChain(TechnicalProvenanceChainQuery {
                stable_key: technical_key("claim:technical-queries-are-scan-based"),
                kinds: Vec::new(),
            }),
            limit: QueryLimit::new(25),
        }))
        .await;
    let MindReply::TechnicalProvenanceChain(provenance) =
        provenance.reply().expect("provenance reply exists")
    else {
        panic!("expected technical provenance chain reply");
    };
    let provenance_keys = provenance
        .nodes
        .iter()
        .map(|node| node.stable_key.clone())
        .collect::<HashSet<_>>();
    assert!(provenance_keys.contains(&technical_key("report:technical-query-design")));
    assert!(provenance_keys.contains(&technical_key("witness:technical-query-check")));
    assert!(provenance_keys.contains(&technical_key("claim:old-technical-query-shape")));
    assert!(provenance.relations.iter().any(|relation| {
        relation.kind == TechnicalRelationKind::ProvenanceDependency
            && relation.target.stable_key == technical_key("report:technical-query-design")
    }));
    assert!(
        provenance
            .relations
            .iter()
            .any(|relation| relation.kind == TechnicalRelationKind::ProvenBy)
    );
    assert!(
        provenance
            .relations
            .iter()
            .any(|relation| relation.kind == TechnicalRelationKind::Supersedes)
    );
    assert!(!provenance.has_more);

    fixture.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn technical_split_dependency_kinds_and_defines_contract_validate_domain_range() {
    let fixture = ActorFixture::new().await;

    for node in [
        technical_component("component:mind", "mind"),
        technical_repository("repo:mind", "/git/github.com/LiGoldragon/mind"),
        technical_repository(
            "repo:signal-mind",
            "/git/github.com/LiGoldragon/signal-mind",
        ),
        technical_crate("crate:mind", "mind", "repo:mind"),
        technical_crate("crate:signal-mind", "signal-mind", "repo:signal-mind"),
        technical_contract(
            "contract:signal-mind:ordinary",
            "signal-mind ordinary contract",
            signal_mind::ContractSurface::Ordinary,
        ),
    ] {
        assert!(matches!(
            fixture
                .submit(MindRequest::SubmitTechnicalNode(node))
                .await
                .reply()
                .expect("node reply exists"),
            MindReply::TechnicalNodeCommitted(_)
        ));
    }

    for relation in [
        technical_relation(
            TechnicalRelationKind::DefinesContract,
            "repo:signal-mind",
            "contract:signal-mind:ordinary",
        ),
        technical_relation(
            TechnicalRelationKind::BuildDependency,
            "component:mind",
            "crate:mind",
        ),
        technical_relation(
            TechnicalRelationKind::RuntimeDependency,
            "component:mind",
            "crate:mind",
        ),
        technical_relation(
            TechnicalRelationKind::WireDependency,
            "component:mind",
            "contract:signal-mind:ordinary",
        ),
    ] {
        assert!(matches!(
            fixture
                .submit(MindRequest::SubmitTechnicalRelation(relation))
                .await
                .reply()
                .expect("relation reply exists"),
            MindReply::TechnicalRelationCommitted(_)
        ));
    }

    let wrong_wire_target = fixture
        .submit(MindRequest::SubmitTechnicalRelation(technical_relation(
            TechnicalRelationKind::WireDependency,
            "component:mind",
            "crate:signal-mind",
        )))
        .await;

    assert!(matches!(
        wrong_wire_target
            .reply()
            .expect("wrong wire target reply exists"),
        MindReply::TechnicalRelationRejected(rejection)
            if matches!(
                rejection.reason,
                TechnicalRelationRejectionReason::DomainRangeViolation(_)
            )
    ));

    fixture.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn technical_supersedes_appends_correction_without_replacing_old_fact() {
    let fixture = ActorFixture::new().await;

    for node in [
        technical_claim(
            "claim:old-storage-shape",
            "mind stores technical facts in prose",
        ),
        technical_claim(
            "claim:new-storage-shape",
            "mind stores technical facts as typed nodes and relations",
        ),
    ] {
        assert!(matches!(
            fixture
                .submit(MindRequest::SubmitTechnicalNode(node))
                .await
                .reply()
                .expect("node reply exists"),
            MindReply::TechnicalNodeCommitted(_)
        ));
    }

    let supersedes = fixture
        .submit(MindRequest::SubmitTechnicalRelation(technical_relation(
            TechnicalRelationKind::Supersedes,
            "claim:new-storage-shape",
            "claim:old-storage-shape",
        )))
        .await;
    assert!(matches!(
        supersedes.reply().expect("supersedes reply exists"),
        MindReply::TechnicalRelationCommitted(_)
    ));

    let claims = fixture
        .submit(MindRequest::QueryTechnicalNodes(QueryTechnicalNodes {
            query: TechnicalNodeQuery::Filter(TechnicalNodeFilter::ByKind(
                signal_mind::ByTechnicalNodeKind {
                    kinds: vec![TechnicalNodeKind::TechnicalClaim],
                },
            )),
            limit: QueryLimit::new(10),
        }))
        .await;
    let corrections = fixture
        .submit(MindRequest::QueryTechnicalRelations(
            QueryTechnicalRelations {
                filter: TechnicalRelationFilter::ByKind(signal_mind::ByTechnicalRelationKind {
                    kinds: vec![TechnicalRelationKind::Supersedes],
                }),
                limit: QueryLimit::new(10),
            },
        ))
        .await;

    let MindReply::TechnicalNodeList(claims) = claims.reply().expect("claim query reply exists")
    else {
        panic!("expected claim list");
    };
    let MindReply::TechnicalRelationList(corrections) =
        corrections.reply().expect("correction query reply exists")
    else {
        panic!("expected correction relation list");
    };

    assert_eq!(claims.nodes.len(), 2);
    assert!(
        claims
            .nodes
            .iter()
            .any(|node| node.stable_key == technical_key("claim:old-storage-shape"))
    );
    assert!(
        claims
            .nodes
            .iter()
            .any(|node| node.stable_key == technical_key("claim:new-storage-shape"))
    );
    assert_eq!(corrections.relations.len(), 1);
    assert_eq!(
        corrections.relations[0].source.stable_key,
        technical_key("claim:new-storage-shape")
    );
    assert_eq!(
        corrections.relations[0].target.stable_key,
        technical_key("claim:old-storage-shape")
    );

    fixture.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn technical_node_subscription_registers_and_returns_initial_snapshot() {
    let fixture = ActorFixture::new().await;
    let component_key = technical_key("component:mind");
    let _component = fixture
        .submit(MindRequest::SubmitTechnicalNode(SubmitTechnicalNode {
            stable_key: component_key.clone(),
            kind: TechnicalNodeKind::Component,
            body: TechnicalNodeBody::Component(signal_mind::ComponentNode {
                component: ComponentName::new("mind"),
                summary: Some(TextBody::new("mind daemon")),
            }),
        }))
        .await;

    let response = fixture
        .submit(MindRequest::SubscribeTechnicalNodes(
            SubscribeTechnicalNodes {
                filter: TechnicalNodeFilter::ByStableKey(ByTechnicalNodeStableKey {
                    stable_key: component_key.clone(),
                }),
                resume_after: None,
                initial_demand: initial_demand(1),
            },
        ))
        .await;

    let MindReply::SubscriptionAccepted(subscription) = response.reply().expect("reply exists")
    else {
        panic!("expected subscription accepted");
    };

    assert_eq!(subscription.subscription.as_str().len(), 3);
    let AcceptedSubscriptionStream::TechnicalNodes(stream) = &subscription.stream else {
        panic!("expected technical node stream");
    };
    assert_eq!(stream.snapshot.len(), 1);
    assert_eq!(stream.cursor, SubscriptionCursor::new(1));
    let node = &stream.snapshot[0];
    assert_eq!(node.stable_key, component_key);
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
async fn technical_node_subscription_delivers_live_delta_through_subscription_actor() {
    let fixture = ActorFixture::new().await;
    let component_key = technical_key("component:mind");
    let subscription_response = fixture
        .submit(MindRequest::SubscribeTechnicalNodes(
            SubscribeTechnicalNodes {
                filter: TechnicalNodeFilter::ByStableKey(ByTechnicalNodeStableKey {
                    stable_key: component_key.clone(),
                }),
                resume_after: None,
                initial_demand: initial_demand(1),
            },
        ))
        .await;

    let MindReply::SubscriptionAccepted(subscription) =
        subscription_response.reply().expect("reply exists")
    else {
        panic!("expected subscription accepted");
    };

    let commit_response = fixture
        .submit(MindRequest::SubmitTechnicalNode(SubmitTechnicalNode {
            stable_key: component_key.clone(),
            kind: TechnicalNodeKind::Component,
            body: TechnicalNodeBody::Component(signal_mind::ComponentNode {
                component: ComponentName::new("mind"),
                summary: None,
            }),
        }))
        .await;
    let events = fixture.subscription_events().await;

    let MindReply::TechnicalNodeCommitted(receipt) = commit_response.reply().expect("reply exists")
    else {
        panic!("expected technical node commit");
    };
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].subscription, subscription.subscription);
    let SubscriptionStreamEvent::TechnicalNodeCommitted(event) = &events[0].event else {
        panic!("expected technical node delta");
    };
    let node = &event.node;
    assert_eq!(event.cursor, SubscriptionCursor::new(1));
    assert_eq!(node.identifier, receipt.node.identifier);
    assert_eq!(node.stable_key, component_key);

    fixture.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn technical_relation_subscription_registers_and_returns_initial_snapshot() {
    let fixture = ActorFixture::new().await;
    let component_key = technical_key("component:mind");
    let repository_key = technical_key("repo:mind");

    let _component = fixture
        .submit(MindRequest::SubmitTechnicalNode(SubmitTechnicalNode {
            stable_key: component_key.clone(),
            kind: TechnicalNodeKind::Component,
            body: TechnicalNodeBody::Component(signal_mind::ComponentNode {
                component: ComponentName::new("mind"),
                summary: None,
            }),
        }))
        .await;
    let _repository = fixture
        .submit(MindRequest::SubmitTechnicalNode(SubmitTechnicalNode {
            stable_key: repository_key.clone(),
            kind: TechnicalNodeKind::Repository,
            body: TechnicalNodeBody::Repository(signal_mind::RepositoryNode {
                path: WirePath::from_absolute_path("/git/github.com/LiGoldragon/mind")
                    .expect("absolute path"),
                remote: None,
            }),
        }))
        .await;
    let _relation = fixture
        .submit(MindRequest::SubmitTechnicalRelation(
            SubmitTechnicalRelation {
                kind: TechnicalRelationKind::OwnsRepository,
                source: component_key.clone(),
                target: repository_key.clone(),
                note: None,
            },
        ))
        .await;

    let response = fixture
        .submit(MindRequest::SubscribeTechnicalRelations(
            SubscribeTechnicalRelations {
                filter: TechnicalRelationFilter::BySource(ByTechnicalRelationSource {
                    source: component_key.clone(),
                }),
                resume_after: None,
                initial_demand: initial_demand(1),
            },
        ))
        .await;

    let MindReply::SubscriptionAccepted(subscription) = response.reply().expect("reply exists")
    else {
        panic!("expected subscription accepted");
    };

    assert_eq!(subscription.subscription.as_str().len(), 3);
    let AcceptedSubscriptionStream::TechnicalRelations(stream) = &subscription.stream else {
        panic!("expected technical relation stream");
    };
    assert_eq!(stream.snapshot.len(), 1);
    assert_eq!(stream.cursor, SubscriptionCursor::new(1));
    let relation = &stream.snapshot[0];
    assert_eq!(relation.source.stable_key, component_key);
    assert_eq!(relation.target.stable_key, repository_key);

    fixture.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn technical_relation_subscription_delivers_live_delta_through_subscription_actor() {
    let fixture = ActorFixture::new().await;
    let component_key = technical_key("component:mind");
    let repository_key = technical_key("repo:mind");

    let _component = fixture
        .submit(MindRequest::SubmitTechnicalNode(SubmitTechnicalNode {
            stable_key: component_key.clone(),
            kind: TechnicalNodeKind::Component,
            body: TechnicalNodeBody::Component(signal_mind::ComponentNode {
                component: ComponentName::new("mind"),
                summary: None,
            }),
        }))
        .await;
    let _repository = fixture
        .submit(MindRequest::SubmitTechnicalNode(SubmitTechnicalNode {
            stable_key: repository_key.clone(),
            kind: TechnicalNodeKind::Repository,
            body: TechnicalNodeBody::Repository(signal_mind::RepositoryNode {
                path: WirePath::from_absolute_path("/git/github.com/LiGoldragon/mind")
                    .expect("absolute path"),
                remote: None,
            }),
        }))
        .await;
    let subscription_response = fixture
        .submit(MindRequest::SubscribeTechnicalRelations(
            SubscribeTechnicalRelations {
                filter: TechnicalRelationFilter::BySource(ByTechnicalRelationSource {
                    source: component_key.clone(),
                }),
                resume_after: None,
                initial_demand: initial_demand(1),
            },
        ))
        .await;

    let MindReply::SubscriptionAccepted(subscription) =
        subscription_response.reply().expect("reply exists")
    else {
        panic!("expected subscription accepted");
    };

    let commit_response = fixture
        .submit(MindRequest::SubmitTechnicalRelation(
            SubmitTechnicalRelation {
                kind: TechnicalRelationKind::OwnsRepository,
                source: component_key.clone(),
                target: repository_key.clone(),
                note: None,
            },
        ))
        .await;
    let events = fixture.subscription_events().await;

    let MindReply::TechnicalRelationCommitted(receipt) =
        commit_response.reply().expect("reply exists")
    else {
        panic!("expected technical relation commit");
    };
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].subscription, subscription.subscription);
    let SubscriptionStreamEvent::TechnicalRelationCommitted(event) = &events[0].event else {
        panic!("expected technical relation delta");
    };
    let relation = &event.relation;
    assert_eq!(event.cursor, SubscriptionCursor::new(1));
    assert_eq!(relation.identifier, receipt.relation.identifier);
    assert_eq!(relation.source.stable_key, component_key);
    assert_eq!(relation.target.stable_key, repository_key);

    fixture.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn public_technical_seed_delivers_subscription_deltas() {
    let fixture = ActorFixture::new().await;
    let dataset = TechnicalSeedDataset::public_first_slice();
    let node_subscription = fixture
        .submit(MindRequest::SubscribeTechnicalNodes(
            SubscribeTechnicalNodes {
                filter: TechnicalNodeFilter::ByKind(signal_mind::ByTechnicalNodeKind {
                    kinds: Vec::new(),
                }),
                resume_after: None,
                initial_demand: initial_demand(100),
            },
        ))
        .await;
    let relation_subscription = fixture
        .submit(MindRequest::SubscribeTechnicalRelations(
            SubscribeTechnicalRelations {
                filter: TechnicalRelationFilter::ByKind(signal_mind::ByTechnicalRelationKind {
                    kinds: Vec::new(),
                }),
                resume_after: None,
                initial_demand: initial_demand(100),
            },
        ))
        .await;

    let MindReply::SubscriptionAccepted(node_subscription) = node_subscription
        .reply()
        .expect("node subscription reply exists")
    else {
        panic!("expected technical node subscription");
    };
    let MindReply::SubscriptionAccepted(relation_subscription) = relation_subscription
        .reply()
        .expect("relation subscription reply exists")
    else {
        panic!("expected technical relation subscription");
    };

    for node in dataset.nodes().iter().cloned() {
        let response = fixture.submit(MindRequest::SubmitTechnicalNode(node)).await;
        assert!(matches!(
            response.reply().expect("node reply exists"),
            MindReply::TechnicalNodeCommitted(_)
        ));
    }
    for relation in dataset.relations().iter().cloned() {
        let response = fixture
            .submit(MindRequest::SubmitTechnicalRelation(relation))
            .await;
        assert!(matches!(
            response.reply().expect("relation reply exists"),
            MindReply::TechnicalRelationCommitted(_)
        ));
    }

    let events = fixture.subscription_events().await;
    let node_delta_count = events
        .iter()
        .filter(|event| {
            event.subscription == node_subscription.subscription
                && matches!(
                    event.event,
                    SubscriptionStreamEvent::TechnicalNodeCommitted(_)
                )
        })
        .count();
    let relation_delta_count = events
        .iter()
        .filter(|event| {
            event.subscription == relation_subscription.subscription
                && matches!(
                    event.event,
                    SubscriptionStreamEvent::TechnicalRelationCommitted(_)
                )
        })
        .count();

    assert_eq!(node_delta_count, dataset.nodes().len());
    assert_eq!(relation_delta_count, dataset.relations().len());

    fixture.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn public_technical_seed_queries_back_exact_facts_through_actor_lane() {
    let fixture = ActorFixture::new().await;
    let dataset = TechnicalSeedDataset::public_first_slice();

    for node in dataset.nodes().iter().cloned() {
        let response = fixture.submit(MindRequest::SubmitTechnicalNode(node)).await;
        assert!(matches!(
            response.reply().expect("node reply exists"),
            MindReply::TechnicalNodeCommitted(_)
        ));
    }
    for relation in dataset.relations().iter().cloned() {
        let response = fixture
            .submit(MindRequest::SubmitTechnicalRelation(relation))
            .await;
        assert!(matches!(
            response.reply().expect("relation reply exists"),
            MindReply::TechnicalRelationCommitted(_)
        ));
    }

    let nodes = fixture
        .submit(MindRequest::QueryTechnicalNodes(QueryTechnicalNodes {
            query: TechnicalNodeQuery::Filter(TechnicalNodeFilter::ByKind(
                signal_mind::ByTechnicalNodeKind { kinds: Vec::new() },
            )),
            limit: QueryLimit::new(100),
        }))
        .await;
    let relations = fixture
        .submit(MindRequest::QueryTechnicalRelations(
            QueryTechnicalRelations {
                filter: TechnicalRelationFilter::BySource(ByTechnicalRelationSource {
                    source: dataset.mind_component_key(),
                }),
                limit: QueryLimit::new(100),
            },
        ))
        .await;

    let MindReply::TechnicalNodeList(nodes) = nodes.reply().expect("node list reply exists") else {
        panic!("expected technical node list");
    };
    let MindReply::TechnicalRelationList(relations) =
        relations.reply().expect("relation list reply exists")
    else {
        panic!("expected technical relation list");
    };
    let actual_node_keys = nodes
        .nodes
        .iter()
        .map(|node| node.stable_key.clone())
        .collect::<HashSet<_>>();
    let expected_node_keys = dataset.all_node_keys().into_iter().collect::<HashSet<_>>();

    assert_eq!(actual_node_keys, expected_node_keys);
    assert!(relations.relations.iter().any(|relation| {
        relation.kind == TechnicalRelationKind::WireDependency
            && relation.source.stable_key == dataset.mind_component_key()
            && relation.target.stable_key == dataset.signal_mind_contract_key()
    }));
    assert!(relations.relations.iter().any(|relation| {
        relation.kind == TechnicalRelationKind::StorageDependency
            && relation.source.stable_key == dataset.mind_component_key()
            && relation.target.stable_key == dataset.durable_storage_claim_key()
    }));

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
            resume_after: None,
            initial_demand: initial_demand(1),
        }))
        .await;

    let MindReply::SubscriptionAccepted(subscription) = response.reply().expect("reply exists")
    else {
        panic!("expected subscription accepted");
    };

    assert_eq!(subscription.subscription.as_str().len(), 3);
    let AcceptedSubscriptionStream::Thoughts(stream) = &subscription.stream else {
        panic!("expected thought stream");
    };
    assert_eq!(stream.snapshot.len(), 1);
    assert_eq!(stream.cursor, SubscriptionCursor::new(1));
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
            resume_after: None,
            initial_demand: initial_demand(1),
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
    let SubscriptionStreamEvent::ThoughtCommitted(event) = &events[0].event else {
        panic!("expected thought delta");
    };
    let thought = &event.thought;
    assert_eq!(event.cursor, SubscriptionCursor::new(1));
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
            resume_after: None,
            initial_demand: initial_demand(1),
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
async fn subscription_resume_after_replays_ordered_available_history() {
    let fixture = ActorFixture::new().await;
    let _first = fixture
        .submit(MindRequest::SubmitThought(SubmitThought {
            kind: ThoughtKind::Goal,
            body: ThoughtBody::Goal(GoalBody {
                description: TextBody::new("first resumable goal"),
                scope: GoalScope::Workspace(WorkspaceGoal {
                    workspace: TextBody::new("primary"),
                }),
            }),
        }))
        .await;
    let second = fixture
        .submit(MindRequest::SubmitThought(SubmitThought {
            kind: ThoughtKind::Goal,
            body: ThoughtBody::Goal(GoalBody {
                description: TextBody::new("second resumable goal"),
                scope: GoalScope::Workspace(WorkspaceGoal {
                    workspace: TextBody::new("primary"),
                }),
            }),
        }))
        .await;
    let MindReply::ThoughtCommitted(second) = second.reply().expect("second reply exists") else {
        panic!("expected second thought commit");
    };

    let response = fixture
        .submit(MindRequest::SubscribeThoughts(SubscribeThoughts {
            filter: ThoughtFilter::ByKind(ByThoughtKind {
                kinds: vec![ThoughtKind::Goal],
            }),
            resume_after: Some(SubscriptionCursor::new(1)),
            initial_demand: initial_demand(1),
        }))
        .await;

    let MindReply::SubscriptionAccepted(subscription) = response.reply().expect("reply exists")
    else {
        panic!("expected subscription accepted");
    };
    let AcceptedSubscriptionStream::Thoughts(stream) = &subscription.stream else {
        panic!("expected thought stream");
    };
    assert_eq!(stream.cursor, SubscriptionCursor::new(2));
    assert_eq!(stream.snapshot.len(), 1);
    assert_eq!(stream.snapshot[0].id, second.record);

    fixture.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn subscription_demand_releases_buffered_delta_without_overrun() {
    let fixture = ActorFixture::new().await;
    let subscription_response = fixture
        .submit(MindRequest::SubscribeThoughts(SubscribeThoughts {
            filter: ThoughtFilter::ByKind(ByThoughtKind {
                kinds: vec![ThoughtKind::Goal],
            }),
            resume_after: None,
            initial_demand: initial_demand(0),
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
                description: TextBody::new("buffered demand goal"),
                scope: GoalScope::Workspace(WorkspaceGoal {
                    workspace: TextBody::new("primary"),
                }),
            }),
        }))
        .await;
    assert!(fixture.subscription_events().await.is_empty());

    let demand_response = fixture
        .submit(MindRequest::SubscriptionDemand(SubscriptionDemand {
            subscription: subscription.subscription.clone(),
            credit: initial_demand(1),
        }))
        .await;
    assert!(matches!(
        demand_response.reply().expect("demand reply exists"),
        MindReply::SubscriptionDemandAccepted(accepted)
            if accepted.subscription == subscription.subscription
                && accepted.accepted == initial_demand(1)
    ));

    let MindReply::ThoughtCommitted(receipt) = commit_response.reply().expect("reply exists")
    else {
        panic!("expected thought commit");
    };
    let events = fixture.subscription_events().await;
    assert_eq!(events.len(), 1);
    let SubscriptionStreamEvent::ThoughtCommitted(event) = &events[0].event else {
        panic!("expected thought event");
    };
    assert_eq!(event.cursor, SubscriptionCursor::new(1));
    assert_eq!(event.thought.id, receipt.record);

    fixture.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn subscription_retraction_cleans_runtime_stream() {
    let fixture = ActorFixture::new().await;
    let subscription_response = fixture
        .submit(MindRequest::SubscribeThoughts(SubscribeThoughts {
            filter: ThoughtFilter::ByKind(ByThoughtKind {
                kinds: vec![ThoughtKind::Goal],
            }),
            resume_after: None,
            initial_demand: initial_demand(1),
        }))
        .await;
    let MindReply::SubscriptionAccepted(subscription) =
        subscription_response.reply().expect("reply exists")
    else {
        panic!("expected subscription accepted");
    };

    let retracted = fixture
        .submit(MindRequest::SubscriptionRetraction(
            subscription.subscription.clone(),
        ))
        .await;
    assert!(matches!(
        retracted.reply().expect("retracted reply exists"),
        MindReply::SubscriptionRetracted(ack)
            if ack.subscription == subscription.subscription
                && ack.stream == SubscriptionStreamKind::Thoughts
                && ack.last_cursor == SubscriptionCursor::initial()
    ));

    let _commit = fixture
        .submit(MindRequest::SubmitThought(SubmitThought {
            kind: ThoughtKind::Goal,
            body: ThoughtBody::Goal(GoalBody {
                description: TextBody::new("retracted stream should not receive this"),
                scope: GoalScope::Workspace(WorkspaceGoal {
                    workspace: TextBody::new("primary"),
                }),
            }),
        }))
        .await;
    assert!(fixture.subscription_events().await.is_empty());

    fixture.stop().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn persisted_subscription_rehydrates_after_restart() {
    let fixture = ActorFixture::new().await;
    let store = fixture.store.clone();
    let component_key = technical_key("component:restart-rehydration");
    let subscription_response = fixture
        .submit(MindRequest::SubscribeTechnicalNodes(
            SubscribeTechnicalNodes {
                filter: TechnicalNodeFilter::ByStableKey(ByTechnicalNodeStableKey {
                    stable_key: component_key.clone(),
                }),
                resume_after: None,
                initial_demand: initial_demand(0),
            },
        ))
        .await;
    let MindReply::SubscriptionAccepted(subscription) = subscription_response
        .reply()
        .expect("subscription reply exists")
    else {
        panic!("expected subscription accepted");
    };
    let subscription_identifier = subscription.subscription.clone();
    fixture.stop_without_removing_store().await;

    let restarted = ActorFixture::from_store(store).await;
    tokio::task::yield_now().await;
    let demand_response = restarted
        .submit(MindRequest::SubscriptionDemand(SubscriptionDemand {
            subscription: subscription_identifier.clone(),
            credit: initial_demand(1),
        }))
        .await;
    match demand_response.reply().expect("demand reply exists") {
        MindReply::SubscriptionDemandAccepted(accepted) => {
            assert_eq!(accepted.subscription, subscription_identifier);
        }
        other => panic!("expected demand accepted after rehydrate, got {other:?}"),
    }
    let commit_response = restarted
        .submit(MindRequest::SubmitTechnicalNode(SubmitTechnicalNode {
            stable_key: component_key.clone(),
            kind: TechnicalNodeKind::Component,
            body: TechnicalNodeBody::Component(signal_mind::ComponentNode {
                component: ComponentName::new("restart-rehydration"),
                summary: None,
            }),
        }))
        .await;
    assert!(matches!(
        commit_response.reply().expect("commit reply exists"),
        MindReply::TechnicalNodeCommitted(_)
    ));

    let events = restarted.subscription_events().await;
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].subscription, subscription_identifier);
    assert!(matches!(
        &events[0].event,
        SubscriptionStreamEvent::TechnicalNodeCommitted(_)
    ));

    restarted.stop().await;
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
                        WirePath::from_absolute_path("/git/github.com/LiGoldragon/mind")
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
            resume_after: None,
            initial_demand: initial_demand(1),
        }))
        .await;

    let MindReply::SubscriptionAccepted(subscription) = response.reply().expect("reply exists")
    else {
        panic!("expected subscription accepted");
    };

    assert_eq!(subscription.subscription.as_str().len(), 3);
    let AcceptedSubscriptionStream::Relations(stream) = &subscription.stream else {
        panic!("expected relation stream");
    };
    assert_eq!(stream.snapshot.len(), 1);
    assert_eq!(stream.cursor, SubscriptionCursor::new(1));
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
                        WirePath::from_absolute_path("/git/github.com/LiGoldragon/mind")
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
            resume_after: None,
            initial_demand: initial_demand(1),
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
    let SubscriptionStreamEvent::RelationCommitted(event) = &events[0].event else {
        panic!("expected relation delta");
    };
    let relation = &event.relation;
    assert_eq!(event.cursor, SubscriptionCursor::new(1));
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
            limit: QueryLimit::new(10),
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
                        WirePath::from_absolute_path("/git/github.com/LiGoldragon/mind")
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
            limit: QueryLimit::new(10),
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
            kind: signal_mind::RelationKind::Supports,
            source: signal_mind::RecordIdentifier::new("missing-source"),
            target: signal_mind::RecordIdentifier::new("missing-target"),
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
                        WirePath::from_absolute_path("/git/github.com/LiGoldragon/mind")
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
            limit: QueryLimit::new(10),
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
                        "/git/github.com/LiGoldragon/mind/ARCHITECTURE.md",
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
            limit: QueryLimit::new(10),
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
