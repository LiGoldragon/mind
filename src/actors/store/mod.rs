mod graph;
mod kernel;
mod memory;
mod persistence;
mod write_trace;

use kameo::actor::{Actor, ActorRef, Spawn, WeakActorRef};
use kameo::error::ActorStopReason;
use kameo::message::{Context, Message};

use crate::tables::RuntimeSubscriptionRegistration;
use crate::{MindEnvelope, StoreLocation};

use super::pipeline::PipelineReply;
use super::trace::{ActorTrace, TraceAction, TraceNode};
use graph::GraphStore;
use kernel::{LoadMemoryGraph, ShutdownKernel, StoreKernel};
use memory::MemoryStore;
use persistence::PersistenceRejection;

#[derive(Clone)]
pub(super) struct Arguments {
    pub(super) store: StoreLocation,
    pub(super) subscription: ActorRef<super::subscription::SubscriptionSupervisor>,
}

pub(super) struct StoreSupervisor {
    kernel: ActorRef<StoreKernel>,
    memory: ActorRef<MemoryStore>,
    graph: ActorRef<GraphStore>,
}

pub struct ApplyMemory {
    pub envelope: MindEnvelope,
    pub trace: ActorTrace,
}

pub struct ReadMemory {
    pub envelope: MindEnvelope,
    pub trace: ActorTrace,
}

pub struct SubmitThought {
    pub envelope: MindEnvelope,
    pub trace: ActorTrace,
}

pub struct SubmitRelation {
    pub envelope: MindEnvelope,
    pub trace: ActorTrace,
}

pub struct SubmitTechnicalNode {
    pub envelope: MindEnvelope,
    pub trace: ActorTrace,
}

pub struct SubmitTechnicalRelation {
    pub envelope: MindEnvelope,
    pub trace: ActorTrace,
}

pub struct QueryThoughts {
    pub envelope: MindEnvelope,
    pub trace: ActorTrace,
}

pub struct QueryRelations {
    pub envelope: MindEnvelope,
    pub trace: ActorTrace,
}

pub struct QueryTechnicalNodes {
    pub envelope: MindEnvelope,
    pub trace: ActorTrace,
}

pub struct QueryTechnicalRelations {
    pub envelope: MindEnvelope,
    pub trace: ActorTrace,
}

pub struct SubscribeThoughts {
    pub envelope: MindEnvelope,
    pub trace: ActorTrace,
}

pub struct SubscribeRelations {
    pub envelope: MindEnvelope,
    pub trace: ActorTrace,
}

pub struct SubscribeTechnicalNodes {
    pub envelope: MindEnvelope,
    pub trace: ActorTrace,
}

pub struct SubscribeTechnicalRelations {
    pub envelope: MindEnvelope,
    pub trace: ActorTrace,
}

pub(super) struct ReadGraphRecords;

pub(super) struct ReadSubscriptionRegistrations;

pub(crate) struct RetractSubscription {
    pub(crate) subscription: signal_mind::SubscriptionIdentifier,
}

pub(super) struct ShutdownStore;

#[derive(kameo::Reply)]
pub(super) struct GraphRecords {
    pub(super) relations: Vec<signal_mind::Relation>,
}

#[derive(kameo::Reply)]
pub(super) struct SubscriptionRegistrations {
    registrations: Vec<RuntimeSubscriptionRegistration>,
}

impl SubscriptionRegistrations {
    pub(super) fn new(registrations: Vec<RuntimeSubscriptionRegistration>) -> Self {
        Self { registrations }
    }

    pub(super) fn into_registrations(self) -> Vec<RuntimeSubscriptionRegistration> {
        self.registrations
    }
}

impl StoreSupervisor {
    fn new(
        kernel: ActorRef<StoreKernel>,
        memory: ActorRef<MemoryStore>,
        graph: ActorRef<GraphStore>,
    ) -> Self {
        Self {
            kernel,
            memory,
            graph,
        }
    }

    async fn apply_memory(
        &self,
        envelope: MindEnvelope,
        mut trace: ActorTrace,
    ) -> crate::Result<PipelineReply> {
        trace.record(TraceNode::STORE_SUPERVISOR, TraceAction::MessageReceived);
        self.memory
            .ask(memory::Apply { envelope, trace })
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
    }

    async fn read_memory(
        &self,
        envelope: MindEnvelope,
        mut trace: ActorTrace,
    ) -> crate::Result<PipelineReply> {
        trace.record(TraceNode::STORE_SUPERVISOR, TraceAction::MessageReceived);
        self.memory
            .ask(memory::Read { envelope, trace })
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
    }

    async fn submit_thought(
        &self,
        envelope: MindEnvelope,
        mut trace: ActorTrace,
    ) -> crate::Result<PipelineReply> {
        trace.record(TraceNode::STORE_SUPERVISOR, TraceAction::MessageReceived);
        self.graph
            .ask(graph::SubmitThought { envelope, trace })
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
    }

    async fn submit_relation(
        &self,
        envelope: MindEnvelope,
        mut trace: ActorTrace,
    ) -> crate::Result<PipelineReply> {
        trace.record(TraceNode::STORE_SUPERVISOR, TraceAction::MessageReceived);
        self.graph
            .ask(graph::SubmitRelation { envelope, trace })
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
    }

    async fn submit_technical_node(
        &self,
        envelope: MindEnvelope,
        mut trace: ActorTrace,
    ) -> crate::Result<PipelineReply> {
        trace.record(TraceNode::STORE_SUPERVISOR, TraceAction::MessageReceived);
        self.graph
            .ask(graph::SubmitTechnicalNode { envelope, trace })
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
    }

    async fn submit_technical_relation(
        &self,
        envelope: MindEnvelope,
        mut trace: ActorTrace,
    ) -> crate::Result<PipelineReply> {
        trace.record(TraceNode::STORE_SUPERVISOR, TraceAction::MessageReceived);
        self.graph
            .ask(graph::SubmitTechnicalRelation { envelope, trace })
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
    }

    async fn query_thoughts(
        &self,
        envelope: MindEnvelope,
        mut trace: ActorTrace,
    ) -> crate::Result<PipelineReply> {
        trace.record(TraceNode::STORE_SUPERVISOR, TraceAction::MessageReceived);
        self.graph
            .ask(graph::QueryThoughts { envelope, trace })
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
    }

    async fn query_technical_nodes(
        &self,
        envelope: MindEnvelope,
        mut trace: ActorTrace,
    ) -> crate::Result<PipelineReply> {
        trace.record(TraceNode::STORE_SUPERVISOR, TraceAction::MessageReceived);
        self.graph
            .ask(graph::QueryTechnicalNodes { envelope, trace })
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
    }

    async fn query_technical_relations(
        &self,
        envelope: MindEnvelope,
        mut trace: ActorTrace,
    ) -> crate::Result<PipelineReply> {
        trace.record(TraceNode::STORE_SUPERVISOR, TraceAction::MessageReceived);
        self.graph
            .ask(graph::QueryTechnicalRelations { envelope, trace })
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
    }

    async fn query_relations(
        &self,
        envelope: MindEnvelope,
        mut trace: ActorTrace,
    ) -> crate::Result<PipelineReply> {
        trace.record(TraceNode::STORE_SUPERVISOR, TraceAction::MessageReceived);
        self.graph
            .ask(graph::QueryRelations { envelope, trace })
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
    }

    async fn subscribe_thoughts(
        &self,
        envelope: MindEnvelope,
        mut trace: ActorTrace,
    ) -> crate::Result<PipelineReply> {
        trace.record(TraceNode::STORE_SUPERVISOR, TraceAction::MessageReceived);
        self.graph
            .ask(graph::OpenThoughtSubscription { envelope, trace })
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
    }

    async fn subscribe_relations(
        &self,
        envelope: MindEnvelope,
        mut trace: ActorTrace,
    ) -> crate::Result<PipelineReply> {
        trace.record(TraceNode::STORE_SUPERVISOR, TraceAction::MessageReceived);
        self.graph
            .ask(graph::OpenRelationSubscription { envelope, trace })
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
    }

    async fn subscribe_technical_nodes(
        &self,
        envelope: MindEnvelope,
        mut trace: ActorTrace,
    ) -> crate::Result<PipelineReply> {
        trace.record(TraceNode::STORE_SUPERVISOR, TraceAction::MessageReceived);
        self.graph
            .ask(graph::OpenTechnicalNodeSubscription { envelope, trace })
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
    }

    async fn subscribe_technical_relations(
        &self,
        envelope: MindEnvelope,
        mut trace: ActorTrace,
    ) -> crate::Result<PipelineReply> {
        trace.record(TraceNode::STORE_SUPERVISOR, TraceAction::MessageReceived);
        self.graph
            .ask(graph::OpenTechnicalRelationSubscription { envelope, trace })
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
    }

    async fn read_graph_records(&self) -> crate::Result<GraphRecords> {
        self.graph
            .ask(graph::ReadGraphRecords)
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
    }

    async fn read_subscription_registrations(&self) -> crate::Result<SubscriptionRegistrations> {
        self.kernel
            .ask(kernel::ReadSubscriptionRegistrations)
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
    }

    async fn retract_subscription(
        &self,
        subscription: signal_mind::SubscriptionIdentifier,
    ) -> crate::Result<bool> {
        self.graph
            .ask(graph::RetractSubscription { subscription })
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
    }

    async fn request_stop_child<Child>(child: &ActorRef<Child>)
    where
        Child: Actor,
    {
        let _ = child.stop_gracefully().await;
    }

    async fn stop_children(&mut self) {
        let _ = self.kernel.ask(ShutdownKernel).await;
        Self::request_stop_child(&self.memory).await;
        Self::request_stop_child(&self.graph).await;
        Self::request_stop_child(&self.kernel).await;
    }
}

impl Actor for StoreSupervisor {
    type Args = Arguments;
    type Error = crate::Error;

    async fn on_start(
        arguments: Self::Args,
        actor_reference: ActorRef<Self>,
    ) -> Result<Self, Self::Error> {
        // `StoreKernel` owns the durable store handle. Under Kameo 0.20 a
        // supervised state-bearing actor must stay on the normal spawn path so
        // terminal notification does not race the actor state's drop.
        let kernel = StoreKernel::supervise(
            &actor_reference,
            kernel::Arguments {
                store: arguments.store.clone(),
                subscription: arguments.subscription.clone(),
            },
        )
        .spawn()
        .await;
        let graph = kernel
            .ask(LoadMemoryGraph)
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))?;
        let memory = MemoryStore::supervise(
            &actor_reference,
            memory::Arguments {
                store: arguments.store,
                graph,
                kernel: kernel.clone(),
            },
        )
        .spawn()
        .await;
        let graph = GraphStore::supervise(
            &actor_reference,
            graph::Arguments {
                kernel: kernel.clone(),
            },
        )
        .spawn()
        .await;

        Ok(Self::new(kernel, memory, graph))
    }

    async fn on_stop(
        &mut self,
        _actor_reference: WeakActorRef<Self>,
        _reason: ActorStopReason,
    ) -> Result<(), Self::Error> {
        self.stop_children().await;
        Ok(())
    }
}

impl Message<ReadGraphRecords> for StoreSupervisor {
    type Reply = GraphRecords;

    async fn handle(
        &mut self,
        _message: ReadGraphRecords,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.read_graph_records()
            .await
            .unwrap_or_else(|_| GraphRecords {
                relations: Vec::new(),
            })
    }
}

impl Message<ReadSubscriptionRegistrations> for StoreSupervisor {
    type Reply = SubscriptionRegistrations;

    async fn handle(
        &mut self,
        _message: ReadSubscriptionRegistrations,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.read_subscription_registrations()
            .await
            .unwrap_or_else(|_| SubscriptionRegistrations::new(Vec::new()))
    }
}

impl Message<ShutdownStore> for StoreSupervisor {
    type Reply = bool;

    async fn handle(
        &mut self,
        _message: ShutdownStore,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.stop_children().await;
        true
    }
}

impl Message<RetractSubscription> for StoreSupervisor {
    type Reply = bool;

    async fn handle(
        &mut self,
        message: RetractSubscription,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.retract_subscription(message.subscription)
            .await
            .unwrap_or(false)
    }
}

impl Message<ApplyMemory> for StoreSupervisor {
    type Reply = PipelineReply;

    async fn handle(
        &mut self,
        message: ApplyMemory,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.apply_memory(message.envelope, message.trace)
            .await
            .unwrap_or_else(PersistenceRejection::pipeline)
    }
}

impl Message<ReadMemory> for StoreSupervisor {
    type Reply = PipelineReply;

    async fn handle(
        &mut self,
        message: ReadMemory,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.read_memory(message.envelope, message.trace)
            .await
            .unwrap_or_else(PersistenceRejection::pipeline)
    }
}

impl Message<SubmitThought> for StoreSupervisor {
    type Reply = PipelineReply;

    async fn handle(
        &mut self,
        message: SubmitThought,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.submit_thought(message.envelope, message.trace)
            .await
            .unwrap_or_else(PersistenceRejection::pipeline)
    }
}

impl Message<SubmitRelation> for StoreSupervisor {
    type Reply = PipelineReply;

    async fn handle(
        &mut self,
        message: SubmitRelation,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.submit_relation(message.envelope, message.trace)
            .await
            .unwrap_or_else(PersistenceRejection::pipeline)
    }
}

impl Message<SubmitTechnicalNode> for StoreSupervisor {
    type Reply = PipelineReply;

    async fn handle(
        &mut self,
        message: SubmitTechnicalNode,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.submit_technical_node(message.envelope, message.trace)
            .await
            .unwrap_or_else(PersistenceRejection::pipeline)
    }
}

impl Message<SubmitTechnicalRelation> for StoreSupervisor {
    type Reply = PipelineReply;

    async fn handle(
        &mut self,
        message: SubmitTechnicalRelation,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.submit_technical_relation(message.envelope, message.trace)
            .await
            .unwrap_or_else(PersistenceRejection::pipeline)
    }
}

impl Message<QueryThoughts> for StoreSupervisor {
    type Reply = PipelineReply;

    async fn handle(
        &mut self,
        message: QueryThoughts,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.query_thoughts(message.envelope, message.trace)
            .await
            .unwrap_or_else(PersistenceRejection::pipeline)
    }
}

impl Message<QueryRelations> for StoreSupervisor {
    type Reply = PipelineReply;

    async fn handle(
        &mut self,
        message: QueryRelations,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.query_relations(message.envelope, message.trace)
            .await
            .unwrap_or_else(PersistenceRejection::pipeline)
    }
}

impl Message<QueryTechnicalNodes> for StoreSupervisor {
    type Reply = PipelineReply;

    async fn handle(
        &mut self,
        message: QueryTechnicalNodes,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.query_technical_nodes(message.envelope, message.trace)
            .await
            .unwrap_or_else(PersistenceRejection::pipeline)
    }
}

impl Message<QueryTechnicalRelations> for StoreSupervisor {
    type Reply = PipelineReply;

    async fn handle(
        &mut self,
        message: QueryTechnicalRelations,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.query_technical_relations(message.envelope, message.trace)
            .await
            .unwrap_or_else(PersistenceRejection::pipeline)
    }
}

impl Message<SubscribeThoughts> for StoreSupervisor {
    type Reply = PipelineReply;

    async fn handle(
        &mut self,
        message: SubscribeThoughts,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.subscribe_thoughts(message.envelope, message.trace)
            .await
            .unwrap_or_else(PersistenceRejection::pipeline)
    }
}

impl Message<SubscribeRelations> for StoreSupervisor {
    type Reply = PipelineReply;

    async fn handle(
        &mut self,
        message: SubscribeRelations,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.subscribe_relations(message.envelope, message.trace)
            .await
            .unwrap_or_else(PersistenceRejection::pipeline)
    }
}

impl Message<SubscribeTechnicalNodes> for StoreSupervisor {
    type Reply = PipelineReply;

    async fn handle(
        &mut self,
        message: SubscribeTechnicalNodes,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.subscribe_technical_nodes(message.envelope, message.trace)
            .await
            .unwrap_or_else(PersistenceRejection::pipeline)
    }
}

impl Message<SubscribeTechnicalRelations> for StoreSupervisor {
    type Reply = PipelineReply;

    async fn handle(
        &mut self,
        message: SubscribeTechnicalRelations,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.subscribe_technical_relations(message.envelope, message.trace)
            .await
            .unwrap_or_else(PersistenceRejection::pipeline)
    }
}
