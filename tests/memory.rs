use persona_mind::{MemoryState, StoreLocation};
use signal_persona_mind::{
    AliasAssignment, ByThoughtKind, EdgeKind, EdgeTarget, Event, ExternalAlias, ExternalReference,
    ItemKind, ItemReference, ItemStatus, Link, LinkTarget, Magnitude, MindReply, MindRequest,
    NoteSubmission, Opening, Query, QueryKind, QueryLimit, QueryThoughts, RejectionReason,
    ReportPath, StableItemIdentifier, StatusChange, TextBody, ThoughtFilter, Title, View,
};

struct Fixture {
    state: MemoryState,
}

impl Fixture {
    fn new() -> Self {
        Self {
            state: MemoryState::open(StoreLocation::new("mind.redb")),
        }
    }

    fn open_task(&mut self, title: &str) -> StableItemIdentifier {
        match self.dispatch(MindRequest::Opening(Opening {
            kind: ItemKind::Task,
            priority: Magnitude::Medium,
            title: Title::new(title),
            body: TextBody::new("body"),
        })) {
            MindReply::OpeningReceipt(receipt) => receipt.event.item.id,
            other => panic!("expected open receipt, got {other:?}"),
        }
    }

    fn open_decision(&mut self, title: &str) -> StableItemIdentifier {
        match self.dispatch(MindRequest::Opening(Opening {
            kind: ItemKind::Decision,
            priority: Magnitude::High,
            title: Title::new(title),
            body: TextBody::new("decision body"),
        })) {
            MindReply::OpeningReceipt(receipt) => receipt.event.item.id,
            other => panic!("expected open receipt, got {other:?}"),
        }
    }

    fn add_note(&mut self, item: &StableItemIdentifier, body: &str) {
        match self.dispatch(MindRequest::NoteSubmission(NoteSubmission {
            item: ItemReference::Stable(item.clone()),
            body: TextBody::new(body),
        })) {
            MindReply::NoteReceipt(_) => {}
            other => panic!("expected note receipt, got {other:?}"),
        }
    }

    fn link_item(
        &mut self,
        source: &StableItemIdentifier,
        kind: EdgeKind,
        target: &StableItemIdentifier,
    ) {
        match self.dispatch(MindRequest::Link(Link {
            source: ItemReference::Stable(source.clone()),
            kind,
            target: LinkTarget::Item(ItemReference::Stable(target.clone())),
            body: None,
        })) {
            MindReply::LinkReceipt(_) => {}
            other => panic!("expected link receipt, got {other:?}"),
        }
    }

    fn link_report(&mut self, source: &StableItemIdentifier, path: &str) {
        match self.dispatch(MindRequest::Link(Link {
            source: ItemReference::Stable(source.clone()),
            kind: EdgeKind::References,
            target: LinkTarget::External(ExternalReference::Report(ReportPath::new(path))),
            body: None,
        })) {
            MindReply::LinkReceipt(_) => {}
            other => panic!("expected report link receipt, got {other:?}"),
        }
    }

    fn change_status(&mut self, item: &StableItemIdentifier, status: ItemStatus) {
        match self.dispatch(MindRequest::StatusChange(StatusChange {
            item: ItemReference::Stable(item.clone()),
            status,
            body: None,
        })) {
            MindReply::StatusReceipt(_) => {}
            other => panic!("expected status receipt, got {other:?}"),
        }
    }

    fn add_alias(&mut self, item: &StableItemIdentifier, alias: &str) {
        match self.dispatch(MindRequest::AliasAssignment(AliasAssignment {
            item: ItemReference::Stable(item.clone()),
            alias: ExternalAlias::new(alias),
        })) {
            MindReply::AliasReceipt(_) => {}
            other => panic!("expected alias receipt, got {other:?}"),
        }
    }

    fn query(&mut self, kind: QueryKind) -> View {
        match self.dispatch(MindRequest::Query(Query {
            kind,
            limit: QueryLimit::new(20),
        })) {
            MindReply::View(view) => view,
            other => panic!("expected view reply, got {other:?}"),
        }
    }

    fn rejected(&mut self, request: MindRequest) -> RejectionReason {
        match self.dispatch(request) {
            MindReply::Rejection(rejection) => rejection.reason,
            other => panic!("expected rejection, got {other:?}"),
        }
    }

    fn dispatch(&mut self, request: MindRequest) -> MindReply {
        self.state
            .dispatch(request)
            .expect("memory fixture only sends memory requests")
    }
}

#[test]
fn opening_item_persists_projection_and_event() {
    let mut fixture = Fixture::new();
    let item = fixture.open_task("Build typed mind graph");
    let view = fixture.query(QueryKind::ByItem(ItemReference::Stable(item.clone())));

    assert_eq!(view.items.len(), 1);
    assert_eq!(view.items[0].id, item);
    assert_eq!(view.items[0].status, ItemStatus::Open);
    assert_eq!(view.items[0].title, Title::new("Build typed mind graph"));
    assert!(
        view.events
            .iter()
            .any(|event| matches!(event, Event::ItemOpened(_)))
    );
}

#[test]
fn adding_note_attaches_note_to_item_view() {
    let mut fixture = Fixture::new();
    let item = fixture.open_task("Record context");

    fixture.add_note(&item, "The note must be queryable from the item.");
    let view = fixture.query(QueryKind::ByItem(ItemReference::Stable(item.clone())));

    assert_eq!(view.notes.len(), 1);
    assert_eq!(view.notes[0].item, item);
    assert_eq!(
        view.notes[0].body,
        TextBody::new("The note must be queryable from the item.")
    );
    assert!(
        view.events
            .iter()
            .any(|event| matches!(event, Event::NoteAdded(_)))
    );
}

#[test]
fn depends_on_edge_controls_ready_and_blocked_views() {
    let mut fixture = Fixture::new();
    let blocker = fixture.open_task("Land Sema tables");
    let dependent = fixture.open_task("Migrate BEADS data");

    fixture.link_item(&dependent, EdgeKind::DependsOn, &blocker);

    let blocked = fixture.query(QueryKind::Blocked);
    assert_eq!(blocked.items.len(), 1);
    assert_eq!(blocked.items[0].id, dependent);

    let ready = fixture.query(QueryKind::Ready);
    assert_eq!(ready.items.len(), 1);
    assert_eq!(ready.items[0].id, blocker);

    fixture.change_status(&blocker, ItemStatus::Closed);

    let ready = fixture.query(QueryKind::Ready);
    assert_eq!(ready.items.len(), 1);
    assert_eq!(ready.items[0].id, dependent);
}

#[test]
fn alias_assignment_resolves_imported_beads_identity() {
    let mut fixture = Fixture::new();
    let item = fixture.open_task("Replace bd");

    fixture.add_alias(&item, "primary-abc");
    let view = fixture.query(QueryKind::ByAlias(ExternalAlias::new("primary-abc")));

    assert_eq!(view.items.len(), 1);
    assert_eq!(view.items[0].id, item);
    assert_eq!(
        view.items[0].aliases,
        vec![ExternalAlias::new("primary-abc")]
    );
}

#[test]
fn report_reference_is_an_edge_not_an_item_kind() {
    let mut fixture = Fixture::new();
    let item = fixture.open_decision("Accept mind graph fold");

    fixture.link_report(&item, "reports/designer/98-critique.md");
    let view = fixture.query(QueryKind::ByItem(ItemReference::Stable(item.clone())));

    assert_eq!(view.items.len(), 1);
    assert_eq!(view.items[0].kind, ItemKind::Decision);
    assert_eq!(view.edges.len(), 1);
    assert_eq!(view.edges[0].source, item);
    assert_eq!(view.edges[0].kind, EdgeKind::References);
    assert_eq!(
        view.edges[0].target,
        EdgeTarget::External(ExternalReference::Report(ReportPath::new(
            "reports/designer/98-critique.md"
        )))
    );
}

#[test]
fn unknown_item_rejects_mutations_and_queries() {
    let mut fixture = Fixture::new();
    let missing = StableItemIdentifier::new("missing-item");

    assert_eq!(
        fixture.rejected(MindRequest::NoteSubmission(NoteSubmission {
            item: ItemReference::Stable(missing.clone()),
            body: TextBody::new("lost"),
        })),
        RejectionReason::UnknownItem
    );
    assert_eq!(
        fixture.rejected(MindRequest::Query(Query {
            kind: QueryKind::ByItem(ItemReference::Stable(missing)),
            limit: QueryLimit::new(1),
        })),
        RejectionReason::UnknownItem
    );
}

#[test]
fn non_memory_requests_are_not_handled_by_the_memory_reducer() {
    let mut fixture = Fixture::new();

    assert!(
        fixture
            .state
            .dispatch(MindRequest::QueryThoughts(QueryThoughts {
                filter: ThoughtFilter::ByKind(ByThoughtKind { kinds: Vec::new() }),
                limit: 1,
            }))
            .is_none()
    );
}
