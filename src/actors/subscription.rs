use kameo::actor::{Actor, ActorRef};
use kameo::error::Infallible;
use kameo::message::{Context, Message};
use signal_mind::{
    MindDelta, Relation, RelationFilter, SubscriptionEvent, SubscriptionIdentifier, TechnicalNode,
    TechnicalNodeFilter, TechnicalRelation, TechnicalRelationFilter, Thought, ThoughtFilter,
};

use crate::graph::{
    RelationSelector, TechnicalNodeSelector, TechnicalRelationSelector, ThoughtSelector,
};

use super::store;
use super::trace::{ActorTrace, TraceAction, TraceNode};

pub(crate) struct SubscriptionSupervisor {
    post_commit_count: u64,
    events: Vec<SubscriptionEvent>,
    store: Option<ActorRef<store::StoreSupervisor>>,
}

#[derive(Clone, Default)]
pub(crate) struct Arguments {
    pub post_commit_count: u64,
}

#[allow(dead_code)]
pub struct PublishPostCommit {
    pub trace: ActorTrace,
}

pub(super) struct BindStore {
    store: ActorRef<store::StoreSupervisor>,
}

pub(crate) struct PublishThoughtDelta {
    subscription: SubscriptionIdentifier,
    filter: ThoughtFilter,
    thought: Thought,
}

pub(crate) struct PublishRelationDelta {
    subscription: SubscriptionIdentifier,
    filter: RelationFilter,
    relation: Relation,
}

pub(crate) struct PublishTechnicalNodeDelta {
    subscription: SubscriptionIdentifier,
    filter: TechnicalNodeFilter,
    node: TechnicalNode,
}

pub(crate) struct PublishTechnicalRelationDelta {
    subscription: SubscriptionIdentifier,
    filter: TechnicalRelationFilter,
    relation: TechnicalRelation,
}

pub struct ReadSubscriptionEvents {
    limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, kameo::Reply)]
pub struct SubscriptionPublishReceipt {
    published_count: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, kameo::Reply)]
pub(crate) struct StoreBindReceipt {
    bound: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, kameo::Reply)]
pub struct SubscriptionEventLog {
    events: Vec<SubscriptionEvent>,
}

impl SubscriptionSupervisor {
    fn new(arguments: Arguments) -> Self {
        Self {
            post_commit_count: arguments.post_commit_count,
            events: Vec::new(),
            store: None,
        }
    }

    fn publish(&mut self, event: SubscriptionEvent) -> SubscriptionPublishReceipt {
        self.post_commit_count += 1;
        self.events.push(event);
        SubscriptionPublishReceipt::new(self.post_commit_count)
    }

    fn bind_store(&mut self, store: ActorRef<store::StoreSupervisor>) -> StoreBindReceipt {
        self.store = Some(store);
        StoreBindReceipt::bound()
    }

    async fn publish_thought(
        &mut self,
        message: PublishThoughtDelta,
    ) -> SubscriptionPublishReceipt {
        let Some(store) = &self.store else {
            return SubscriptionPublishReceipt::new(self.post_commit_count);
        };
        let Ok(records) = store.ask(store::ReadGraphRecords).await else {
            return SubscriptionPublishReceipt::new(self.post_commit_count);
        };
        let selector = ThoughtSelector::new(message.filter, records.relations);
        if selector.accepts(&message.thought) {
            self.publish(SubscriptionEvent {
                subscription: message.subscription,
                delta: MindDelta::ThoughtCommitted(message.thought),
            })
        } else {
            SubscriptionPublishReceipt::new(self.post_commit_count)
        }
    }

    fn publish_relation(&mut self, message: PublishRelationDelta) -> SubscriptionPublishReceipt {
        let selector = RelationSelector::new(message.filter);
        if selector.accepts(&message.relation) {
            self.publish(SubscriptionEvent {
                subscription: message.subscription,
                delta: MindDelta::RelationCommitted(message.relation),
            })
        } else {
            SubscriptionPublishReceipt::new(self.post_commit_count)
        }
    }

    fn publish_technical_node(
        &mut self,
        message: PublishTechnicalNodeDelta,
    ) -> SubscriptionPublishReceipt {
        let selector = TechnicalNodeSelector::new(message.filter);
        if selector.accepts(&message.node) {
            self.publish(SubscriptionEvent {
                subscription: message.subscription,
                delta: MindDelta::TechnicalNodeCommitted(message.node),
            })
        } else {
            SubscriptionPublishReceipt::new(self.post_commit_count)
        }
    }

    fn publish_technical_relation(
        &mut self,
        message: PublishTechnicalRelationDelta,
    ) -> SubscriptionPublishReceipt {
        let selector = TechnicalRelationSelector::new(message.filter);
        if selector.accepts(&message.relation) {
            self.publish(SubscriptionEvent {
                subscription: message.subscription,
                delta: MindDelta::TechnicalRelationCommitted(message.relation),
            })
        } else {
            SubscriptionPublishReceipt::new(self.post_commit_count)
        }
    }

    fn event_log(&self, request: ReadSubscriptionEvents) -> SubscriptionEventLog {
        SubscriptionEventLog::new(self.events.iter().take(request.limit()).cloned().collect())
    }
}

impl BindStore {
    pub(super) fn new(store: ActorRef<store::StoreSupervisor>) -> Self {
        Self { store }
    }
}

impl PublishThoughtDelta {
    pub(crate) fn new(
        subscription: SubscriptionIdentifier,
        filter: ThoughtFilter,
        thought: Thought,
    ) -> Self {
        Self {
            subscription,
            filter,
            thought,
        }
    }
}

impl PublishRelationDelta {
    pub(crate) fn new(
        subscription: SubscriptionIdentifier,
        filter: RelationFilter,
        relation: Relation,
    ) -> Self {
        Self {
            subscription,
            filter,
            relation,
        }
    }
}

impl PublishTechnicalNodeDelta {
    pub(crate) fn new(
        subscription: SubscriptionIdentifier,
        filter: TechnicalNodeFilter,
        node: TechnicalNode,
    ) -> Self {
        Self {
            subscription,
            filter,
            node,
        }
    }
}

impl PublishTechnicalRelationDelta {
    pub(crate) fn new(
        subscription: SubscriptionIdentifier,
        filter: TechnicalRelationFilter,
        relation: TechnicalRelation,
    ) -> Self {
        Self {
            subscription,
            filter,
            relation,
        }
    }
}

impl ReadSubscriptionEvents {
    pub fn all() -> Self {
        Self { limit: usize::MAX }
    }

    fn limit(&self) -> usize {
        self.limit
    }
}

impl SubscriptionPublishReceipt {
    fn new(published_count: u64) -> Self {
        Self { published_count }
    }
}

impl StoreBindReceipt {
    fn bound() -> Self {
        Self { bound: true }
    }

    pub(super) fn is_bound(&self) -> bool {
        self.bound
    }
}

impl SubscriptionEventLog {
    fn new(events: Vec<SubscriptionEvent>) -> Self {
        Self { events }
    }

    pub fn empty() -> Self {
        Self::new(Vec::new())
    }

    pub fn events(&self) -> &[SubscriptionEvent] {
        &self.events
    }
}

impl Actor for SubscriptionSupervisor {
    type Args = Arguments;
    type Error = Infallible;

    async fn on_start(
        arguments: Self::Args,
        _actor_reference: ActorRef<Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self::new(arguments))
    }
}

impl Message<PublishPostCommit> for SubscriptionSupervisor {
    type Reply = ActorTrace;

    async fn handle(
        &mut self,
        message: PublishPostCommit,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.post_commit_count += 1;
        let mut trace = message.trace;
        trace.record(
            TraceNode::SUBSCRIPTION_SUPERVISOR,
            TraceAction::MessageReceived,
        );
        trace.record(TraceNode::COMMIT_BUS, TraceAction::MessageReceived);
        trace
    }
}

impl Message<BindStore> for SubscriptionSupervisor {
    type Reply = StoreBindReceipt;

    async fn handle(
        &mut self,
        message: BindStore,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.bind_store(message.store)
    }
}

impl Message<PublishThoughtDelta> for SubscriptionSupervisor {
    type Reply = SubscriptionPublishReceipt;

    async fn handle(
        &mut self,
        message: PublishThoughtDelta,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.publish_thought(message).await
    }
}

impl Message<PublishRelationDelta> for SubscriptionSupervisor {
    type Reply = SubscriptionPublishReceipt;

    async fn handle(
        &mut self,
        message: PublishRelationDelta,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.publish_relation(message)
    }
}

impl Message<PublishTechnicalNodeDelta> for SubscriptionSupervisor {
    type Reply = SubscriptionPublishReceipt;

    async fn handle(
        &mut self,
        message: PublishTechnicalNodeDelta,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.publish_technical_node(message)
    }
}

impl Message<PublishTechnicalRelationDelta> for SubscriptionSupervisor {
    type Reply = SubscriptionPublishReceipt;

    async fn handle(
        &mut self,
        message: PublishTechnicalRelationDelta,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.publish_technical_relation(message)
    }
}

impl Message<ReadSubscriptionEvents> for SubscriptionSupervisor {
    type Reply = SubscriptionEventLog;

    async fn handle(
        &mut self,
        message: ReadSubscriptionEvents,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.event_log(message)
    }
}
