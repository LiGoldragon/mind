#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TraceNode {
    label: &'static str,
}

impl TraceNode {
    pub const MIND_ROOT: Self = Self::new("MindRoot");
    pub const INGRESS_PHASE: Self = Self::new("IngressPhase");
    pub const REQUEST_SESSION: Self = Self::new("RequestSession");
    pub const NOTA_DECODER: Self = Self::new("NotaDecoder");
    pub const CALLER_IDENTITY_RESOLVER: Self = Self::new("CallerIdentityResolver");
    pub const ENVELOPE_BUILDER: Self = Self::new("EnvelopeBuilder");
    pub const DISPATCH_PHASE: Self = Self::new("DispatchPhase");
    pub const REQUEST_DISPATCHER: Self = Self::new("RequestDispatcher");
    pub const MEMORY_FLOW: Self = Self::new("MemoryFlow");
    pub const QUERY_FLOW: Self = Self::new("QueryFlow");
    pub const GRAPH_FLOW: Self = Self::new("GraphFlow");
    pub const GRAPH_QUERY_FLOW: Self = Self::new("GraphQueryFlow");
    pub const DOMAIN_PHASE: Self = Self::new("DomainPhase");
    pub const MEMORY_GRAPH_SUPERVISOR: Self = Self::new("MemoryGraphSupervisor");
    pub const MIND_GRAPH_SUPERVISOR: Self = Self::new("MindGraphSupervisor");
    pub const QUERY_SUPERVISOR: Self = Self::new("QuerySupervisor");
    pub const ITEM_OPEN: Self = Self::new("ItemOpen");
    pub const NOTE_ADD: Self = Self::new("NoteAdd");
    pub const LINK: Self = Self::new("Link");
    pub const STATUS_CHANGE: Self = Self::new("StatusChange");
    pub const ALIAS_ADD: Self = Self::new("AliasAdd");
    pub const QUERY_PLANNER: Self = Self::new("QueryPlanner");
    pub const GRAPH_TRAVERSAL: Self = Self::new("GraphTraversal");
    pub const THOUGHT_COMMIT: Self = Self::new("ThoughtCommit");
    pub const RELATION_COMMIT: Self = Self::new("RelationCommit");
    pub const THOUGHT_QUERY: Self = Self::new("ThoughtQuery");
    pub const RELATION_QUERY: Self = Self::new("RelationQuery");
    pub const QUERY_RESULT_SHAPER: Self = Self::new("QueryResultShaper");
    pub const STORE_SUPERVISOR: Self = Self::new("StoreSupervisor");
    pub const STORE_KERNEL: Self = Self::new("StoreKernel");
    pub const MEMORY_STORE: Self = Self::new("MemoryStore");
    pub const GRAPH_STORE: Self = Self::new("GraphStore");
    pub const SEMA_WRITER: Self = Self::new("SemaWriter");
    pub const SEMA_READER: Self = Self::new("SemaReader");
    pub const ID_MINT: Self = Self::new("IdMint");
    pub const CLOCK: Self = Self::new("Clock");
    pub const EVENT_APPENDER: Self = Self::new("EventAppender");
    pub const COMMIT: Self = Self::new("Commit");
    pub const VIEW_PHASE: Self = Self::new("ViewPhase");
    pub const READY_WORK_VIEW: Self = Self::new("ReadyWorkView");
    pub const BLOCKED_WORK_VIEW: Self = Self::new("BlockedWorkView");
    pub const RECENT_ACTIVITY_VIEW: Self = Self::new("RecentActivityView");
    pub const SUBSCRIPTION_SUPERVISOR: Self = Self::new("SubscriptionSupervisor");
    pub const COMMIT_BUS: Self = Self::new("CommitBus");
    pub const SUBSCRIBER: Self = Self::new("Subscriber");
    pub const REPLY_SHAPER: Self = Self::new("ReplyShaper");
    pub const NOTA_REPLY_ENCODER: Self = Self::new("NotaReplyEncoder");
    pub const ERROR_SHAPER: Self = Self::new("ErrorShaper");

    pub const fn new(label: &'static str) -> Self {
        Self { label }
    }

    pub fn label(self) -> &'static str {
        self.label
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TraceAction {
    ActorStarted,
    MessageReceived,
    MessageReplied,
    ChildSpawned,
    WriteIntentSent,
    CommitCompleted,
    ViewRefreshed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TraceEvent {
    actor: TraceNode,
    action: TraceAction,
}

impl TraceEvent {
    pub fn new(actor: TraceNode, action: TraceAction) -> Self {
        Self { actor, action }
    }

    pub fn actor(&self) -> TraceNode {
        self.actor
    }

    pub fn action(&self) -> TraceAction {
        self.action
    }
}

#[derive(Debug, Clone, PartialEq, Eq, kameo::Reply)]
pub struct ActorTrace {
    events: Vec<TraceEvent>,
}

impl ActorTrace {
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    pub fn events(&self) -> &[TraceEvent] {
        &self.events
    }

    pub fn record(&mut self, actor: TraceNode, action: TraceAction) {
        self.events.push(TraceEvent::new(actor, action));
    }

    pub fn contains(&self, actor: TraceNode) -> bool {
        self.events.iter().any(|event| event.actor == actor)
    }

    pub fn contains_action(&self, actor: TraceNode, action: TraceAction) -> bool {
        self.events
            .iter()
            .any(|event| event.actor == actor && event.action == action)
    }

    pub fn contains_ordered(&self, actors: &[TraceNode]) -> bool {
        let mut remaining = actors.iter();
        let Some(mut expected) = remaining.next() else {
            return true;
        };

        for event in &self.events {
            if event.actor == *expected {
                match remaining.next() {
                    Some(next) => expected = next,
                    None => return true,
                }
            }
        }

        false
    }
}

impl Default for ActorTrace {
    fn default() -> Self {
        Self::new()
    }
}
