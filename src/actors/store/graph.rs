use kameo::actor::{Actor, ActorRef};
use kameo::message::{Context, Message};

use crate::MindEnvelope;

use super::kernel::{
    KernelReply, ReadGraphRecords as ReadKernelGraphRecords, ReadRelations, ReadTechnicalNodes,
    ReadTechnicalRelations, ReadThoughts, StoreKernel, SubscribeRelations, SubscribeThoughts,
    WriteRelation, WriteTechnicalNode, WriteTechnicalRelation, WriteThought,
};
use super::persistence::PersistenceRejection;
use super::{ActorTrace, GraphRecords, PipelineReply, TraceAction, TraceNode};

#[derive(Clone)]
pub(super) struct Arguments {
    pub(super) kernel: ActorRef<StoreKernel>,
}

pub(super) struct SubmitThought {
    pub(super) envelope: MindEnvelope,
    pub(super) trace: ActorTrace,
}

pub(super) struct SubmitRelation {
    pub(super) envelope: MindEnvelope,
    pub(super) trace: ActorTrace,
}

pub(super) struct SubmitTechnicalNode {
    pub(super) envelope: MindEnvelope,
    pub(super) trace: ActorTrace,
}

pub(super) struct SubmitTechnicalRelation {
    pub(super) envelope: MindEnvelope,
    pub(super) trace: ActorTrace,
}

pub(super) struct QueryThoughts {
    pub(super) envelope: MindEnvelope,
    pub(super) trace: ActorTrace,
}

pub(super) struct QueryRelations {
    pub(super) envelope: MindEnvelope,
    pub(super) trace: ActorTrace,
}

pub(super) struct QueryTechnicalNodes {
    pub(super) envelope: MindEnvelope,
    pub(super) trace: ActorTrace,
}

pub(super) struct QueryTechnicalRelations {
    pub(super) envelope: MindEnvelope,
    pub(super) trace: ActorTrace,
}

pub(super) struct OpenThoughtSubscription {
    pub(super) envelope: MindEnvelope,
    pub(super) trace: ActorTrace,
}

pub(super) struct OpenRelationSubscription {
    pub(super) envelope: MindEnvelope,
    pub(super) trace: ActorTrace,
}

pub(super) struct ReadGraphRecords;

pub(super) struct GraphStore {
    kernel: ActorRef<StoreKernel>,
}

impl GraphStore {
    fn new(arguments: Arguments) -> Self {
        Self {
            kernel: arguments.kernel,
        }
    }

    async fn submit_thought(&self, envelope: MindEnvelope, mut trace: ActorTrace) -> PipelineReply {
        trace.record(TraceNode::GRAPH_STORE, TraceAction::MessageReceived);
        trace.record(TraceNode::ID_MINT, TraceAction::MessageReceived);
        trace.record(TraceNode::CLOCK, TraceAction::MessageReceived);
        trace.record(TraceNode::SEMA_WRITER, TraceAction::WriteIntentSent);
        let reply = self
            .kernel
            .ask(WriteThought::new(envelope))
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
            .map(KernelReply::into_reply)
            .unwrap_or_else(|error| Some(PersistenceRejection::reply(error)));
        trace.record(TraceNode::EVENT_APPENDER, TraceAction::MessageReceived);
        trace.record(TraceNode::COMMIT, TraceAction::CommitCompleted);
        PipelineReply::new(reply, trace)
    }

    async fn submit_relation(
        &self,
        envelope: MindEnvelope,
        mut trace: ActorTrace,
    ) -> PipelineReply {
        trace.record(TraceNode::GRAPH_STORE, TraceAction::MessageReceived);
        trace.record(TraceNode::ID_MINT, TraceAction::MessageReceived);
        trace.record(TraceNode::CLOCK, TraceAction::MessageReceived);
        trace.record(TraceNode::SEMA_WRITER, TraceAction::WriteIntentSent);
        let reply = self
            .kernel
            .ask(WriteRelation::new(envelope))
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
            .map(KernelReply::into_reply)
            .unwrap_or_else(|error| Some(PersistenceRejection::reply(error)));
        trace.record(TraceNode::EVENT_APPENDER, TraceAction::MessageReceived);
        trace.record(TraceNode::COMMIT, TraceAction::CommitCompleted);
        PipelineReply::new(reply, trace)
    }

    async fn submit_technical_node(
        &self,
        envelope: MindEnvelope,
        mut trace: ActorTrace,
    ) -> PipelineReply {
        trace.record(TraceNode::GRAPH_STORE, TraceAction::MessageReceived);
        trace.record(TraceNode::ID_MINT, TraceAction::MessageReceived);
        trace.record(TraceNode::CLOCK, TraceAction::MessageReceived);
        trace.record(TraceNode::SEMA_WRITER, TraceAction::WriteIntentSent);
        let reply = self
            .kernel
            .ask(WriteTechnicalNode::new(envelope))
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
            .map(KernelReply::into_reply)
            .unwrap_or_else(|error| Some(PersistenceRejection::reply(error)));
        trace.record(TraceNode::EVENT_APPENDER, TraceAction::MessageReceived);
        trace.record(TraceNode::COMMIT, TraceAction::CommitCompleted);
        PipelineReply::new(reply, trace)
    }

    async fn submit_technical_relation(
        &self,
        envelope: MindEnvelope,
        mut trace: ActorTrace,
    ) -> PipelineReply {
        trace.record(TraceNode::GRAPH_STORE, TraceAction::MessageReceived);
        trace.record(TraceNode::ID_MINT, TraceAction::MessageReceived);
        trace.record(TraceNode::CLOCK, TraceAction::MessageReceived);
        trace.record(TraceNode::SEMA_WRITER, TraceAction::WriteIntentSent);
        let reply = self
            .kernel
            .ask(WriteTechnicalRelation::new(envelope))
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
            .map(KernelReply::into_reply)
            .unwrap_or_else(|error| Some(PersistenceRejection::reply(error)));
        trace.record(TraceNode::EVENT_APPENDER, TraceAction::MessageReceived);
        trace.record(TraceNode::COMMIT, TraceAction::CommitCompleted);
        PipelineReply::new(reply, trace)
    }

    async fn query_thoughts(&self, envelope: MindEnvelope, mut trace: ActorTrace) -> PipelineReply {
        trace.record(TraceNode::GRAPH_STORE, TraceAction::MessageReceived);
        trace.record(TraceNode::SEMA_READER, TraceAction::MessageReceived);
        let reply = self
            .kernel
            .ask(ReadThoughts::new(envelope))
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
            .map(KernelReply::into_reply)
            .unwrap_or_else(|error| Some(PersistenceRejection::reply(error)));
        PipelineReply::new(reply, trace)
    }

    async fn query_relations(
        &self,
        envelope: MindEnvelope,
        mut trace: ActorTrace,
    ) -> PipelineReply {
        trace.record(TraceNode::GRAPH_STORE, TraceAction::MessageReceived);
        trace.record(TraceNode::SEMA_READER, TraceAction::MessageReceived);
        let reply = self
            .kernel
            .ask(ReadRelations::new(envelope))
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
            .map(KernelReply::into_reply)
            .unwrap_or_else(|error| Some(PersistenceRejection::reply(error)));
        PipelineReply::new(reply, trace)
    }

    async fn query_technical_nodes(
        &self,
        envelope: MindEnvelope,
        mut trace: ActorTrace,
    ) -> PipelineReply {
        trace.record(TraceNode::GRAPH_STORE, TraceAction::MessageReceived);
        trace.record(TraceNode::SEMA_READER, TraceAction::MessageReceived);
        let reply = self
            .kernel
            .ask(ReadTechnicalNodes::new(envelope))
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
            .map(KernelReply::into_reply)
            .unwrap_or_else(|error| Some(PersistenceRejection::reply(error)));
        PipelineReply::new(reply, trace)
    }

    async fn query_technical_relations(
        &self,
        envelope: MindEnvelope,
        mut trace: ActorTrace,
    ) -> PipelineReply {
        trace.record(TraceNode::GRAPH_STORE, TraceAction::MessageReceived);
        trace.record(TraceNode::SEMA_READER, TraceAction::MessageReceived);
        let reply = self
            .kernel
            .ask(ReadTechnicalRelations::new(envelope))
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
            .map(KernelReply::into_reply)
            .unwrap_or_else(|error| Some(PersistenceRejection::reply(error)));
        PipelineReply::new(reply, trace)
    }

    async fn subscribe_thoughts(
        &self,
        envelope: MindEnvelope,
        mut trace: ActorTrace,
    ) -> PipelineReply {
        trace.record(TraceNode::GRAPH_STORE, TraceAction::MessageReceived);
        trace.record(
            TraceNode::SUBSCRIPTION_SUPERVISOR,
            TraceAction::MessageReceived,
        );
        trace.record(TraceNode::ID_MINT, TraceAction::MessageReceived);
        trace.record(TraceNode::SEMA_READER, TraceAction::MessageReceived);
        trace.record(TraceNode::SEMA_WRITER, TraceAction::WriteIntentSent);
        let reply = self
            .kernel
            .ask(SubscribeThoughts::new(envelope))
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
            .map(KernelReply::into_reply)
            .unwrap_or_else(|error| Some(PersistenceRejection::reply(error)));
        trace.record(TraceNode::COMMIT, TraceAction::CommitCompleted);
        PipelineReply::new(reply, trace)
    }

    async fn subscribe_relations(
        &self,
        envelope: MindEnvelope,
        mut trace: ActorTrace,
    ) -> PipelineReply {
        trace.record(TraceNode::GRAPH_STORE, TraceAction::MessageReceived);
        trace.record(
            TraceNode::SUBSCRIPTION_SUPERVISOR,
            TraceAction::MessageReceived,
        );
        trace.record(TraceNode::ID_MINT, TraceAction::MessageReceived);
        trace.record(TraceNode::SEMA_READER, TraceAction::MessageReceived);
        trace.record(TraceNode::SEMA_WRITER, TraceAction::WriteIntentSent);
        let reply = self
            .kernel
            .ask(SubscribeRelations::new(envelope))
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
            .map(KernelReply::into_reply)
            .unwrap_or_else(|error| Some(PersistenceRejection::reply(error)));
        trace.record(TraceNode::COMMIT, TraceAction::CommitCompleted);
        PipelineReply::new(reply, trace)
    }

    async fn read_graph_records(&self) -> crate::Result<GraphRecords> {
        self.kernel
            .ask(ReadKernelGraphRecords)
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
    }
}

impl Actor for GraphStore {
    type Args = Arguments;
    type Error = std::convert::Infallible;

    async fn on_start(
        arguments: Self::Args,
        _actor_reference: ActorRef<Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self::new(arguments))
    }
}

impl Message<SubmitThought> for GraphStore {
    type Reply = PipelineReply;

    async fn handle(
        &mut self,
        message: SubmitThought,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.submit_thought(message.envelope, message.trace).await
    }
}

impl Message<SubmitRelation> for GraphStore {
    type Reply = PipelineReply;

    async fn handle(
        &mut self,
        message: SubmitRelation,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.submit_relation(message.envelope, message.trace).await
    }
}

impl Message<SubmitTechnicalNode> for GraphStore {
    type Reply = PipelineReply;

    async fn handle(
        &mut self,
        message: SubmitTechnicalNode,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.submit_technical_node(message.envelope, message.trace)
            .await
    }
}

impl Message<SubmitTechnicalRelation> for GraphStore {
    type Reply = PipelineReply;

    async fn handle(
        &mut self,
        message: SubmitTechnicalRelation,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.submit_technical_relation(message.envelope, message.trace)
            .await
    }
}

impl Message<QueryThoughts> for GraphStore {
    type Reply = PipelineReply;

    async fn handle(
        &mut self,
        message: QueryThoughts,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.query_thoughts(message.envelope, message.trace).await
    }
}

impl Message<QueryRelations> for GraphStore {
    type Reply = PipelineReply;

    async fn handle(
        &mut self,
        message: QueryRelations,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.query_relations(message.envelope, message.trace).await
    }
}

impl Message<QueryTechnicalNodes> for GraphStore {
    type Reply = PipelineReply;

    async fn handle(
        &mut self,
        message: QueryTechnicalNodes,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.query_technical_nodes(message.envelope, message.trace)
            .await
    }
}

impl Message<QueryTechnicalRelations> for GraphStore {
    type Reply = PipelineReply;

    async fn handle(
        &mut self,
        message: QueryTechnicalRelations,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.query_technical_relations(message.envelope, message.trace)
            .await
    }
}

impl Message<OpenThoughtSubscription> for GraphStore {
    type Reply = PipelineReply;

    async fn handle(
        &mut self,
        message: OpenThoughtSubscription,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.subscribe_thoughts(message.envelope, message.trace)
            .await
    }
}

impl Message<OpenRelationSubscription> for GraphStore {
    type Reply = PipelineReply;

    async fn handle(
        &mut self,
        message: OpenRelationSubscription,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.subscribe_relations(message.envelope, message.trace)
            .await
    }
}

impl Message<ReadGraphRecords> for GraphStore {
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
