use kameo::actor::{Actor, ActorRef};
use kameo::error::Infallible;
use kameo::message::{Context, Message};
use signal_persona_mind::MindRequest;

use crate::{MindEnvelope, Result as CrateResult};

use super::pipeline::PipelineReply;
use super::store;
use super::trace::{ActorTrace, TraceAction, TraceNode};

pub(super) struct DomainPhase {
    store: ActorRef<store::StoreSupervisor>,
}

#[derive(Clone)]
pub(super) struct Arguments {
    pub(super) store: ActorRef<store::StoreSupervisor>,
}

pub struct ApplyMemory {
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

impl DomainPhase {
    fn new(store: ActorRef<store::StoreSupervisor>) -> Self {
        Self { store }
    }

    async fn apply_memory(
        &self,
        envelope: MindEnvelope,
        mut trace: ActorTrace,
    ) -> CrateResult<PipelineReply> {
        trace.record(TraceNode::DOMAIN_PHASE, TraceAction::MessageReceived);
        trace.record(
            TraceNode::MEMORY_GRAPH_SUPERVISOR,
            TraceAction::MessageReceived,
        );
        MemoryOperation::from_request(envelope.request()).record_into(&mut trace);

        self.store
            .ask(store::ApplyMemory { envelope, trace })
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
    }

    async fn submit_thought(
        &self,
        envelope: MindEnvelope,
        mut trace: ActorTrace,
    ) -> CrateResult<PipelineReply> {
        trace.record(TraceNode::DOMAIN_PHASE, TraceAction::MessageReceived);
        trace.record(
            TraceNode::MIND_GRAPH_SUPERVISOR,
            TraceAction::MessageReceived,
        );
        trace.record(TraceNode::THOUGHT_COMMIT, TraceAction::MessageReceived);

        self.store
            .ask(store::SubmitThought { envelope, trace })
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
    }

    async fn submit_relation(
        &self,
        envelope: MindEnvelope,
        mut trace: ActorTrace,
    ) -> CrateResult<PipelineReply> {
        trace.record(TraceNode::DOMAIN_PHASE, TraceAction::MessageReceived);
        trace.record(
            TraceNode::MIND_GRAPH_SUPERVISOR,
            TraceAction::MessageReceived,
        );
        trace.record(TraceNode::RELATION_COMMIT, TraceAction::MessageReceived);

        self.store
            .ask(store::SubmitRelation { envelope, trace })
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
    }
}

impl Actor for DomainPhase {
    type Args = Arguments;
    type Error = Infallible;

    async fn on_start(
        arguments: Self::Args,
        _actor_reference: ActorRef<Self>,
    ) -> std::result::Result<Self, Self::Error> {
        Ok(Self::new(arguments.store))
    }
}

impl Message<ApplyMemory> for DomainPhase {
    type Reply = PipelineReply;

    async fn handle(
        &mut self,
        message: ApplyMemory,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        match self.apply_memory(message.envelope, message.trace).await {
            Ok(reply) => reply,
            Err(_error) => PipelineReply::new(None, ActorTrace::new()),
        }
    }
}

impl Message<SubmitThought> for DomainPhase {
    type Reply = PipelineReply;

    async fn handle(
        &mut self,
        message: SubmitThought,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        match self.submit_thought(message.envelope, message.trace).await {
            Ok(reply) => reply,
            Err(_error) => PipelineReply::new(None, ActorTrace::new()),
        }
    }
}

impl Message<SubmitRelation> for DomainPhase {
    type Reply = PipelineReply;

    async fn handle(
        &mut self,
        message: SubmitRelation,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        match self.submit_relation(message.envelope, message.trace).await {
            Ok(reply) => reply,
            Err(_error) => PipelineReply::new(None, ActorTrace::new()),
        }
    }
}

struct MemoryOperation {
    actor: TraceNode,
}

impl MemoryOperation {
    fn from_request(request: &MindRequest) -> Self {
        let actor = match request {
            MindRequest::Opening(_) => TraceNode::ITEM_OPEN,
            MindRequest::NoteSubmission(_) => TraceNode::NOTE_ADD,
            MindRequest::Link(_) => TraceNode::LINK,
            MindRequest::StatusChange(_) => TraceNode::STATUS_CHANGE,
            MindRequest::AliasAssignment(_) => TraceNode::ALIAS_ADD,
            _ => TraceNode::ERROR_SHAPER,
        };
        Self { actor }
    }

    fn record_into(&self, trace: &mut ActorTrace) {
        trace.record(self.actor, TraceAction::MessageReceived);
    }
}
