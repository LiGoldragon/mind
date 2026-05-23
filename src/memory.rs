//! In-memory mind graph used by the first typed tests.
//!
//! Production storage moves through mind-owned Sema tables over the
//! `sema` kernel; this module keeps the graph reducer honest while
//! durable tables land.

use std::path::Path;

use crate::MindEnvelope;
use signal_persona_mind::{
    ActorName, AliasAddedEvent, AliasAssignment, DisplayIdentifier, Edge, EdgeAddedEvent, EdgeKind,
    EdgeTarget, Event, EventHeader, EventSeq, ExternalAlias, Item, ItemKind, ItemOpenedEvent,
    ItemReference, ItemStatus, Link, LinkTarget, MindReply, MindRequest, Note, NoteAddedEvent,
    NoteSubmission, Opening, OpeningReceipt, OperationIdentifier, Query, QueryKind, QueryLimit,
    Rejection, RejectionReason, StableItemIdentifier, StatusChange, StatusChangedEvent, View,
};

pub struct MemoryState {
    store: StoreLocation,
    graph: MemoryGraph,
}

impl MemoryState {
    pub fn open(store: StoreLocation) -> Self {
        Self::open_with_graph(store, None)
    }

    pub(crate) fn open_with_graph(store: StoreLocation, graph: Option<MemoryGraph>) -> Self {
        Self {
            store,
            graph: graph.unwrap_or_else(|| MemoryGraph::new(ActorName::new("persona-mind"))),
        }
    }

    pub fn store(&self) -> &StoreLocation {
        &self.store
    }

    pub fn dispatch(&mut self, request: MindRequest) -> Option<MindReply> {
        self.graph.dispatch(request)
    }

    pub fn dispatch_envelope(&mut self, envelope: MindEnvelope) -> Option<MindReply> {
        self.graph.dispatch_envelope(envelope)
    }

    pub(crate) fn stage_envelope(&self, envelope: MindEnvelope) -> MemoryStage {
        let write = MemoryWrite::from_request(envelope.request());
        let mut next_graph = self.graph.clone();
        let reply = next_graph.dispatch_envelope(envelope);
        let graph =
            (write.persists() && MemoryReply::new(&reply).committed()).then_some(next_graph);

        MemoryStage::new(reply, graph)
    }

    pub(crate) fn replace_graph(&mut self, graph: MemoryGraph) {
        self.graph = graph;
    }
}

pub(crate) struct MemoryStage {
    reply: Option<MindReply>,
    graph: Option<MemoryGraph>,
}

impl MemoryStage {
    fn new(reply: Option<MindReply>, graph: Option<MemoryGraph>) -> Self {
        Self { reply, graph }
    }

    pub(crate) fn reply(&self) -> Option<MindReply> {
        self.reply.clone()
    }

    pub(crate) fn graph(&self) -> Option<&MemoryGraph> {
        self.graph.as_ref()
    }

    pub(crate) fn into_graph(self) -> Option<MemoryGraph> {
        self.graph
    }
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone, PartialEq, Eq)]
pub(crate) struct MemoryGraph {
    default_actor: ActorName,
    next_item: u64,
    next_event: u64,
    next_operation: u64,
    items: Vec<Item>,
    edges: Vec<Edge>,
    notes: Vec<Note>,
    events: Vec<Event>,
}

impl MemoryGraph {
    fn new(default_actor: ActorName) -> Self {
        Self {
            default_actor,
            next_item: 0,
            next_event: 0,
            next_operation: 0,
            items: Vec::new(),
            edges: Vec::new(),
            notes: Vec::new(),
            events: Vec::new(),
        }
    }

    fn dispatch(&mut self, request: MindRequest) -> Option<MindReply> {
        self.dispatch_envelope(MindEnvelope::new(self.default_actor.clone(), request))
    }

    fn dispatch_envelope(&mut self, envelope: MindEnvelope) -> Option<MindReply> {
        let MindEnvelope { actor, request } = envelope;
        match request {
            MindRequest::Opening(opening) => Some(self.open(opening, &actor)),
            MindRequest::NoteSubmission(note) => Some(self.add_note(note, &actor)),
            MindRequest::Link(link) => Some(self.link(link, &actor)),
            MindRequest::StatusChange(change) => Some(self.change_status(change, &actor)),
            MindRequest::AliasAssignment(alias) => Some(self.add_alias(alias, &actor)),
            MindRequest::Query(query) => Some(self.query(query)),
            MindRequest::SubmitThought(_)
            | MindRequest::SubmitRelation(_)
            | MindRequest::QueryThoughts(_)
            | MindRequest::QueryRelations(_)
            | MindRequest::SubscribeThoughts(_)
            | MindRequest::SubscribeRelations(_)
            | MindRequest::SubscriptionRetraction(_)
            | MindRequest::AdjudicationRequest(_)
            | MindRequest::ChannelList(_) => None,
        }
    }

    fn open(&mut self, opening: Opening, actor: &ActorName) -> MindReply {
        self.next_item += 1;
        let id = ShortIdMint::new(self.next_item);
        let header = self.next_header(actor);
        let item = Item {
            id: id.stable_item_identifier(),
            display_identifier: id.display_identifier(),
            aliases: Vec::new(),
            kind: opening.kind,
            status: ItemStatus::Open,
            priority: opening.priority,
            title: opening.title,
            body: opening.body,
        };
        let event = ItemOpenedEvent { header, item };

        self.items.push(event.item.clone());
        self.events.push(Event::ItemOpened(event.clone()));

        MindReply::OpeningReceipt(OpeningReceipt { event })
    }

    fn add_note(&mut self, submission: NoteSubmission, actor: &ActorName) -> MindReply {
        let Some(item) = self.resolve_item(&submission.item) else {
            return Self::rejected(RejectionReason::UnknownItem);
        };
        let header = self.next_header(actor);
        let note = Note {
            event: header.event,
            item,
            author: actor.clone(),
            body: submission.body,
        };
        let event = NoteAddedEvent { header, note };

        self.notes.push(event.note.clone());
        self.events.push(Event::NoteAdded(event.clone()));

        MindReply::NoteReceipt(signal_persona_mind::NoteReceipt { event })
    }

    fn link(&mut self, link: Link, actor: &ActorName) -> MindReply {
        let Some(source) = self.resolve_item(&link.source) else {
            return Self::rejected(RejectionReason::UnknownItem);
        };
        let Some(target) = self.resolve_link_target(link.target) else {
            return Self::rejected(RejectionReason::UnknownItem);
        };
        let header = self.next_header(actor);
        let edge = Edge {
            event: header.event,
            source,
            kind: link.kind,
            target,
            body: link.body,
        };
        let event = EdgeAddedEvent { header, edge };

        self.edges.push(event.edge.clone());
        self.events.push(Event::EdgeAdded(event.clone()));

        MindReply::LinkReceipt(signal_persona_mind::LinkReceipt { event })
    }

    fn change_status(&mut self, change: StatusChange, actor: &ActorName) -> MindReply {
        let Some(position) = self.resolve_item_position(&change.item) else {
            return Self::rejected(RejectionReason::UnknownItem);
        };
        self.items[position].status = change.status;
        let item = self.items[position].id.clone();
        let header = self.next_header(actor);
        let event = StatusChangedEvent {
            header,
            item,
            status: change.status,
            body: change.body,
        };

        self.events.push(Event::StatusChanged(event.clone()));

        MindReply::StatusReceipt(signal_persona_mind::StatusReceipt { event })
    }

    fn add_alias(&mut self, assignment: AliasAssignment, actor: &ActorName) -> MindReply {
        if self.alias_exists(&assignment.alias) {
            return Self::rejected(RejectionReason::DuplicateAlias);
        }
        let Some(position) = self.resolve_item_position(&assignment.item) else {
            return Self::rejected(RejectionReason::UnknownItem);
        };

        let item = self.items[position].id.clone();
        self.items[position].aliases.push(assignment.alias.clone());
        let header = self.next_header(actor);
        let event = AliasAddedEvent {
            header,
            item,
            alias: assignment.alias,
        };

        self.events.push(Event::AliasAdded(event.clone()));

        MindReply::AliasReceipt(signal_persona_mind::AliasReceipt { event })
    }

    fn query(&self, query: Query) -> MindReply {
        let limit = query.limit.into_usize();
        let reply = match query.kind {
            QueryKind::Ready => self.ready_view(limit),
            QueryKind::Blocked => self.blocked_view(limit),
            QueryKind::Open => self.status_view(ItemStatus::Open, limit),
            QueryKind::RecentEvents => self.recent_events_view(limit),
            QueryKind::ByItem(reference) => {
                let Some(item) = self.resolve_item(&reference) else {
                    return Self::rejected(RejectionReason::UnknownItem);
                };
                self.item_view(&item, limit)
            }
            QueryKind::ByKind(kind) => self.kind_view(kind, limit),
            QueryKind::ByStatus(status) => self.status_view(status, limit),
            QueryKind::ByAlias(alias) => {
                let Some(item) = self.resolve_item(&ItemReference::Alias(alias)) else {
                    return Self::rejected(RejectionReason::UnknownItem);
                };
                self.item_view(&item, limit)
            }
        };

        MindReply::View(reply)
    }

    fn ready_view(&self, limit: usize) -> View {
        self.view_for_items(
            self.items
                .iter()
                .filter(|item| item.status == ItemStatus::Open && self.item_is_ready(&item.id))
                .take(limit)
                .cloned()
                .collect(),
        )
    }

    fn blocked_view(&self, limit: usize) -> View {
        self.view_for_items(
            self.items
                .iter()
                .filter(|item| item.status == ItemStatus::Open && !self.item_is_ready(&item.id))
                .take(limit)
                .cloned()
                .collect(),
        )
    }

    fn kind_view(&self, kind: ItemKind, limit: usize) -> View {
        self.view_for_items(
            self.items
                .iter()
                .filter(|item| item.kind == kind)
                .take(limit)
                .cloned()
                .collect(),
        )
    }

    fn status_view(&self, status: ItemStatus, limit: usize) -> View {
        self.view_for_items(
            self.items
                .iter()
                .filter(|item| item.status == status)
                .take(limit)
                .cloned()
                .collect(),
        )
    }

    fn recent_events_view(&self, limit: usize) -> View {
        let mut events = self
            .events
            .iter()
            .rev()
            .take(limit)
            .cloned()
            .collect::<Vec<_>>();
        events.reverse();

        View {
            items: Vec::new(),
            edges: Vec::new(),
            notes: Vec::new(),
            events,
        }
    }

    fn item_view(&self, item: &StableItemIdentifier, limit: usize) -> View {
        self.view_for_items(
            self.items
                .iter()
                .filter(|candidate| &candidate.id == item)
                .take(limit)
                .cloned()
                .collect(),
        )
    }

    fn view_for_items(&self, items: Vec<Item>) -> View {
        let item_ids = items
            .iter()
            .map(|item| item.id.clone())
            .collect::<Vec<StableItemIdentifier>>();

        View {
            edges: self.edges_for_items(&item_ids),
            notes: self.notes_for_items(&item_ids),
            events: self.events.clone(),
            items,
        }
    }

    fn edges_for_items(&self, item_ids: &[StableItemIdentifier]) -> Vec<Edge> {
        self.edges
            .iter()
            .filter(|edge| {
                item_ids.contains(&edge.source)
                    || match &edge.target {
                        EdgeTarget::Item(target) => item_ids.contains(target),
                        EdgeTarget::External(_) => false,
                    }
            })
            .cloned()
            .collect()
    }

    fn notes_for_items(&self, item_ids: &[StableItemIdentifier]) -> Vec<Note> {
        self.notes
            .iter()
            .filter(|note| item_ids.contains(&note.item))
            .cloned()
            .collect()
    }

    fn item_is_ready(&self, item: &StableItemIdentifier) -> bool {
        self.edges.iter().all(|edge| {
            if edge.kind != EdgeKind::DependsOn || &edge.source != item {
                return true;
            }

            match &edge.target {
                EdgeTarget::Item(target) => self
                    .item_by_id(target)
                    .map(|target_item| target_item.status == ItemStatus::Closed)
                    .unwrap_or(false),
                EdgeTarget::External(_) => false,
            }
        })
    }

    fn resolve_link_target(&self, target: LinkTarget) -> Option<EdgeTarget> {
        match target {
            LinkTarget::Item(reference) => self.resolve_item(&reference).map(EdgeTarget::Item),
            LinkTarget::External(external) => Some(EdgeTarget::External(external)),
        }
    }

    fn resolve_item(&self, reference: &ItemReference) -> Option<StableItemIdentifier> {
        match reference {
            ItemReference::Stable(id) => self.item_by_id(id).map(|item| item.id.clone()),
            ItemReference::Display(display_identifier) => self
                .items
                .iter()
                .find(|item| &item.display_identifier == display_identifier)
                .map(|item| item.id.clone()),
            ItemReference::Alias(alias) => self
                .items
                .iter()
                .find(|item| item.aliases.iter().any(|candidate| candidate == alias))
                .map(|item| item.id.clone()),
        }
    }

    fn resolve_item_position(&self, reference: &ItemReference) -> Option<usize> {
        self.resolve_item(reference)
            .and_then(|id| self.items.iter().position(|item| item.id == id))
    }

    fn item_by_id(&self, id: &StableItemIdentifier) -> Option<&Item> {
        self.items.iter().find(|item| &item.id == id)
    }

    fn alias_exists(&self, alias: &ExternalAlias) -> bool {
        self.items
            .iter()
            .any(|item| item.aliases.iter().any(|candidate| candidate == alias))
    }

    fn next_header(&mut self, actor: &ActorName) -> EventHeader {
        self.next_event += 1;
        self.next_operation += 1;
        EventHeader {
            event: EventSeq::new(self.next_event),
            operation: ShortIdMint::new(self.next_operation).operation_identifier(),
            actor: actor.clone(),
        }
    }

    fn rejected(reason: RejectionReason) -> MindReply {
        MindReply::Rejection(Rejection { reason })
    }
}

struct MemoryWrite {
    persists: bool,
}

impl MemoryWrite {
    fn from_request(request: &MindRequest) -> Self {
        let persists = matches!(
            request,
            MindRequest::Opening(_)
                | MindRequest::NoteSubmission(_)
                | MindRequest::Link(_)
                | MindRequest::StatusChange(_)
                | MindRequest::AliasAssignment(_)
        );
        Self { persists }
    }

    fn persists(&self) -> bool {
        self.persists
    }
}

struct MemoryReply<'reply> {
    reply: &'reply Option<MindReply>,
}

impl<'reply> MemoryReply<'reply> {
    fn new(reply: &'reply Option<MindReply>) -> Self {
        Self { reply }
    }

    fn committed(&self) -> bool {
        matches!(
            self.reply,
            Some(
                MindReply::OpeningReceipt(_)
                    | MindReply::NoteReceipt(_)
                    | MindReply::LinkReceipt(_)
                    | MindReply::StatusReceipt(_)
                    | MindReply::AliasReceipt(_)
            )
        )
    }
}

struct ShortIdMint {
    value: u64,
}

impl ShortIdMint {
    fn new(value: u64) -> Self {
        Self { value }
    }

    fn stable_item_identifier(&self) -> StableItemIdentifier {
        StableItemIdentifier::new(self.token())
    }

    fn operation_identifier(&self) -> OperationIdentifier {
        OperationIdentifier::new(self.token())
    }

    fn display_identifier(&self) -> DisplayIdentifier {
        DisplayIdentifier::new(self.token())
    }

    fn token(&self) -> String {
        let alphabet = b"abcdefghjkmnpqrstvwxyz23456789";
        let mut value = self.value;
        let mut text = [b'a'; 3];

        for slot in text.iter_mut().rev() {
            let index = (value % 32) as usize;
            *slot = alphabet[index];
            value /= 32;
        }

        String::from_utf8(text.to_vec()).expect("short ids are ascii")
    }
}

trait QueryLimitExt {
    fn into_usize(self) -> usize;
}

impl QueryLimitExt for QueryLimit {
    fn into_usize(self) -> usize {
        usize::from(self.into_u16())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, kameo::Reply)]
pub struct StoreLocation {
    path: String,
}

impl StoreLocation {
    pub fn new(path: impl Into<String>) -> Self {
        Self { path: path.into() }
    }

    pub fn as_str(&self) -> &str {
        &self.path
    }

    pub fn as_path(&self) -> &Path {
        Path::new(&self.path)
    }
}
