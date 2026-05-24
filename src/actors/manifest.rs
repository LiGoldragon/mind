use super::trace::TraceNode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActorResidency {
    Root,
    LongLived,
    TracePhase,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestEntry {
    kind: TraceNode,
    residency: ActorResidency,
}

impl ManifestEntry {
    pub fn new(kind: TraceNode, residency: ActorResidency) -> Self {
        Self { kind, residency }
    }

    pub fn kind(&self) -> TraceNode {
        self.kind
    }

    pub fn residency(&self) -> ActorResidency {
        self.residency
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ManifestEdge {
    parent: TraceNode,
    child: TraceNode,
}

impl ManifestEdge {
    pub fn new(parent: TraceNode, child: TraceNode) -> Self {
        Self { parent, child }
    }

    pub fn parent(&self) -> TraceNode {
        self.parent
    }

    pub fn child(&self) -> TraceNode {
        self.child
    }
}

#[derive(Debug, Clone, PartialEq, Eq, kameo::Reply)]
pub struct ActorManifest {
    actors: Vec<ManifestEntry>,
    edges: Vec<ManifestEdge>,
}

impl ActorManifest {
    pub fn mind_phase_one() -> Self {
        let root = ActorResidency::Root;
        let long_lived = ActorResidency::LongLived;
        let trace_phase = ActorResidency::TracePhase;

        let actors = vec![
            ManifestEntry::new(TraceNode::MIND_ROOT, root),
            ManifestEntry::new(TraceNode::INGRESS_PHASE, long_lived),
            ManifestEntry::new(TraceNode::REQUEST_SESSION, trace_phase),
            ManifestEntry::new(TraceNode::NOTA_DECODER, trace_phase),
            ManifestEntry::new(TraceNode::CALLER_IDENTITY_RESOLVER, trace_phase),
            ManifestEntry::new(TraceNode::ENVELOPE_BUILDER, trace_phase),
            ManifestEntry::new(TraceNode::DISPATCH_PHASE, long_lived),
            ManifestEntry::new(TraceNode::REQUEST_DISPATCHER, trace_phase),
            ManifestEntry::new(TraceNode::MEMORY_FLOW, trace_phase),
            ManifestEntry::new(TraceNode::QUERY_FLOW, trace_phase),
            ManifestEntry::new(TraceNode::GRAPH_FLOW, trace_phase),
            ManifestEntry::new(TraceNode::GRAPH_QUERY_FLOW, trace_phase),
            ManifestEntry::new(TraceNode::CHOREOGRAPHY_ADJUDICATOR, long_lived),
            ManifestEntry::new(TraceNode::MIND_ORCHESTRATE_CALLER, long_lived),
            ManifestEntry::new(TraceNode::DOMAIN_PHASE, long_lived),
            ManifestEntry::new(TraceNode::MEMORY_GRAPH_SUPERVISOR, trace_phase),
            ManifestEntry::new(TraceNode::MIND_GRAPH_SUPERVISOR, trace_phase),
            ManifestEntry::new(TraceNode::QUERY_SUPERVISOR, trace_phase),
            ManifestEntry::new(TraceNode::ITEM_OPEN, trace_phase),
            ManifestEntry::new(TraceNode::NOTE_ADD, trace_phase),
            ManifestEntry::new(TraceNode::LINK, trace_phase),
            ManifestEntry::new(TraceNode::STATUS_CHANGE, trace_phase),
            ManifestEntry::new(TraceNode::ALIAS_ADD, trace_phase),
            ManifestEntry::new(TraceNode::QUERY_PLANNER, trace_phase),
            ManifestEntry::new(TraceNode::GRAPH_TRAVERSAL, trace_phase),
            ManifestEntry::new(TraceNode::THOUGHT_COMMIT, trace_phase),
            ManifestEntry::new(TraceNode::RELATION_COMMIT, trace_phase),
            ManifestEntry::new(TraceNode::THOUGHT_QUERY, trace_phase),
            ManifestEntry::new(TraceNode::RELATION_QUERY, trace_phase),
            ManifestEntry::new(TraceNode::QUERY_RESULT_SHAPER, trace_phase),
            ManifestEntry::new(TraceNode::STORE_SUPERVISOR, long_lived),
            ManifestEntry::new(TraceNode::STORE_KERNEL, long_lived),
            ManifestEntry::new(TraceNode::MEMORY_STORE, long_lived),
            ManifestEntry::new(TraceNode::GRAPH_STORE, long_lived),
            ManifestEntry::new(TraceNode::SEMA_WRITER, trace_phase),
            ManifestEntry::new(TraceNode::SEMA_READER, trace_phase),
            ManifestEntry::new(TraceNode::ID_MINT, trace_phase),
            ManifestEntry::new(TraceNode::CLOCK, trace_phase),
            ManifestEntry::new(TraceNode::EVENT_APPENDER, trace_phase),
            ManifestEntry::new(TraceNode::COMMIT, trace_phase),
            ManifestEntry::new(TraceNode::VIEW_PHASE, long_lived),
            ManifestEntry::new(TraceNode::READY_WORK_VIEW, trace_phase),
            ManifestEntry::new(TraceNode::BLOCKED_WORK_VIEW, trace_phase),
            ManifestEntry::new(TraceNode::RECENT_ACTIVITY_VIEW, trace_phase),
            ManifestEntry::new(TraceNode::SUBSCRIPTION_SUPERVISOR, long_lived),
            ManifestEntry::new(TraceNode::COMMIT_BUS, trace_phase),
            ManifestEntry::new(TraceNode::SUBSCRIBER, trace_phase),
            ManifestEntry::new(TraceNode::REPLY_SHAPER, long_lived),
            ManifestEntry::new(TraceNode::NOTA_REPLY_ENCODER, trace_phase),
            ManifestEntry::new(TraceNode::ERROR_SHAPER, trace_phase),
        ];

        let edges = vec![
            ManifestEdge::new(TraceNode::MIND_ROOT, TraceNode::INGRESS_PHASE),
            ManifestEdge::new(TraceNode::MIND_ROOT, TraceNode::DISPATCH_PHASE),
            ManifestEdge::new(TraceNode::MIND_ROOT, TraceNode::DOMAIN_PHASE),
            ManifestEdge::new(TraceNode::MIND_ROOT, TraceNode::STORE_SUPERVISOR),
            ManifestEdge::new(TraceNode::MIND_ROOT, TraceNode::VIEW_PHASE),
            ManifestEdge::new(TraceNode::MIND_ROOT, TraceNode::SUBSCRIPTION_SUPERVISOR),
            ManifestEdge::new(TraceNode::MIND_ROOT, TraceNode::CHOREOGRAPHY_ADJUDICATOR),
            ManifestEdge::new(TraceNode::MIND_ROOT, TraceNode::REPLY_SHAPER),
            ManifestEdge::new(TraceNode::INGRESS_PHASE, TraceNode::REQUEST_SESSION),
            ManifestEdge::new(TraceNode::INGRESS_PHASE, TraceNode::NOTA_DECODER),
            ManifestEdge::new(
                TraceNode::INGRESS_PHASE,
                TraceNode::CALLER_IDENTITY_RESOLVER,
            ),
            ManifestEdge::new(TraceNode::INGRESS_PHASE, TraceNode::ENVELOPE_BUILDER),
            ManifestEdge::new(TraceNode::DISPATCH_PHASE, TraceNode::REQUEST_DISPATCHER),
            ManifestEdge::new(TraceNode::DISPATCH_PHASE, TraceNode::MEMORY_FLOW),
            ManifestEdge::new(TraceNode::DISPATCH_PHASE, TraceNode::QUERY_FLOW),
            ManifestEdge::new(TraceNode::DISPATCH_PHASE, TraceNode::GRAPH_FLOW),
            ManifestEdge::new(TraceNode::DISPATCH_PHASE, TraceNode::GRAPH_QUERY_FLOW),
            ManifestEdge::new(
                TraceNode::CHOREOGRAPHY_ADJUDICATOR,
                TraceNode::MIND_ORCHESTRATE_CALLER,
            ),
            ManifestEdge::new(TraceNode::DOMAIN_PHASE, TraceNode::MEMORY_GRAPH_SUPERVISOR),
            ManifestEdge::new(TraceNode::DOMAIN_PHASE, TraceNode::MIND_GRAPH_SUPERVISOR),
            ManifestEdge::new(TraceNode::DOMAIN_PHASE, TraceNode::QUERY_SUPERVISOR),
            ManifestEdge::new(TraceNode::MIND_GRAPH_SUPERVISOR, TraceNode::THOUGHT_COMMIT),
            ManifestEdge::new(TraceNode::MIND_GRAPH_SUPERVISOR, TraceNode::RELATION_COMMIT),
            ManifestEdge::new(TraceNode::MEMORY_GRAPH_SUPERVISOR, TraceNode::ITEM_OPEN),
            ManifestEdge::new(TraceNode::MEMORY_GRAPH_SUPERVISOR, TraceNode::NOTE_ADD),
            ManifestEdge::new(TraceNode::MEMORY_GRAPH_SUPERVISOR, TraceNode::LINK),
            ManifestEdge::new(TraceNode::MEMORY_GRAPH_SUPERVISOR, TraceNode::STATUS_CHANGE),
            ManifestEdge::new(TraceNode::MEMORY_GRAPH_SUPERVISOR, TraceNode::ALIAS_ADD),
            ManifestEdge::new(TraceNode::QUERY_SUPERVISOR, TraceNode::QUERY_PLANNER),
            ManifestEdge::new(TraceNode::QUERY_SUPERVISOR, TraceNode::GRAPH_TRAVERSAL),
            ManifestEdge::new(TraceNode::QUERY_SUPERVISOR, TraceNode::THOUGHT_QUERY),
            ManifestEdge::new(TraceNode::QUERY_SUPERVISOR, TraceNode::RELATION_QUERY),
            ManifestEdge::new(TraceNode::QUERY_SUPERVISOR, TraceNode::QUERY_RESULT_SHAPER),
            ManifestEdge::new(TraceNode::STORE_SUPERVISOR, TraceNode::STORE_KERNEL),
            ManifestEdge::new(TraceNode::STORE_SUPERVISOR, TraceNode::MEMORY_STORE),
            ManifestEdge::new(TraceNode::STORE_SUPERVISOR, TraceNode::GRAPH_STORE),
            ManifestEdge::new(TraceNode::MEMORY_STORE, TraceNode::STORE_KERNEL),
            ManifestEdge::new(TraceNode::GRAPH_STORE, TraceNode::STORE_KERNEL),
            ManifestEdge::new(TraceNode::MEMORY_STORE, TraceNode::SEMA_WRITER),
            ManifestEdge::new(TraceNode::MEMORY_STORE, TraceNode::SEMA_READER),
            ManifestEdge::new(TraceNode::MEMORY_STORE, TraceNode::ID_MINT),
            ManifestEdge::new(TraceNode::MEMORY_STORE, TraceNode::CLOCK),
            ManifestEdge::new(TraceNode::MEMORY_STORE, TraceNode::EVENT_APPENDER),
            ManifestEdge::new(TraceNode::MEMORY_STORE, TraceNode::COMMIT),
            ManifestEdge::new(TraceNode::GRAPH_STORE, TraceNode::SEMA_WRITER),
            ManifestEdge::new(TraceNode::GRAPH_STORE, TraceNode::SEMA_READER),
            ManifestEdge::new(TraceNode::GRAPH_STORE, TraceNode::ID_MINT),
            ManifestEdge::new(TraceNode::GRAPH_STORE, TraceNode::CLOCK),
            ManifestEdge::new(TraceNode::GRAPH_STORE, TraceNode::EVENT_APPENDER),
            ManifestEdge::new(TraceNode::GRAPH_STORE, TraceNode::COMMIT),
            ManifestEdge::new(TraceNode::VIEW_PHASE, TraceNode::READY_WORK_VIEW),
            ManifestEdge::new(TraceNode::VIEW_PHASE, TraceNode::BLOCKED_WORK_VIEW),
            ManifestEdge::new(TraceNode::VIEW_PHASE, TraceNode::RECENT_ACTIVITY_VIEW),
            ManifestEdge::new(TraceNode::SUBSCRIPTION_SUPERVISOR, TraceNode::COMMIT_BUS),
            ManifestEdge::new(TraceNode::SUBSCRIPTION_SUPERVISOR, TraceNode::SUBSCRIBER),
            ManifestEdge::new(TraceNode::REPLY_SHAPER, TraceNode::NOTA_REPLY_ENCODER),
            ManifestEdge::new(TraceNode::REPLY_SHAPER, TraceNode::ERROR_SHAPER),
        ];

        Self { actors, edges }
    }

    pub fn actors(&self) -> &[ManifestEntry] {
        &self.actors
    }

    pub fn edges(&self) -> &[ManifestEdge] {
        &self.edges
    }

    pub fn contains(&self, actor: TraceNode) -> bool {
        self.actors.iter().any(|entry| entry.kind == actor)
    }

    pub fn contains_edge(&self, parent: TraceNode, child: TraceNode) -> bool {
        self.edges
            .iter()
            .any(|edge| edge.parent == parent && edge.child == child)
    }

    pub fn actor_count_for(&self, residency: ActorResidency) -> usize {
        self.actors
            .iter()
            .filter(|actor| actor.residency == residency)
            .count()
    }
}
