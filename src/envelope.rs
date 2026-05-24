use signal_mind::{ActorName, MindRequest};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MindEnvelope {
    pub(crate) actor: ActorName,
    pub(crate) request: MindRequest,
}

impl MindEnvelope {
    pub fn new(actor: ActorName, request: MindRequest) -> Self {
        Self { actor, request }
    }

    pub fn actor(&self) -> &ActorName {
        &self.actor
    }

    pub fn request(&self) -> &MindRequest {
        &self.request
    }
}
