use kameo::actor::{Actor, ActorRef};
use kameo::error::Infallible;
use kameo::message::{Context, Message};
use signal_mind::MindReply;

use super::pipeline::PipelineReply;
use super::trace::{ActorTrace, TraceAction, TraceNode};

pub(super) struct ReplyShaper {
    shaped_reply_count: u64,
}

#[derive(Clone, Default)]
pub(super) struct Arguments {
    pub shaped_reply_count: u64,
}

pub struct ShapeReply {
    pub reply: Option<MindReply>,
    pub trace: ActorTrace,
}

impl ReplyShaper {
    fn new(arguments: Arguments) -> Self {
        Self {
            shaped_reply_count: arguments.shaped_reply_count,
        }
    }

    fn shape(&mut self, reply: Option<MindReply>, mut trace: ActorTrace) -> PipelineReply {
        self.shaped_reply_count += 1;
        trace.record(TraceNode::REPLY_SHAPER, TraceAction::MessageReceived);
        match reply {
            Some(reply) => {
                trace.record(TraceNode::NOTA_REPLY_ENCODER, TraceAction::MessageReplied);
                PipelineReply::new(Some(reply), trace)
            }
            None => {
                trace.record(TraceNode::ERROR_SHAPER, TraceAction::MessageReplied);
                PipelineReply::new(None, trace)
            }
        }
    }
}

impl Actor for ReplyShaper {
    type Args = Arguments;
    type Error = Infallible;

    async fn on_start(
        arguments: Self::Args,
        _actor_reference: ActorRef<Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self::new(arguments))
    }
}

impl Message<ShapeReply> for ReplyShaper {
    type Reply = PipelineReply;

    async fn handle(
        &mut self,
        message: ShapeReply,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.shape(message.reply, message.trace)
    }
}
