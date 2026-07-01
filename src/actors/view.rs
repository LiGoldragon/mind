use kameo::actor::{Actor, ActorRef};
use kameo::error::Infallible;
use kameo::message::{Context, Message};
use signal_mind::{MindRequest, QueryKind};

use crate::{MindEnvelope, Result as CrateResult};

use super::pipeline::PipelineReply;
use super::store;
use super::trace::{ActorTrace, TraceAction, TraceNode};

pub(super) struct ViewPhase {
    store: ActorRef<store::StoreSupervisor>,
}

#[derive(Clone)]
pub(super) struct Arguments {
    pub(super) store: ActorRef<store::StoreSupervisor>,
}

pub struct ReadMemory {
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

pub struct QueryKnowledge {
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

impl ViewPhase {
    fn new(store: ActorRef<store::StoreSupervisor>) -> Self {
        Self { store }
    }

    async fn read_memory(
        &self,
        envelope: MindEnvelope,
        mut trace: ActorTrace,
    ) -> CrateResult<PipelineReply> {
        trace.record(TraceNode::VIEW_PHASE, TraceAction::MessageReceived);
        trace.record(TraceNode::QUERY_SUPERVISOR, TraceAction::MessageReceived);
        trace.record(TraceNode::QUERY_PLANNER, TraceAction::MessageReceived);
        QueryOperation::from_request(envelope.request()).record_into(&mut trace);
        trace.record(TraceNode::GRAPH_TRAVERSAL, TraceAction::MessageReceived);

        let mut reply = self
            .store
            .ask(store::ReadMemory { envelope, trace })
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))?;
        reply
            .trace
            .record(TraceNode::QUERY_RESULT_SHAPER, TraceAction::MessageReceived);
        Ok(reply)
    }

    async fn query_thoughts(
        &self,
        envelope: MindEnvelope,
        mut trace: ActorTrace,
    ) -> CrateResult<PipelineReply> {
        trace.record(TraceNode::VIEW_PHASE, TraceAction::MessageReceived);
        trace.record(TraceNode::QUERY_SUPERVISOR, TraceAction::MessageReceived);
        trace.record(TraceNode::QUERY_PLANNER, TraceAction::MessageReceived);
        trace.record(TraceNode::THOUGHT_QUERY, TraceAction::MessageReceived);

        let mut reply = self
            .store
            .ask(store::QueryThoughts { envelope, trace })
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))?;
        reply
            .trace
            .record(TraceNode::QUERY_RESULT_SHAPER, TraceAction::MessageReceived);
        Ok(reply)
    }

    async fn query_relations(
        &self,
        envelope: MindEnvelope,
        mut trace: ActorTrace,
    ) -> CrateResult<PipelineReply> {
        trace.record(TraceNode::VIEW_PHASE, TraceAction::MessageReceived);
        trace.record(TraceNode::QUERY_SUPERVISOR, TraceAction::MessageReceived);
        trace.record(TraceNode::QUERY_PLANNER, TraceAction::MessageReceived);
        trace.record(TraceNode::RELATION_QUERY, TraceAction::MessageReceived);

        let mut reply = self
            .store
            .ask(store::QueryRelations { envelope, trace })
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))?;
        reply
            .trace
            .record(TraceNode::QUERY_RESULT_SHAPER, TraceAction::MessageReceived);
        Ok(reply)
    }

    async fn query_technical_nodes(
        &self,
        envelope: MindEnvelope,
        mut trace: ActorTrace,
    ) -> CrateResult<PipelineReply> {
        trace.record(TraceNode::VIEW_PHASE, TraceAction::MessageReceived);
        trace.record(TraceNode::QUERY_SUPERVISOR, TraceAction::MessageReceived);
        trace.record(TraceNode::QUERY_PLANNER, TraceAction::MessageReceived);
        trace.record(TraceNode::THOUGHT_QUERY, TraceAction::MessageReceived);

        let mut reply = self
            .store
            .ask(store::QueryTechnicalNodes { envelope, trace })
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))?;
        reply
            .trace
            .record(TraceNode::QUERY_RESULT_SHAPER, TraceAction::MessageReceived);
        Ok(reply)
    }

    async fn query_technical_relations(
        &self,
        envelope: MindEnvelope,
        mut trace: ActorTrace,
    ) -> CrateResult<PipelineReply> {
        trace.record(TraceNode::VIEW_PHASE, TraceAction::MessageReceived);
        trace.record(TraceNode::QUERY_SUPERVISOR, TraceAction::MessageReceived);
        trace.record(TraceNode::QUERY_PLANNER, TraceAction::MessageReceived);
        trace.record(TraceNode::RELATION_QUERY, TraceAction::MessageReceived);

        let mut reply = self
            .store
            .ask(store::QueryTechnicalRelations { envelope, trace })
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))?;
        reply
            .trace
            .record(TraceNode::QUERY_RESULT_SHAPER, TraceAction::MessageReceived);
        Ok(reply)
    }

    async fn query_knowledge(
        &self,
        envelope: MindEnvelope,
        mut trace: ActorTrace,
    ) -> CrateResult<PipelineReply> {
        trace.record(TraceNode::VIEW_PHASE, TraceAction::MessageReceived);
        trace.record(TraceNode::QUERY_SUPERVISOR, TraceAction::MessageReceived);
        trace.record(TraceNode::QUERY_PLANNER, TraceAction::MessageReceived);
        trace.record(TraceNode::THOUGHT_QUERY, TraceAction::MessageReceived);

        let mut reply = self
            .store
            .ask(store::QueryKnowledge { envelope, trace })
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))?;
        reply
            .trace
            .record(TraceNode::QUERY_RESULT_SHAPER, TraceAction::MessageReceived);
        Ok(reply)
    }

    async fn subscribe_thoughts(
        &self,
        envelope: MindEnvelope,
        mut trace: ActorTrace,
    ) -> CrateResult<PipelineReply> {
        trace.record(TraceNode::VIEW_PHASE, TraceAction::MessageReceived);
        trace.record(
            TraceNode::SUBSCRIPTION_SUPERVISOR,
            TraceAction::MessageReceived,
        );
        self.store
            .ask(store::SubscribeThoughts { envelope, trace })
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
    }

    async fn subscribe_relations(
        &self,
        envelope: MindEnvelope,
        mut trace: ActorTrace,
    ) -> CrateResult<PipelineReply> {
        trace.record(TraceNode::VIEW_PHASE, TraceAction::MessageReceived);
        trace.record(
            TraceNode::SUBSCRIPTION_SUPERVISOR,
            TraceAction::MessageReceived,
        );
        self.store
            .ask(store::SubscribeRelations { envelope, trace })
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
    }

    async fn subscribe_technical_nodes(
        &self,
        envelope: MindEnvelope,
        mut trace: ActorTrace,
    ) -> CrateResult<PipelineReply> {
        trace.record(TraceNode::VIEW_PHASE, TraceAction::MessageReceived);
        trace.record(
            TraceNode::SUBSCRIPTION_SUPERVISOR,
            TraceAction::MessageReceived,
        );
        self.store
            .ask(store::SubscribeTechnicalNodes { envelope, trace })
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
    }

    async fn subscribe_technical_relations(
        &self,
        envelope: MindEnvelope,
        mut trace: ActorTrace,
    ) -> CrateResult<PipelineReply> {
        trace.record(TraceNode::VIEW_PHASE, TraceAction::MessageReceived);
        trace.record(
            TraceNode::SUBSCRIPTION_SUPERVISOR,
            TraceAction::MessageReceived,
        );
        self.store
            .ask(store::SubscribeTechnicalRelations { envelope, trace })
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
    }
}

impl Actor for ViewPhase {
    type Args = Arguments;
    type Error = Infallible;

    async fn on_start(
        arguments: Self::Args,
        _actor_reference: ActorRef<Self>,
    ) -> std::result::Result<Self, Self::Error> {
        Ok(Self::new(arguments.store))
    }
}

impl Message<ReadMemory> for ViewPhase {
    type Reply = PipelineReply;

    async fn handle(
        &mut self,
        message: ReadMemory,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        match self.read_memory(message.envelope, message.trace).await {
            Ok(reply) => reply,
            Err(_error) => PipelineReply::new(None, ActorTrace::new()),
        }
    }
}

impl Message<QueryThoughts> for ViewPhase {
    type Reply = PipelineReply;

    async fn handle(
        &mut self,
        message: QueryThoughts,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        match self.query_thoughts(message.envelope, message.trace).await {
            Ok(reply) => reply,
            Err(_error) => PipelineReply::new(None, ActorTrace::new()),
        }
    }
}

impl Message<QueryRelations> for ViewPhase {
    type Reply = PipelineReply;

    async fn handle(
        &mut self,
        message: QueryRelations,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        match self.query_relations(message.envelope, message.trace).await {
            Ok(reply) => reply,
            Err(_error) => PipelineReply::new(None, ActorTrace::new()),
        }
    }
}

impl Message<QueryTechnicalNodes> for ViewPhase {
    type Reply = PipelineReply;

    async fn handle(
        &mut self,
        message: QueryTechnicalNodes,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        match self
            .query_technical_nodes(message.envelope, message.trace)
            .await
        {
            Ok(reply) => reply,
            Err(_error) => PipelineReply::new(None, ActorTrace::new()),
        }
    }
}

impl Message<QueryTechnicalRelations> for ViewPhase {
    type Reply = PipelineReply;

    async fn handle(
        &mut self,
        message: QueryTechnicalRelations,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        match self
            .query_technical_relations(message.envelope, message.trace)
            .await
        {
            Ok(reply) => reply,
            Err(_error) => PipelineReply::new(None, ActorTrace::new()),
        }
    }
}

impl Message<QueryKnowledge> for ViewPhase {
    type Reply = PipelineReply;

    async fn handle(
        &mut self,
        message: QueryKnowledge,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        match self.query_knowledge(message.envelope, message.trace).await {
            Ok(reply) => reply,
            Err(_error) => PipelineReply::new(None, ActorTrace::new()),
        }
    }
}

impl Message<SubscribeThoughts> for ViewPhase {
    type Reply = PipelineReply;

    async fn handle(
        &mut self,
        message: SubscribeThoughts,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        match self
            .subscribe_thoughts(message.envelope, message.trace)
            .await
        {
            Ok(reply) => reply,
            Err(_error) => PipelineReply::new(None, ActorTrace::new()),
        }
    }
}

impl Message<SubscribeRelations> for ViewPhase {
    type Reply = PipelineReply;

    async fn handle(
        &mut self,
        message: SubscribeRelations,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        match self
            .subscribe_relations(message.envelope, message.trace)
            .await
        {
            Ok(reply) => reply,
            Err(_error) => PipelineReply::new(None, ActorTrace::new()),
        }
    }
}

impl Message<SubscribeTechnicalNodes> for ViewPhase {
    type Reply = PipelineReply;

    async fn handle(
        &mut self,
        message: SubscribeTechnicalNodes,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        match self
            .subscribe_technical_nodes(message.envelope, message.trace)
            .await
        {
            Ok(reply) => reply,
            Err(_error) => PipelineReply::new(None, ActorTrace::new()),
        }
    }
}

impl Message<SubscribeTechnicalRelations> for ViewPhase {
    type Reply = PipelineReply;

    async fn handle(
        &mut self,
        message: SubscribeTechnicalRelations,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        match self
            .subscribe_technical_relations(message.envelope, message.trace)
            .await
        {
            Ok(reply) => reply,
            Err(_error) => PipelineReply::new(None, ActorTrace::new()),
        }
    }
}

struct QueryOperation {
    actor: TraceNode,
}

impl QueryOperation {
    fn from_request(request: &MindRequest) -> Self {
        let actor = match request {
            MindRequest::Query(query) => match &query.kind {
                QueryKind::Ready => TraceNode::READY_WORK_VIEW,
                QueryKind::Blocked => TraceNode::BLOCKED_WORK_VIEW,
                QueryKind::RecentEvents => TraceNode::RECENT_ACTIVITY_VIEW,
                QueryKind::Open
                | QueryKind::ByItem(_)
                | QueryKind::ByKind(_)
                | QueryKind::ByStatus(_)
                | QueryKind::ByAlias(_) => TraceNode::GRAPH_TRAVERSAL,
            },
            _ => TraceNode::ERROR_SHAPER,
        };
        Self { actor }
    }

    fn record_into(&self, trace: &mut ActorTrace) {
        trace.record(self.actor, TraceAction::MessageReceived);
    }
}
