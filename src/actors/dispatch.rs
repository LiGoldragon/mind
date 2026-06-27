use kameo::actor::{Actor, ActorRef};
use kameo::error::Infallible;
use kameo::message::{Context, Message};
use signal_mind::{MindReply, MindRequest, MindRequestUnimplemented, MindUnimplementedReason};

use crate::{MindEnvelope, Result as CrateResult};

use super::domain;
use super::pipeline::PipelineReply;
use super::reply;
use super::trace::{ActorTrace, TraceAction, TraceNode};
use super::view;

pub(super) struct DispatchPhase {
    domain: ActorRef<domain::DomainPhase>,
    view: ActorRef<view::ViewPhase>,
    reply: ActorRef<reply::ReplyShaper>,
}

#[derive(Clone)]
pub(super) struct Arguments {
    pub(super) domain: ActorRef<domain::DomainPhase>,
    pub(super) view: ActorRef<view::ViewPhase>,
    pub(super) reply: ActorRef<reply::ReplyShaper>,
}

pub struct RouteEnvelope {
    pub envelope: MindEnvelope,
    pub trace: ActorTrace,
}

impl DispatchPhase {
    fn new(
        domain: ActorRef<domain::DomainPhase>,
        view: ActorRef<view::ViewPhase>,
        reply: ActorRef<reply::ReplyShaper>,
    ) -> Self {
        Self {
            domain,
            view,
            reply,
        }
    }

    async fn route(
        &self,
        envelope: MindEnvelope,
        mut trace: ActorTrace,
    ) -> CrateResult<PipelineReply> {
        trace.record(TraceNode::DISPATCH_PHASE, TraceAction::MessageReceived);
        trace.record(TraceNode::REQUEST_DISPATCHER, TraceAction::MessageReceived);

        let pipeline = match envelope.request() {
            MindRequest::SubmitThought(_) => {
                trace.record(TraceNode::GRAPH_FLOW, TraceAction::MessageReceived);
                self.submit_thought(envelope, trace).await?
            }
            MindRequest::SubmitRelation(_) => {
                trace.record(TraceNode::GRAPH_FLOW, TraceAction::MessageReceived);
                self.submit_relation(envelope, trace).await?
            }
            MindRequest::SubmitTechnicalNode(_) => {
                trace.record(TraceNode::GRAPH_FLOW, TraceAction::MessageReceived);
                self.submit_technical_node(envelope, trace).await?
            }
            MindRequest::SubmitTechnicalRelation(_) => {
                trace.record(TraceNode::GRAPH_FLOW, TraceAction::MessageReceived);
                self.submit_technical_relation(envelope, trace).await?
            }
            MindRequest::QueryThoughts(_) => {
                trace.record(TraceNode::GRAPH_QUERY_FLOW, TraceAction::MessageReceived);
                self.query_thoughts(envelope, trace).await?
            }
            MindRequest::QueryRelations(_) => {
                trace.record(TraceNode::GRAPH_QUERY_FLOW, TraceAction::MessageReceived);
                self.query_relations(envelope, trace).await?
            }
            MindRequest::QueryTechnicalNodes(_) => {
                trace.record(TraceNode::GRAPH_QUERY_FLOW, TraceAction::MessageReceived);
                self.query_technical_nodes(envelope, trace).await?
            }
            MindRequest::QueryTechnicalRelations(_) => {
                trace.record(TraceNode::GRAPH_QUERY_FLOW, TraceAction::MessageReceived);
                self.query_technical_relations(envelope, trace).await?
            }
            MindRequest::SubscribeThoughts(_) => {
                trace.record(TraceNode::GRAPH_QUERY_FLOW, TraceAction::MessageReceived);
                self.subscribe_thoughts(envelope, trace).await?
            }
            MindRequest::SubscribeRelations(_) => {
                trace.record(TraceNode::GRAPH_QUERY_FLOW, TraceAction::MessageReceived);
                self.subscribe_relations(envelope, trace).await?
            }
            MindRequest::SubscribeTechnicalNodes(_) => {
                trace.record(TraceNode::GRAPH_QUERY_FLOW, TraceAction::MessageReceived);
                self.subscribe_technical_nodes(envelope, trace).await?
            }
            MindRequest::SubscribeTechnicalRelations(_) => {
                trace.record(TraceNode::GRAPH_QUERY_FLOW, TraceAction::MessageReceived);
                self.subscribe_technical_relations(envelope, trace).await?
            }
            MindRequest::Opening(_)
            | MindRequest::NoteSubmission(_)
            | MindRequest::Link(_)
            | MindRequest::StatusChange(_)
            | MindRequest::AliasAssignment(_) => {
                trace.record(TraceNode::MEMORY_FLOW, TraceAction::MessageReceived);
                self.apply_memory(envelope, trace).await?
            }
            MindRequest::Query(_) => {
                trace.record(TraceNode::QUERY_FLOW, TraceAction::MessageReceived);
                self.read_memory(envelope, trace).await?
            }
            MindRequest::AdjudicationRequest(_)
            | MindRequest::ChannelList(_)
            | MindRequest::SubscriptionRetraction(_) => self.unimplemented(trace),
        };

        self.shape_reply(pipeline).await
    }

    async fn apply_memory(
        &self,
        envelope: MindEnvelope,
        trace: ActorTrace,
    ) -> CrateResult<PipelineReply> {
        self.domain
            .ask(domain::ApplyMemory { envelope, trace })
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
    }

    async fn submit_thought(
        &self,
        envelope: MindEnvelope,
        trace: ActorTrace,
    ) -> CrateResult<PipelineReply> {
        self.domain
            .ask(domain::SubmitThought { envelope, trace })
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
    }

    async fn submit_relation(
        &self,
        envelope: MindEnvelope,
        trace: ActorTrace,
    ) -> CrateResult<PipelineReply> {
        self.domain
            .ask(domain::SubmitRelation { envelope, trace })
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
    }

    async fn submit_technical_node(
        &self,
        envelope: MindEnvelope,
        trace: ActorTrace,
    ) -> CrateResult<PipelineReply> {
        self.domain
            .ask(domain::SubmitTechnicalNode { envelope, trace })
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
    }

    async fn submit_technical_relation(
        &self,
        envelope: MindEnvelope,
        trace: ActorTrace,
    ) -> CrateResult<PipelineReply> {
        self.domain
            .ask(domain::SubmitTechnicalRelation { envelope, trace })
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
    }

    async fn query_thoughts(
        &self,
        envelope: MindEnvelope,
        trace: ActorTrace,
    ) -> CrateResult<PipelineReply> {
        self.view
            .ask(view::QueryThoughts { envelope, trace })
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
    }

    async fn query_technical_nodes(
        &self,
        envelope: MindEnvelope,
        trace: ActorTrace,
    ) -> CrateResult<PipelineReply> {
        self.view
            .ask(view::QueryTechnicalNodes { envelope, trace })
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
    }

    async fn query_technical_relations(
        &self,
        envelope: MindEnvelope,
        trace: ActorTrace,
    ) -> CrateResult<PipelineReply> {
        self.view
            .ask(view::QueryTechnicalRelations { envelope, trace })
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
    }

    async fn query_relations(
        &self,
        envelope: MindEnvelope,
        trace: ActorTrace,
    ) -> CrateResult<PipelineReply> {
        self.view
            .ask(view::QueryRelations { envelope, trace })
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
    }

    async fn subscribe_thoughts(
        &self,
        envelope: MindEnvelope,
        trace: ActorTrace,
    ) -> CrateResult<PipelineReply> {
        self.view
            .ask(view::SubscribeThoughts { envelope, trace })
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
    }

    async fn subscribe_relations(
        &self,
        envelope: MindEnvelope,
        trace: ActorTrace,
    ) -> CrateResult<PipelineReply> {
        self.view
            .ask(view::SubscribeRelations { envelope, trace })
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
    }

    async fn subscribe_technical_nodes(
        &self,
        envelope: MindEnvelope,
        trace: ActorTrace,
    ) -> CrateResult<PipelineReply> {
        self.view
            .ask(view::SubscribeTechnicalNodes { envelope, trace })
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
    }

    async fn subscribe_technical_relations(
        &self,
        envelope: MindEnvelope,
        trace: ActorTrace,
    ) -> CrateResult<PipelineReply> {
        self.view
            .ask(view::SubscribeTechnicalRelations { envelope, trace })
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
    }

    async fn read_memory(
        &self,
        envelope: MindEnvelope,
        trace: ActorTrace,
    ) -> CrateResult<PipelineReply> {
        self.view
            .ask(view::ReadMemory { envelope, trace })
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
    }

    async fn shape_reply(&self, pipeline: PipelineReply) -> CrateResult<PipelineReply> {
        self.reply
            .ask(reply::ShapeReply {
                reply: pipeline.reply,
                trace: pipeline.trace,
            })
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
    }

    fn unimplemented(&self, mut trace: ActorTrace) -> PipelineReply {
        trace.record(TraceNode::ERROR_SHAPER, TraceAction::MessageReplied);
        PipelineReply::new(
            Some(MindReply::MindRequestUnimplemented(
                MindRequestUnimplemented {
                    reason: MindUnimplementedReason::NotInPrototypeScope,
                },
            )),
            trace,
        )
    }
}

impl Actor for DispatchPhase {
    type Args = Arguments;
    type Error = Infallible;

    async fn on_start(
        arguments: Self::Args,
        _actor_reference: ActorRef<Self>,
    ) -> std::result::Result<Self, Self::Error> {
        Ok(Self::new(arguments.domain, arguments.view, arguments.reply))
    }
}

impl Message<RouteEnvelope> for DispatchPhase {
    type Reply = PipelineReply;

    async fn handle(
        &mut self,
        message: RouteEnvelope,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        match self.route(message.envelope, message.trace).await {
            Ok(reply) => reply,
            Err(_error) => PipelineReply::new(None, ActorTrace::new()),
        }
    }
}
