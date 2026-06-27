use std::collections::{HashMap, VecDeque};

use kameo::actor::{Actor, ActorRef};
use kameo::error::Infallible;
use kameo::message::{Context, Message};
use signal_mind::{
    MindReply, MindRequestUnimplemented, MindUnimplementedReason, Relation, RelationFilter,
    RelationSubscriptionEvent, SubscriptionBufferBound, SubscriptionCursor, SubscriptionDemand,
    SubscriptionDemandAccepted, SubscriptionDemandCredit, SubscriptionEvent,
    SubscriptionIdentifier, SubscriptionRetracted, SubscriptionStreamEvent, SubscriptionStreamKind,
    TechnicalNode, TechnicalNodeFilter, TechnicalNodeSubscriptionEvent, TechnicalRelation,
    TechnicalRelationFilter, TechnicalRelationSubscriptionEvent, Thought, ThoughtFilter,
    ThoughtSubscriptionEvent,
};

use crate::graph::{
    RelationSelector, TechnicalNodeSelector, TechnicalRelationSelector, ThoughtSelector,
};

use super::store;
use super::trace::{ActorTrace, TraceAction, TraceNode};

pub(crate) struct SubscriptionSupervisor {
    post_commit_count: u64,
    subscriptions: HashMap<SubscriptionIdentifier, RuntimeSubscription>,
    delivered_events: Vec<SubscriptionEvent>,
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
    thought: Thought,
}

pub(crate) struct PublishRelationDelta {
    subscription: SubscriptionIdentifier,
    relation: Relation,
}

pub(crate) struct PublishTechnicalNodeDelta {
    subscription: SubscriptionIdentifier,
    node: TechnicalNode,
}

pub(crate) struct PublishTechnicalRelationDelta {
    subscription: SubscriptionIdentifier,
    relation: TechnicalRelation,
}

pub struct ReadSubscriptionEvents {
    limit: usize,
}

pub(crate) struct RegisterThoughtSubscription {
    subscription: SubscriptionIdentifier,
    filter: ThoughtFilter,
    cursor: SubscriptionCursor,
    initial_demand: SubscriptionDemandCredit,
}

pub(crate) struct RegisterRelationSubscription {
    subscription: SubscriptionIdentifier,
    filter: RelationFilter,
    cursor: SubscriptionCursor,
    initial_demand: SubscriptionDemandCredit,
}

pub(crate) struct RegisterTechnicalNodeSubscription {
    subscription: SubscriptionIdentifier,
    filter: TechnicalNodeFilter,
    cursor: SubscriptionCursor,
    initial_demand: SubscriptionDemandCredit,
}

pub(crate) struct RegisterTechnicalRelationSubscription {
    subscription: SubscriptionIdentifier,
    filter: TechnicalRelationFilter,
    cursor: SubscriptionCursor,
    initial_demand: SubscriptionDemandCredit,
}

pub(crate) struct AcceptSubscriptionDemand {
    demand: SubscriptionDemand,
}

pub(crate) struct RetractSubscription {
    subscription: SubscriptionIdentifier,
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

#[derive(Debug, Clone, PartialEq, Eq, kameo::Reply)]
pub(crate) struct SubscriptionLifecycleReply {
    reply: MindReply,
}

impl SubscriptionSupervisor {
    pub(crate) const BUFFER_BOUND: SubscriptionBufferBound = SubscriptionBufferBound::new(64);

    fn new(arguments: Arguments) -> Self {
        Self {
            post_commit_count: arguments.post_commit_count,
            subscriptions: HashMap::new(),
            delivered_events: Vec::new(),
            store: None,
        }
    }

    fn publish(&mut self, event: SubscriptionEvent) -> SubscriptionPublishReceipt {
        self.post_commit_count += 1;
        self.delivered_events.push(event);
        SubscriptionPublishReceipt::new(self.post_commit_count)
    }

    fn register(&mut self, subscription: RuntimeSubscription) {
        self.subscriptions
            .insert(subscription.subscription.clone(), subscription);
    }

    fn bind_store(&mut self, store: ActorRef<store::StoreSupervisor>) -> StoreBindReceipt {
        self.store = Some(store);
        StoreBindReceipt::bound()
    }

    async fn publish_thought(
        &mut self,
        message: PublishThoughtDelta,
    ) -> SubscriptionPublishReceipt {
        let Some(subscription) = self.subscriptions.get(&message.subscription) else {
            return SubscriptionPublishReceipt::new(self.post_commit_count);
        };
        let Some(filter) = subscription.thought_filter() else {
            return SubscriptionPublishReceipt::new(self.post_commit_count);
        };
        let Some(store) = &self.store else {
            return SubscriptionPublishReceipt::new(self.post_commit_count);
        };
        let Ok(records) = store.ask(store::ReadGraphRecords).await else {
            return SubscriptionPublishReceipt::new(self.post_commit_count);
        };
        let selector = ThoughtSelector::new(filter.clone(), records.relations);
        if selector.accepts(&message.thought) {
            self.publish_for_subscription(
                message.subscription,
                SubscriptionPayload::Thought(message.thought),
            )
        } else {
            SubscriptionPublishReceipt::new(self.post_commit_count)
        }
    }

    fn publish_relation(&mut self, message: PublishRelationDelta) -> SubscriptionPublishReceipt {
        let Some(subscription) = self.subscriptions.get(&message.subscription) else {
            return SubscriptionPublishReceipt::new(self.post_commit_count);
        };
        let Some(filter) = subscription.relation_filter() else {
            return SubscriptionPublishReceipt::new(self.post_commit_count);
        };
        let selector = RelationSelector::new(filter.clone());
        if selector.accepts(&message.relation) {
            self.publish_for_subscription(
                message.subscription,
                SubscriptionPayload::Relation(message.relation),
            )
        } else {
            SubscriptionPublishReceipt::new(self.post_commit_count)
        }
    }

    fn publish_technical_node(
        &mut self,
        message: PublishTechnicalNodeDelta,
    ) -> SubscriptionPublishReceipt {
        let Some(subscription) = self.subscriptions.get(&message.subscription) else {
            return SubscriptionPublishReceipt::new(self.post_commit_count);
        };
        let Some(filter) = subscription.technical_node_filter() else {
            return SubscriptionPublishReceipt::new(self.post_commit_count);
        };
        let selector = TechnicalNodeSelector::new(filter.clone());
        if selector.accepts(&message.node) {
            self.publish_for_subscription(
                message.subscription,
                SubscriptionPayload::TechnicalNode(message.node),
            )
        } else {
            SubscriptionPublishReceipt::new(self.post_commit_count)
        }
    }

    fn publish_technical_relation(
        &mut self,
        message: PublishTechnicalRelationDelta,
    ) -> SubscriptionPublishReceipt {
        let Some(subscription) = self.subscriptions.get(&message.subscription) else {
            return SubscriptionPublishReceipt::new(self.post_commit_count);
        };
        let Some(filter) = subscription.technical_relation_filter() else {
            return SubscriptionPublishReceipt::new(self.post_commit_count);
        };
        let selector = TechnicalRelationSelector::new(filter.clone());
        if selector.accepts(&message.relation) {
            self.publish_for_subscription(
                message.subscription,
                SubscriptionPayload::TechnicalRelation(message.relation),
            )
        } else {
            SubscriptionPublishReceipt::new(self.post_commit_count)
        }
    }

    fn publish_for_subscription(
        &mut self,
        subscription: SubscriptionIdentifier,
        payload: SubscriptionPayload,
    ) -> SubscriptionPublishReceipt {
        let Some(runtime_subscription) = self.subscriptions.get_mut(&subscription) else {
            return SubscriptionPublishReceipt::new(self.post_commit_count);
        };
        let Some(event) = runtime_subscription.accept_payload(payload) else {
            return SubscriptionPublishReceipt::new(self.post_commit_count);
        };
        self.publish(event)
    }

    fn event_log(&self, request: ReadSubscriptionEvents) -> SubscriptionEventLog {
        SubscriptionEventLog::new(
            self.delivered_events
                .iter()
                .take(request.limit())
                .cloned()
                .collect(),
        )
    }

    fn accept_demand(&mut self, demand: SubscriptionDemand) -> MindReply {
        let accepted = demand.credit;
        let Some(subscription) = self.subscriptions.get_mut(&demand.subscription) else {
            return Self::unimplemented();
        };
        let delivered = subscription.accept_demand(accepted);
        for event in delivered {
            self.publish(event);
        }
        MindReply::SubscriptionDemandAccepted(SubscriptionDemandAccepted {
            subscription: demand.subscription,
            accepted,
        })
    }

    async fn retract_subscription(&mut self, subscription: SubscriptionIdentifier) -> MindReply {
        let Some(runtime_subscription) = self.subscriptions.remove(&subscription) else {
            return Self::unimplemented();
        };
        if let Some(store) = &self.store {
            let _cleanup = store
                .ask(store::RetractSubscription {
                    subscription: subscription.clone(),
                })
                .await;
        }
        MindReply::SubscriptionRetracted(SubscriptionRetracted {
            subscription,
            stream: runtime_subscription.stream,
            last_cursor: runtime_subscription.last_cursor,
        })
    }

    fn unimplemented() -> MindReply {
        MindReply::MindRequestUnimplemented(MindRequestUnimplemented {
            reason: MindUnimplementedReason::NotInPrototypeScope,
        })
    }
}

impl BindStore {
    pub(super) fn new(store: ActorRef<store::StoreSupervisor>) -> Self {
        Self { store }
    }
}

impl PublishThoughtDelta {
    pub(crate) fn new(subscription: SubscriptionIdentifier, thought: Thought) -> Self {
        Self {
            subscription,
            thought,
        }
    }
}

impl PublishRelationDelta {
    pub(crate) fn new(subscription: SubscriptionIdentifier, relation: Relation) -> Self {
        Self {
            subscription,
            relation,
        }
    }
}

impl PublishTechnicalNodeDelta {
    pub(crate) fn new(subscription: SubscriptionIdentifier, node: TechnicalNode) -> Self {
        Self { subscription, node }
    }
}

impl PublishTechnicalRelationDelta {
    pub(crate) fn new(subscription: SubscriptionIdentifier, relation: TechnicalRelation) -> Self {
        Self {
            subscription,
            relation,
        }
    }
}

impl RegisterThoughtSubscription {
    pub(crate) fn new(
        subscription: SubscriptionIdentifier,
        filter: ThoughtFilter,
        cursor: SubscriptionCursor,
        initial_demand: SubscriptionDemandCredit,
    ) -> Self {
        Self {
            subscription,
            filter,
            cursor,
            initial_demand,
        }
    }
}

impl RegisterRelationSubscription {
    pub(crate) fn new(
        subscription: SubscriptionIdentifier,
        filter: RelationFilter,
        cursor: SubscriptionCursor,
        initial_demand: SubscriptionDemandCredit,
    ) -> Self {
        Self {
            subscription,
            filter,
            cursor,
            initial_demand,
        }
    }
}

impl RegisterTechnicalNodeSubscription {
    pub(crate) fn new(
        subscription: SubscriptionIdentifier,
        filter: TechnicalNodeFilter,
        cursor: SubscriptionCursor,
        initial_demand: SubscriptionDemandCredit,
    ) -> Self {
        Self {
            subscription,
            filter,
            cursor,
            initial_demand,
        }
    }
}

impl RegisterTechnicalRelationSubscription {
    pub(crate) fn new(
        subscription: SubscriptionIdentifier,
        filter: TechnicalRelationFilter,
        cursor: SubscriptionCursor,
        initial_demand: SubscriptionDemandCredit,
    ) -> Self {
        Self {
            subscription,
            filter,
            cursor,
            initial_demand,
        }
    }
}

impl AcceptSubscriptionDemand {
    pub(crate) fn new(demand: SubscriptionDemand) -> Self {
        Self { demand }
    }
}

impl RetractSubscription {
    pub(crate) fn new(subscription: SubscriptionIdentifier) -> Self {
        Self { subscription }
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

impl SubscriptionLifecycleReply {
    fn new(reply: MindReply) -> Self {
        Self { reply }
    }

    pub(crate) fn into_reply(self) -> MindReply {
        self.reply
    }
}

enum SubscriptionPayload {
    Thought(Thought),
    Relation(Relation),
    TechnicalNode(TechnicalNode),
    TechnicalRelation(TechnicalRelation),
}

enum RuntimeSubscriptionFilter {
    Thoughts(ThoughtFilter),
    Relations(RelationFilter),
    TechnicalNodes(TechnicalNodeFilter),
    TechnicalRelations(TechnicalRelationFilter),
}

struct RuntimeSubscription {
    subscription: SubscriptionIdentifier,
    stream: SubscriptionStreamKind,
    filter: RuntimeSubscriptionFilter,
    last_cursor: SubscriptionCursor,
    demand: u16,
    pending: VecDeque<SubscriptionStreamEvent>,
    buffer_bound: SubscriptionBufferBound,
    overflowed: bool,
}

impl RuntimeSubscription {
    fn thoughts(
        subscription: SubscriptionIdentifier,
        filter: ThoughtFilter,
        cursor: SubscriptionCursor,
        initial_demand: SubscriptionDemandCredit,
    ) -> Self {
        Self::new(
            subscription,
            SubscriptionStreamKind::Thoughts,
            RuntimeSubscriptionFilter::Thoughts(filter),
            cursor,
            initial_demand,
        )
    }

    fn relations(
        subscription: SubscriptionIdentifier,
        filter: RelationFilter,
        cursor: SubscriptionCursor,
        initial_demand: SubscriptionDemandCredit,
    ) -> Self {
        Self::new(
            subscription,
            SubscriptionStreamKind::Relations,
            RuntimeSubscriptionFilter::Relations(filter),
            cursor,
            initial_demand,
        )
    }

    fn technical_nodes(
        subscription: SubscriptionIdentifier,
        filter: TechnicalNodeFilter,
        cursor: SubscriptionCursor,
        initial_demand: SubscriptionDemandCredit,
    ) -> Self {
        Self::new(
            subscription,
            SubscriptionStreamKind::TechnicalNodes,
            RuntimeSubscriptionFilter::TechnicalNodes(filter),
            cursor,
            initial_demand,
        )
    }

    fn technical_relations(
        subscription: SubscriptionIdentifier,
        filter: TechnicalRelationFilter,
        cursor: SubscriptionCursor,
        initial_demand: SubscriptionDemandCredit,
    ) -> Self {
        Self::new(
            subscription,
            SubscriptionStreamKind::TechnicalRelations,
            RuntimeSubscriptionFilter::TechnicalRelations(filter),
            cursor,
            initial_demand,
        )
    }

    fn new(
        subscription: SubscriptionIdentifier,
        stream: SubscriptionStreamKind,
        filter: RuntimeSubscriptionFilter,
        cursor: SubscriptionCursor,
        initial_demand: SubscriptionDemandCredit,
    ) -> Self {
        Self {
            subscription,
            stream,
            filter,
            last_cursor: cursor,
            demand: initial_demand.into_u16(),
            pending: VecDeque::new(),
            buffer_bound: SubscriptionSupervisor::BUFFER_BOUND,
            overflowed: false,
        }
    }

    fn thought_filter(&self) -> Option<&ThoughtFilter> {
        match &self.filter {
            RuntimeSubscriptionFilter::Thoughts(filter) => Some(filter),
            _ => None,
        }
    }

    fn relation_filter(&self) -> Option<&RelationFilter> {
        match &self.filter {
            RuntimeSubscriptionFilter::Relations(filter) => Some(filter),
            _ => None,
        }
    }

    fn technical_node_filter(&self) -> Option<&TechnicalNodeFilter> {
        match &self.filter {
            RuntimeSubscriptionFilter::TechnicalNodes(filter) => Some(filter),
            _ => None,
        }
    }

    fn technical_relation_filter(&self) -> Option<&TechnicalRelationFilter> {
        match &self.filter {
            RuntimeSubscriptionFilter::TechnicalRelations(filter) => Some(filter),
            _ => None,
        }
    }

    fn accept_payload(&mut self, payload: SubscriptionPayload) -> Option<SubscriptionEvent> {
        if self.overflowed {
            return None;
        }
        let event = self.event_from_payload(payload);
        if self.demand > 0 {
            self.demand -= 1;
            Some(SubscriptionEvent {
                subscription: self.subscription.clone(),
                event,
            })
        } else if self.pending.len() < self.buffer_bound.into_u16() as usize {
            self.pending.push_back(event);
            None
        } else {
            self.overflowed = true;
            None
        }
    }

    fn accept_demand(&mut self, credit: SubscriptionDemandCredit) -> Vec<SubscriptionEvent> {
        self.demand = self.demand.saturating_add(credit.into_u16());
        let mut delivered = Vec::new();
        while self.demand > 0 {
            let Some(event) = self.pending.pop_front() else {
                break;
            };
            self.demand -= 1;
            delivered.push(SubscriptionEvent {
                subscription: self.subscription.clone(),
                event,
            });
        }
        delivered
    }

    fn event_from_payload(&mut self, payload: SubscriptionPayload) -> SubscriptionStreamEvent {
        let cursor = self.next_cursor();
        match payload {
            SubscriptionPayload::Thought(thought) => {
                SubscriptionStreamEvent::ThoughtCommitted(ThoughtSubscriptionEvent {
                    cursor,
                    thought,
                })
            }
            SubscriptionPayload::Relation(relation) => {
                SubscriptionStreamEvent::RelationCommitted(RelationSubscriptionEvent {
                    cursor,
                    relation,
                })
            }
            SubscriptionPayload::TechnicalNode(node) => {
                SubscriptionStreamEvent::TechnicalNodeCommitted(TechnicalNodeSubscriptionEvent {
                    cursor,
                    node,
                })
            }
            SubscriptionPayload::TechnicalRelation(relation) => {
                SubscriptionStreamEvent::TechnicalRelationCommitted(
                    TechnicalRelationSubscriptionEvent { cursor, relation },
                )
            }
        }
    }

    fn next_cursor(&mut self) -> SubscriptionCursor {
        let next = SubscriptionCursor::new(self.last_cursor.into_u64().saturating_add(1));
        self.last_cursor = next;
        next
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

impl Message<RegisterThoughtSubscription> for SubscriptionSupervisor {
    type Reply = ();

    async fn handle(
        &mut self,
        message: RegisterThoughtSubscription,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.register(RuntimeSubscription::thoughts(
            message.subscription,
            message.filter,
            message.cursor,
            message.initial_demand,
        ));
    }
}

impl Message<RegisterRelationSubscription> for SubscriptionSupervisor {
    type Reply = ();

    async fn handle(
        &mut self,
        message: RegisterRelationSubscription,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.register(RuntimeSubscription::relations(
            message.subscription,
            message.filter,
            message.cursor,
            message.initial_demand,
        ));
    }
}

impl Message<RegisterTechnicalNodeSubscription> for SubscriptionSupervisor {
    type Reply = ();

    async fn handle(
        &mut self,
        message: RegisterTechnicalNodeSubscription,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.register(RuntimeSubscription::technical_nodes(
            message.subscription,
            message.filter,
            message.cursor,
            message.initial_demand,
        ));
    }
}

impl Message<RegisterTechnicalRelationSubscription> for SubscriptionSupervisor {
    type Reply = ();

    async fn handle(
        &mut self,
        message: RegisterTechnicalRelationSubscription,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.register(RuntimeSubscription::technical_relations(
            message.subscription,
            message.filter,
            message.cursor,
            message.initial_demand,
        ));
    }
}

impl Message<AcceptSubscriptionDemand> for SubscriptionSupervisor {
    type Reply = SubscriptionLifecycleReply;

    async fn handle(
        &mut self,
        message: AcceptSubscriptionDemand,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        SubscriptionLifecycleReply::new(self.accept_demand(message.demand))
    }
}

impl Message<RetractSubscription> for SubscriptionSupervisor {
    type Reply = SubscriptionLifecycleReply;

    async fn handle(
        &mut self,
        message: RetractSubscription,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        SubscriptionLifecycleReply::new(self.retract_subscription(message.subscription).await)
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
