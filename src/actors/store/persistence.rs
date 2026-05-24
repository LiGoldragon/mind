use signal_mind::{MindReply, Rejection, RejectionReason};

use crate::Error;

use super::super::pipeline::PipelineReply;
use super::super::trace::ActorTrace;

pub(super) struct PersistenceRejection {
    reason: RejectionReason,
}

impl PersistenceRejection {
    pub(super) fn new(_error: Error) -> Self {
        Self {
            reason: RejectionReason::PersistenceRejected,
        }
    }

    pub(super) fn reply(error: Error) -> MindReply {
        let rejection = Self::new(error);
        MindReply::Rejection(Rejection {
            reason: rejection.reason,
        })
    }

    pub(super) fn pipeline(error: Error) -> PipelineReply {
        PipelineReply::new(Some(Self::reply(error)), ActorTrace::new())
    }
}
