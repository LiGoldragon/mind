mod activity;
mod claims;
mod graph;
mod kernel;
mod memory;
mod persistence;
mod write_trace;

use kameo::actor::{Actor, ActorRef, Spawn};
use kameo::message::{Context, Message};

use crate::{MindEnvelope, StoreLocation};

use super::pipeline::PipelineReply;
use super::trace::{ActorTrace, TraceAction, TraceNode};
use activity::ActivityStore;
use claims::ClaimStore;
use graph::GraphStore;
use kernel::{LoadMemoryGraph, StoreKernel};
use memory::MemoryStore;
use persistence::PersistenceRejection;

#[derive(Clone)]
pub(super) struct Arguments {
    pub(super) store: StoreLocation,
    pub(super) subscription: ActorRef<super::subscription::SubscriptionSupervisor>,
}

pub(super) struct StoreSupervisor {
    memory: ActorRef<MemoryStore>,
    claims: ActorRef<ClaimStore>,
    activity: ActorRef<ActivityStore>,
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

pub struct ApplyClaim {
    pub envelope: MindEnvelope,
    pub trace: ActorTrace,
}

pub struct ApplyHandoff {
    pub envelope: MindEnvelope,
    pub trace: ActorTrace,
}

pub struct ReadClaims {
    pub envelope: MindEnvelope,
    pub trace: ActorTrace,
}

pub struct ApplyActivity {
    pub envelope: MindEnvelope,
    pub trace: ActorTrace,
}

pub struct ReadActivity {
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

pub struct QueryThoughts {
    pub envelope: MindEnvelope,
    pub trace: ActorTrace,
}

pub struct QueryRelations {
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

pub(super) struct ReadGraphRecords;

#[derive(kameo::Reply)]
pub(super) struct GraphRecords {
    pub(super) relations: Vec<signal_persona_mind::Relation>,
}

impl StoreSupervisor {
    fn new(
        memory: ActorRef<MemoryStore>,
        claims: ActorRef<ClaimStore>,
        activity: ActorRef<ActivityStore>,
        graph: ActorRef<GraphStore>,
    ) -> Self {
        Self {
            memory,
            claims,
            activity,
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

    async fn apply_claim(
        &self,
        envelope: MindEnvelope,
        mut trace: ActorTrace,
    ) -> crate::Result<PipelineReply> {
        trace.record(TraceNode::STORE_SUPERVISOR, TraceAction::MessageReceived);
        self.claims
            .ask(claims::Apply { envelope, trace })
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
    }

    async fn apply_handoff(
        &self,
        envelope: MindEnvelope,
        mut trace: ActorTrace,
    ) -> crate::Result<PipelineReply> {
        trace.record(TraceNode::STORE_SUPERVISOR, TraceAction::MessageReceived);
        self.claims
            .ask(claims::ApplyHandoffRequest { envelope, trace })
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
    }

    async fn read_claims(
        &self,
        envelope: MindEnvelope,
        mut trace: ActorTrace,
    ) -> crate::Result<PipelineReply> {
        trace.record(TraceNode::STORE_SUPERVISOR, TraceAction::MessageReceived);
        self.claims
            .ask(claims::Read { envelope, trace })
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
    }

    async fn apply_activity(
        &self,
        envelope: MindEnvelope,
        mut trace: ActorTrace,
    ) -> crate::Result<PipelineReply> {
        trace.record(TraceNode::STORE_SUPERVISOR, TraceAction::MessageReceived);
        self.activity
            .ask(activity::Apply { envelope, trace })
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
    }

    async fn read_activity(
        &self,
        envelope: MindEnvelope,
        mut trace: ActorTrace,
    ) -> crate::Result<PipelineReply> {
        trace.record(TraceNode::STORE_SUPERVISOR, TraceAction::MessageReceived);
        self.activity
            .ask(activity::Read { envelope, trace })
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

    async fn read_graph_records(&self) -> crate::Result<GraphRecords> {
        self.graph
            .ask(graph::ReadGraphRecords)
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
    }
}

impl Actor for StoreSupervisor {
    type Args = Arguments;
    type Error = crate::Error;

    async fn on_start(
        arguments: Self::Args,
        actor_reference: ActorRef<Self>,
    ) -> Result<Self, Self::Error> {
        // `StoreKernel` performs synchronous redb/sema-engine transactions on every
        // message, so it runs on its own OS thread. The forked Kameo lifecycle
        // contract makes supervised replacement wait until the old actor state has
        // dropped, which releases the redb handle before a replacement opens the
        // same store path.
        let kernel = StoreKernel::supervise(
            &actor_reference,
            kernel::Arguments {
                store: arguments.store.clone(),
                subscription: arguments.subscription.clone(),
            },
        )
        .spawn_in_thread()
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
        let claims = ClaimStore::supervise(
            &actor_reference,
            claims::Arguments {
                kernel: kernel.clone(),
            },
        )
        .spawn()
        .await;
        let activity = ActivityStore::supervise(
            &actor_reference,
            activity::Arguments {
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

        Ok(Self::new(memory, claims, activity, graph))
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

impl Message<ApplyClaim> for StoreSupervisor {
    type Reply = PipelineReply;

    async fn handle(
        &mut self,
        message: ApplyClaim,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.apply_claim(message.envelope, message.trace)
            .await
            .unwrap_or_else(PersistenceRejection::pipeline)
    }
}

impl Message<ApplyHandoff> for StoreSupervisor {
    type Reply = PipelineReply;

    async fn handle(
        &mut self,
        message: ApplyHandoff,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.apply_handoff(message.envelope, message.trace)
            .await
            .unwrap_or_else(PersistenceRejection::pipeline)
    }
}

impl Message<ReadClaims> for StoreSupervisor {
    type Reply = PipelineReply;

    async fn handle(
        &mut self,
        message: ReadClaims,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.read_claims(message.envelope, message.trace)
            .await
            .unwrap_or_else(PersistenceRejection::pipeline)
    }
}

impl Message<ApplyActivity> for StoreSupervisor {
    type Reply = PipelineReply;

    async fn handle(
        &mut self,
        message: ApplyActivity,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.apply_activity(message.envelope, message.trace)
            .await
            .unwrap_or_else(PersistenceRejection::pipeline)
    }
}

impl Message<ReadActivity> for StoreSupervisor {
    type Reply = PipelineReply;

    async fn handle(
        &mut self,
        message: ReadActivity,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.read_activity(message.envelope, message.trace)
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
