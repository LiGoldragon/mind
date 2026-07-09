use kameo::actor::{Actor, ActorRef};
use kameo::message::{Context, Message};
use signal_mind::{KnowledgeJudgeVerdict, MindReply, MindRequest};

use crate::MindEnvelope;
use crate::knowledge::{
    KnowledgeAdmissionPreparation, KnowledgeJudgeAppliedDecision, KnowledgeJudgeDecision,
    KnowledgeJudgePort, KnowledgeJudgeRequest,
};

use super::kernel::{
    ApplyKnowledgeAcceptance, KernelReply, PrepareKnowledgeAdmission, StoreKernel,
};
use super::persistence::PersistenceRejection;
use super::{ActorTrace, PipelineReply, TraceAction, TraceNode};

#[derive(Clone)]
pub(super) struct AdmissionArguments {
    pub(super) kernel: ActorRef<StoreKernel>,
    pub(super) caller: ActorRef<KnowledgeJudgeCaller>,
}

#[derive(Clone)]
pub(super) struct CallerArguments {
    pub(super) judge: KnowledgeJudgePort,
}

pub(super) struct AdmitKnowledge {
    pub(super) envelope: MindEnvelope,
    pub(super) trace: ActorTrace,
}

pub(super) struct CallKnowledgeJudge {
    request: KnowledgeJudgeRequest,
}

pub(super) struct RecordAppliedKnowledgeDecision {
    decision: KnowledgeJudgeAppliedDecision,
}

pub(super) struct KnowledgeAdmission {
    kernel: ActorRef<StoreKernel>,
    caller: ActorRef<KnowledgeJudgeCaller>,
}

pub(super) struct KnowledgeJudgeCaller {
    judge: KnowledgeJudgePort,
}

impl CallKnowledgeJudge {
    fn new(request: KnowledgeJudgeRequest) -> Self {
        Self { request }
    }
}

impl RecordAppliedKnowledgeDecision {
    fn new(decision: KnowledgeJudgeAppliedDecision) -> Self {
        Self { decision }
    }
}

impl KnowledgeAdmission {
    fn new(arguments: AdmissionArguments) -> Self {
        Self {
            kernel: arguments.kernel,
            caller: arguments.caller,
        }
    }

    async fn admit(&self, envelope: MindEnvelope, mut trace: ActorTrace) -> PipelineReply {
        trace.record(TraceNode::KNOWLEDGE_ADMISSION, TraceAction::MessageReceived);
        trace.record(TraceNode::SEMA_READER, TraceAction::MessageReceived);
        let preparation = self
            .kernel
            .ask(PrepareKnowledgeAdmission::new(envelope))
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
            .unwrap_or_else(|error| {
                KnowledgeAdmissionPreparation::Rejected(PersistenceRejection::reply(error))
            });

        let prepared = match preparation {
            KnowledgeAdmissionPreparation::Prepared(prepared) => prepared,
            KnowledgeAdmissionPreparation::Rejected(reply) => {
                trace.record(TraceNode::COMMIT, TraceAction::CommitCompleted);
                return PipelineReply::new(Some(reply), trace);
            }
        };

        trace.record(
            TraceNode::KNOWLEDGE_JUDGE_CALLER,
            TraceAction::MessageReceived,
        );
        let decision = self
            .caller
            .ask(CallKnowledgeJudge::new(prepared.request().clone()))
            .await
            .map_err(|error| crate::Error::ActorCall(error.to_string()))
            .unwrap_or_else(|error| KnowledgeJudgeDecision::judge_unavailable(error.to_string()));

        let reply = match decision.verdict() {
            KnowledgeJudgeVerdict::Accept => {
                trace.record(TraceNode::SEMA_WRITER, TraceAction::WriteIntentSent);
                self.kernel
                    .ask(ApplyKnowledgeAcceptance::new(
                        prepared.actor().clone(),
                        prepared.submission().clone(),
                        decision.clone(),
                    ))
                    .await
                    .map_err(|error| crate::Error::ActorCall(error.to_string()))
                    .map(KernelReply::into_reply)
                    .unwrap_or_else(|error| Some(PersistenceRejection::reply(error)))
                    .unwrap_or_else(|| {
                        MindReply::Rejected(
                            signal_mind::KnowledgeRejectionReason::PersistenceRejected,
                        )
                    })
            }
            KnowledgeJudgeVerdict::Reject(reason) => MindReply::Rejected(reason.clone()),
        };

        trace.record(TraceNode::EVENT_APPENDER, TraceAction::MessageReceived);
        trace.record(TraceNode::COMMIT, TraceAction::CommitCompleted);
        let applied = KnowledgeJudgeAppliedDecision::new(
            MindRequest::Submit(prepared.submission().clone()),
            decision,
            reply.clone(),
        );
        let _ = self
            .caller
            .ask(RecordAppliedKnowledgeDecision::new(applied))
            .await;
        PipelineReply::new(Some(reply), trace)
    }
}

impl KnowledgeJudgeCaller {
    fn new(arguments: CallerArguments) -> Self {
        Self {
            judge: arguments.judge,
        }
    }

    async fn judge(&self, request: KnowledgeJudgeRequest) -> KnowledgeJudgeDecision {
        let judge = self.judge.clone();
        tokio::task::spawn_blocking(move || judge.judge(request))
            .await
            .unwrap_or_else(|error| {
                KnowledgeJudgeDecision::judge_unavailable(format!(
                    "mind judge blocking task failed: {error}"
                ))
            })
    }

    fn record_applied_decision(&self, decision: KnowledgeJudgeAppliedDecision) {
        self.judge.record_applied_decision(decision);
    }
}

impl Actor for KnowledgeAdmission {
    type Args = AdmissionArguments;
    type Error = crate::Error;

    async fn on_start(
        arguments: Self::Args,
        _actor_reference: ActorRef<Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self::new(arguments))
    }
}

impl Actor for KnowledgeJudgeCaller {
    type Args = CallerArguments;
    type Error = crate::Error;

    async fn on_start(
        arguments: Self::Args,
        _actor_reference: ActorRef<Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self::new(arguments))
    }
}

impl Message<AdmitKnowledge> for KnowledgeAdmission {
    type Reply = PipelineReply;

    async fn handle(
        &mut self,
        message: AdmitKnowledge,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.admit(message.envelope, message.trace).await
    }
}

impl Message<CallKnowledgeJudge> for KnowledgeJudgeCaller {
    type Reply = KnowledgeJudgeDecision;

    async fn handle(
        &mut self,
        message: CallKnowledgeJudge,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.judge(message.request).await
    }
}

impl Message<RecordAppliedKnowledgeDecision> for KnowledgeJudgeCaller {
    type Reply = bool;

    async fn handle(
        &mut self,
        message: RecordAppliedKnowledgeDecision,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.record_applied_decision(message.decision);
        true
    }
}
