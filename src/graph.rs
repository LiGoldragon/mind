use std::collections::{HashSet, VecDeque};

use signal_mind::{
    AboutTechnicalNode, AcceptedSubscriptionStream, ByRelationKind, ByRelationSource,
    ByRelationTarget, ByTechnicalNodeKind, ByTechnicalNodeStableKey, ByTechnicalRelationEndpoints,
    ByTechnicalRelationKind, ByTechnicalRelationSource, ByTechnicalRelationTarget,
    ByTechnicalSourceLocator, ByThoughtAuthor, ByThoughtKind, ByThoughtTimeRange,
    CompositeRelationFilter, CompositeTechnicalNodeFilter, CompositeTechnicalRelationFilter,
    CompositeThoughtFilter, DisplayIdentifier, MindReply, MindRequestUnimplemented,
    MindUnimplementedReason, QueryLimit, QueryRelations, QueryTechnicalNodes,
    QueryTechnicalRelations, QueryThoughts, Relation, RelationCommitted, RelationFilter,
    RelationKind, RelationList, RelationStreamAccepted, SubmitRelation, SubmitTechnicalNode,
    SubmitTechnicalRelation, SubmitThought, SubscriptionAccepted, SubscriptionCursor,
    TechnicalDependencyClosure, TechnicalDependencyClosureQuery, TechnicalNode,
    TechnicalNodeCommitted, TechnicalNodeFilter, TechnicalNodeList, TechnicalNodeNeighborhood,
    TechnicalNodeQuery, TechnicalNodeRejected, TechnicalNodeRejectionReason,
    TechnicalNodeStreamAccepted, TechnicalProvenanceChain, TechnicalProvenanceChainQuery,
    TechnicalRelation, TechnicalRelationCommitted, TechnicalRelationFilter, TechnicalRelationKind,
    TechnicalRelationList, TechnicalRelationNeighborhoodDirection,
    TechnicalRelationNeighborhoodQuery, TechnicalRelationRejected,
    TechnicalRelationRejectionReason, TechnicalRelationStreamAccepted, TechnicalSourceLocator,
    Thought, ThoughtCommitted, ThoughtFilter, ThoughtList, ThoughtStreamAccepted,
};

use crate::{MindEnvelope, MindTables, Result};

pub(crate) struct MindGraphLedger<'tables> {
    tables: &'tables MindTables,
}

impl<'tables> MindGraphLedger<'tables> {
    pub(crate) fn new(tables: &'tables MindTables) -> Self {
        Self { tables }
    }

    pub(crate) fn submit_thought(&self, envelope: MindEnvelope) -> Result<MindReply> {
        let actor = envelope.actor().clone();
        let MindEnvelope { request, .. } = envelope;
        match request {
            signal_mind::MindRequest::SubmitThought(submission) => {
                self.commit_thought(actor, submission)
            }
            _ => Ok(Self::unimplemented()),
        }
    }

    pub(crate) fn submit_relation(&self, envelope: MindEnvelope) -> Result<MindReply> {
        let actor = envelope.actor().clone();
        let MindEnvelope { request, .. } = envelope;
        match request {
            signal_mind::MindRequest::SubmitRelation(submission) => {
                self.commit_relation(actor, submission)
            }
            _ => Ok(Self::unimplemented()),
        }
    }

    pub(crate) fn submit_technical_node(&self, envelope: MindEnvelope) -> Result<MindReply> {
        let actor = envelope.actor().clone();
        let MindEnvelope { request, .. } = envelope;
        match request {
            signal_mind::MindRequest::SubmitTechnicalNode(submission) => {
                Ok(self.commit_technical_node(actor, submission))
            }
            _ => Ok(Self::unimplemented()),
        }
    }

    pub(crate) fn submit_technical_relation(&self, envelope: MindEnvelope) -> Result<MindReply> {
        let actor = envelope.actor().clone();
        let MindEnvelope { request, .. } = envelope;
        match request {
            signal_mind::MindRequest::SubmitTechnicalRelation(submission) => {
                Ok(self.commit_technical_relation(actor, submission))
            }
            _ => Ok(Self::unimplemented()),
        }
    }

    pub(crate) fn query_thoughts(&self, envelope: MindEnvelope) -> Result<MindReply> {
        let MindEnvelope { request, .. } = envelope;
        match request {
            signal_mind::MindRequest::QueryThoughts(query) => self.read_thoughts(query),
            _ => Ok(Self::unimplemented()),
        }
    }

    pub(crate) fn query_relations(&self, envelope: MindEnvelope) -> Result<MindReply> {
        let MindEnvelope { request, .. } = envelope;
        match request {
            signal_mind::MindRequest::QueryRelations(query) => self.read_relations(query),
            _ => Ok(Self::unimplemented()),
        }
    }

    pub(crate) fn query_technical_nodes(&self, envelope: MindEnvelope) -> Result<MindReply> {
        let MindEnvelope { request, .. } = envelope;
        match request {
            signal_mind::MindRequest::QueryTechnicalNodes(query) => {
                self.read_technical_nodes(query)
            }
            _ => Ok(Self::unimplemented()),
        }
    }

    pub(crate) fn query_technical_relations(&self, envelope: MindEnvelope) -> Result<MindReply> {
        let MindEnvelope { request, .. } = envelope;
        match request {
            signal_mind::MindRequest::QueryTechnicalRelations(query) => {
                self.read_technical_relations(query)
            }
            _ => Ok(Self::unimplemented()),
        }
    }

    pub(crate) fn subscribe_thoughts(&self, envelope: MindEnvelope) -> Result<MindReply> {
        let MindEnvelope { request, .. } = envelope;
        match request {
            signal_mind::MindRequest::SubscribeThoughts(subscription) => {
                self.open_thought_subscription(subscription)
            }
            _ => Ok(Self::unimplemented()),
        }
    }

    pub(crate) fn subscribe_relations(&self, envelope: MindEnvelope) -> Result<MindReply> {
        let MindEnvelope { request, .. } = envelope;
        match request {
            signal_mind::MindRequest::SubscribeRelations(subscription) => {
                self.open_relation_subscription(subscription)
            }
            _ => Ok(Self::unimplemented()),
        }
    }

    pub(crate) fn subscribe_technical_nodes(&self, envelope: MindEnvelope) -> Result<MindReply> {
        let MindEnvelope { request, .. } = envelope;
        match request {
            signal_mind::MindRequest::SubscribeTechnicalNodes(subscription) => {
                self.open_technical_node_subscription(subscription)
            }
            _ => Ok(Self::unimplemented()),
        }
    }

    pub(crate) fn subscribe_technical_relations(
        &self,
        envelope: MindEnvelope,
    ) -> Result<MindReply> {
        let MindEnvelope { request, .. } = envelope;
        match request {
            signal_mind::MindRequest::SubscribeTechnicalRelations(subscription) => {
                self.open_technical_relation_subscription(subscription)
            }
            _ => Ok(Self::unimplemented()),
        }
    }

    fn commit_thought(
        &self,
        actor: signal_mind::ActorName,
        submission: SubmitThought,
    ) -> Result<MindReply> {
        let thought = self.tables.append_thought(actor, submission)?;
        Ok(MindReply::ThoughtCommitted(ThoughtCommitted {
            display: DisplayIdentifier::new(thought.id.as_str()),
            record: thought.id,
            occurred_at: thought.occurred_at,
        }))
    }

    fn commit_relation(
        &self,
        actor: signal_mind::ActorName,
        submission: SubmitRelation,
    ) -> Result<MindReply> {
        let relation = self.tables.append_relation(actor, submission)?;
        Ok(MindReply::RelationCommitted(RelationCommitted {
            relation: relation.id,
            occurred_at: relation.occurred_at,
        }))
    }

    fn commit_technical_node(
        &self,
        actor: signal_mind::ActorName,
        submission: SubmitTechnicalNode,
    ) -> MindReply {
        match self.tables.append_technical_node(actor, submission) {
            Ok(Ok(node)) => MindReply::TechnicalNodeCommitted(TechnicalNodeCommitted { node }),
            Ok(Err(reason)) => MindReply::TechnicalNodeRejected(TechnicalNodeRejected { reason }),
            Err(_error) => MindReply::TechnicalNodeRejected(TechnicalNodeRejected {
                reason: TechnicalNodeRejectionReason::PersistenceRejected,
            }),
        }
    }

    fn commit_technical_relation(
        &self,
        actor: signal_mind::ActorName,
        submission: SubmitTechnicalRelation,
    ) -> MindReply {
        match self.tables.append_technical_relation(actor, submission) {
            Ok(Ok(relation)) => {
                MindReply::TechnicalRelationCommitted(TechnicalRelationCommitted { relation })
            }
            Ok(Err(reason)) => {
                MindReply::TechnicalRelationRejected(TechnicalRelationRejected { reason })
            }
            Err(_error) => MindReply::TechnicalRelationRejected(TechnicalRelationRejected {
                reason: TechnicalRelationRejectionReason::PersistenceRejected,
            }),
        }
    }

    fn read_thoughts(&self, query: QueryThoughts) -> Result<MindReply> {
        let relations = self.tables.relation_records()?;
        let selector = ThoughtSelector::new(query.filter, relations);
        let mut matches = self
            .tables
            .thought_records()?
            .into_iter()
            .filter(|thought| selector.accepts(thought))
            .collect::<Vec<_>>();
        matches.sort_by_key(|thought| thought.occurred_at.value());
        let limited = GraphLimit::new(query.limit).apply(matches);
        Ok(MindReply::ThoughtList(ThoughtList {
            thoughts: limited.records,
            has_more: limited.has_more,
        }))
    }

    fn read_relations(&self, query: QueryRelations) -> Result<MindReply> {
        let selector = RelationSelector::new(query.filter);
        let mut matches = self
            .tables
            .relation_records()?
            .into_iter()
            .filter(|relation| selector.accepts(relation))
            .collect::<Vec<_>>();
        matches.sort_by_key(|relation| relation.occurred_at.value());
        let limited = GraphLimit::new(query.limit).apply(matches);
        Ok(MindReply::RelationList(RelationList {
            relations: limited.records,
            has_more: limited.has_more,
        }))
    }

    fn read_technical_nodes(&self, query: QueryTechnicalNodes) -> Result<MindReply> {
        Ok(TechnicalNodeQueryEngine::new(
            self.tables.technical_node_records()?,
            self.tables.technical_relation_records()?,
            query.limit,
        )
        .reply(query.query))
    }

    fn read_technical_relations(&self, query: QueryTechnicalRelations) -> Result<MindReply> {
        let selector = TechnicalRelationSelector::new(query.filter);
        let mut matches = self
            .tables
            .technical_relation_records()?
            .into_iter()
            .filter(|relation| selector.accepts(relation))
            .collect::<Vec<_>>();
        matches.sort_by_key(|relation| relation.occurred_at.value());
        let limited = GraphLimit::new(query.limit).apply(matches);
        Ok(MindReply::TechnicalRelationList(TechnicalRelationList {
            relations: limited.records,
            has_more: limited.has_more,
        }))
    }

    fn open_thought_subscription(
        &self,
        subscription: signal_mind::SubscribeThoughts,
    ) -> Result<MindReply> {
        let opened = self.tables.append_thought_subscription(subscription)?;
        let relations = self.tables.relation_records()?;
        let selector = ThoughtSelector::new(opened.record().filter.clone(), relations);
        let resumed = ResumedSnapshot::new(
            opened
                .initial()
                .iter()
                .filter(|thought| selector.accepts(thought))
                .cloned()
                .collect(),
            opened.resume_after(),
        );
        self.tables.register_thought_runtime(
            opened.record(),
            resumed.cursor,
            opened.initial_demand(),
        );
        Ok(MindReply::SubscriptionAccepted(SubscriptionAccepted {
            subscription: opened.record().subscription.clone(),
            stream: AcceptedSubscriptionStream::Thoughts(ThoughtStreamAccepted {
                cursor: resumed.cursor,
                buffer_bound: crate::actors::subscription::SubscriptionSupervisor::BUFFER_BOUND,
                snapshot: resumed.records,
            }),
        }))
    }

    fn open_relation_subscription(
        &self,
        subscription: signal_mind::SubscribeRelations,
    ) -> Result<MindReply> {
        let opened = self.tables.append_relation_subscription(subscription)?;
        let selector = RelationSelector::new(opened.record().filter.clone());
        let resumed = ResumedSnapshot::new(
            opened
                .initial()
                .iter()
                .filter(|relation| selector.accepts(relation))
                .cloned()
                .collect(),
            opened.resume_after(),
        );
        self.tables.register_relation_runtime(
            opened.record(),
            resumed.cursor,
            opened.initial_demand(),
        );
        Ok(MindReply::SubscriptionAccepted(SubscriptionAccepted {
            subscription: opened.record().subscription.clone(),
            stream: AcceptedSubscriptionStream::Relations(RelationStreamAccepted {
                cursor: resumed.cursor,
                buffer_bound: crate::actors::subscription::SubscriptionSupervisor::BUFFER_BOUND,
                snapshot: resumed.records,
            }),
        }))
    }

    fn open_technical_node_subscription(
        &self,
        subscription: signal_mind::SubscribeTechnicalNodes,
    ) -> Result<MindReply> {
        let opened = self
            .tables
            .append_technical_node_subscription(subscription)?;
        let selector = TechnicalNodeSelector::new(opened.record().filter.clone());
        let resumed = ResumedSnapshot::new(
            opened
                .initial()
                .iter()
                .filter(|node| selector.accepts(node))
                .cloned()
                .collect(),
            opened.resume_after(),
        );
        self.tables.register_technical_node_runtime(
            opened.record(),
            resumed.cursor,
            opened.initial_demand(),
        );
        Ok(MindReply::SubscriptionAccepted(SubscriptionAccepted {
            subscription: opened.record().subscription.clone(),
            stream: AcceptedSubscriptionStream::TechnicalNodes(TechnicalNodeStreamAccepted {
                cursor: resumed.cursor,
                buffer_bound: crate::actors::subscription::SubscriptionSupervisor::BUFFER_BOUND,
                snapshot: resumed.records,
            }),
        }))
    }

    fn open_technical_relation_subscription(
        &self,
        subscription: signal_mind::SubscribeTechnicalRelations,
    ) -> Result<MindReply> {
        let opened = self
            .tables
            .append_technical_relation_subscription(subscription)?;
        let selector = TechnicalRelationSelector::new(opened.record().filter.clone());
        let resumed = ResumedSnapshot::new(
            opened
                .initial()
                .iter()
                .filter(|relation| selector.accepts(relation))
                .cloned()
                .collect(),
            opened.resume_after(),
        );
        self.tables.register_technical_relation_runtime(
            opened.record(),
            resumed.cursor,
            opened.initial_demand(),
        );
        Ok(MindReply::SubscriptionAccepted(SubscriptionAccepted {
            subscription: opened.record().subscription.clone(),
            stream: AcceptedSubscriptionStream::TechnicalRelations(
                TechnicalRelationStreamAccepted {
                    cursor: resumed.cursor,
                    buffer_bound: crate::actors::subscription::SubscriptionSupervisor::BUFFER_BOUND,
                    snapshot: resumed.records,
                },
            ),
        }))
    }

    fn unimplemented() -> MindReply {
        MindReply::MindRequestUnimplemented(MindRequestUnimplemented {
            reason: MindUnimplementedReason::NotInPrototypeScope,
        })
    }
}

struct ResumedSnapshot<Record> {
    records: Vec<Record>,
    cursor: SubscriptionCursor,
}

impl<Record> ResumedSnapshot<Record> {
    fn new(records: Vec<Record>, resume_after: SubscriptionCursor) -> Self {
        let records = records
            .into_iter()
            .skip(resume_after.into_u64() as usize)
            .collect::<Vec<_>>();
        let cursor = SubscriptionCursor::new(resume_after.into_u64() + records.len() as u64);
        Self { records, cursor }
    }
}

pub(crate) struct ThoughtSelector {
    filter: ThoughtFilter,
    relations: Vec<Relation>,
}

impl ThoughtSelector {
    pub(crate) fn new(filter: ThoughtFilter, relations: Vec<Relation>) -> Self {
        Self { filter, relations }
    }

    pub(crate) fn accepts(&self, thought: &Thought) -> bool {
        !self.is_superseded(thought) && self.accepts_filter(thought, &self.filter)
    }

    fn is_superseded(&self, thought: &Thought) -> bool {
        self.relations.iter().any(|relation| {
            relation.kind == RelationKind::Supersedes && relation.target == thought.id
        })
    }

    fn accepts_filter(&self, thought: &Thought, filter: &ThoughtFilter) -> bool {
        match filter {
            ThoughtFilter::ByKind(kind) => self.accepts_kind(thought, kind),
            ThoughtFilter::ByAuthor(author) => self.accepts_author(thought, author),
            ThoughtFilter::ByTimeRange(range) => self.accepts_time_range(thought, range),
            ThoughtFilter::InGoal(goal) => self.accepts_membership(thought, &goal.goal),
            ThoughtFilter::InMemory(memory) => self.accepts_membership(thought, &memory.memory),
            ThoughtFilter::Composite(composite) => self.accepts_composite(thought, composite),
        }
    }

    fn accepts_kind(&self, thought: &Thought, kind: &ByThoughtKind) -> bool {
        kind.kinds.is_empty() || kind.kinds.contains(&thought.kind)
    }

    fn accepts_author(&self, thought: &Thought, author: &ByThoughtAuthor) -> bool {
        thought.author == author.author
    }

    fn accepts_time_range(&self, thought: &Thought, range: &ByThoughtTimeRange) -> bool {
        let occurred = thought.occurred_at.value();
        let starts_after = occurred >= range.start.value();
        let ends_before = range.end.map(|end| occurred <= end.value()).unwrap_or(true);
        starts_after && ends_before
    }

    fn accepts_membership(
        &self,
        thought: &Thought,
        container: &signal_mind::RecordIdentifier,
    ) -> bool {
        thought.id == *container
            || self.relations.iter().any(|relation| {
                relation.kind == RelationKind::Belongs
                    && relation.source == thought.id
                    && relation.target == *container
            })
    }

    fn accepts_composite(&self, thought: &Thought, composite: &CompositeThoughtFilter) -> bool {
        let kind_ok = composite.kinds.is_empty() || composite.kinds.contains(&thought.kind);
        let author_ok = composite
            .author
            .as_ref()
            .map(|author| thought.author == *author)
            .unwrap_or(true);
        let time_ok = composite
            .time_range
            .as_ref()
            .map(|range| self.accepts_time_range(thought, range))
            .unwrap_or(true);
        let goal_ok = composite
            .goal
            .as_ref()
            .map(|goal| self.accepts_membership(thought, goal))
            .unwrap_or(true);
        let memory_ok = composite
            .memory
            .as_ref()
            .map(|memory| self.accepts_membership(thought, memory))
            .unwrap_or(true);
        kind_ok && author_ok && time_ok && goal_ok && memory_ok
    }
}

pub(crate) struct RelationSelector {
    filter: RelationFilter,
}

struct TechnicalNodeQueryEngine {
    nodes: Vec<TechnicalNode>,
    relations: Vec<TechnicalRelation>,
    limit: GraphLimit,
}

impl TechnicalNodeQueryEngine {
    fn new(
        mut nodes: Vec<TechnicalNode>,
        mut relations: Vec<TechnicalRelation>,
        limit: QueryLimit,
    ) -> Self {
        nodes.sort_by_key(|node| node.occurred_at.value());
        relations.sort_by_key(|relation| relation.occurred_at.value());
        Self {
            nodes,
            relations,
            limit: GraphLimit::new(limit),
        }
    }

    fn reply(&self, query: TechnicalNodeQuery) -> MindReply {
        match query {
            TechnicalNodeQuery::Filter(filter) => self.filtered(filter),
            TechnicalNodeQuery::About(query) => self.about(query),
            TechnicalNodeQuery::RelationNeighborhood(query) => self.neighborhood(query),
            TechnicalNodeQuery::DependencyClosure(query) => self.dependency_closure(query),
            TechnicalNodeQuery::ProvenanceChain(query) => self.provenance_chain(query),
        }
    }

    fn filtered(&self, filter: TechnicalNodeFilter) -> MindReply {
        let selector = TechnicalNodeSelector::new(filter);
        let matches = self
            .nodes
            .iter()
            .filter(|node| selector.accepts(node))
            .cloned()
            .collect::<Vec<_>>();
        let limited = self.limit.apply(matches);
        MindReply::TechnicalNodeList(TechnicalNodeList {
            nodes: limited.records,
            has_more: limited.has_more,
        })
    }

    fn about(&self, query: AboutTechnicalNode) -> MindReply {
        self.neighborhood(TechnicalRelationNeighborhoodQuery {
            stable_key: query.stable_key,
            direction: TechnicalRelationNeighborhoodDirection::Both,
            kinds: Vec::new(),
        })
    }

    fn neighborhood(&self, query: TechnicalRelationNeighborhoodQuery) -> MindReply {
        let records = TechnicalNeighborhoodRecords::new(
            self.node_by_key(&query.stable_key),
            self.relations
                .iter()
                .filter(|relation| {
                    TechnicalRelationKindSet::new(&query.kinds).contains(relation.kind)
                        && self.neighborhood_direction_accepts(&query, relation)
                })
                .cloned()
                .collect(),
        )
        .limited(&self.limit);
        MindReply::TechnicalNodeNeighborhood(TechnicalNodeNeighborhood {
            center: records.center,
            incoming: records.incoming,
            outgoing: records.outgoing,
            has_more: records.has_more,
        })
    }

    fn dependency_closure(&self, query: TechnicalDependencyClosureQuery) -> MindReply {
        let records = self.traverse(
            query.stable_key,
            TechnicalRelationKindSet::dependency(query.kinds),
        );
        MindReply::TechnicalDependencyClosure(TechnicalDependencyClosure {
            root: records.root,
            nodes: records.nodes,
            relations: records.relations,
            has_more: records.has_more,
        })
    }

    fn provenance_chain(&self, query: TechnicalProvenanceChainQuery) -> MindReply {
        let records = self.traverse(
            query.stable_key,
            TechnicalRelationKindSet::provenance(query.kinds),
        );
        MindReply::TechnicalProvenanceChain(TechnicalProvenanceChain {
            root: records.root,
            nodes: records.nodes,
            relations: records.relations,
            has_more: records.has_more,
        })
    }

    fn traverse(
        &self,
        stable_key: signal_mind::TechnicalNodeKey,
        kinds: TechnicalRelationKindSet,
    ) -> TechnicalTraversalRecords {
        let Some(root) = self.node_by_key(&stable_key) else {
            return TechnicalTraversalRecords::empty();
        };
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        let mut nodes = Vec::new();
        let mut relations = Vec::new();
        let mut has_more = false;
        visited.insert(stable_key.clone());
        queue.push_back(stable_key);

        while let Some(current) = queue.pop_front() {
            for relation in self
                .relations
                .iter()
                .filter(|relation| relation.source.stable_key == current)
                .filter(|relation| kinds.contains(relation.kind))
            {
                if relations.len() >= self.limit.value {
                    has_more = true;
                    return TechnicalTraversalRecords {
                        root: Some(root),
                        nodes,
                        relations,
                        has_more,
                    };
                }
                relations.push(relation.clone());

                if visited.insert(relation.target.stable_key.clone()) {
                    if nodes.len() >= self.limit.value {
                        has_more = true;
                        return TechnicalTraversalRecords {
                            root: Some(root),
                            nodes,
                            relations,
                            has_more,
                        };
                    }
                    if let Some(node) = self.node_by_key(&relation.target.stable_key) {
                        nodes.push(node);
                        queue.push_back(relation.target.stable_key.clone());
                    }
                }
            }
        }

        TechnicalTraversalRecords {
            root: Some(root),
            nodes,
            relations,
            has_more,
        }
    }

    fn neighborhood_direction_accepts(
        &self,
        query: &TechnicalRelationNeighborhoodQuery,
        relation: &TechnicalRelation,
    ) -> bool {
        match query.direction {
            TechnicalRelationNeighborhoodDirection::Incoming => {
                relation.target.stable_key == query.stable_key
            }
            TechnicalRelationNeighborhoodDirection::Outgoing => {
                relation.source.stable_key == query.stable_key
            }
            TechnicalRelationNeighborhoodDirection::Both => {
                relation.source.stable_key == query.stable_key
                    || relation.target.stable_key == query.stable_key
            }
        }
    }

    fn node_by_key(&self, stable_key: &signal_mind::TechnicalNodeKey) -> Option<TechnicalNode> {
        self.nodes
            .iter()
            .find(|node| node.stable_key == *stable_key)
            .cloned()
    }
}

struct TechnicalRelationKindSet {
    kinds: Vec<TechnicalRelationKind>,
}

impl TechnicalRelationKindSet {
    fn new(kinds: &[TechnicalRelationKind]) -> Self {
        Self {
            kinds: kinds.to_vec(),
        }
    }

    fn dependency(kinds: Vec<TechnicalRelationKind>) -> Self {
        if kinds.is_empty() {
            Self {
                kinds: vec![
                    TechnicalRelationKind::BuildDependency,
                    TechnicalRelationKind::RuntimeDependency,
                    TechnicalRelationKind::WireDependency,
                    TechnicalRelationKind::StorageDependency,
                    TechnicalRelationKind::TaskDependency,
                ],
            }
        } else {
            Self { kinds }
        }
    }

    fn provenance(kinds: Vec<TechnicalRelationKind>) -> Self {
        if kinds.is_empty() {
            Self {
                kinds: vec![
                    TechnicalRelationKind::ProvenanceDependency,
                    TechnicalRelationKind::ProvenBy,
                    TechnicalRelationKind::Supersedes,
                ],
            }
        } else {
            Self { kinds }
        }
    }

    fn contains(&self, kind: TechnicalRelationKind) -> bool {
        self.kinds.is_empty() || self.kinds.contains(&kind)
    }
}

struct TechnicalNeighborhoodRecords {
    center: Option<TechnicalNode>,
    incoming: Vec<TechnicalRelation>,
    outgoing: Vec<TechnicalRelation>,
    has_more: bool,
}

impl TechnicalNeighborhoodRecords {
    fn new(center: Option<TechnicalNode>, relations: Vec<TechnicalRelation>) -> Self {
        let incoming = relations
            .iter()
            .filter(|relation| {
                center
                    .as_ref()
                    .map(|node| relation.target.stable_key == node.stable_key)
                    .unwrap_or(false)
            })
            .cloned()
            .collect();
        let outgoing = relations
            .into_iter()
            .filter(|relation| {
                center
                    .as_ref()
                    .map(|node| relation.source.stable_key == node.stable_key)
                    .unwrap_or(false)
            })
            .collect();
        Self {
            center,
            incoming,
            outgoing,
            has_more: false,
        }
    }

    fn limited(mut self, limit: &GraphLimit) -> Self {
        let incoming_count = self.incoming.len();
        let outgoing_count = self.outgoing.len();
        let mut remaining = limit.value;
        self.incoming.truncate(remaining);
        remaining = remaining.saturating_sub(self.incoming.len());
        self.outgoing.truncate(remaining);
        self.has_more = incoming_count + outgoing_count > limit.value;
        self
    }
}

struct TechnicalTraversalRecords {
    root: Option<TechnicalNode>,
    nodes: Vec<TechnicalNode>,
    relations: Vec<TechnicalRelation>,
    has_more: bool,
}

impl TechnicalTraversalRecords {
    fn empty() -> Self {
        Self {
            root: None,
            nodes: Vec::new(),
            relations: Vec::new(),
            has_more: false,
        }
    }
}

pub(crate) struct TechnicalNodeSelector {
    filter: TechnicalNodeFilter,
}

impl TechnicalNodeSelector {
    pub(crate) fn new(filter: TechnicalNodeFilter) -> Self {
        Self { filter }
    }

    pub(crate) fn accepts(&self, node: &TechnicalNode) -> bool {
        self.accepts_filter(node, &self.filter)
    }

    fn accepts_filter(&self, node: &TechnicalNode, filter: &TechnicalNodeFilter) -> bool {
        match filter {
            TechnicalNodeFilter::ByKind(kind) => self.accepts_kind(node, kind),
            TechnicalNodeFilter::ByStableKey(stable_key) => {
                self.accepts_stable_key(node, stable_key)
            }
            TechnicalNodeFilter::BySourceLocator(locator) => {
                self.accepts_source_locator(node, locator)
            }
            TechnicalNodeFilter::Composite(composite) => self.accepts_composite(node, composite),
        }
    }

    fn accepts_kind(&self, node: &TechnicalNode, kind: &ByTechnicalNodeKind) -> bool {
        kind.kinds.is_empty() || kind.kinds.contains(&node.kind)
    }

    fn accepts_stable_key(
        &self,
        node: &TechnicalNode,
        stable_key: &ByTechnicalNodeStableKey,
    ) -> bool {
        node.stable_key == stable_key.stable_key
    }

    fn accepts_source_locator(
        &self,
        node: &TechnicalNode,
        locator: &ByTechnicalSourceLocator,
    ) -> bool {
        TechnicalNodeSourceLocator::new(node).matches(&locator.locator)
    }

    fn accepts_composite(
        &self,
        node: &TechnicalNode,
        composite: &CompositeTechnicalNodeFilter,
    ) -> bool {
        let kind_ok = composite.kinds.is_empty() || composite.kinds.contains(&node.kind);
        let stable_key_ok = composite
            .stable_key
            .as_ref()
            .map(|stable_key| node.stable_key == *stable_key)
            .unwrap_or(true);
        let source_locator_ok = composite
            .source_locator
            .as_ref()
            .map(|locator| TechnicalNodeSourceLocator::new(node).matches(locator))
            .unwrap_or(true);
        kind_ok && stable_key_ok && source_locator_ok
    }
}

struct TechnicalNodeSourceLocator<'node> {
    node: &'node TechnicalNode,
}

impl<'node> TechnicalNodeSourceLocator<'node> {
    fn new(node: &'node TechnicalNode) -> Self {
        Self { node }
    }

    fn matches(&self, locator: &TechnicalSourceLocator) -> bool {
        match &self.node.body {
            signal_mind::TechnicalNodeBody::SourceArtifact(artifact) => {
                artifact.locator == *locator
            }
            signal_mind::TechnicalNodeBody::Witness(witness) => {
                witness.locator.as_ref() == Some(locator)
            }
            signal_mind::TechnicalNodeBody::Report(report) => {
                *locator == TechnicalSourceLocator::Report(report.path.clone())
            }
            _ => false,
        }
    }
}

pub(crate) struct TechnicalRelationSelector {
    filter: TechnicalRelationFilter,
}

impl TechnicalRelationSelector {
    pub(crate) fn new(filter: TechnicalRelationFilter) -> Self {
        Self { filter }
    }

    pub(crate) fn accepts(&self, relation: &TechnicalRelation) -> bool {
        self.accepts_filter(relation, &self.filter)
    }

    fn accepts_filter(
        &self,
        relation: &TechnicalRelation,
        filter: &TechnicalRelationFilter,
    ) -> bool {
        match filter {
            TechnicalRelationFilter::ByKind(kind) => self.accepts_kind(relation, kind),
            TechnicalRelationFilter::BySource(source) => self.accepts_source(relation, source),
            TechnicalRelationFilter::ByTarget(target) => self.accepts_target(relation, target),
            TechnicalRelationFilter::BetweenEndpoints(endpoints) => {
                self.accepts_endpoints(relation, endpoints)
            }
            TechnicalRelationFilter::Composite(composite) => {
                self.accepts_composite(relation, composite)
            }
        }
    }

    fn accepts_kind(&self, relation: &TechnicalRelation, kind: &ByTechnicalRelationKind) -> bool {
        kind.kinds.is_empty() || kind.kinds.contains(&relation.kind)
    }

    fn accepts_source(
        &self,
        relation: &TechnicalRelation,
        source: &ByTechnicalRelationSource,
    ) -> bool {
        relation.source.stable_key == source.source
    }

    fn accepts_target(
        &self,
        relation: &TechnicalRelation,
        target: &ByTechnicalRelationTarget,
    ) -> bool {
        relation.target.stable_key == target.target
    }

    fn accepts_endpoints(
        &self,
        relation: &TechnicalRelation,
        endpoints: &ByTechnicalRelationEndpoints,
    ) -> bool {
        relation.source.stable_key == endpoints.source
            && relation.target.stable_key == endpoints.target
    }

    fn accepts_composite(
        &self,
        relation: &TechnicalRelation,
        composite: &CompositeTechnicalRelationFilter,
    ) -> bool {
        let kind_ok = composite.kinds.is_empty() || composite.kinds.contains(&relation.kind);
        let source_ok = composite
            .source
            .as_ref()
            .map(|source| relation.source.stable_key == *source)
            .unwrap_or(true);
        let target_ok = composite
            .target
            .as_ref()
            .map(|target| relation.target.stable_key == *target)
            .unwrap_or(true);
        kind_ok && source_ok && target_ok
    }
}

impl RelationSelector {
    pub(crate) fn new(filter: RelationFilter) -> Self {
        Self { filter }
    }

    pub(crate) fn accepts(&self, relation: &Relation) -> bool {
        self.accepts_filter(relation, &self.filter)
    }

    fn accepts_filter(&self, relation: &Relation, filter: &RelationFilter) -> bool {
        match filter {
            RelationFilter::ByKind(kind) => self.accepts_kind(relation, kind),
            RelationFilter::BySource(source) => self.accepts_source(relation, source),
            RelationFilter::ByTarget(target) => self.accepts_target(relation, target),
            RelationFilter::Composite(composite) => self.accepts_composite(relation, composite),
        }
    }

    fn accepts_kind(&self, relation: &Relation, kind: &ByRelationKind) -> bool {
        kind.kinds.is_empty() || kind.kinds.contains(&relation.kind)
    }

    fn accepts_source(&self, relation: &Relation, source: &ByRelationSource) -> bool {
        relation.source == source.source
    }

    fn accepts_target(&self, relation: &Relation, target: &ByRelationTarget) -> bool {
        relation.target == target.target
    }

    fn accepts_composite(&self, relation: &Relation, composite: &CompositeRelationFilter) -> bool {
        let kind_ok = composite.kinds.is_empty() || composite.kinds.contains(&relation.kind);
        let source_ok = composite
            .source
            .as_ref()
            .map(|source| relation.source == *source)
            .unwrap_or(true);
        let target_ok = composite
            .target
            .as_ref()
            .map(|target| relation.target == *target)
            .unwrap_or(true);
        kind_ok && source_ok && target_ok
    }
}

struct GraphLimit {
    value: usize,
}

struct LimitedRecords<T> {
    records: Vec<T>,
    has_more: bool,
}

impl GraphLimit {
    fn new(limit: QueryLimit) -> Self {
        Self {
            value: usize::from(limit.into_u16()),
        }
    }

    fn apply<T>(&self, records: Vec<T>) -> LimitedRecords<T> {
        let has_more = records.len() > self.value;
        let records = records.into_iter().take(self.value).collect();
        LimitedRecords { records, has_more }
    }
}
