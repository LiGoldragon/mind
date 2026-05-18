use nota_codec::{Decoder, Encoder, NotaDecode, NotaEncode, NotaEnum, NotaRecord};
use signal_persona_mind as contract;

use crate::Result;

#[derive(NotaEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKindText {
    Task,
    Defect,
    Question,
    Decision,
    Note,
    Handoff,
}

impl ItemKindText {
    fn from_contract(kind: contract::ItemKind) -> Self {
        match kind {
            contract::ItemKind::Task => Self::Task,
            contract::ItemKind::Defect => Self::Defect,
            contract::ItemKind::Question => Self::Question,
            contract::ItemKind::Decision => Self::Decision,
            contract::ItemKind::Note => Self::Note,
            contract::ItemKind::Handoff => Self::Handoff,
        }
    }

    fn into_contract(self) -> contract::ItemKind {
        match self {
            Self::Task => contract::ItemKind::Task,
            Self::Defect => contract::ItemKind::Defect,
            Self::Question => contract::ItemKind::Question,
            Self::Decision => contract::ItemKind::Decision,
            Self::Note => contract::ItemKind::Note,
            Self::Handoff => contract::ItemKind::Handoff,
        }
    }
}

#[derive(NotaEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemPriorityText {
    Critical,
    High,
    Normal,
    Low,
    Backlog,
}

impl ItemPriorityText {
    fn from_contract(priority: contract::ItemPriority) -> Self {
        match priority {
            contract::ItemPriority::Critical => Self::Critical,
            contract::ItemPriority::High => Self::High,
            contract::ItemPriority::Normal => Self::Normal,
            contract::ItemPriority::Low => Self::Low,
            contract::ItemPriority::Backlog => Self::Backlog,
        }
    }

    fn into_contract(self) -> contract::ItemPriority {
        match self {
            Self::Critical => contract::ItemPriority::Critical,
            Self::High => contract::ItemPriority::High,
            Self::Normal => contract::ItemPriority::Normal,
            Self::Low => contract::ItemPriority::Low,
            Self::Backlog => contract::ItemPriority::Backlog,
        }
    }
}

#[derive(NotaEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemStatusText {
    Open,
    InProgress,
    Blocked,
    Closed,
    Deferred,
}

impl ItemStatusText {
    fn from_contract(status: contract::ItemStatus) -> Self {
        match status {
            contract::ItemStatus::Open => Self::Open,
            contract::ItemStatus::InProgress => Self::InProgress,
            contract::ItemStatus::Blocked => Self::Blocked,
            contract::ItemStatus::Closed => Self::Closed,
            contract::ItemStatus::Deferred => Self::Deferred,
        }
    }

    fn into_contract(self) -> contract::ItemStatus {
        match self {
            Self::Open => contract::ItemStatus::Open,
            Self::InProgress => contract::ItemStatus::InProgress,
            Self::Blocked => contract::ItemStatus::Blocked,
            Self::Closed => contract::ItemStatus::Closed,
            Self::Deferred => contract::ItemStatus::Deferred,
        }
    }
}

#[derive(NotaEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeKindText {
    DependsOn,
    ParentOf,
    RelatesTo,
    Duplicates,
    Supersedes,
    Answers,
    References,
}

impl EdgeKindText {
    fn from_contract(kind: contract::EdgeKind) -> Self {
        match kind {
            contract::EdgeKind::DependsOn => Self::DependsOn,
            contract::EdgeKind::ParentOf => Self::ParentOf,
            contract::EdgeKind::RelatesTo => Self::RelatesTo,
            contract::EdgeKind::Duplicates => Self::Duplicates,
            contract::EdgeKind::Supersedes => Self::Supersedes,
            contract::EdgeKind::Answers => Self::Answers,
            contract::EdgeKind::References => Self::References,
        }
    }

    fn into_contract(self) -> contract::EdgeKind {
        match self {
            Self::DependsOn => contract::EdgeKind::DependsOn,
            Self::ParentOf => contract::EdgeKind::ParentOf,
            Self::RelatesTo => contract::EdgeKind::RelatesTo,
            Self::Duplicates => contract::EdgeKind::Duplicates,
            Self::Supersedes => contract::EdgeKind::Supersedes,
            Self::Answers => contract::EdgeKind::Answers,
            Self::References => contract::EdgeKind::References,
        }
    }
}

#[derive(NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct Opening {
    pub kind: ItemKindText,
    pub priority: ItemPriorityText,
    pub title: String,
    pub body: String,
}

impl Opening {
    fn into_contract(self) -> contract::MindRequest {
        contract::MindRequest::Opening(contract::Opening {
            kind: self.kind.into_contract(),
            priority: self.priority.into_contract(),
            title: contract::Title::new(self.title),
            body: contract::TextBody::new(self.body),
        })
    }
}

#[derive(NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct Stable {
    pub id: String,
}

#[derive(NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct Display {
    pub id: String,
}

#[derive(NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct Alias {
    pub alias: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ItemReferenceText {
    Stable(Stable),
    Display(Display),
    Alias(Alias),
}

impl ItemReferenceText {
    fn into_contract(self) -> contract::ItemReference {
        match self {
            Self::Stable(stable) => {
                contract::ItemReference::Stable(contract::StableItemId::new(stable.id))
            }
            Self::Display(display) => {
                contract::ItemReference::Display(contract::DisplayId::new(display.id))
            }
            Self::Alias(alias) => {
                contract::ItemReference::Alias(contract::ExternalAlias::new(alias.alias))
            }
        }
    }
}

#[derive(NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct ItemReferenceTarget {
    pub item: ItemReferenceText,
}

#[derive(NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub path: String,
}

#[derive(NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct GitCommit {
    pub hash: String,
}

#[derive(NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct BeadsTask {
    pub token: String,
}

#[derive(NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct File {
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkTargetText {
    ItemReferenceTarget(ItemReferenceTarget),
    Report(Report),
    GitCommit(GitCommit),
    BeadsTask(BeadsTask),
    File(File),
}

impl LinkTargetText {
    fn into_contract(self) -> contract::LinkTarget {
        match self {
            Self::ItemReferenceTarget(target) => {
                contract::LinkTarget::Item(target.item.into_contract())
            }
            Self::Report(report) => contract::LinkTarget::External(
                contract::ExternalReference::Report(contract::ReportPath::new(report.path)),
            ),
            Self::GitCommit(commit) => contract::LinkTarget::External(
                contract::ExternalReference::GitCommit(contract::CommitHash::new(commit.hash)),
            ),
            Self::BeadsTask(task) => contract::LinkTarget::External(
                contract::ExternalReference::BeadsTask(contract::BeadsToken::new(task.token)),
            ),
            Self::File(file) => contract::LinkTarget::External(contract::ExternalReference::File(
                contract::ReferencePath::new(file.path),
            )),
        }
    }
}

impl NotaEncode for LinkTargetText {
    fn encode(&self, encoder: &mut Encoder) -> nota_codec::Result<()> {
        match self {
            Self::ItemReferenceTarget(target) => target.encode(encoder),
            Self::Report(report) => report.encode(encoder),
            Self::GitCommit(commit) => commit.encode(encoder),
            Self::BeadsTask(task) => task.encode(encoder),
            Self::File(file) => file.encode(encoder),
        }
    }
}

impl NotaDecode for LinkTargetText {
    fn decode(decoder: &mut Decoder<'_>) -> nota_codec::Result<Self> {
        match decoder.peek_record_head()?.as_str() {
            "ItemReferenceTarget" => Ok(Self::ItemReferenceTarget(ItemReferenceTarget::decode(
                decoder,
            )?)),
            "Report" => Ok(Self::Report(Report::decode(decoder)?)),
            "GitCommit" => Ok(Self::GitCommit(GitCommit::decode(decoder)?)),
            "BeadsTask" => Ok(Self::BeadsTask(BeadsTask::decode(decoder)?)),
            "File" => Ok(Self::File(File::decode(decoder)?)),
            other => Err(nota_codec::Error::UnknownKindForVerb {
                verb: "LinkTarget",
                got: other.to_string(),
            }),
        }
    }
}

#[derive(NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct NoteSubmission {
    pub item: ItemReferenceText,
    pub body: String,
}

impl NoteSubmission {
    fn into_contract(self) -> contract::MindRequest {
        contract::MindRequest::NoteSubmission(contract::NoteSubmission {
            item: self.item.into_contract(),
            body: contract::TextBody::new(self.body),
        })
    }
}

#[derive(NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub source: ItemReferenceText,
    pub kind: EdgeKindText,
    pub target: LinkTargetText,
    pub body: Option<String>,
}

impl Link {
    fn into_contract(self) -> contract::MindRequest {
        contract::MindRequest::Link(contract::Link {
            source: self.source.into_contract(),
            kind: self.kind.into_contract(),
            target: self.target.into_contract(),
            body: self.body.map(contract::TextBody::new),
        })
    }
}

#[derive(NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct StatusChange {
    pub item: ItemReferenceText,
    pub status: ItemStatusText,
    pub body: Option<String>,
}

impl StatusChange {
    fn into_contract(self) -> contract::MindRequest {
        contract::MindRequest::StatusChange(contract::StatusChange {
            item: self.item.into_contract(),
            status: self.status.into_contract(),
            body: self.body.map(contract::TextBody::new),
        })
    }
}

#[derive(NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct AliasAssignment {
    pub item: ItemReferenceText,
    pub alias: String,
}

impl AliasAssignment {
    fn into_contract(self) -> contract::MindRequest {
        contract::MindRequest::AliasAssignment(contract::AliasAssignment {
            item: self.item.into_contract(),
            alias: contract::ExternalAlias::new(self.alias),
        })
    }
}

impl NotaEncode for ItemReferenceText {
    fn encode(&self, encoder: &mut Encoder) -> nota_codec::Result<()> {
        match self {
            Self::Stable(stable) => stable.encode(encoder),
            Self::Display(display) => display.encode(encoder),
            Self::Alias(alias) => alias.encode(encoder),
        }
    }
}

impl NotaDecode for ItemReferenceText {
    fn decode(decoder: &mut Decoder<'_>) -> nota_codec::Result<Self> {
        match decoder.peek_record_head()?.as_str() {
            "Stable" => Ok(Self::Stable(Stable::decode(decoder)?)),
            "Display" => Ok(Self::Display(Display::decode(decoder)?)),
            "Alias" => Ok(Self::Alias(Alias::decode(decoder)?)),
            other => Err(nota_codec::Error::UnknownKindForVerb {
                verb: "ItemReference",
                got: other.to_string(),
            }),
        }
    }
}

#[derive(NotaRecord, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ready {}

#[derive(NotaRecord, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Blocked {}

#[derive(NotaRecord, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Open {}

#[derive(NotaRecord, Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecentEvents {}

#[derive(NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct ByItem {
    pub item: ItemReferenceText,
}

#[derive(NotaRecord, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByKind {
    pub kind: ItemKindText,
}

#[derive(NotaRecord, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByStatus {
    pub status: ItemStatusText,
}

#[derive(NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct ByAlias {
    pub alias: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryKindText {
    Ready(Ready),
    Blocked(Blocked),
    Open(Open),
    RecentEvents(RecentEvents),
    ByItem(ByItem),
    ByKind(ByKind),
    ByStatus(ByStatus),
    ByAlias(ByAlias),
}

impl QueryKindText {
    fn into_contract(self) -> contract::QueryKind {
        match self {
            Self::Ready(_) => contract::QueryKind::Ready,
            Self::Blocked(_) => contract::QueryKind::Blocked,
            Self::Open(_) => contract::QueryKind::Open,
            Self::RecentEvents(_) => contract::QueryKind::RecentEvents,
            Self::ByItem(query) => contract::QueryKind::ByItem(query.item.into_contract()),
            Self::ByKind(query) => contract::QueryKind::ByKind(query.kind.into_contract()),
            Self::ByStatus(query) => contract::QueryKind::ByStatus(query.status.into_contract()),
            Self::ByAlias(query) => {
                contract::QueryKind::ByAlias(contract::ExternalAlias::new(query.alias))
            }
        }
    }
}

impl NotaEncode for QueryKindText {
    fn encode(&self, encoder: &mut Encoder) -> nota_codec::Result<()> {
        match self {
            Self::Ready(query) => query.encode(encoder),
            Self::Blocked(query) => query.encode(encoder),
            Self::Open(query) => query.encode(encoder),
            Self::RecentEvents(query) => query.encode(encoder),
            Self::ByItem(query) => query.encode(encoder),
            Self::ByKind(query) => query.encode(encoder),
            Self::ByStatus(query) => query.encode(encoder),
            Self::ByAlias(query) => query.encode(encoder),
        }
    }
}

impl NotaDecode for QueryKindText {
    fn decode(decoder: &mut Decoder<'_>) -> nota_codec::Result<Self> {
        match decoder.peek_record_head()?.as_str() {
            "Ready" => Ok(Self::Ready(Ready::decode(decoder)?)),
            "Blocked" => Ok(Self::Blocked(Blocked::decode(decoder)?)),
            "Open" => Ok(Self::Open(Open::decode(decoder)?)),
            "RecentEvents" => Ok(Self::RecentEvents(RecentEvents::decode(decoder)?)),
            "ByItem" => Ok(Self::ByItem(ByItem::decode(decoder)?)),
            "ByKind" => Ok(Self::ByKind(ByKind::decode(decoder)?)),
            "ByStatus" => Ok(Self::ByStatus(ByStatus::decode(decoder)?)),
            "ByAlias" => Ok(Self::ByAlias(ByAlias::decode(decoder)?)),
            other => Err(nota_codec::Error::UnknownKindForVerb {
                verb: "QueryKind",
                got: other.to_string(),
            }),
        }
    }
}

#[derive(NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct Query {
    pub kind: QueryKindText,
    pub limit: u16,
}

impl Query {
    fn into_contract(self) -> contract::MindRequest {
        contract::MindRequest::Query(contract::Query {
            kind: self.kind.into_contract(),
            limit: contract::QueryLimit::new(self.limit),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MindTextRequest {
    Opening(Opening),
    NoteSubmission(NoteSubmission),
    Link(Link),
    StatusChange(StatusChange),
    AliasAssignment(AliasAssignment),
    Query(Query),
}

impl MindTextRequest {
    pub fn from_nota(text: &str) -> Result<Self> {
        let mut decoder = Decoder::new(text);
        let request = Self::decode(&mut decoder)?;
        MindTextEnd::new(&mut decoder).expect()?;
        Ok(request)
    }

    pub fn into_request(self) -> Result<contract::MindRequest> {
        match self {
            Self::Opening(opening) => Ok(opening.into_contract()),
            Self::NoteSubmission(submission) => Ok(submission.into_contract()),
            Self::Link(link) => Ok(link.into_contract()),
            Self::StatusChange(change) => Ok(change.into_contract()),
            Self::AliasAssignment(assignment) => Ok(assignment.into_contract()),
            Self::Query(query) => Ok(query.into_contract()),
        }
    }
}

impl NotaEncode for MindTextRequest {
    fn encode(&self, encoder: &mut Encoder) -> nota_codec::Result<()> {
        match self {
            Self::Opening(opening) => opening.encode(encoder),
            Self::NoteSubmission(submission) => submission.encode(encoder),
            Self::Link(link) => link.encode(encoder),
            Self::StatusChange(change) => change.encode(encoder),
            Self::AliasAssignment(assignment) => assignment.encode(encoder),
            Self::Query(query) => query.encode(encoder),
        }
    }
}

impl NotaDecode for MindTextRequest {
    fn decode(decoder: &mut Decoder<'_>) -> nota_codec::Result<Self> {
        match decoder.peek_record_head()?.as_str() {
            "Opening" => Ok(Self::Opening(Opening::decode(decoder)?)),
            "NoteSubmission" => Ok(Self::NoteSubmission(NoteSubmission::decode(decoder)?)),
            "Link" => Ok(Self::Link(Link::decode(decoder)?)),
            "StatusChange" => Ok(Self::StatusChange(StatusChange::decode(decoder)?)),
            "AliasAssignment" => Ok(Self::AliasAssignment(AliasAssignment::decode(decoder)?)),
            "Query" => Ok(Self::Query(Query::decode(decoder)?)),
            other => Err(nota_codec::Error::UnknownKindForVerb {
                verb: "MindTextRequest",
                got: other.to_string(),
            }),
        }
    }
}

#[derive(NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub id: String,
    pub display_id: String,
    pub aliases: Vec<String>,
    pub kind: ItemKindText,
    pub status: ItemStatusText,
    pub priority: ItemPriorityText,
    pub title: String,
    pub body: String,
}

impl Item {
    fn from_contract(item: contract::Item) -> Self {
        Self {
            id: item.id.as_str().to_string(),
            display_id: item.display_id.as_str().to_string(),
            aliases: item
                .aliases
                .into_iter()
                .map(|alias| alias.as_str().to_string())
                .collect(),
            kind: ItemKindText::from_contract(item.kind),
            status: ItemStatusText::from_contract(item.status),
            priority: ItemPriorityText::from_contract(item.priority),
            title: item.title.as_str().to_string(),
            body: item.body.as_str().to_string(),
        }
    }
}

#[derive(NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub event: u64,
    pub item: String,
    pub author: String,
    pub body: String,
}

impl Note {
    fn from_contract(note: contract::Note) -> Self {
        Self {
            event: note.event.into_u64(),
            item: note.item.as_str().to_string(),
            author: note.author.as_str().to_string(),
            body: note.body.as_str().to_string(),
        }
    }
}

#[derive(NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct ItemTarget {
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EdgeTargetText {
    ItemTarget(ItemTarget),
    Report(Report),
    GitCommit(GitCommit),
    BeadsTask(BeadsTask),
    File(File),
}

impl EdgeTargetText {
    fn from_contract(target: contract::EdgeTarget) -> Self {
        match target {
            contract::EdgeTarget::Item(id) => Self::ItemTarget(ItemTarget {
                id: id.as_str().to_string(),
            }),
            contract::EdgeTarget::External(external) => match external {
                contract::ExternalReference::Report(path) => Self::Report(Report {
                    path: path.as_str().to_string(),
                }),
                contract::ExternalReference::GitCommit(hash) => Self::GitCommit(GitCommit {
                    hash: hash.as_str().to_string(),
                }),
                contract::ExternalReference::BeadsTask(token) => Self::BeadsTask(BeadsTask {
                    token: token.as_str().to_string(),
                }),
                contract::ExternalReference::File(path) => Self::File(File {
                    path: path.as_str().to_string(),
                }),
            },
        }
    }
}

impl NotaEncode for EdgeTargetText {
    fn encode(&self, encoder: &mut Encoder) -> nota_codec::Result<()> {
        match self {
            Self::ItemTarget(target) => target.encode(encoder),
            Self::Report(report) => report.encode(encoder),
            Self::GitCommit(commit) => commit.encode(encoder),
            Self::BeadsTask(task) => task.encode(encoder),
            Self::File(file) => file.encode(encoder),
        }
    }
}

impl NotaDecode for EdgeTargetText {
    fn decode(decoder: &mut Decoder<'_>) -> nota_codec::Result<Self> {
        match decoder.peek_record_head()?.as_str() {
            "ItemTarget" => Ok(Self::ItemTarget(ItemTarget::decode(decoder)?)),
            "Report" => Ok(Self::Report(Report::decode(decoder)?)),
            "GitCommit" => Ok(Self::GitCommit(GitCommit::decode(decoder)?)),
            "BeadsTask" => Ok(Self::BeadsTask(BeadsTask::decode(decoder)?)),
            "File" => Ok(Self::File(File::decode(decoder)?)),
            other => Err(nota_codec::Error::UnknownKindForVerb {
                verb: "EdgeTarget",
                got: other.to_string(),
            }),
        }
    }
}

#[derive(NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    pub event: u64,
    pub source: String,
    pub kind: EdgeKindText,
    pub target: EdgeTargetText,
    pub body: Option<String>,
}

impl Edge {
    fn from_contract(edge: contract::Edge) -> Self {
        Self {
            event: edge.event.into_u64(),
            source: edge.source.as_str().to_string(),
            kind: EdgeKindText::from_contract(edge.kind),
            target: EdgeTargetText::from_contract(edge.target),
            body: edge.body.map(|body| body.as_str().to_string()),
        }
    }
}

#[derive(NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct EventHeader {
    pub event: u64,
    pub operation: String,
    pub actor: String,
}

impl EventHeader {
    fn from_contract(header: contract::EventHeader) -> Self {
        Self {
            event: header.event.into_u64(),
            operation: header.operation.as_str().to_string(),
            actor: header.actor.as_str().to_string(),
        }
    }
}

#[derive(NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct ItemOpenedEvent {
    pub header: EventHeader,
    pub item: Item,
}

impl ItemOpenedEvent {
    fn from_contract(event: contract::ItemOpenedEvent) -> Self {
        Self {
            header: EventHeader::from_contract(event.header),
            item: Item::from_contract(event.item),
        }
    }
}

#[derive(NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct NoteAddedEvent {
    pub header: EventHeader,
    pub note: Note,
}

impl NoteAddedEvent {
    fn from_contract(event: contract::NoteAddedEvent) -> Self {
        Self {
            header: EventHeader::from_contract(event.header),
            note: Note::from_contract(event.note),
        }
    }
}

#[derive(NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct EdgeAddedEvent {
    pub header: EventHeader,
    pub edge: Edge,
}

impl EdgeAddedEvent {
    fn from_contract(event: contract::EdgeAddedEvent) -> Self {
        Self {
            header: EventHeader::from_contract(event.header),
            edge: Edge::from_contract(event.edge),
        }
    }
}

#[derive(NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct StatusChangedEvent {
    pub header: EventHeader,
    pub item: String,
    pub status: ItemStatusText,
    pub body: Option<String>,
}

impl StatusChangedEvent {
    fn from_contract(event: contract::StatusChangedEvent) -> Self {
        Self {
            header: EventHeader::from_contract(event.header),
            item: event.item.as_str().to_string(),
            status: ItemStatusText::from_contract(event.status),
            body: event.body.map(|body| body.as_str().to_string()),
        }
    }
}

#[derive(NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct AliasAddedEvent {
    pub header: EventHeader,
    pub item: String,
    pub alias: String,
}

impl AliasAddedEvent {
    fn from_contract(event: contract::AliasAddedEvent) -> Self {
        Self {
            header: EventHeader::from_contract(event.header),
            item: event.item.as_str().to_string(),
            alias: event.alias.as_str().to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    ItemOpened(ItemOpenedEvent),
    NoteAdded(NoteAddedEvent),
    EdgeAdded(EdgeAddedEvent),
    StatusChanged(StatusChangedEvent),
    AliasAdded(AliasAddedEvent),
}

impl Event {
    fn from_contract(event: contract::Event) -> Self {
        match event {
            contract::Event::ItemOpened(event) => {
                Self::ItemOpened(ItemOpenedEvent::from_contract(event))
            }
            contract::Event::NoteAdded(event) => {
                Self::NoteAdded(NoteAddedEvent::from_contract(event))
            }
            contract::Event::EdgeAdded(event) => {
                Self::EdgeAdded(EdgeAddedEvent::from_contract(event))
            }
            contract::Event::StatusChanged(event) => {
                Self::StatusChanged(StatusChangedEvent::from_contract(event))
            }
            contract::Event::AliasAdded(event) => {
                Self::AliasAdded(AliasAddedEvent::from_contract(event))
            }
        }
    }
}

impl NotaEncode for Event {
    fn encode(&self, encoder: &mut Encoder) -> nota_codec::Result<()> {
        match self {
            Self::ItemOpened(event) => event.encode(encoder),
            Self::NoteAdded(event) => event.encode(encoder),
            Self::EdgeAdded(event) => event.encode(encoder),
            Self::StatusChanged(event) => event.encode(encoder),
            Self::AliasAdded(event) => event.encode(encoder),
        }
    }
}

impl NotaDecode for Event {
    fn decode(decoder: &mut Decoder<'_>) -> nota_codec::Result<Self> {
        match decoder.peek_record_head()?.as_str() {
            "ItemOpenedEvent" => Ok(Self::ItemOpened(ItemOpenedEvent::decode(decoder)?)),
            "NoteAddedEvent" => Ok(Self::NoteAdded(NoteAddedEvent::decode(decoder)?)),
            "EdgeAddedEvent" => Ok(Self::EdgeAdded(EdgeAddedEvent::decode(decoder)?)),
            "StatusChangedEvent" => Ok(Self::StatusChanged(StatusChangedEvent::decode(decoder)?)),
            "AliasAddedEvent" => Ok(Self::AliasAdded(AliasAddedEvent::decode(decoder)?)),
            other => Err(nota_codec::Error::UnknownKindForVerb {
                verb: "Event",
                got: other.to_string(),
            }),
        }
    }
}

#[derive(NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct OpeningReceipt {
    pub event: ItemOpenedEvent,
}

impl OpeningReceipt {
    fn from_contract(receipt: contract::OpeningReceipt) -> Self {
        Self {
            event: ItemOpenedEvent::from_contract(receipt.event),
        }
    }
}

#[derive(NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct NoteReceipt {
    pub event: NoteAddedEvent,
}

impl NoteReceipt {
    fn from_contract(receipt: contract::NoteReceipt) -> Self {
        Self {
            event: NoteAddedEvent::from_contract(receipt.event),
        }
    }
}

#[derive(NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct LinkReceipt {
    pub event: EdgeAddedEvent,
}

impl LinkReceipt {
    fn from_contract(receipt: contract::LinkReceipt) -> Self {
        Self {
            event: EdgeAddedEvent::from_contract(receipt.event),
        }
    }
}

#[derive(NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct StatusReceipt {
    pub event: StatusChangedEvent,
}

impl StatusReceipt {
    fn from_contract(receipt: contract::StatusReceipt) -> Self {
        Self {
            event: StatusChangedEvent::from_contract(receipt.event),
        }
    }
}

#[derive(NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct AliasReceipt {
    pub event: AliasAddedEvent,
}

impl AliasReceipt {
    fn from_contract(receipt: contract::AliasReceipt) -> Self {
        Self {
            event: AliasAddedEvent::from_contract(receipt.event),
        }
    }
}

#[derive(NotaRecord, Debug, Clone, PartialEq, Eq)]
pub struct View {
    pub items: Vec<Item>,
    pub edges: Vec<Edge>,
    pub notes: Vec<Note>,
    pub events: Vec<Event>,
}

impl View {
    fn from_contract(view: contract::View) -> Self {
        Self {
            items: view.items.into_iter().map(Item::from_contract).collect(),
            edges: view.edges.into_iter().map(Edge::from_contract).collect(),
            notes: view.notes.into_iter().map(Note::from_contract).collect(),
            events: view.events.into_iter().map(Event::from_contract).collect(),
        }
    }
}

#[derive(NotaEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectionReasonText {
    UnknownItem,
    DuplicateAlias,
    InvalidEdge,
    PersistenceRejected,
    UnsupportedQuery,
    CollisionUnresolved,
}

impl RejectionReasonText {
    fn from_contract(reason: contract::RejectionReason) -> Self {
        match reason {
            contract::RejectionReason::UnknownItem => Self::UnknownItem,
            contract::RejectionReason::DuplicateAlias => Self::DuplicateAlias,
            contract::RejectionReason::InvalidEdge => Self::InvalidEdge,
            contract::RejectionReason::PersistenceRejected => Self::PersistenceRejected,
            contract::RejectionReason::UnsupportedQuery => Self::UnsupportedQuery,
            contract::RejectionReason::CollisionUnresolved => Self::CollisionUnresolved,
        }
    }
}

#[derive(NotaRecord, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rejection {
    pub reason: RejectionReasonText,
}

impl Rejection {
    fn from_contract(rejection: contract::Rejection) -> Self {
        Self {
            reason: RejectionReasonText::from_contract(rejection.reason),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MindTextReply {
    OpeningReceipt(OpeningReceipt),
    NoteReceipt(NoteReceipt),
    LinkReceipt(LinkReceipt),
    StatusReceipt(StatusReceipt),
    AliasReceipt(AliasReceipt),
    View(View),
    Rejection(Rejection),
}

impl MindTextReply {
    pub fn from_reply(reply: contract::MindReply) -> Result<Self> {
        match reply {
            contract::MindReply::OpeningReceipt(receipt) => {
                Ok(Self::OpeningReceipt(OpeningReceipt::from_contract(receipt)))
            }
            contract::MindReply::NoteReceipt(receipt) => {
                Ok(Self::NoteReceipt(NoteReceipt::from_contract(receipt)))
            }
            contract::MindReply::LinkReceipt(receipt) => {
                Ok(Self::LinkReceipt(LinkReceipt::from_contract(receipt)))
            }
            contract::MindReply::StatusReceipt(receipt) => {
                Ok(Self::StatusReceipt(StatusReceipt::from_contract(receipt)))
            }
            contract::MindReply::AliasReceipt(receipt) => {
                Ok(Self::AliasReceipt(AliasReceipt::from_contract(receipt)))
            }
            contract::MindReply::View(view) => Ok(Self::View(View::from_contract(view))),
            contract::MindReply::Rejection(rejection) => {
                Ok(Self::Rejection(Rejection::from_contract(rejection)))
            }
            contract::MindReply::ThoughtCommitted(_)
            | contract::MindReply::RelationCommitted(_)
            | contract::MindReply::ThoughtList(_)
            | contract::MindReply::RelationList(_)
            | contract::MindReply::SubscriptionAccepted(_)
            | contract::MindReply::SubscriptionRetracted(_)
            | contract::MindReply::AdjudicationReceipt(_)
            | contract::MindReply::ChannelReceipt(_)
            | contract::MindReply::AdjudicationDenyReceipt(_)
            | contract::MindReply::ChannelListView(_)
            | contract::MindReply::MindRequestUnimplemented(_) => Err(
                crate::Error::UnexpectedFrame("mind reply has no MindTextReply projection"),
            ),
        }
    }

    pub fn to_nota(&self) -> Result<String> {
        let mut encoder = Encoder::new();
        self.encode(&mut encoder)?;
        Ok(encoder.into_string())
    }
}

impl NotaEncode for MindTextReply {
    fn encode(&self, encoder: &mut Encoder) -> nota_codec::Result<()> {
        match self {
            Self::OpeningReceipt(receipt) => receipt.encode(encoder),
            Self::NoteReceipt(receipt) => receipt.encode(encoder),
            Self::LinkReceipt(receipt) => receipt.encode(encoder),
            Self::StatusReceipt(receipt) => receipt.encode(encoder),
            Self::AliasReceipt(receipt) => receipt.encode(encoder),
            Self::View(view) => view.encode(encoder),
            Self::Rejection(rejection) => rejection.encode(encoder),
        }
    }
}

struct MindTextEnd<'decoder, 'input> {
    decoder: &'decoder mut Decoder<'input>,
}

impl<'decoder, 'input> MindTextEnd<'decoder, 'input> {
    fn new(decoder: &'decoder mut Decoder<'input>) -> Self {
        Self { decoder }
    }

    fn expect(&mut self) -> nota_codec::Result<()> {
        if let Some(token) = self.decoder.peek_token()? {
            Err(nota_codec::Error::UnexpectedToken {
                expected: "end of input",
                got: token,
            })
        } else {
            Ok(())
        }
    }
}
