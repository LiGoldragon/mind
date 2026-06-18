use kameo::actor::{Actor, ActorRef, Spawn};
use kameo::error::Infallible;
use kameo::message::{Context, Message};
use signal_mind::MindReply;

use crate::{Error, MindEnvelope, Result, StoreLocation, supervision};

use super::trace::{ActorTrace, TraceAction, TraceNode};
use super::{choreography, dispatch, domain, ingress, reply, store, subscription, view};

pub struct MindRoot {
    ingress: ActorRef<ingress::IngressPhase>,
    dispatch: ActorRef<dispatch::DispatchPhase>,
    domain: ActorRef<domain::DomainPhase>,
    view: ActorRef<view::ViewPhase>,
    store: ActorRef<store::StoreSupervisor>,
    reply: ActorRef<reply::ReplyShaper>,
    subscription: ActorRef<subscription::SubscriptionSupervisor>,
    supervision: ActorRef<supervision::SupervisionPhase>,
    _choreography: Option<ActorRef<choreography::ChoreographyAdjudicator>>,
}

struct RootChildren {
    ingress: ActorRef<ingress::IngressPhase>,
    dispatch: ActorRef<dispatch::DispatchPhase>,
    domain: ActorRef<domain::DomainPhase>,
    view: ActorRef<view::ViewPhase>,
    store: ActorRef<store::StoreSupervisor>,
    reply: ActorRef<reply::ReplyShaper>,
    subscription: ActorRef<subscription::SubscriptionSupervisor>,
    supervision: ActorRef<supervision::SupervisionPhase>,
    choreography: Option<ActorRef<choreography::ChoreographyAdjudicator>>,
}

pub struct Arguments {
    pub store: StoreLocation,
    pub orchestrate_meta_endpoint: Option<choreography::MetaEndpoint>,
}

impl Arguments {
    pub fn new(store: StoreLocation) -> Self {
        Self {
            store,
            orchestrate_meta_endpoint: None,
        }
    }

    pub fn with_orchestrate_meta_endpoint(mut self, endpoint: choreography::MetaEndpoint) -> Self {
        self.orchestrate_meta_endpoint = Some(endpoint);
        self
    }
}

pub struct SubmitEnvelope {
    pub envelope: MindEnvelope,
}

struct ShutdownChildren;

#[derive(Debug, kameo::Reply)]
pub struct RootReply {
    reply: Option<MindReply>,
    trace: ActorTrace,
}

impl RootReply {
    pub fn new(reply: Option<MindReply>, trace: ActorTrace) -> Self {
        Self { reply, trace }
    }

    pub fn reply(&self) -> Option<&MindReply> {
        self.reply.as_ref()
    }

    pub fn trace(&self) -> &ActorTrace {
        &self.trace
    }
}

impl From<RootChildren> for MindRoot {
    fn from(children: RootChildren) -> Self {
        Self {
            ingress: children.ingress,
            dispatch: children.dispatch,
            domain: children.domain,
            view: children.view,
            store: children.store,
            reply: children.reply,
            subscription: children.subscription,
            supervision: children.supervision,
            _choreography: children.choreography,
        }
    }
}

impl MindRoot {
    pub async fn start(arguments: Arguments) -> Result<ActorRef<Self>> {
        let actor_reference = Self::spawn(arguments);
        actor_reference.wait_for_startup().await;
        Ok(actor_reference)
    }

    pub async fn stop(actor_reference: ActorRef<Self>) -> Result<()> {
        let _ = actor_reference.ask(ShutdownChildren).await;
        actor_reference.kill();
        let _ = actor_reference.wait_for_shutdown().await;
        Ok(())
    }

    async fn submit(&self, envelope: MindEnvelope) -> Result<RootReply> {
        let mut trace = ActorTrace::new();
        trace.record(TraceNode::MIND_ROOT, TraceAction::MessageReceived);

        let mut pipeline = self
            .ingress
            .ask(ingress::AcceptEnvelope { envelope, trace })
            .await
            .map_err(|error| Error::ActorCall(error.to_string()))?;
        pipeline
            .trace
            .record(TraceNode::MIND_ROOT, TraceAction::MessageReplied);

        Ok(RootReply::new(pipeline.reply, pipeline.trace))
    }

    async fn request_stop_child<Child>(child: &ActorRef<Child>)
    where
        Child: Actor,
    {
        let _ = child.stop_gracefully().await;
    }

    async fn stop_children(&mut self) {
        Self::request_stop_child(&self.ingress).await;
        Self::request_stop_child(&self.dispatch).await;
        Self::request_stop_child(&self.domain).await;
        Self::request_stop_child(&self.view).await;
        Self::request_stop_child(&self.reply).await;
        let _ = self.store.ask(store::ShutdownStore).await;
        Self::request_stop_child(&self.store).await;
        if let Some(choreography) = &self._choreography {
            Self::request_stop_child(choreography).await;
        }
        Self::request_stop_child(&self.supervision).await;
        Self::request_stop_child(&self.subscription).await;
    }
}

impl Actor for MindRoot {
    type Args = Arguments;
    type Error = Infallible;

    async fn on_start(
        arguments: Self::Args,
        actor_reference: ActorRef<Self>,
    ) -> std::result::Result<Self, Self::Error> {
        let subscription = subscription::SubscriptionSupervisor::supervise(
            &actor_reference,
            subscription::Arguments::default(),
        )
        .spawn()
        .await;
        let supervision = supervision::SupervisionPhase::supervise(
            &actor_reference,
            supervision::SupervisionArguments::new(supervision::SupervisionProfile::mind()),
        )
        .spawn()
        .await;

        let choreography = if let Some(endpoint) = arguments.orchestrate_meta_endpoint.clone() {
            let caller = choreography::MindOrchestrateCaller::supervise(
                &actor_reference,
                choreography::CallerArguments::new(endpoint),
            )
            .spawn()
            .await;
            Some(
                choreography::ChoreographyAdjudicator::supervise(
                    &actor_reference,
                    choreography::AdjudicatorArguments::new(caller),
                )
                .spawn()
                .await,
            )
        } else {
            None
        };

        let store = store::StoreSupervisor::supervise(
            &actor_reference,
            store::Arguments {
                store: arguments.store.clone(),
                subscription: subscription.clone(),
            },
        )
        .spawn()
        .await;
        let _store_is_bound = subscription
            .ask(subscription::BindStore::new(store.clone()))
            .await
            .map(|receipt| receipt.is_bound())
            .unwrap_or(false);

        let reply = reply::ReplyShaper::supervise(&actor_reference, reply::Arguments::default())
            .spawn()
            .await;

        let view = view::ViewPhase::supervise(
            &actor_reference,
            view::Arguments {
                store: store.clone(),
            },
        )
        .spawn()
        .await;

        let domain = domain::DomainPhase::supervise(
            &actor_reference,
            domain::Arguments {
                store: store.clone(),
            },
        )
        .spawn()
        .await;

        let dispatch = dispatch::DispatchPhase::supervise(
            &actor_reference,
            dispatch::Arguments {
                domain: domain.clone(),
                view: view.clone(),
                reply: reply.clone(),
            },
        )
        .spawn()
        .await;

        let ingress = ingress::IngressPhase::supervise(
            &actor_reference,
            ingress::Arguments {
                dispatch: dispatch.clone(),
            },
        )
        .spawn()
        .await;

        Ok(RootChildren {
            ingress,
            dispatch,
            domain,
            view,
            store,
            reply,
            subscription,
            supervision,
            choreography,
        }
        .into())
    }
}

impl Message<SubmitEnvelope> for MindRoot {
    type Reply = RootReply;

    async fn handle(
        &mut self,
        message: SubmitEnvelope,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        match self.submit(message.envelope).await {
            Ok(reply) => reply,
            Err(_error) => {
                let mut trace = ActorTrace::new();
                trace.record(TraceNode::MIND_ROOT, TraceAction::MessageReceived);
                trace.record(TraceNode::ERROR_SHAPER, TraceAction::MessageReplied);
                RootReply::new(None, trace)
            }
        }
    }
}

impl Message<ShutdownChildren> for MindRoot {
    type Reply = bool;

    async fn handle(
        &mut self,
        _message: ShutdownChildren,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.stop_children().await;
        true
    }
}

impl Message<supervision::HandleSupervisionRequest> for MindRoot {
    type Reply = supervision::SupervisionPhaseReply;

    async fn handle(
        &mut self,
        message: supervision::HandleSupervisionRequest,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.supervision
            .ask(message)
            .await
            .unwrap_or_else(|_| supervision::SupervisionPhaseReply::unavailable())
    }
}

impl Message<subscription::ReadSubscriptionEvents> for MindRoot {
    type Reply = subscription::SubscriptionEventLog;

    async fn handle(
        &mut self,
        message: subscription::ReadSubscriptionEvents,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.subscription
            .ask(message)
            .await
            .unwrap_or_else(|_| subscription::SubscriptionEventLog::empty())
    }
}
