use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use kameo::actor::ActorRef;
use redb::{Database, TableDefinition};
use sema_engine::{
    Assertion, Engine, EngineOpen, EngineRecord, FamilyName, Mutation, QueryPlan, RecordKey,
    Retraction, SchemaHash, SchemaVersion, SinkError, SubscriptionDeliveryMode,
    SubscriptionEvent as EngineSubscriptionEvent, SubscriptionSink, TableDescriptor, TableName,
    TableReference, VersionedStoreName, VersioningPolicy,
};
use signal_mind::{
    AcceptedKnowledge, ActorName, KnowledgeIdentity, RecordIdentifier, Relation,
    RelationIdentifier, SubmitRelation, SubmitTechnicalNode, SubmitTechnicalRelation,
    SubmitThought, SubscribeRelations, SubscribeTechnicalNodes, SubscribeTechnicalRelations,
    SubscribeThoughts, SubscriptionCursor, SubscriptionDemandCredit, SubscriptionIdentifier,
    TechnicalNode, TechnicalNodeIdentifier, TechnicalNodeKey, TechnicalNodeKindMismatch,
    TechnicalNodeRejectionReason, TechnicalRelation, TechnicalRelationEndpoint,
    TechnicalRelationIdentifier, TechnicalRelationRejectionReason, Thought, TimestampNanos,
};

use crate::actors::subscription::{
    PublishRelationDelta, PublishTechnicalNodeDelta, PublishTechnicalRelationDelta,
    PublishThoughtDelta, RegisterRelationSubscription, RegisterTechnicalNodeSubscription,
    RegisterTechnicalRelationSubscription, RegisterThoughtSubscription, SubscriptionSupervisor,
};
use crate::{MemoryGraph, Result, StoreLocation};

const MIND_SCHEMA_VERSION: SchemaVersion = SchemaVersion::new(11);
const MIND_SCHEMA_VERSION_V10: SchemaVersion = SchemaVersion::new(10);
const MIND_SCHEMA_VERSION_V9: SchemaVersion = SchemaVersion::new(9);
const MIND_SCHEMA_VERSION_V8: SchemaVersion = SchemaVersion::new(8);

const MEMORY_GRAPH: TableName = TableName::new("memory_graph");
const THOUGHT_SUBSCRIPTIONS: TableName = TableName::new("thought_subscriptions");
const RELATION_SUBSCRIPTIONS: TableName = TableName::new("relation_subscriptions");
const TECHNICAL_NODE_SUBSCRIPTIONS: TableName = TableName::new("technical_node_subscriptions");
const TECHNICAL_RELATION_SUBSCRIPTIONS: TableName =
    TableName::new("technical_relation_subscriptions");
const MEMORY_GRAPH_KEY: &str = "current";
const THOUGHTS: TableName = TableName::new("thoughts");
const RELATIONS: TableName = TableName::new("relations");
const TECHNICAL_NODES: TableName = TableName::new("technical_nodes");
const TECHNICAL_RELATIONS: TableName = TableName::new("technical_relations");
const ACCEPTED_KNOWLEDGE: TableName = TableName::new("accepted_knowledge");
const SEMA_META: TableDefinition<&str, u64> = TableDefinition::new("__sema_meta");
const SEMA_SCHEMA_VERSION_KEY: &str = "schema_version";

pub struct MindTables {
    engine: Engine,
    memory: TableReference<MemoryGraph>,
    thoughts: TableReference<StoredThought>,
    relations: TableReference<StoredRelation>,
    technical_nodes: TableReference<StoredTechnicalNode>,
    technical_relations: TableReference<StoredTechnicalRelation>,
    accepted_knowledge: TableReference<StoredAcceptedKnowledge>,
    thought_subscriptions: TableReference<StoredThoughtSubscription>,
    relation_subscriptions: TableReference<StoredRelationSubscription>,
    technical_node_subscriptions: TableReference<StoredTechnicalNodeSubscription>,
    technical_relation_subscriptions: TableReference<StoredTechnicalRelationSubscription>,
    subscription_publisher: GraphSubscriptionPublisher,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub(crate) struct StoredThoughtSubscription {
    pub subscription: SubscriptionIdentifier,
    pub filter: signal_mind::ThoughtFilter,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub(crate) struct StoredRelationSubscription {
    pub subscription: SubscriptionIdentifier,
    pub filter: signal_mind::RelationFilter,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub(crate) struct StoredTechnicalNodeSubscription {
    pub subscription: SubscriptionIdentifier,
    pub filter: signal_mind::TechnicalNodeFilter,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub(crate) struct StoredTechnicalRelationSubscription {
    pub subscription: SubscriptionIdentifier,
    pub filter: signal_mind::TechnicalRelationFilter,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub(crate) struct StoredThought {
    record: Thought,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub(crate) struct StoredRelation {
    record: Relation,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub(crate) struct StoredTechnicalNode {
    record: TechnicalNode,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub(crate) struct StoredTechnicalRelation {
    record: TechnicalRelation,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub(crate) struct StoredAcceptedKnowledge {
    record: AcceptedKnowledge,
}

pub(crate) struct OpenedThoughtSubscription {
    record: StoredThoughtSubscription,
    initial: Vec<Thought>,
    resume_after: SubscriptionCursor,
    initial_demand: SubscriptionDemandCredit,
}

pub(crate) struct OpenedRelationSubscription {
    record: StoredRelationSubscription,
    initial: Vec<Relation>,
    resume_after: SubscriptionCursor,
    initial_demand: SubscriptionDemandCredit,
}

pub(crate) struct OpenedTechnicalNodeSubscription {
    record: StoredTechnicalNodeSubscription,
    initial: Vec<TechnicalNode>,
    resume_after: SubscriptionCursor,
    initial_demand: SubscriptionDemandCredit,
}

pub(crate) struct OpenedTechnicalRelationSubscription {
    record: StoredTechnicalRelationSubscription,
    initial: Vec<TechnicalRelation>,
    resume_after: SubscriptionCursor,
    initial_demand: SubscriptionDemandCredit,
}

#[derive(Clone)]
pub(crate) enum GraphSubscriptionPublisher {
    Actor(ActorRef<SubscriptionSupervisor>),
    #[cfg(test)]
    Disabled,
}

pub(crate) enum RuntimeSubscriptionRegistration {
    Thoughts {
        subscription: SubscriptionIdentifier,
        filter: signal_mind::ThoughtFilter,
    },
    Relations {
        subscription: SubscriptionIdentifier,
        filter: signal_mind::RelationFilter,
    },
    TechnicalNodes {
        subscription: SubscriptionIdentifier,
        filter: signal_mind::TechnicalNodeFilter,
    },
    TechnicalRelations {
        subscription: SubscriptionIdentifier,
        filter: signal_mind::TechnicalRelationFilter,
    },
}

impl RuntimeSubscriptionRegistration {
    pub(crate) async fn register(self, actor: &ActorRef<SubscriptionSupervisor>) {
        let cursor = SubscriptionCursor::initial();
        let initial_demand = SubscriptionDemandCredit::new(0);
        match self {
            Self::Thoughts {
                subscription,
                filter,
            } => {
                let _registered = actor
                    .ask(RegisterThoughtSubscription::new(
                        subscription,
                        filter,
                        cursor,
                        initial_demand,
                    ))
                    .await;
            }
            Self::Relations {
                subscription,
                filter,
            } => {
                let _registered = actor
                    .ask(RegisterRelationSubscription::new(
                        subscription,
                        filter,
                        cursor,
                        initial_demand,
                    ))
                    .await;
            }
            Self::TechnicalNodes {
                subscription,
                filter,
            } => {
                let _registered = actor
                    .ask(RegisterTechnicalNodeSubscription::new(
                        subscription,
                        filter,
                        cursor,
                        initial_demand,
                    ))
                    .await;
            }
            Self::TechnicalRelations {
                subscription,
                filter,
            } => {
                let _registered = actor
                    .ask(RegisterTechnicalRelationSubscription::new(
                        subscription,
                        filter,
                        cursor,
                        initial_demand,
                    ))
                    .await;
            }
        }
    }
}

impl StoredThought {
    fn new(record: Thought) -> Self {
        Self { record }
    }

    fn into_record(self) -> Thought {
        self.record
    }
}

impl StoredRelation {
    fn new(record: Relation) -> Self {
        Self { record }
    }

    fn into_record(self) -> Relation {
        self.record
    }
}

impl StoredTechnicalNode {
    fn new(record: TechnicalNode) -> Self {
        Self { record }
    }

    fn into_record(self) -> TechnicalNode {
        self.record
    }
}

impl StoredTechnicalRelation {
    fn new(record: TechnicalRelation) -> Self {
        Self { record }
    }

    fn into_record(self) -> TechnicalRelation {
        self.record
    }
}

impl StoredAcceptedKnowledge {
    fn new(record: AcceptedKnowledge) -> Self {
        Self { record }
    }

    fn identifier(&self) -> &KnowledgeIdentity {
        &self.record.identity
    }

    fn into_record(self) -> AcceptedKnowledge {
        self.record
    }
}

impl OpenedThoughtSubscription {
    fn new(
        record: StoredThoughtSubscription,
        initial: Vec<Thought>,
        resume_after: SubscriptionCursor,
        initial_demand: SubscriptionDemandCredit,
    ) -> Self {
        Self {
            record,
            initial,
            resume_after,
            initial_demand,
        }
    }

    pub(crate) fn record(&self) -> &StoredThoughtSubscription {
        &self.record
    }

    pub(crate) fn initial(&self) -> &[Thought] {
        &self.initial
    }

    pub(crate) fn resume_after(&self) -> SubscriptionCursor {
        self.resume_after
    }

    pub(crate) fn initial_demand(&self) -> SubscriptionDemandCredit {
        self.initial_demand
    }
}

impl OpenedRelationSubscription {
    fn new(
        record: StoredRelationSubscription,
        initial: Vec<Relation>,
        resume_after: SubscriptionCursor,
        initial_demand: SubscriptionDemandCredit,
    ) -> Self {
        Self {
            record,
            initial,
            resume_after,
            initial_demand,
        }
    }

    pub(crate) fn record(&self) -> &StoredRelationSubscription {
        &self.record
    }

    pub(crate) fn initial(&self) -> &[Relation] {
        &self.initial
    }

    pub(crate) fn resume_after(&self) -> SubscriptionCursor {
        self.resume_after
    }

    pub(crate) fn initial_demand(&self) -> SubscriptionDemandCredit {
        self.initial_demand
    }
}

impl OpenedTechnicalNodeSubscription {
    fn new(
        record: StoredTechnicalNodeSubscription,
        initial: Vec<TechnicalNode>,
        resume_after: SubscriptionCursor,
        initial_demand: SubscriptionDemandCredit,
    ) -> Self {
        Self {
            record,
            initial,
            resume_after,
            initial_demand,
        }
    }

    pub(crate) fn record(&self) -> &StoredTechnicalNodeSubscription {
        &self.record
    }

    pub(crate) fn initial(&self) -> &[TechnicalNode] {
        &self.initial
    }

    pub(crate) fn resume_after(&self) -> SubscriptionCursor {
        self.resume_after
    }

    pub(crate) fn initial_demand(&self) -> SubscriptionDemandCredit {
        self.initial_demand
    }
}

impl OpenedTechnicalRelationSubscription {
    fn new(
        record: StoredTechnicalRelationSubscription,
        initial: Vec<TechnicalRelation>,
        resume_after: SubscriptionCursor,
        initial_demand: SubscriptionDemandCredit,
    ) -> Self {
        Self {
            record,
            initial,
            resume_after,
            initial_demand,
        }
    }

    pub(crate) fn record(&self) -> &StoredTechnicalRelationSubscription {
        &self.record
    }

    pub(crate) fn initial(&self) -> &[TechnicalRelation] {
        &self.initial
    }

    pub(crate) fn resume_after(&self) -> SubscriptionCursor {
        self.resume_after
    }

    pub(crate) fn initial_demand(&self) -> SubscriptionDemandCredit {
        self.initial_demand
    }
}

impl EngineRecord for StoredThought {
    fn record_key(&self) -> RecordKey {
        RecordKey::new(self.record.id.as_str())
    }
}

impl EngineRecord for StoredRelation {
    fn record_key(&self) -> RecordKey {
        RecordKey::new(self.record.id.as_str())
    }
}

impl EngineRecord for StoredTechnicalNode {
    fn record_key(&self) -> RecordKey {
        RecordKey::new(self.record.identifier.as_str())
    }
}

impl EngineRecord for StoredTechnicalRelation {
    fn record_key(&self) -> RecordKey {
        RecordKey::new(self.record.identifier.as_str())
    }
}

impl EngineRecord for StoredAcceptedKnowledge {
    fn record_key(&self) -> RecordKey {
        RecordKey::new(self.identifier().as_str())
    }
}

impl EngineRecord for MemoryGraph {
    fn record_key(&self) -> RecordKey {
        RecordKey::new(MEMORY_GRAPH_KEY)
    }
}

impl EngineRecord for StoredThoughtSubscription {
    fn record_key(&self) -> RecordKey {
        RecordKey::new(self.subscription.as_str())
    }
}

impl EngineRecord for StoredRelationSubscription {
    fn record_key(&self) -> RecordKey {
        RecordKey::new(self.subscription.as_str())
    }
}

impl EngineRecord for StoredTechnicalNodeSubscription {
    fn record_key(&self) -> RecordKey {
        RecordKey::new(self.subscription.as_str())
    }
}

impl EngineRecord for StoredTechnicalRelationSubscription {
    fn record_key(&self) -> RecordKey {
        RecordKey::new(self.subscription.as_str())
    }
}

impl MindTables {
    pub(crate) fn open(
        store: &StoreLocation,
        subscription_publisher: GraphSubscriptionPublisher,
    ) -> Result<Self> {
        let mut engine = match Engine::open(Self::engine_open(store)) {
            Ok(engine) => engine,
            Err(error) if Self::is_older_store_opened_as_current(&error) => {
                Self::migrate_older_store_to_current(store, Self::older_store_version(&error))?;
                Engine::open(Self::engine_open(store))?
            }
            Err(error) => return Err(error.into()),
        };
        let memory = engine.register_table(Self::family_descriptor(
            MEMORY_GRAPH,
            "memory-graph",
            MIND_SCHEMA_VERSION_V8,
        ))?;
        let thoughts = engine.register_table(Self::family_descriptor(
            THOUGHTS,
            "thought",
            MIND_SCHEMA_VERSION_V8,
        ))?;
        let relations = engine.register_table(Self::family_descriptor(
            RELATIONS,
            "relation",
            MIND_SCHEMA_VERSION_V8,
        ))?;
        let technical_nodes = engine.register_table(Self::family_descriptor(
            TECHNICAL_NODES,
            "technical-node",
            MIND_SCHEMA_VERSION,
        ))?;
        let technical_relations = engine.register_table(Self::family_descriptor(
            TECHNICAL_RELATIONS,
            "technical-relation",
            MIND_SCHEMA_VERSION,
        ))?;
        let accepted_knowledge = engine.register_table(Self::family_descriptor(
            ACCEPTED_KNOWLEDGE,
            "accepted-knowledge",
            MIND_SCHEMA_VERSION,
        ))?;
        let thought_subscriptions = engine.register_table(Self::family_descriptor(
            THOUGHT_SUBSCRIPTIONS,
            "thought-subscription",
            MIND_SCHEMA_VERSION_V8,
        ))?;
        let relation_subscriptions = engine.register_table(Self::family_descriptor(
            RELATION_SUBSCRIPTIONS,
            "relation-subscription",
            MIND_SCHEMA_VERSION_V8,
        ))?;
        let technical_node_subscriptions = engine.register_table(Self::family_descriptor(
            TECHNICAL_NODE_SUBSCRIPTIONS,
            "technical-node-subscription",
            MIND_SCHEMA_VERSION,
        ))?;
        let technical_relation_subscriptions = engine.register_table(Self::family_descriptor(
            TECHNICAL_RELATION_SUBSCRIPTIONS,
            "technical-relation-subscription",
            MIND_SCHEMA_VERSION,
        ))?;
        let tables = Self {
            engine,
            memory,
            thoughts,
            relations,
            technical_nodes,
            technical_relations,
            accepted_knowledge,
            thought_subscriptions,
            relation_subscriptions,
            technical_node_subscriptions,
            technical_relation_subscriptions,
            subscription_publisher,
        };
        tables.rehydrate_engine_subscriptions()?;
        Ok(tables)
    }

    fn engine_open(store: &StoreLocation) -> EngineOpen {
        Self::engine_open_with_version(store, MIND_SCHEMA_VERSION)
    }

    fn engine_open_with_version(store: &StoreLocation, version: SchemaVersion) -> EngineOpen {
        EngineOpen::new(store.as_path(), version).with_versioning(Self::versioning_policy())
    }

    fn versioning_policy() -> VersioningPolicy {
        VersioningPolicy::new(VersionedStoreName::new("mind"))
    }

    /// Family identity per registered table. Existing family hashes
    /// stay at their introduction version so schema bumps can add
    /// families without rewriting older catalog rows.
    fn family_descriptor<RecordValue>(
        table: TableName,
        family: &str,
        version: SchemaVersion,
    ) -> TableDescriptor<RecordValue> {
        TableDescriptor::new(
            table,
            FamilyName::new(family),
            SchemaHash::for_label(format!("mind-{family}-v{}", version.value())),
        )
    }

    fn is_older_store_opened_as_current(error: &sema_engine::Error) -> bool {
        matches!(
            error,
            sema_engine::Error::Sema(sema_engine::StorageKernelError::SchemaVersionMismatch {
                expected,
                found,
            }) if *expected == MIND_SCHEMA_VERSION
                && (*found == MIND_SCHEMA_VERSION_V8
                    || *found == MIND_SCHEMA_VERSION_V9
                    || *found == MIND_SCHEMA_VERSION_V10)
        )
    }

    fn older_store_version(error: &sema_engine::Error) -> SchemaVersion {
        match error {
            sema_engine::Error::Sema(sema_engine::StorageKernelError::SchemaVersionMismatch {
                found,
                ..
            }) => *found,
            _ => MIND_SCHEMA_VERSION,
        }
    }

    fn migrate_older_store_to_current(store: &StoreLocation, version: SchemaVersion) -> Result<()> {
        let mut engine = Engine::open(Self::engine_open_with_version(store, version))?;
        engine.register_table(Self::family_descriptor::<MemoryGraph>(
            MEMORY_GRAPH,
            "memory-graph",
            MIND_SCHEMA_VERSION_V8,
        ))?;
        engine.register_table(Self::family_descriptor::<StoredThought>(
            THOUGHTS,
            "thought",
            MIND_SCHEMA_VERSION_V8,
        ))?;
        engine.register_table(Self::family_descriptor::<StoredRelation>(
            RELATIONS,
            "relation",
            MIND_SCHEMA_VERSION_V8,
        ))?;
        engine.register_table(Self::family_descriptor::<StoredThoughtSubscription>(
            THOUGHT_SUBSCRIPTIONS,
            "thought-subscription",
            MIND_SCHEMA_VERSION_V8,
        ))?;
        engine.register_table(Self::family_descriptor::<StoredRelationSubscription>(
            RELATION_SUBSCRIPTIONS,
            "relation-subscription",
            MIND_SCHEMA_VERSION_V8,
        ))?;
        if version == MIND_SCHEMA_VERSION_V9 || version == MIND_SCHEMA_VERSION_V10 {
            let technical_version = if version == MIND_SCHEMA_VERSION_V10 {
                MIND_SCHEMA_VERSION_V10
            } else {
                MIND_SCHEMA_VERSION_V9
            };
            engine.register_table(Self::family_descriptor::<StoredTechnicalNode>(
                TECHNICAL_NODES,
                "technical-node",
                technical_version,
            ))?;
            engine.register_table(Self::family_descriptor::<StoredTechnicalRelation>(
                TECHNICAL_RELATIONS,
                "technical-relation",
                technical_version,
            ))?;
            engine.register_table(Self::family_descriptor::<StoredTechnicalNodeSubscription>(
                TECHNICAL_NODE_SUBSCRIPTIONS,
                "technical-node-subscription",
                technical_version,
            ))?;
            engine.register_table(
                Self::family_descriptor::<StoredTechnicalRelationSubscription>(
                    TECHNICAL_RELATION_SUBSCRIPTIONS,
                    "technical-relation-subscription",
                    technical_version,
                ),
            )?;
        }
        drop(engine);

        let database = Database::create(store.as_path())?;
        let transaction = database.begin_write()?;
        {
            let mut meta = transaction.open_table(SEMA_META)?;
            meta.insert(SEMA_SCHEMA_VERSION_KEY, MIND_SCHEMA_VERSION.value() as u64)?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn memory_graph(&self) -> Result<Option<MemoryGraph>> {
        Ok(self
            .engine
            .match_records(QueryPlan::key(
                self.memory,
                RecordKey::new(MEMORY_GRAPH_KEY),
            ))?
            .records()
            .first()
            .cloned())
    }

    pub(crate) fn replace_memory_graph(&self, graph: &MemoryGraph) -> Result<()> {
        if self.memory_graph()?.is_some() {
            self.engine
                .mutate(Mutation::new(self.memory, graph.clone()))?;
        } else {
            self.engine
                .assert(Assertion::new(self.memory, graph.clone()))?;
        }
        Ok(())
    }

    pub(crate) fn append_thought(
        &self,
        actor: ActorName,
        submission: SubmitThought,
    ) -> Result<Thought> {
        let actual = submission.body.kind();
        if submission.kind != actual {
            return Err(crate::Error::MindGraphThoughtKindMismatch {
                declared: submission.kind,
                actual,
            });
        }

        let id = GraphIdMint::new(&self.engine).next_record_id()?;
        let thought = Thought {
            id,
            kind: submission.kind,
            body: submission.body,
            author: actor,
            occurred_at: StoreClock::system().timestamp()?,
        };

        self.engine.assert(Assertion::new(
            self.thoughts,
            StoredThought::new(thought.clone()),
        ))?;
        Ok(thought)
    }

    pub(crate) fn append_relation(
        &self,
        actor: ActorName,
        submission: SubmitRelation,
    ) -> Result<Relation> {
        let source = self.read_thought(&submission.source)?;
        let target = self.read_thought(&submission.target)?;
        submission
            .kind
            .validate_endpoints(&source, &target)
            .map_err(|mismatch| crate::Error::MindGraphRelationKindMismatch { mismatch })?;

        let id = GraphIdMint::new(&self.engine).next_relation_id()?;
        let relation = Relation {
            id,
            kind: submission.kind,
            source: submission.source,
            target: submission.target,
            author: actor,
            occurred_at: StoreClock::system().timestamp()?,
            note: submission.note,
        };

        self.engine.assert(Assertion::new(
            self.relations,
            StoredRelation::new(relation.clone()),
        ))?;
        Ok(relation)
    }

    pub(crate) fn thought_records(&self) -> Result<Vec<Thought>> {
        Ok(self
            .engine
            .match_records(QueryPlan::all(self.thoughts))?
            .records()
            .iter()
            .cloned()
            .map(StoredThought::into_record)
            .collect())
    }

    pub(crate) fn relation_records(&self) -> Result<Vec<Relation>> {
        Ok(self
            .engine
            .match_records(QueryPlan::all(self.relations))?
            .records()
            .iter()
            .cloned()
            .map(StoredRelation::into_record)
            .collect())
    }

    pub(crate) fn append_technical_node(
        &self,
        actor: ActorName,
        submission: SubmitTechnicalNode,
    ) -> Result<std::result::Result<TechnicalNode, TechnicalNodeRejectionReason>> {
        if let Err(rejection) = TechnicalNodeKey::from_canonical(submission.stable_key.as_str()) {
            return Ok(Err(TechnicalNodeRejectionReason::InvalidStableNodeKey(
                rejection,
            )));
        }

        let expected_kind = submission.stable_key.expected_node_kind();
        if submission.kind != expected_kind {
            return Ok(Err(TechnicalNodeRejectionReason::KindBodyMismatch(
                TechnicalNodeKindMismatch {
                    expected_kind,
                    got_body_kind: submission.kind,
                },
            )));
        }

        if let Err(mismatch) = submission.kind.validate_body(&submission.body) {
            return Ok(Err(TechnicalNodeRejectionReason::KindBodyMismatch(
                mismatch,
            )));
        }

        if self
            .technical_node_records()?
            .iter()
            .any(|node| node.stable_key == submission.stable_key)
        {
            return Ok(Err(TechnicalNodeRejectionReason::DuplicateStableNodeKey(
                submission.stable_key,
            )));
        }

        let node = TechnicalNode {
            identifier: GraphIdMint::new(&self.engine).next_technical_node_identifier()?,
            stable_key: submission.stable_key,
            kind: submission.kind,
            body: submission.body,
            author: actor,
            occurred_at: StoreClock::system().timestamp()?,
        };
        self.assert_technical_node(node.clone())?;
        Ok(Ok(node))
    }

    pub(crate) fn append_technical_relation(
        &self,
        actor: ActorName,
        submission: SubmitTechnicalRelation,
    ) -> Result<std::result::Result<TechnicalRelation, TechnicalRelationRejectionReason>> {
        let Some(source) = self.read_technical_node_by_stable_key(&submission.source)? else {
            return Ok(Err(TechnicalRelationRejectionReason::MissingEndpoint(
                submission.source,
            )));
        };
        let Some(target) = self.read_technical_node_by_stable_key(&submission.target)? else {
            return Ok(Err(TechnicalRelationRejectionReason::MissingEndpoint(
                submission.target,
            )));
        };

        if let Err(mismatch) = submission
            .kind
            .validate_endpoint_kinds(source.kind, target.kind)
        {
            return Ok(Err(TechnicalRelationRejectionReason::DomainRangeViolation(
                mismatch,
            )));
        }

        if self.technical_relation_records()?.iter().any(|relation| {
            relation.kind == submission.kind
                && relation.source.stable_key == submission.source
                && relation.target.stable_key == submission.target
        }) {
            return Ok(Err(TechnicalRelationRejectionReason::DuplicateRelation));
        }

        let relation = TechnicalRelation {
            identifier: GraphIdMint::new(&self.engine).next_technical_relation_identifier()?,
            kind: submission.kind,
            source: TechnicalRelationEndpoint {
                identifier: source.identifier,
                stable_key: source.stable_key,
            },
            target: TechnicalRelationEndpoint {
                identifier: target.identifier,
                stable_key: target.stable_key,
            },
            author: actor,
            occurred_at: StoreClock::system().timestamp()?,
            note: submission.note,
        };
        self.assert_technical_relation(relation.clone())?;
        Ok(Ok(relation))
    }

    pub(crate) fn assert_technical_node(&self, node: TechnicalNode) -> Result<TechnicalNode> {
        self.engine.assert(Assertion::new(
            self.technical_nodes,
            StoredTechnicalNode::new(node.clone()),
        ))?;
        Ok(node)
    }

    pub(crate) fn assert_technical_relation(
        &self,
        relation: TechnicalRelation,
    ) -> Result<TechnicalRelation> {
        self.engine.assert(Assertion::new(
            self.technical_relations,
            StoredTechnicalRelation::new(relation.clone()),
        ))?;
        Ok(relation)
    }

    pub(crate) fn technical_node_records(&self) -> Result<Vec<TechnicalNode>> {
        Ok(self
            .engine
            .match_records(QueryPlan::all(self.technical_nodes))?
            .records()
            .iter()
            .cloned()
            .map(StoredTechnicalNode::into_record)
            .collect())
    }

    pub(crate) fn technical_relation_records(&self) -> Result<Vec<TechnicalRelation>> {
        Ok(self
            .engine
            .match_records(QueryPlan::all(self.technical_relations))?
            .records()
            .iter()
            .cloned()
            .map(StoredTechnicalRelation::into_record)
            .collect())
    }

    pub(crate) fn assert_accepted_knowledge(
        &self,
        record: AcceptedKnowledge,
    ) -> Result<AcceptedKnowledge> {
        self.engine.assert(Assertion::new(
            self.accepted_knowledge,
            StoredAcceptedKnowledge::new(record.clone()),
        ))?;
        Ok(record)
    }

    pub(crate) fn accepted_knowledge_records(&self) -> Result<Vec<AcceptedKnowledge>> {
        Ok(self
            .engine
            .match_records(QueryPlan::all(self.accepted_knowledge))?
            .records()
            .iter()
            .cloned()
            .map(StoredAcceptedKnowledge::into_record)
            .collect())
    }

    pub(crate) fn append_thought_subscription(
        &self,
        subscription: SubscribeThoughts,
    ) -> Result<OpenedThoughtSubscription> {
        let resume_after = subscription
            .resume_after
            .unwrap_or_else(SubscriptionCursor::initial);
        let initial_demand = subscription.initial_demand;
        let filter = subscription.filter;
        let receipt = self.engine.subscribe(
            QueryPlan::all(self.thoughts),
            ThoughtSubscriptionSink::new(THOUGHTS, self.subscription_publisher.clone()),
        )?;
        let initial = receipt
            .initial()
            .snapshot()
            .records()
            .iter()
            .cloned()
            .map(StoredThought::into_record)
            .collect();
        let record = StoredThoughtSubscription {
            subscription: Self::subscription_identifier_from_engine(receipt.handle().id()),
            filter,
        };
        self.engine
            .assert(Assertion::new(self.thought_subscriptions, record.clone()))?;
        Ok(OpenedThoughtSubscription::new(
            record,
            initial,
            resume_after,
            initial_demand,
        ))
    }

    pub(crate) fn append_relation_subscription(
        &self,
        subscription: SubscribeRelations,
    ) -> Result<OpenedRelationSubscription> {
        let resume_after = subscription
            .resume_after
            .unwrap_or_else(SubscriptionCursor::initial);
        let initial_demand = subscription.initial_demand;
        let filter = subscription.filter;
        let receipt = self.engine.subscribe(
            QueryPlan::all(self.relations),
            RelationSubscriptionSink::new(RELATIONS, self.subscription_publisher.clone()),
        )?;
        let initial = receipt
            .initial()
            .snapshot()
            .records()
            .iter()
            .cloned()
            .map(StoredRelation::into_record)
            .collect();
        let record = StoredRelationSubscription {
            subscription: Self::subscription_identifier_from_engine(receipt.handle().id()),
            filter,
        };
        self.engine
            .assert(Assertion::new(self.relation_subscriptions, record.clone()))?;
        Ok(OpenedRelationSubscription::new(
            record,
            initial,
            resume_after,
            initial_demand,
        ))
    }

    pub(crate) fn append_technical_node_subscription(
        &self,
        subscription: SubscribeTechnicalNodes,
    ) -> Result<OpenedTechnicalNodeSubscription> {
        let resume_after = subscription
            .resume_after
            .unwrap_or_else(SubscriptionCursor::initial);
        let initial_demand = subscription.initial_demand;
        let filter = subscription.filter;
        let receipt = self.engine.subscribe(
            QueryPlan::all(self.technical_nodes),
            TechnicalNodeSubscriptionSink::new(
                TECHNICAL_NODES,
                self.subscription_publisher.clone(),
            ),
        )?;
        let initial = receipt
            .initial()
            .snapshot()
            .records()
            .iter()
            .cloned()
            .map(StoredTechnicalNode::into_record)
            .collect();
        let record = StoredTechnicalNodeSubscription {
            subscription: Self::subscription_identifier_from_engine(receipt.handle().id()),
            filter,
        };
        self.engine.assert(Assertion::new(
            self.technical_node_subscriptions,
            record.clone(),
        ))?;
        Ok(OpenedTechnicalNodeSubscription::new(
            record,
            initial,
            resume_after,
            initial_demand,
        ))
    }

    pub(crate) fn append_technical_relation_subscription(
        &self,
        subscription: SubscribeTechnicalRelations,
    ) -> Result<OpenedTechnicalRelationSubscription> {
        let resume_after = subscription
            .resume_after
            .unwrap_or_else(SubscriptionCursor::initial);
        let initial_demand = subscription.initial_demand;
        let filter = subscription.filter;
        let receipt = self.engine.subscribe(
            QueryPlan::all(self.technical_relations),
            TechnicalRelationSubscriptionSink::new(
                TECHNICAL_RELATIONS,
                self.subscription_publisher.clone(),
            ),
        )?;
        let initial = receipt
            .initial()
            .snapshot()
            .records()
            .iter()
            .cloned()
            .map(StoredTechnicalRelation::into_record)
            .collect();
        let record = StoredTechnicalRelationSubscription {
            subscription: Self::subscription_identifier_from_engine(receipt.handle().id()),
            filter,
        };
        self.engine.assert(Assertion::new(
            self.technical_relation_subscriptions,
            record.clone(),
        ))?;
        Ok(OpenedTechnicalRelationSubscription::new(
            record,
            initial,
            resume_after,
            initial_demand,
        ))
    }

    pub(crate) fn register_thought_runtime(
        &self,
        record: &StoredThoughtSubscription,
        cursor: SubscriptionCursor,
        initial_demand: SubscriptionDemandCredit,
    ) {
        self.subscription_publisher.register_thought(
            record.subscription.clone(),
            record.filter.clone(),
            cursor,
            initial_demand,
        );
    }

    pub(crate) fn register_relation_runtime(
        &self,
        record: &StoredRelationSubscription,
        cursor: SubscriptionCursor,
        initial_demand: SubscriptionDemandCredit,
    ) {
        self.subscription_publisher.register_relation(
            record.subscription.clone(),
            record.filter.clone(),
            cursor,
            initial_demand,
        );
    }

    pub(crate) fn register_technical_node_runtime(
        &self,
        record: &StoredTechnicalNodeSubscription,
        cursor: SubscriptionCursor,
        initial_demand: SubscriptionDemandCredit,
    ) {
        self.subscription_publisher.register_technical_node(
            record.subscription.clone(),
            record.filter.clone(),
            cursor,
            initial_demand,
        );
    }

    pub(crate) fn register_technical_relation_runtime(
        &self,
        record: &StoredTechnicalRelationSubscription,
        cursor: SubscriptionCursor,
        initial_demand: SubscriptionDemandCredit,
    ) {
        self.subscription_publisher.register_technical_relation(
            record.subscription.clone(),
            record.filter.clone(),
            cursor,
            initial_demand,
        );
    }

    pub(crate) fn retract_subscription(&self, subscription: &SubscriptionIdentifier) -> Result<()> {
        let key = RecordKey::new(subscription.as_str());
        if !self
            .engine
            .match_records(QueryPlan::key(self.thought_subscriptions, key.clone()))?
            .records()
            .is_empty()
        {
            self.engine
                .retract(Retraction::<StoredThoughtSubscription>::new(
                    self.thought_subscriptions,
                    key,
                ))?;
            return Ok(());
        }

        let key = RecordKey::new(subscription.as_str());
        if !self
            .engine
            .match_records(QueryPlan::key(self.relation_subscriptions, key.clone()))?
            .records()
            .is_empty()
        {
            self.engine
                .retract(Retraction::<StoredRelationSubscription>::new(
                    self.relation_subscriptions,
                    key,
                ))?;
            return Ok(());
        }

        let key = RecordKey::new(subscription.as_str());
        if !self
            .engine
            .match_records(QueryPlan::key(
                self.technical_node_subscriptions,
                key.clone(),
            ))?
            .records()
            .is_empty()
        {
            self.engine
                .retract(Retraction::<StoredTechnicalNodeSubscription>::new(
                    self.technical_node_subscriptions,
                    key,
                ))?;
            return Ok(());
        }

        let key = RecordKey::new(subscription.as_str());
        if !self
            .engine
            .match_records(QueryPlan::key(
                self.technical_relation_subscriptions,
                key.clone(),
            ))?
            .records()
            .is_empty()
        {
            self.engine
                .retract(Retraction::<StoredTechnicalRelationSubscription>::new(
                    self.technical_relation_subscriptions,
                    key,
                ))?;
        }
        Ok(())
    }

    pub(crate) fn runtime_subscription_registrations(
        &self,
    ) -> Result<Vec<RuntimeSubscriptionRegistration>> {
        let mut registrations = Vec::new();
        registrations.extend(
            self.engine
                .match_records(QueryPlan::all(self.thought_subscriptions))?
                .records()
                .iter()
                .cloned()
                .map(|record| RuntimeSubscriptionRegistration::Thoughts {
                    subscription: record.subscription,
                    filter: record.filter,
                }),
        );
        registrations.extend(
            self.engine
                .match_records(QueryPlan::all(self.relation_subscriptions))?
                .records()
                .iter()
                .cloned()
                .map(|record| RuntimeSubscriptionRegistration::Relations {
                    subscription: record.subscription,
                    filter: record.filter,
                }),
        );
        registrations.extend(
            self.engine
                .match_records(QueryPlan::all(self.technical_node_subscriptions))?
                .records()
                .iter()
                .cloned()
                .map(|record| RuntimeSubscriptionRegistration::TechnicalNodes {
                    subscription: record.subscription,
                    filter: record.filter,
                }),
        );
        registrations.extend(
            self.engine
                .match_records(QueryPlan::all(self.technical_relation_subscriptions))?
                .records()
                .iter()
                .cloned()
                .map(
                    |record| RuntimeSubscriptionRegistration::TechnicalRelations {
                        subscription: record.subscription,
                        filter: record.filter,
                    },
                ),
        );
        Ok(registrations)
    }

    fn rehydrate_engine_subscriptions(&self) -> Result<()> {
        for record in self
            .engine
            .match_records(QueryPlan::all(self.thought_subscriptions))?
            .records()
            .iter()
            .cloned()
        {
            self.engine.subscribe(
                QueryPlan::all(self.thoughts),
                ThoughtSubscriptionSink::new_for_subscription(
                    THOUGHTS,
                    record.subscription,
                    self.subscription_publisher.clone(),
                ),
            )?;
        }
        for record in self
            .engine
            .match_records(QueryPlan::all(self.relation_subscriptions))?
            .records()
            .iter()
            .cloned()
        {
            self.engine.subscribe(
                QueryPlan::all(self.relations),
                RelationSubscriptionSink::new_for_subscription(
                    RELATIONS,
                    record.subscription,
                    self.subscription_publisher.clone(),
                ),
            )?;
        }
        for record in self
            .engine
            .match_records(QueryPlan::all(self.technical_node_subscriptions))?
            .records()
            .iter()
            .cloned()
        {
            self.engine.subscribe(
                QueryPlan::all(self.technical_nodes),
                TechnicalNodeSubscriptionSink::new_for_subscription(
                    TECHNICAL_NODES,
                    record.subscription,
                    self.subscription_publisher.clone(),
                ),
            )?;
        }
        for record in self
            .engine
            .match_records(QueryPlan::all(self.technical_relation_subscriptions))?
            .records()
            .iter()
            .cloned()
        {
            self.engine.subscribe(
                QueryPlan::all(self.technical_relations),
                TechnicalRelationSubscriptionSink::new_for_subscription(
                    TECHNICAL_RELATIONS,
                    record.subscription,
                    self.subscription_publisher.clone(),
                ),
            )?;
        }
        Ok(())
    }

    fn read_thought(&self, record: &RecordIdentifier) -> Result<Thought> {
        self.engine
            .match_records(QueryPlan::key(
                self.thoughts,
                RecordKey::new(record.as_str()),
            ))?
            .records()
            .first()
            .cloned()
            .map(StoredThought::into_record)
            .ok_or_else(|| crate::Error::MindGraphMissingRecord {
                record: record.as_str().to_string(),
            })
    }

    fn read_technical_node_by_stable_key(
        &self,
        stable_key: &TechnicalNodeKey,
    ) -> Result<Option<TechnicalNode>> {
        if TechnicalNodeKey::from_canonical(stable_key.as_str()).is_err() {
            return Ok(None);
        }

        Ok(self
            .technical_node_records()?
            .into_iter()
            .find(|node| node.stable_key == *stable_key))
    }

    fn subscription_identifier_from_engine(
        engine_identifier: sema_engine::SubscriptionIdentifier,
    ) -> SubscriptionIdentifier {
        SubscriptionIdentifier::new(
            CompactGraphIdentifier::from_zero_based_sequence(
                engine_identifier.value().saturating_sub(1),
            )
            .into_string(),
        )
    }
}

impl GraphSubscriptionPublisher {
    pub(crate) fn actor(actor: ActorRef<SubscriptionSupervisor>) -> Self {
        Self::Actor(actor)
    }

    #[cfg(test)]
    fn disabled() -> Self {
        Self::Disabled
    }

    fn register_thought(
        &self,
        subscription: SubscriptionIdentifier,
        filter: signal_mind::ThoughtFilter,
        cursor: SubscriptionCursor,
        initial_demand: SubscriptionDemandCredit,
    ) {
        match self {
            Self::Actor(actor) => {
                let _sent = actor
                    .tell(RegisterThoughtSubscription::new(
                        subscription,
                        filter,
                        cursor,
                        initial_demand,
                    ))
                    .try_send();
            }
            #[cfg(test)]
            Self::Disabled => {}
        }
    }

    fn register_relation(
        &self,
        subscription: SubscriptionIdentifier,
        filter: signal_mind::RelationFilter,
        cursor: SubscriptionCursor,
        initial_demand: SubscriptionDemandCredit,
    ) {
        match self {
            Self::Actor(actor) => {
                let _sent = actor
                    .tell(RegisterRelationSubscription::new(
                        subscription,
                        filter,
                        cursor,
                        initial_demand,
                    ))
                    .try_send();
            }
            #[cfg(test)]
            Self::Disabled => {}
        }
    }

    fn register_technical_node(
        &self,
        subscription: SubscriptionIdentifier,
        filter: signal_mind::TechnicalNodeFilter,
        cursor: SubscriptionCursor,
        initial_demand: SubscriptionDemandCredit,
    ) {
        match self {
            Self::Actor(actor) => {
                let _sent = actor
                    .tell(RegisterTechnicalNodeSubscription::new(
                        subscription,
                        filter,
                        cursor,
                        initial_demand,
                    ))
                    .try_send();
            }
            #[cfg(test)]
            Self::Disabled => {}
        }
    }

    fn register_technical_relation(
        &self,
        subscription: SubscriptionIdentifier,
        filter: signal_mind::TechnicalRelationFilter,
        cursor: SubscriptionCursor,
        initial_demand: SubscriptionDemandCredit,
    ) {
        match self {
            Self::Actor(actor) => {
                let _sent = actor
                    .tell(RegisterTechnicalRelationSubscription::new(
                        subscription,
                        filter,
                        cursor,
                        initial_demand,
                    ))
                    .try_send();
            }
            #[cfg(test)]
            Self::Disabled => {}
        }
    }

    fn publish_thought(
        &self,
        subscription: SubscriptionIdentifier,
        thought: Thought,
    ) -> std::result::Result<(), SinkError> {
        match self {
            Self::Actor(actor) => actor
                .tell(PublishThoughtDelta::new(subscription, thought))
                .try_send()
                .map_err(|error| SinkError::new(error.to_string())),
            #[cfg(test)]
            Self::Disabled => Ok(()),
        }
    }

    fn publish_relation(
        &self,
        subscription: SubscriptionIdentifier,
        relation: Relation,
    ) -> std::result::Result<(), SinkError> {
        match self {
            Self::Actor(actor) => actor
                .tell(PublishRelationDelta::new(subscription, relation))
                .try_send()
                .map_err(|error| SinkError::new(error.to_string())),
            #[cfg(test)]
            Self::Disabled => Ok(()),
        }
    }

    fn publish_technical_node(
        &self,
        subscription: SubscriptionIdentifier,
        node: TechnicalNode,
    ) -> std::result::Result<(), SinkError> {
        match self {
            Self::Actor(actor) => actor
                .tell(PublishTechnicalNodeDelta::new(subscription, node))
                .try_send()
                .map_err(|error| SinkError::new(error.to_string())),
            #[cfg(test)]
            Self::Disabled => Ok(()),
        }
    }

    fn publish_technical_relation(
        &self,
        subscription: SubscriptionIdentifier,
        relation: TechnicalRelation,
    ) -> std::result::Result<(), SinkError> {
        match self {
            Self::Actor(actor) => actor
                .tell(PublishTechnicalRelationDelta::new(subscription, relation))
                .try_send()
                .map_err(|error| SinkError::new(error.to_string())),
            #[cfg(test)]
            Self::Disabled => Ok(()),
        }
    }
}

struct GraphIdMint<'engine> {
    engine: &'engine Engine,
}

struct ThoughtSubscriptionSink {
    table: TableName,
    subscription: Option<SubscriptionIdentifier>,
    publisher: GraphSubscriptionPublisher,
}

struct RelationSubscriptionSink {
    table: TableName,
    subscription: Option<SubscriptionIdentifier>,
    publisher: GraphSubscriptionPublisher,
}

struct TechnicalNodeSubscriptionSink {
    table: TableName,
    subscription: Option<SubscriptionIdentifier>,
    publisher: GraphSubscriptionPublisher,
}

struct TechnicalRelationSubscriptionSink {
    table: TableName,
    subscription: Option<SubscriptionIdentifier>,
    publisher: GraphSubscriptionPublisher,
}

impl<'engine> GraphIdMint<'engine> {
    fn new(engine: &'engine Engine) -> Self {
        Self { engine }
    }

    fn next_record_id(&self) -> Result<RecordIdentifier> {
        Ok(RecordIdentifier::new(self.next_token()?))
    }

    fn next_relation_id(&self) -> Result<RelationIdentifier> {
        Ok(RelationIdentifier::new(self.next_token()?))
    }

    fn next_technical_node_identifier(&self) -> Result<TechnicalNodeIdentifier> {
        Ok(TechnicalNodeIdentifier::new(self.next_token()?))
    }

    fn next_technical_relation_identifier(&self) -> Result<TechnicalRelationIdentifier> {
        Ok(TechnicalRelationIdentifier::new(self.next_token()?))
    }

    fn next_token(&self) -> Result<String> {
        let next_snapshot = self.engine.latest_snapshot()?.next();
        Ok(CompactGraphIdentifier::from_zero_based_sequence(
            next_snapshot.value().saturating_sub(1),
        )
        .into_string())
    }
}

struct CompactGraphIdentifier {
    value: u64,
}

impl ThoughtSubscriptionSink {
    fn new(table: TableName, publisher: GraphSubscriptionPublisher) -> Arc<Self> {
        Arc::new(Self {
            table,
            subscription: None,
            publisher,
        })
    }

    fn new_for_subscription(
        table: TableName,
        subscription: SubscriptionIdentifier,
        publisher: GraphSubscriptionPublisher,
    ) -> Arc<Self> {
        Arc::new(Self {
            table,
            subscription: Some(subscription),
            publisher,
        })
    }

    fn ensure_table(&self, table: &TableName) -> std::result::Result<(), SinkError> {
        if self.table == *table {
            return Ok(());
        }

        Err(SinkError::new(format!(
            "subscription sink for {} received {}",
            self.table.as_str(),
            table.as_str()
        )))
    }
}

impl RelationSubscriptionSink {
    fn new(table: TableName, publisher: GraphSubscriptionPublisher) -> Arc<Self> {
        Arc::new(Self {
            table,
            subscription: None,
            publisher,
        })
    }

    fn new_for_subscription(
        table: TableName,
        subscription: SubscriptionIdentifier,
        publisher: GraphSubscriptionPublisher,
    ) -> Arc<Self> {
        Arc::new(Self {
            table,
            subscription: Some(subscription),
            publisher,
        })
    }

    fn ensure_table(&self, table: &TableName) -> std::result::Result<(), SinkError> {
        if self.table == *table {
            return Ok(());
        }

        Err(SinkError::new(format!(
            "subscription sink for {} received {}",
            self.table.as_str(),
            table.as_str()
        )))
    }
}

impl TechnicalNodeSubscriptionSink {
    fn new(table: TableName, publisher: GraphSubscriptionPublisher) -> Arc<Self> {
        Arc::new(Self {
            table,
            subscription: None,
            publisher,
        })
    }

    fn new_for_subscription(
        table: TableName,
        subscription: SubscriptionIdentifier,
        publisher: GraphSubscriptionPublisher,
    ) -> Arc<Self> {
        Arc::new(Self {
            table,
            subscription: Some(subscription),
            publisher,
        })
    }

    fn ensure_table(&self, table: &TableName) -> std::result::Result<(), SinkError> {
        if self.table == *table {
            return Ok(());
        }

        Err(SinkError::new(format!(
            "subscription sink for {} received {}",
            self.table.as_str(),
            table.as_str()
        )))
    }
}

impl TechnicalRelationSubscriptionSink {
    fn new(table: TableName, publisher: GraphSubscriptionPublisher) -> Arc<Self> {
        Arc::new(Self {
            table,
            subscription: None,
            publisher,
        })
    }

    fn new_for_subscription(
        table: TableName,
        subscription: SubscriptionIdentifier,
        publisher: GraphSubscriptionPublisher,
    ) -> Arc<Self> {
        Arc::new(Self {
            table,
            subscription: Some(subscription),
            publisher,
        })
    }

    fn ensure_table(&self, table: &TableName) -> std::result::Result<(), SinkError> {
        if self.table == *table {
            return Ok(());
        }

        Err(SinkError::new(format!(
            "subscription sink for {} received {}",
            self.table.as_str(),
            table.as_str()
        )))
    }
}

impl SubscriptionSink<StoredThought> for ThoughtSubscriptionSink {
    fn delivery_mode(&self) -> SubscriptionDeliveryMode {
        SubscriptionDeliveryMode::Inline
    }

    fn deliver(
        &self,
        event: EngineSubscriptionEvent<StoredThought>,
    ) -> std::result::Result<(), SinkError> {
        match event {
            EngineSubscriptionEvent::InitialSnapshot(snapshot) => {
                self.ensure_table(snapshot.handle().table())
            }
            EngineSubscriptionEvent::Delta(delta) => {
                self.ensure_table(delta.table())?;
                self.publisher.publish_thought(
                    self.subscription.clone().unwrap_or_else(|| {
                        MindTables::subscription_identifier_from_engine(delta.handle().id())
                    }),
                    delta.record().clone().into_record(),
                )
            }
        }
    }
}

impl SubscriptionSink<StoredRelation> for RelationSubscriptionSink {
    fn delivery_mode(&self) -> SubscriptionDeliveryMode {
        SubscriptionDeliveryMode::Inline
    }

    fn deliver(
        &self,
        event: EngineSubscriptionEvent<StoredRelation>,
    ) -> std::result::Result<(), SinkError> {
        match event {
            EngineSubscriptionEvent::InitialSnapshot(snapshot) => {
                self.ensure_table(snapshot.handle().table())
            }
            EngineSubscriptionEvent::Delta(delta) => {
                self.ensure_table(delta.table())?;
                self.publisher.publish_relation(
                    self.subscription.clone().unwrap_or_else(|| {
                        MindTables::subscription_identifier_from_engine(delta.handle().id())
                    }),
                    delta.record().clone().into_record(),
                )
            }
        }
    }
}

impl SubscriptionSink<StoredTechnicalNode> for TechnicalNodeSubscriptionSink {
    fn delivery_mode(&self) -> SubscriptionDeliveryMode {
        SubscriptionDeliveryMode::Inline
    }

    fn deliver(
        &self,
        event: EngineSubscriptionEvent<StoredTechnicalNode>,
    ) -> std::result::Result<(), SinkError> {
        match event {
            EngineSubscriptionEvent::InitialSnapshot(snapshot) => {
                self.ensure_table(snapshot.handle().table())
            }
            EngineSubscriptionEvent::Delta(delta) => {
                self.ensure_table(delta.table())?;
                self.publisher.publish_technical_node(
                    self.subscription.clone().unwrap_or_else(|| {
                        MindTables::subscription_identifier_from_engine(delta.handle().id())
                    }),
                    delta.record().clone().into_record(),
                )
            }
        }
    }
}

impl SubscriptionSink<StoredTechnicalRelation> for TechnicalRelationSubscriptionSink {
    fn delivery_mode(&self) -> SubscriptionDeliveryMode {
        SubscriptionDeliveryMode::Inline
    }

    fn deliver(
        &self,
        event: EngineSubscriptionEvent<StoredTechnicalRelation>,
    ) -> std::result::Result<(), SinkError> {
        match event {
            EngineSubscriptionEvent::InitialSnapshot(snapshot) => {
                self.ensure_table(snapshot.handle().table())
            }
            EngineSubscriptionEvent::Delta(delta) => {
                self.ensure_table(delta.table())?;
                self.publisher.publish_technical_relation(
                    self.subscription.clone().unwrap_or_else(|| {
                        MindTables::subscription_identifier_from_engine(delta.handle().id())
                    }),
                    delta.record().clone().into_record(),
                )
            }
        }
    }
}

impl CompactGraphIdentifier {
    fn from_zero_based_sequence(value: u64) -> Self {
        Self { value }
    }

    fn into_string(self) -> String {
        let alphabet = b"abcdefghijklmnopqrstuvwxyz";
        let mut value = self.value;
        let mut bytes = Vec::new();
        loop {
            bytes.push(alphabet[(value % 26) as usize]);
            value /= 26;
            if value == 0 {
                break;
            }
        }
        while bytes.len() < 3 {
            bytes.push(alphabet[0]);
        }
        bytes.reverse();
        String::from_utf8(bytes).expect("compact graph id is ascii")
    }
}

pub(crate) struct StoreClock {
    epoch: SystemTime,
}

impl StoreClock {
    pub(crate) fn system() -> Self {
        Self { epoch: UNIX_EPOCH }
    }

    pub(crate) fn timestamp(&self) -> Result<TimestampNanos> {
        let nanos = SystemTime::now()
            .duration_since(self.epoch)?
            .as_nanos()
            .min(u64::MAX as u128) as u64;
        Ok(TimestampNanos::new(nanos))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signal_mind::{
        ByTechnicalNodeStableKey, ByTechnicalRelationSource, ByThoughtKind, ComponentNode,
        GoalBody, GoalScope, RelationKind, SubmitRelation, SubmitTechnicalNode,
        SubmitTechnicalRelation, SubmitThought, SubscriptionCursor, SubscriptionDemandCredit,
        TechnicalNodeBody, TechnicalNodeFilter, TechnicalNodeIdentifier, TechnicalNodeKind,
        TechnicalRelationEndpoint, TechnicalRelationFilter, TechnicalRelationIdentifier,
        TechnicalRelationKind, TextBody, ThoughtBody, ThoughtFilter, ThoughtKind, WirePath,
        WorkspaceGoal,
    };
    use signal_persona::ComponentName;

    fn technical_key(value: &str) -> TechnicalNodeKey {
        TechnicalNodeKey::from_canonical(value).expect("test technical key is canonical")
    }

    #[test]
    fn thought_subscription_is_durable_table_data() {
        let store = StoreLocation::new(unique_store_path("thought-subscription-durable"));
        let tables =
            MindTables::open(&store, GraphSubscriptionPublisher::disabled()).expect("tables open");
        let opened = tables
            .append_thought_subscription(SubscribeThoughts {
                filter: ThoughtFilter::ByKind(ByThoughtKind {
                    kinds: vec![ThoughtKind::Goal],
                }),
                resume_after: None,
                initial_demand: SubscriptionDemandCredit::new(1),
            })
            .expect("subscription appends");
        let stored = opened.record().clone();
        drop(tables);

        let reopened = MindTables::open(&store, GraphSubscriptionPublisher::disabled())
            .expect("tables reopen");
        let persisted = reopened
            .engine
            .match_records(QueryPlan::key(
                reopened.thought_subscriptions,
                RecordKey::new(stored.subscription.as_str()),
            ))
            .expect("subscription lookup")
            .records()
            .first()
            .cloned()
            .expect("subscription stored");

        assert_eq!(persisted, stored);
        assert_eq!(persisted.subscription.as_str().len(), 3);
    }

    #[test]
    fn typed_subscription_registration_uses_sema_engine_catalog() {
        let store = StoreLocation::new(unique_store_path("subscription-engine-catalog"));
        let tables =
            MindTables::open(&store, GraphSubscriptionPublisher::disabled()).expect("tables open");
        let opened = tables
            .append_thought_subscription(SubscribeThoughts {
                filter: ThoughtFilter::ByKind(ByThoughtKind {
                    kinds: vec![ThoughtKind::Goal],
                }),
                resume_after: Some(SubscriptionCursor::initial()),
                initial_demand: SubscriptionDemandCredit::new(1),
            })
            .expect("subscription appends");

        let registrations = tables
            .engine
            .subscription_registrations()
            .expect("subscription registrations read");

        assert_eq!(opened.record().subscription.as_str(), "aaa");
        assert_eq!(registrations.len(), 1);
        assert_eq!(registrations[0].table_name(), "thoughts");
        assert_eq!(registrations[0].id().value(), 1);
    }

    #[test]
    fn typed_thought_append_uses_sema_engine_operation_log() {
        let store = StoreLocation::new(unique_store_path("thought-operation-log"));
        let tables =
            MindTables::open(&store, GraphSubscriptionPublisher::disabled()).expect("tables open");
        let thought = tables
            .append_thought(
                ActorName::new("operator"),
                SubmitThought {
                    kind: ThoughtKind::Goal,
                    body: ThoughtBody::Goal(GoalBody {
                        description: TextBody::new("prove engine path"),
                        scope: GoalScope::Workspace(WorkspaceGoal {
                            workspace: TextBody::new("primary"),
                        }),
                    }),
                },
            )
            .expect("thought appends");

        let log = tables.engine.commit_log().expect("commit log reads");
        let versioned_log = tables
            .engine
            .versioned_commit_log()
            .expect("versioned commit log reads");
        let records = tables.thought_records().expect("thoughts read");

        assert_eq!(thought.id.as_str(), "aaa");
        assert_eq!(records, vec![thought.clone()]);
        assert_eq!(log.len(), 1);
        let head = log[0].operations().head();
        assert_eq!(head.operation().as_record_head(), "Assert");
        assert_eq!(head.table_name(), "thoughts");
        assert_eq!(
            head.key().map(RecordKey::to_owned_string).as_deref(),
            Some(thought.id.as_str())
        );
        assert_eq!(versioned_log.len(), 1);
        let versioned_head = versioned_log[0].operations().head();
        assert_eq!(versioned_log[0].store_name().as_str(), "mind");
        assert_eq!(
            versioned_log[0].schema_hash(),
            tables.engine.store_schema_hash()
        );
        assert_eq!(versioned_head.operation().as_record_head(), "Assert");
        assert_eq!(versioned_head.table_name(), "thoughts");
        assert_eq!(
            versioned_head
                .key()
                .map(RecordKey::to_owned_string)
                .as_deref(),
            Some(thought.id.as_str())
        );
        let stored = rkyv::from_bytes::<StoredThought, rkyv::rancor::Error>(
            versioned_head
                .payload()
                .bytes()
                .expect("versioned thought payload carries record bytes"),
        )
        .expect("versioned thought payload decodes");
        assert_eq!(stored.record, thought);
    }

    #[test]
    fn technical_node_family_persists_compact_identifier_and_stable_key() {
        let store = StoreLocation::new(unique_store_path("technical-node-family"));
        let tables =
            MindTables::open(&store, GraphSubscriptionPublisher::disabled()).expect("tables open");
        let node = technical_node("aaa", "component:mind");
        tables
            .assert_technical_node(node.clone())
            .expect("technical node asserts");
        drop(tables);

        let reopened = MindTables::open(&store, GraphSubscriptionPublisher::disabled())
            .expect("tables reopen");
        let records = reopened
            .technical_node_records()
            .expect("technical nodes read");
        let stored = reopened
            .engine
            .match_records(QueryPlan::key(
                reopened.technical_nodes,
                RecordKey::new(node.identifier.as_str()),
            ))
            .expect("technical node lookup")
            .records()
            .first()
            .cloned()
            .expect("technical node stored")
            .into_record();

        assert_eq!(records, vec![node.clone()]);
        assert_eq!(stored.identifier.as_str(), "aaa");
        assert_eq!(stored.stable_key.as_str(), "component:mind");
    }

    #[test]
    fn technical_relation_family_persists_endpoint_stable_keys() {
        let store = StoreLocation::new(unique_store_path("technical-relation-family"));
        let tables =
            MindTables::open(&store, GraphSubscriptionPublisher::disabled()).expect("tables open");
        let relation = technical_relation("aaa", "component:mind", "component:router");
        tables
            .assert_technical_relation(relation.clone())
            .expect("technical relation asserts");
        drop(tables);

        let reopened = MindTables::open(&store, GraphSubscriptionPublisher::disabled())
            .expect("tables reopen");
        let records = reopened
            .technical_relation_records()
            .expect("technical relations read");
        let stored = reopened
            .engine
            .match_records(QueryPlan::key(
                reopened.technical_relations,
                RecordKey::new(relation.identifier.as_str()),
            ))
            .expect("technical relation lookup")
            .records()
            .first()
            .cloned()
            .expect("technical relation stored")
            .into_record();

        assert_eq!(records, vec![relation.clone()]);
        assert_eq!(stored.identifier.as_str(), "aaa");
        assert_eq!(stored.source.stable_key.as_str(), "component:mind");
        assert_eq!(stored.target.stable_key.as_str(), "component:router");
    }

    #[test]
    fn technical_node_append_mints_compact_identifier_and_rejects_kind_body_mismatch() {
        let store = StoreLocation::new(unique_store_path("technical-node-append"));
        let tables =
            MindTables::open(&store, GraphSubscriptionPublisher::disabled()).expect("tables open");
        let accepted = tables
            .append_technical_node(
                ActorName::new("operator"),
                SubmitTechnicalNode {
                    stable_key: technical_key("component:mind"),
                    kind: TechnicalNodeKind::Component,
                    body: TechnicalNodeBody::Component(ComponentNode {
                        component: ComponentName::new("mind"),
                        summary: None,
                    }),
                },
            )
            .expect("node append evaluates")
            .expect("node accepted");
        let rejected = tables
            .append_technical_node(
                ActorName::new("operator"),
                SubmitTechnicalNode {
                    stable_key: technical_key("repo:wrong-body"),
                    kind: TechnicalNodeKind::Repository,
                    body: TechnicalNodeBody::Component(ComponentNode {
                        component: ComponentName::new("wrong-body"),
                        summary: None,
                    }),
                },
            )
            .expect("mismatch evaluates")
            .expect_err("kind/body mismatch rejects");
        let records = tables
            .technical_node_records()
            .expect("technical nodes read");

        assert_eq!(accepted.identifier.as_str(), "aaa");
        assert_eq!(accepted.author, ActorName::new("operator"));
        assert_eq!(records, vec![accepted]);
        assert!(matches!(
            rejected,
            TechnicalNodeRejectionReason::KindBodyMismatch(mismatch)
                if mismatch.expected_kind == TechnicalNodeKind::Repository
                    && mismatch.got_body_kind == TechnicalNodeKind::Component
        ));
    }

    #[test]
    fn technical_node_append_rejects_duplicate_stable_key() {
        let store = StoreLocation::new(unique_store_path("technical-node-duplicate"));
        let tables =
            MindTables::open(&store, GraphSubscriptionPublisher::disabled()).expect("tables open");
        let stable_key = technical_key("component:mind");
        tables
            .append_technical_node(
                ActorName::new("operator"),
                SubmitTechnicalNode {
                    stable_key: stable_key.clone(),
                    kind: TechnicalNodeKind::Component,
                    body: TechnicalNodeBody::Component(ComponentNode {
                        component: ComponentName::new("mind"),
                        summary: None,
                    }),
                },
            )
            .expect("first append evaluates")
            .expect("first node accepted");
        let rejected = tables
            .append_technical_node(
                ActorName::new("operator"),
                SubmitTechnicalNode {
                    stable_key: stable_key.clone(),
                    kind: TechnicalNodeKind::Component,
                    body: TechnicalNodeBody::Component(ComponentNode {
                        component: ComponentName::new("mind-duplicate"),
                        summary: None,
                    }),
                },
            )
            .expect("duplicate evaluates")
            .expect_err("duplicate stable key rejects");

        assert_eq!(
            rejected,
            TechnicalNodeRejectionReason::DuplicateStableNodeKey(stable_key)
        );
        assert_eq!(
            tables
                .technical_node_records()
                .expect("technical nodes read")
                .len(),
            1
        );
    }

    #[test]
    fn technical_relation_append_resolves_endpoints_and_rejects_invalid_triples() {
        let store = StoreLocation::new(unique_store_path("technical-relation-append"));
        let tables =
            MindTables::open(&store, GraphSubscriptionPublisher::disabled()).expect("tables open");
        let component_key = technical_key("component:mind");
        let repository_key = technical_key("repo:mind");
        let missing_key = technical_key("component:missing");
        let component = tables
            .append_technical_node(
                ActorName::new("operator"),
                SubmitTechnicalNode {
                    stable_key: component_key.clone(),
                    kind: TechnicalNodeKind::Component,
                    body: TechnicalNodeBody::Component(ComponentNode {
                        component: ComponentName::new("mind"),
                        summary: None,
                    }),
                },
            )
            .expect("component evaluates")
            .expect("component accepted");
        let repository = tables
            .append_technical_node(
                ActorName::new("operator"),
                SubmitTechnicalNode {
                    stable_key: repository_key.clone(),
                    kind: TechnicalNodeKind::Repository,
                    body: TechnicalNodeBody::Repository(signal_mind::RepositoryNode {
                        path: WirePath::from_absolute_path("/git/github.com/LiGoldragon/mind")
                            .expect("absolute path"),
                        remote: None,
                    }),
                },
            )
            .expect("repository evaluates")
            .expect("repository accepted");

        let missing = tables
            .append_technical_relation(
                ActorName::new("operator"),
                SubmitTechnicalRelation {
                    kind: TechnicalRelationKind::OwnsRepository,
                    source: missing_key.clone(),
                    target: repository_key.clone(),
                    note: None,
                },
            )
            .expect("missing endpoint evaluates")
            .expect_err("missing endpoint rejects");
        let accepted = tables
            .append_technical_relation(
                ActorName::new("operator"),
                SubmitTechnicalRelation {
                    kind: TechnicalRelationKind::OwnsRepository,
                    source: component_key.clone(),
                    target: repository_key.clone(),
                    note: None,
                },
            )
            .expect("relation evaluates")
            .expect("relation accepted");
        let duplicate = tables
            .append_technical_relation(
                ActorName::new("operator"),
                SubmitTechnicalRelation {
                    kind: TechnicalRelationKind::OwnsRepository,
                    source: component_key.clone(),
                    target: repository_key.clone(),
                    note: Some(TextBody::new("same triple")),
                },
            )
            .expect("duplicate relation evaluates")
            .expect_err("duplicate relation rejects");
        let wrong_domain = tables
            .append_technical_relation(
                ActorName::new("operator"),
                SubmitTechnicalRelation {
                    kind: TechnicalRelationKind::OwnsRepository,
                    source: repository_key.clone(),
                    target: component_key.clone(),
                    note: None,
                },
            )
            .expect("domain range evaluates")
            .expect_err("domain range rejects");

        assert_eq!(
            missing,
            TechnicalRelationRejectionReason::MissingEndpoint(missing_key)
        );
        assert_eq!(accepted.identifier.as_str(), "aac");
        assert_eq!(accepted.source.identifier, component.identifier);
        assert_eq!(accepted.target.identifier, repository.identifier);
        assert_eq!(
            duplicate,
            TechnicalRelationRejectionReason::DuplicateRelation
        );
        assert!(matches!(
            wrong_domain,
            TechnicalRelationRejectionReason::DomainRangeViolation(_)
        ));
        assert_eq!(
            tables
                .technical_relation_records()
                .expect("technical relations read"),
            vec![accepted]
        );
    }

    #[test]
    fn technical_subscription_families_register_and_persist_filters() {
        let store = StoreLocation::new(unique_store_path("technical-subscription-family"));
        let tables =
            MindTables::open(&store, GraphSubscriptionPublisher::disabled()).expect("tables open");
        let node_subscription = tables
            .append_technical_node_subscription(SubscribeTechnicalNodes {
                filter: TechnicalNodeFilter::ByStableKey(ByTechnicalNodeStableKey {
                    stable_key: technical_key("component:mind"),
                }),
                resume_after: None,
                initial_demand: SubscriptionDemandCredit::new(1),
            })
            .expect("technical node subscription asserts");
        let relation_subscription = tables
            .append_technical_relation_subscription(SubscribeTechnicalRelations {
                filter: TechnicalRelationFilter::BySource(ByTechnicalRelationSource {
                    source: technical_key("component:mind"),
                }),
                resume_after: None,
                initial_demand: SubscriptionDemandCredit::new(1),
            })
            .expect("technical relation subscription asserts");
        let node_record = node_subscription.record().clone();
        let relation_record = relation_subscription.record().clone();
        drop(tables);

        let reopened = MindTables::open(&store, GraphSubscriptionPublisher::disabled())
            .expect("tables reopen");
        let persisted_node_subscription = reopened
            .engine
            .match_records(QueryPlan::key(
                reopened.technical_node_subscriptions,
                RecordKey::new(node_record.subscription.as_str()),
            ))
            .expect("technical node subscription lookup")
            .records()
            .first()
            .cloned()
            .expect("technical node subscription stored");
        let persisted_relation_subscription = reopened
            .engine
            .match_records(QueryPlan::key(
                reopened.technical_relation_subscriptions,
                RecordKey::new(relation_record.subscription.as_str()),
            ))
            .expect("technical relation subscription lookup")
            .records()
            .first()
            .cloned()
            .expect("technical relation subscription stored");

        assert!(node_subscription.initial().is_empty());
        assert!(relation_subscription.initial().is_empty());
        assert_eq!(persisted_node_subscription, node_record);
        assert_eq!(persisted_relation_subscription, relation_record);
        assert_eq!(persisted_node_subscription.subscription.as_str(), "aaa");
        assert_eq!(persisted_relation_subscription.subscription.as_str(), "aab");
    }

    #[test]
    fn v8_store_opens_as_current_and_preserves_existing_graph_rows() {
        let store = StoreLocation::new(unique_store_path("v8-to-current-open"));
        let original = seed_v8_thought_store(&store);

        let tables =
            MindTables::open(&store, GraphSubscriptionPublisher::disabled()).expect("tables open");
        let preserved = tables.thought_records().expect("thoughts read");
        let technical = tables
            .assert_technical_node(technical_node("aab", "component:mind"))
            .expect("technical node asserts after migration");
        drop(tables);

        let reopened = MindTables::open(&store, GraphSubscriptionPublisher::disabled())
            .expect("tables reopen");

        assert_eq!(preserved, vec![original.clone()]);
        assert_eq!(
            reopened.thought_records().expect("thoughts reread"),
            vec![original]
        );
        assert_eq!(
            reopened
                .technical_node_records()
                .expect("technical nodes reread"),
            vec![technical]
        );
    }

    #[test]
    fn graph_id_policy_mints_compact_typed_sequence_ids_without_prefixes() {
        let store = StoreLocation::new(unique_store_path("graph-id-policy"));
        let tables =
            MindTables::open(&store, GraphSubscriptionPublisher::disabled()).expect("tables open");
        let first = tables
            .append_thought(ActorName::new("operator"), goal_submission("first goal"))
            .expect("first thought appends");
        let second = tables
            .append_thought(ActorName::new("operator"), goal_submission("second goal"))
            .expect("second thought appends");
        let relation = tables
            .append_relation(
                ActorName::new("operator"),
                SubmitRelation {
                    kind: RelationKind::Requires,
                    source: first.id.clone(),
                    target: second.id.clone(),
                    note: None,
                },
            )
            .expect("relation appends");

        assert_eq!(first.id.as_str(), "aaa");
        assert_eq!(second.id.as_str(), "aab");
        assert_eq!(relation.id.as_str(), "aac");
        for token in [first.id.as_str(), second.id.as_str(), relation.id.as_str()] {
            assert_eq!(token.len(), 3);
            assert!(token.bytes().all(|byte| byte.is_ascii_lowercase()));
            assert!(!token.contains('-'));
            assert!(!token.starts_with("thought"));
            assert!(!token.starts_with("relation"));
        }
    }

    #[test]
    fn graph_id_policy_continues_after_reopen_without_collision() {
        let store = StoreLocation::new(unique_store_path("graph-id-reopen"));
        let first_id = {
            let tables = MindTables::open(&store, GraphSubscriptionPublisher::disabled())
                .expect("tables open");
            tables
                .append_thought(ActorName::new("operator"), goal_submission("before reopen"))
                .expect("first thought appends")
                .id
        };
        assert_eq!(first_id.as_str(), "aaa");

        let reopened = MindTables::open(&store, GraphSubscriptionPublisher::disabled())
            .expect("tables reopen");
        let second = reopened
            .append_thought(ActorName::new("operator"), goal_submission("after reopen"))
            .expect("second thought appends");
        let records = reopened.thought_records().expect("thoughts read");
        let log = reopened.engine.commit_log().expect("commit log reads");

        assert_eq!(second.id.as_str(), "aab");
        assert_eq!(records.len(), 2);
        assert!(records.iter().any(|thought| thought.id == first_id));
        assert!(records.iter().any(|thought| thought.id == second.id));
        assert_eq!(log.len(), 2);
        assert_ne!(
            log[0].operations().head().key(),
            log[1].operations().head().key()
        );
    }

    fn seed_v8_thought_store(store: &StoreLocation) -> Thought {
        let mut engine = Engine::open(MindTables::engine_open_with_version(
            store,
            MIND_SCHEMA_VERSION_V8,
        ))
        .expect("v8 engine opens");
        let thoughts = engine
            .register_table(MindTables::family_descriptor::<StoredThought>(
                THOUGHTS,
                "thought",
                MIND_SCHEMA_VERSION_V8,
            ))
            .expect("v8 thoughts table registers");
        engine
            .register_table(MindTables::family_descriptor::<MemoryGraph>(
                MEMORY_GRAPH,
                "memory-graph",
                MIND_SCHEMA_VERSION_V8,
            ))
            .expect("v8 memory graph registers");
        engine
            .register_table(MindTables::family_descriptor::<StoredRelation>(
                RELATIONS,
                "relation",
                MIND_SCHEMA_VERSION_V8,
            ))
            .expect("v8 relations register");
        engine
            .register_table(MindTables::family_descriptor::<StoredThoughtSubscription>(
                THOUGHT_SUBSCRIPTIONS,
                "thought-subscription",
                MIND_SCHEMA_VERSION_V8,
            ))
            .expect("v8 thought subscriptions register");
        engine
            .register_table(MindTables::family_descriptor::<StoredRelationSubscription>(
                RELATION_SUBSCRIPTIONS,
                "relation-subscription",
                MIND_SCHEMA_VERSION_V8,
            ))
            .expect("v8 relation subscriptions register");

        let thought = Thought {
            id: RecordIdentifier::new("aaa"),
            kind: ThoughtKind::Goal,
            body: goal_submission("v8 graph row").body,
            author: ActorName::new("operator"),
            occurred_at: TimestampNanos::new(1),
        };
        engine
            .assert(Assertion::new(
                thoughts,
                StoredThought::new(thought.clone()),
            ))
            .expect("v8 thought asserts");
        thought
    }

    fn technical_node(identifier: &str, stable_key: &str) -> TechnicalNode {
        TechnicalNode {
            identifier: TechnicalNodeIdentifier::new(identifier),
            stable_key: technical_key(stable_key),
            kind: TechnicalNodeKind::Component,
            body: TechnicalNodeBody::Component(ComponentNode {
                component: ComponentName::new(stable_key.replace(':', "-")),
                summary: Some(TextBody::new("technical component node")),
            }),
            author: ActorName::new("operator"),
            occurred_at: TimestampNanos::new(1),
        }
    }

    fn technical_relation(
        identifier: &str,
        source_key: &str,
        target_key: &str,
    ) -> TechnicalRelation {
        TechnicalRelation {
            identifier: TechnicalRelationIdentifier::new(identifier),
            kind: TechnicalRelationKind::RuntimeDependency,
            source: TechnicalRelationEndpoint {
                identifier: TechnicalNodeIdentifier::new("aaa"),
                stable_key: technical_key(source_key),
            },
            target: TechnicalRelationEndpoint {
                identifier: TechnicalNodeIdentifier::new("aab"),
                stable_key: technical_key(target_key),
            },
            author: ActorName::new("operator"),
            occurred_at: TimestampNanos::new(2),
            note: Some(TextBody::new("technical relation storage fixture")),
        }
    }

    fn goal_submission(description: &str) -> SubmitThought {
        SubmitThought {
            kind: ThoughtKind::Goal,
            body: ThoughtBody::Goal(GoalBody {
                description: TextBody::new(description),
                scope: GoalScope::Workspace(WorkspaceGoal {
                    workspace: TextBody::new("primary"),
                }),
            }),
        }
    }

    fn unique_store_path(name: &str) -> String {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        std::env::temp_dir()
            .join(format!("mind-{name}-{}-{stamp}.sema", std::process::id()))
            .to_string_lossy()
            .to_string()
    }
}
