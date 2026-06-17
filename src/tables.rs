use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use kameo::actor::ActorRef;
use sema_engine::{
    Assertion, Engine, EngineOpen, EngineRecord, FamilyName, Mutation, QueryPlan, RecordKey,
    SchemaHash, SchemaVersion, SinkError, SubscriptionDeliveryMode,
    SubscriptionEvent as EngineSubscriptionEvent, SubscriptionSink, TableDescriptor, TableName,
    TableReference, VersionedStoreName, VersioningPolicy,
};
use signal_mind::{
    ActorName, RecordIdentifier, Relation, RelationIdentifier, SubmitRelation, SubmitThought,
    SubscribeRelations, SubscribeThoughts, SubscriptionIdentifier, Thought, TimestampNanos,
};

use crate::actors::subscription::{
    PublishRelationDelta, PublishThoughtDelta, SubscriptionSupervisor,
};
use crate::{MemoryGraph, Result, StoreLocation};

const MIND_SCHEMA_VERSION: SchemaVersion = SchemaVersion::new(8);

const MEMORY_GRAPH: TableName = TableName::new("memory_graph");
const THOUGHT_SUBSCRIPTIONS: TableName = TableName::new("thought_subscriptions");
const RELATION_SUBSCRIPTIONS: TableName = TableName::new("relation_subscriptions");
const MEMORY_GRAPH_KEY: &str = "current";
const THOUGHTS: TableName = TableName::new("thoughts");
const RELATIONS: TableName = TableName::new("relations");

pub struct MindTables {
    engine: Engine,
    memory: TableReference<MemoryGraph>,
    thoughts: TableReference<StoredThought>,
    relations: TableReference<StoredRelation>,
    thought_subscriptions: TableReference<StoredThoughtSubscription>,
    relation_subscriptions: TableReference<StoredRelationSubscription>,
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
pub(crate) struct StoredThought {
    record: Thought,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub(crate) struct StoredRelation {
    record: Relation,
}

pub(crate) struct OpenedThoughtSubscription {
    record: StoredThoughtSubscription,
    initial: Vec<Thought>,
}

pub(crate) struct OpenedRelationSubscription {
    record: StoredRelationSubscription,
    initial: Vec<Relation>,
}

#[derive(Clone)]
pub(crate) enum GraphSubscriptionPublisher {
    Actor(ActorRef<SubscriptionSupervisor>),
    #[cfg(test)]
    Disabled,
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

impl OpenedThoughtSubscription {
    fn new(record: StoredThoughtSubscription, initial: Vec<Thought>) -> Self {
        Self { record, initial }
    }

    pub(crate) fn record(&self) -> &StoredThoughtSubscription {
        &self.record
    }

    pub(crate) fn initial(&self) -> &[Thought] {
        &self.initial
    }
}

impl OpenedRelationSubscription {
    fn new(record: StoredRelationSubscription, initial: Vec<Relation>) -> Self {
        Self { record, initial }
    }

    pub(crate) fn record(&self) -> &StoredRelationSubscription {
        &self.record
    }

    pub(crate) fn initial(&self) -> &[Relation] {
        &self.initial
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

impl MindTables {
    pub(crate) fn open(
        store: &StoreLocation,
        subscription_publisher: GraphSubscriptionPublisher,
    ) -> Result<Self> {
        let mut engine = Engine::open(Self::engine_open(store))?;
        let memory =
            engine.register_table(Self::family_descriptor(MEMORY_GRAPH, "memory-graph"))?;
        let thoughts = engine.register_table(Self::family_descriptor(THOUGHTS, "thought"))?;
        let relations = engine.register_table(Self::family_descriptor(RELATIONS, "relation"))?;
        let thought_subscriptions = engine.register_table(Self::family_descriptor(
            THOUGHT_SUBSCRIPTIONS,
            "thought-subscription",
        ))?;
        let relation_subscriptions = engine.register_table(Self::family_descriptor(
            RELATION_SUBSCRIPTIONS,
            "relation-subscription",
        ))?;
        Ok(Self {
            engine,
            memory,
            thoughts,
            relations,
            thought_subscriptions,
            relation_subscriptions,
            subscription_publisher,
        })
    }

    fn engine_open(store: &StoreLocation) -> EngineOpen {
        EngineOpen::new(store.as_path(), MIND_SCHEMA_VERSION)
            .with_versioning(Self::versioning_policy())
    }

    fn versioning_policy() -> VersioningPolicy {
        VersioningPolicy::new(VersionedStoreName::new("mind"))
    }

    /// Family identity per registered table. The schema hash is a
    /// typed stand-in derived from the family label and mind schema
    /// version until schema generation supplies content hashes from
    /// the `.schema` source.
    fn family_descriptor<RecordValue>(
        table: TableName,
        family: &str,
    ) -> TableDescriptor<RecordValue> {
        TableDescriptor::new(
            table,
            FamilyName::new(family),
            SchemaHash::for_label(format!("mind-{family}-v{}", MIND_SCHEMA_VERSION.value())),
        )
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

    pub(crate) fn append_thought_subscription(
        &self,
        subscription: SubscribeThoughts,
    ) -> Result<OpenedThoughtSubscription> {
        let filter = subscription.filter;
        let receipt = self.engine.subscribe(
            QueryPlan::all(self.thoughts),
            ThoughtSubscriptionSink::new(
                THOUGHTS,
                filter.clone(),
                self.subscription_publisher.clone(),
            ),
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
        Ok(OpenedThoughtSubscription::new(record, initial))
    }

    pub(crate) fn append_relation_subscription(
        &self,
        subscription: SubscribeRelations,
    ) -> Result<OpenedRelationSubscription> {
        let filter = subscription.filter;
        let receipt = self.engine.subscribe(
            QueryPlan::all(self.relations),
            RelationSubscriptionSink::new(
                RELATIONS,
                filter.clone(),
                self.subscription_publisher.clone(),
            ),
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
        Ok(OpenedRelationSubscription::new(record, initial))
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

    fn publish_thought(
        &self,
        subscription: SubscriptionIdentifier,
        filter: signal_mind::ThoughtFilter,
        thought: Thought,
    ) -> std::result::Result<(), SinkError> {
        match self {
            Self::Actor(actor) => actor
                .tell(PublishThoughtDelta::new(subscription, filter, thought))
                .try_send()
                .map_err(|error| SinkError::new(error.to_string())),
            #[cfg(test)]
            Self::Disabled => Ok(()),
        }
    }

    fn publish_relation(
        &self,
        subscription: SubscriptionIdentifier,
        filter: signal_mind::RelationFilter,
        relation: Relation,
    ) -> std::result::Result<(), SinkError> {
        match self {
            Self::Actor(actor) => actor
                .tell(PublishRelationDelta::new(subscription, filter, relation))
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
    filter: signal_mind::ThoughtFilter,
    publisher: GraphSubscriptionPublisher,
}

struct RelationSubscriptionSink {
    table: TableName,
    filter: signal_mind::RelationFilter,
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
    fn new(
        table: TableName,
        filter: signal_mind::ThoughtFilter,
        publisher: GraphSubscriptionPublisher,
    ) -> Arc<Self> {
        Arc::new(Self {
            table,
            filter,
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
    fn new(
        table: TableName,
        filter: signal_mind::RelationFilter,
        publisher: GraphSubscriptionPublisher,
    ) -> Arc<Self> {
        Arc::new(Self {
            table,
            filter,
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
                    MindTables::subscription_identifier_from_engine(delta.handle().id()),
                    self.filter.clone(),
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
                    MindTables::subscription_identifier_from_engine(delta.handle().id()),
                    self.filter.clone(),
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

struct StoreClock {
    epoch: SystemTime,
}

impl StoreClock {
    fn system() -> Self {
        Self { epoch: UNIX_EPOCH }
    }

    fn timestamp(&self) -> Result<TimestampNanos> {
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
        ByThoughtKind, GoalBody, GoalScope, RelationKind, SubmitRelation, SubmitThought, TextBody,
        ThoughtBody, ThoughtFilter, ThoughtKind, WorkspaceGoal,
    };

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
