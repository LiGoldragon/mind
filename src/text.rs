use nota_next::{Block, Delimiter, NotaBlock, NotaDecode, NotaDecodeError, NotaEncode, NotaSource};
use signal_mind as contract;

use crate::Result as MindResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

macro_rules! bare_enum_codec {
    ($type_name:ident { $($variant:ident),+ $(,)? }) => {
        impl NotaEncode for $type_name {
            fn to_nota(&self) -> String {
                match self {
                    $(Self::$variant => stringify!($variant),)+
                }
                .to_owned()
            }
        }

        impl NotaDecode for $type_name {
            fn from_nota_block(block: &Block) -> Result<Self, NotaDecodeError> {
                let variant = block.demote_to_string().ok_or(NotaDecodeError::ExpectedAtom {
                    type_name: stringify!($type_name),
                })?;
                match variant {
                    $(stringify!($variant) => Ok(Self::$variant),)+
                    other => Err(NotaDecodeError::UnknownVariant {
                        enum_name: stringify!($type_name),
                        variant: other.to_owned(),
                    }),
                }
            }
        }
    };
}

#[derive(Debug, Clone, Copy)]
struct TextVariantRecord<'block> {
    enum_name: &'static str,
    variant: &'block str,
    fields: &'block [Block],
}

impl<'block> TextVariantRecord<'block> {
    fn from_block(block: &'block Block, enum_name: &'static str) -> Result<Self, NotaDecodeError> {
        let children = NotaBlock::new(block).expect_delimited(Delimiter::Parenthesis, enum_name)?;
        let Some((head, fields)) = children.split_first() else {
            return Err(NotaDecodeError::ExpectedRootCount {
                type_name: enum_name,
                expected: 1,
                found: 0,
            });
        };
        let variant = head
            .demote_to_string()
            .ok_or(NotaDecodeError::ExpectedAtom {
                type_name: "variant head",
            })?;
        Ok(Self {
            enum_name,
            variant,
            fields,
        })
    }

    fn variant(&self) -> &'block str {
        self.variant
    }

    fn expect_fields(&self, expected: usize) -> Result<&'block [Block], NotaDecodeError> {
        let found = self.fields.len();
        if found != expected {
            return Err(NotaDecodeError::ExpectedRootCount {
                type_name: self.enum_name,
                expected,
                found,
            });
        }
        Ok(self.fields)
    }

    fn field<Value>(&self, index: usize) -> Result<Value, NotaDecodeError>
    where
        Value: NotaDecode,
    {
        Value::from_nota_block(&self.fields[index])
    }

    fn optional_string(&self, index: usize) -> Result<Option<String>, NotaDecodeError> {
        TextOptionalString::from_nota_block(&self.fields[index]).map(TextOptionalString::into_inner)
    }

    fn unknown_variant(&self) -> NotaDecodeError {
        NotaDecodeError::UnknownVariant {
            enum_name: self.enum_name,
            variant: self.variant.to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TextVariantEncoding {
    variant: &'static str,
    fields: Vec<String>,
}

impl TextVariantEncoding {
    fn new(variant: &'static str, fields: Vec<String>) -> Self {
        Self { variant, fields }
    }

    fn to_nota(&self) -> String {
        Delimiter::Parenthesis
            .wrap(std::iter::once(self.variant.to_owned()).chain(self.fields.iter().cloned()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TextOptionalString {
    value: Option<String>,
}

impl TextOptionalString {
    fn from_option(value: &Option<String>) -> Self {
        Self {
            value: value.clone(),
        }
    }

    fn into_inner(self) -> Option<String> {
        self.value
    }
}

impl NotaEncode for TextOptionalString {
    fn to_nota(&self) -> String {
        match &self.value {
            Some(value) => value.to_nota(),
            None => "None".to_owned(),
        }
    }
}

impl NotaDecode for TextOptionalString {
    fn from_nota_block(block: &Block) -> Result<Self, NotaDecodeError> {
        if block.demote_to_string() == Some("None") {
            return Ok(Self { value: None });
        }
        Ok(Self {
            value: Some(String::from_nota_block(block)?),
        })
    }
}

bare_enum_codec!(ItemKindText {
    Task,
    Defect,
    Question,
    Decision,
    Note,
    Handoff,
});

bare_enum_codec!(ItemStatusText {
    Open,
    InProgress,
    Blocked,
    Closed,
    Deferred,
});

bare_enum_codec!(EdgeKindText {
    DependsOn,
    ParentOf,
    RelatesTo,
    Duplicates,
    Supersedes,
    Answers,
    References,
});

#[derive(NotaEncode, NotaDecode, Debug, Clone, PartialEq, Eq)]
pub struct Opening {
    pub kind: ItemKindText,
    pub priority: contract::Magnitude,
    pub title: String,
    pub body: String,
}

impl Opening {
    fn into_contract(self) -> contract::MindRequest {
        contract::MindRequest::Opening(contract::Opening {
            kind: self.kind.into_contract(),
            priority: self.priority,
            title: contract::Title::new(self.title),
            body: contract::TextBody::new(self.body),
        })
    }
}

#[derive(NotaEncode, NotaDecode, Debug, Clone, PartialEq, Eq)]
pub struct Stable {
    pub id: String,
}

#[derive(NotaEncode, NotaDecode, Debug, Clone, PartialEq, Eq)]
pub struct Display {
    pub id: String,
}

#[derive(NotaEncode, NotaDecode, Debug, Clone, PartialEq, Eq)]
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
                contract::ItemReference::Stable(contract::StableItemIdentifier::new(stable.id))
            }
            Self::Display(display) => {
                contract::ItemReference::Display(contract::DisplayIdentifier::new(display.id))
            }
            Self::Alias(alias) => {
                contract::ItemReference::Alias(contract::ExternalAlias::new(alias.alias))
            }
        }
    }
}

impl NotaEncode for ItemReferenceText {
    fn to_nota(&self) -> String {
        match self {
            Self::Stable(stable) => {
                TextVariantEncoding::new("Stable", vec![stable.id.to_nota()]).to_nota()
            }
            Self::Display(display) => {
                TextVariantEncoding::new("Display", vec![display.id.to_nota()]).to_nota()
            }
            Self::Alias(alias) => {
                TextVariantEncoding::new("Alias", vec![alias.alias.to_nota()]).to_nota()
            }
        }
    }
}

impl NotaDecode for ItemReferenceText {
    fn from_nota_block(block: &Block) -> Result<Self, NotaDecodeError> {
        let record = TextVariantRecord::from_block(block, "ItemReferenceText")?;
        match record.variant() {
            "Stable" => {
                record.expect_fields(1)?;
                Ok(Self::Stable(Stable {
                    id: record.field(0)?,
                }))
            }
            "Display" => {
                record.expect_fields(1)?;
                Ok(Self::Display(Display {
                    id: record.field(0)?,
                }))
            }
            "Alias" => {
                record.expect_fields(1)?;
                Ok(Self::Alias(Alias {
                    alias: record.field(0)?,
                }))
            }
            _ => Err(record.unknown_variant()),
        }
    }
}

#[derive(NotaEncode, NotaDecode, Debug, Clone, PartialEq, Eq)]
pub struct ItemReferenceTarget {
    pub item: ItemReferenceText,
}

#[derive(NotaEncode, NotaDecode, Debug, Clone, PartialEq, Eq)]
pub struct Report {
    pub path: String,
}

#[derive(NotaEncode, NotaDecode, Debug, Clone, PartialEq, Eq)]
pub struct GitCommit {
    pub hash: String,
}

#[derive(NotaEncode, NotaDecode, Debug, Clone, PartialEq, Eq)]
pub struct BeadsTask {
    pub token: String,
}

#[derive(NotaEncode, NotaDecode, Debug, Clone, PartialEq, Eq)]
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
    fn to_nota(&self) -> String {
        match self {
            Self::ItemReferenceTarget(target) => {
                TextVariantEncoding::new("ItemReferenceTarget", vec![target.item.to_nota()])
                    .to_nota()
            }
            Self::Report(report) => {
                TextVariantEncoding::new("Report", vec![report.path.to_nota()]).to_nota()
            }
            Self::GitCommit(commit) => {
                TextVariantEncoding::new("GitCommit", vec![commit.hash.to_nota()]).to_nota()
            }
            Self::BeadsTask(task) => {
                TextVariantEncoding::new("BeadsTask", vec![task.token.to_nota()]).to_nota()
            }
            Self::File(file) => {
                TextVariantEncoding::new("File", vec![file.path.to_nota()]).to_nota()
            }
        }
    }
}

impl NotaDecode for LinkTargetText {
    fn from_nota_block(block: &Block) -> Result<Self, NotaDecodeError> {
        let record = TextVariantRecord::from_block(block, "LinkTargetText")?;
        match record.variant() {
            "ItemReferenceTarget" => {
                record.expect_fields(1)?;
                Ok(Self::ItemReferenceTarget(ItemReferenceTarget {
                    item: record.field(0)?,
                }))
            }
            "Report" => {
                record.expect_fields(1)?;
                Ok(Self::Report(Report {
                    path: record.field(0)?,
                }))
            }
            "GitCommit" => {
                record.expect_fields(1)?;
                Ok(Self::GitCommit(GitCommit {
                    hash: record.field(0)?,
                }))
            }
            "BeadsTask" => {
                record.expect_fields(1)?;
                Ok(Self::BeadsTask(BeadsTask {
                    token: record.field(0)?,
                }))
            }
            "File" => {
                record.expect_fields(1)?;
                Ok(Self::File(File {
                    path: record.field(0)?,
                }))
            }
            _ => Err(record.unknown_variant()),
        }
    }
}

#[derive(NotaEncode, NotaDecode, Debug, Clone, PartialEq, Eq)]
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

#[derive(NotaEncode, NotaDecode, Debug, Clone, PartialEq, Eq)]
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

#[derive(NotaEncode, NotaDecode, Debug, Clone, PartialEq, Eq)]
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

#[derive(NotaEncode, NotaDecode, Debug, Clone, PartialEq, Eq)]
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

#[derive(NotaEncode, NotaDecode, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ready {}

#[derive(NotaEncode, NotaDecode, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Blocked {}

#[derive(NotaEncode, NotaDecode, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Open {}

#[derive(NotaEncode, NotaDecode, Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecentEvents {}

#[derive(NotaEncode, NotaDecode, Debug, Clone, PartialEq, Eq)]
pub struct ByItem {
    pub item: ItemReferenceText,
}

#[derive(NotaEncode, NotaDecode, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByKind {
    pub kind: ItemKindText,
}

#[derive(NotaEncode, NotaDecode, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByStatus {
    pub status: ItemStatusText,
}

#[derive(NotaEncode, NotaDecode, Debug, Clone, PartialEq, Eq)]
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
    fn to_nota(&self) -> String {
        match self {
            Self::Ready(_) => TextVariantEncoding::new("Ready", Vec::new()).to_nota(),
            Self::Blocked(_) => TextVariantEncoding::new("Blocked", Vec::new()).to_nota(),
            Self::Open(_) => TextVariantEncoding::new("Open", Vec::new()).to_nota(),
            Self::RecentEvents(_) => TextVariantEncoding::new("RecentEvents", Vec::new()).to_nota(),
            Self::ByItem(query) => {
                TextVariantEncoding::new("ByItem", vec![query.item.to_nota()]).to_nota()
            }
            Self::ByKind(query) => {
                TextVariantEncoding::new("ByKind", vec![query.kind.to_nota()]).to_nota()
            }
            Self::ByStatus(query) => {
                TextVariantEncoding::new("ByStatus", vec![query.status.to_nota()]).to_nota()
            }
            Self::ByAlias(query) => {
                TextVariantEncoding::new("ByAlias", vec![query.alias.to_nota()]).to_nota()
            }
        }
    }
}

impl NotaDecode for QueryKindText {
    fn from_nota_block(block: &Block) -> Result<Self, NotaDecodeError> {
        let record = TextVariantRecord::from_block(block, "QueryKindText")?;
        match record.variant() {
            "Ready" => {
                record.expect_fields(0)?;
                Ok(Self::Ready(Ready {}))
            }
            "Blocked" => {
                record.expect_fields(0)?;
                Ok(Self::Blocked(Blocked {}))
            }
            "Open" => {
                record.expect_fields(0)?;
                Ok(Self::Open(Open {}))
            }
            "RecentEvents" => {
                record.expect_fields(0)?;
                Ok(Self::RecentEvents(RecentEvents {}))
            }
            "ByItem" => {
                record.expect_fields(1)?;
                Ok(Self::ByItem(ByItem {
                    item: record.field(0)?,
                }))
            }
            "ByKind" => {
                record.expect_fields(1)?;
                Ok(Self::ByKind(ByKind {
                    kind: record.field(0)?,
                }))
            }
            "ByStatus" => {
                record.expect_fields(1)?;
                Ok(Self::ByStatus(ByStatus {
                    status: record.field(0)?,
                }))
            }
            "ByAlias" => {
                record.expect_fields(1)?;
                Ok(Self::ByAlias(ByAlias {
                    alias: record.field(0)?,
                }))
            }
            _ => Err(record.unknown_variant()),
        }
    }
}

#[derive(NotaEncode, NotaDecode, Debug, Clone, PartialEq, Eq)]
pub struct Query {
    pub kind: QueryKindText,
    pub limit: contract::QueryLimit,
}

impl Query {
    fn into_contract(self) -> contract::MindRequest {
        contract::MindRequest::Query(contract::Query {
            kind: self.kind.into_contract(),
            limit: self.limit,
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
    pub fn from_nota(text: &str) -> MindResult<Self> {
        NotaSource::new(text).parse::<Self>().map_err(Into::into)
    }

    pub fn into_request(self) -> MindResult<contract::MindRequest> {
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
    fn to_nota(&self) -> String {
        match self {
            Self::Opening(opening) => TextVariantEncoding::new(
                "Opening",
                vec![
                    opening.kind.to_nota(),
                    opening.priority.to_nota(),
                    opening.title.to_nota(),
                    opening.body.to_nota(),
                ],
            )
            .to_nota(),
            Self::NoteSubmission(submission) => TextVariantEncoding::new(
                "NoteSubmission",
                vec![submission.item.to_nota(), submission.body.to_nota()],
            )
            .to_nota(),
            Self::Link(link) => TextVariantEncoding::new(
                "Link",
                vec![
                    link.source.to_nota(),
                    link.kind.to_nota(),
                    link.target.to_nota(),
                    TextOptionalString::from_option(&link.body).to_nota(),
                ],
            )
            .to_nota(),
            Self::StatusChange(change) => TextVariantEncoding::new(
                "StatusChange",
                vec![
                    change.item.to_nota(),
                    change.status.to_nota(),
                    TextOptionalString::from_option(&change.body).to_nota(),
                ],
            )
            .to_nota(),
            Self::AliasAssignment(assignment) => TextVariantEncoding::new(
                "AliasAssignment",
                vec![assignment.item.to_nota(), assignment.alias.to_nota()],
            )
            .to_nota(),
            Self::Query(query) => {
                TextVariantEncoding::new("Query", vec![query.kind.to_nota(), query.limit.to_nota()])
                    .to_nota()
            }
        }
    }
}

impl NotaDecode for MindTextRequest {
    fn from_nota_block(block: &Block) -> Result<Self, NotaDecodeError> {
        let record = TextVariantRecord::from_block(block, "MindTextRequest")?;
        match record.variant() {
            "Opening" => {
                record.expect_fields(4)?;
                Ok(Self::Opening(Opening {
                    kind: record.field(0)?,
                    priority: record.field(1)?,
                    title: record.field(2)?,
                    body: record.field(3)?,
                }))
            }
            "NoteSubmission" => {
                record.expect_fields(2)?;
                Ok(Self::NoteSubmission(NoteSubmission {
                    item: record.field(0)?,
                    body: record.field(1)?,
                }))
            }
            "Link" => {
                record.expect_fields(4)?;
                Ok(Self::Link(Link {
                    source: record.field(0)?,
                    kind: record.field(1)?,
                    target: record.field(2)?,
                    body: record.optional_string(3)?,
                }))
            }
            "StatusChange" => {
                record.expect_fields(3)?;
                Ok(Self::StatusChange(StatusChange {
                    item: record.field(0)?,
                    status: record.field(1)?,
                    body: record.optional_string(2)?,
                }))
            }
            "AliasAssignment" => {
                record.expect_fields(2)?;
                Ok(Self::AliasAssignment(AliasAssignment {
                    item: record.field(0)?,
                    alias: record.field(1)?,
                }))
            }
            "Query" => {
                record.expect_fields(2)?;
                Ok(Self::Query(Query {
                    kind: record.field(0)?,
                    limit: record.field(1)?,
                }))
            }
            _ => Err(record.unknown_variant()),
        }
    }
}

#[derive(NotaEncode, NotaDecode, Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub id: String,
    pub display_identifier: String,
    pub aliases: Vec<String>,
    pub kind: ItemKindText,
    pub status: ItemStatusText,
    pub priority: contract::Magnitude,
    pub title: String,
    pub body: String,
}

impl Item {
    fn from_contract(item: contract::Item) -> Self {
        Self {
            id: item.id.as_str().to_string(),
            display_identifier: item.display_identifier.as_str().to_string(),
            aliases: item
                .aliases
                .into_iter()
                .map(|alias| alias.as_str().to_string())
                .collect(),
            kind: ItemKindText::from_contract(item.kind),
            status: ItemStatusText::from_contract(item.status),
            priority: item.priority,
            title: item.title.as_str().to_string(),
            body: item.body.as_str().to_string(),
        }
    }
}

#[derive(NotaEncode, NotaDecode, Debug, Clone, PartialEq, Eq)]
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

#[derive(NotaEncode, NotaDecode, Debug, Clone, PartialEq, Eq)]
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
    fn to_nota(&self) -> String {
        match self {
            Self::ItemTarget(target) => {
                TextVariantEncoding::new("ItemTarget", vec![target.id.to_nota()]).to_nota()
            }
            Self::Report(report) => {
                TextVariantEncoding::new("Report", vec![report.path.to_nota()]).to_nota()
            }
            Self::GitCommit(commit) => {
                TextVariantEncoding::new("GitCommit", vec![commit.hash.to_nota()]).to_nota()
            }
            Self::BeadsTask(task) => {
                TextVariantEncoding::new("BeadsTask", vec![task.token.to_nota()]).to_nota()
            }
            Self::File(file) => {
                TextVariantEncoding::new("File", vec![file.path.to_nota()]).to_nota()
            }
        }
    }
}

impl NotaDecode for EdgeTargetText {
    fn from_nota_block(block: &Block) -> Result<Self, NotaDecodeError> {
        let record = TextVariantRecord::from_block(block, "EdgeTargetText")?;
        match record.variant() {
            "ItemTarget" => {
                record.expect_fields(1)?;
                Ok(Self::ItemTarget(ItemTarget {
                    id: record.field(0)?,
                }))
            }
            "Report" => {
                record.expect_fields(1)?;
                Ok(Self::Report(Report {
                    path: record.field(0)?,
                }))
            }
            "GitCommit" => {
                record.expect_fields(1)?;
                Ok(Self::GitCommit(GitCommit {
                    hash: record.field(0)?,
                }))
            }
            "BeadsTask" => {
                record.expect_fields(1)?;
                Ok(Self::BeadsTask(BeadsTask {
                    token: record.field(0)?,
                }))
            }
            "File" => {
                record.expect_fields(1)?;
                Ok(Self::File(File {
                    path: record.field(0)?,
                }))
            }
            _ => Err(record.unknown_variant()),
        }
    }
}

#[derive(NotaEncode, NotaDecode, Debug, Clone, PartialEq, Eq)]
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

#[derive(NotaEncode, NotaDecode, Debug, Clone, PartialEq, Eq)]
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

#[derive(NotaEncode, NotaDecode, Debug, Clone, PartialEq, Eq)]
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

#[derive(NotaEncode, NotaDecode, Debug, Clone, PartialEq, Eq)]
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

#[derive(NotaEncode, NotaDecode, Debug, Clone, PartialEq, Eq)]
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

#[derive(NotaEncode, NotaDecode, Debug, Clone, PartialEq, Eq)]
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

#[derive(NotaEncode, NotaDecode, Debug, Clone, PartialEq, Eq)]
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

#[derive(NotaEncode, NotaDecode, Debug, Clone, PartialEq, Eq)]
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

#[derive(NotaEncode, NotaDecode, Debug, Clone, PartialEq, Eq)]
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

#[derive(NotaEncode, NotaDecode, Debug, Clone, PartialEq, Eq)]
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

#[derive(NotaEncode, NotaDecode, Debug, Clone, PartialEq, Eq)]
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

#[derive(NotaEncode, NotaDecode, Debug, Clone, PartialEq, Eq)]
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

#[derive(NotaEncode, NotaDecode, Debug, Clone, PartialEq, Eq)]
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

#[derive(NotaEncode, NotaDecode, Debug, Clone, PartialEq, Eq)]
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

#[derive(NotaEncode, NotaDecode, Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(NotaEncode, NotaDecode, Debug, Clone, Copy, PartialEq, Eq)]
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
    pub fn from_reply(reply: contract::MindReply) -> MindResult<Self> {
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
            | contract::MindReply::TechnicalNodeCommitted(_)
            | contract::MindReply::TechnicalRelationCommitted(_)
            | contract::MindReply::TechnicalNodeList(_)
            | contract::MindReply::TechnicalRelationList(_)
            | contract::MindReply::TechnicalNodeNeighborhood(_)
            | contract::MindReply::TechnicalDependencyClosure(_)
            | contract::MindReply::TechnicalProvenanceChain(_)
            | contract::MindReply::TechnicalNodeRejected(_)
            | contract::MindReply::TechnicalRelationRejected(_)
            | contract::MindReply::KnowledgeAccepted(_)
            | contract::MindReply::KnowledgeRejected(_)
            | contract::MindReply::KnowledgeList(_)
            | contract::MindReply::SubscriptionAccepted(_)
            | contract::MindReply::SubscriptionRetracted(_)
            | contract::MindReply::SubscriptionDemandAccepted(_)
            | contract::MindReply::AdjudicationReceipt(_)
            | contract::MindReply::ChannelListView(_)
            | contract::MindReply::MindRequestUnimplemented(_) => Err(
                crate::Error::UnexpectedFrame("mind reply has no MindTextReply projection"),
            ),
        }
    }

    pub fn to_nota(&self) -> MindResult<String> {
        Ok(NotaEncode::to_nota(self))
    }
}

impl NotaEncode for MindTextReply {
    fn to_nota(&self) -> String {
        match self {
            Self::OpeningReceipt(receipt) => {
                TextVariantEncoding::new("OpeningReceipt", vec![receipt.event.to_nota()]).to_nota()
            }
            Self::NoteReceipt(receipt) => {
                TextVariantEncoding::new("NoteReceipt", vec![receipt.event.to_nota()]).to_nota()
            }
            Self::LinkReceipt(receipt) => {
                TextVariantEncoding::new("LinkReceipt", vec![receipt.event.to_nota()]).to_nota()
            }
            Self::StatusReceipt(receipt) => {
                TextVariantEncoding::new("StatusReceipt", vec![receipt.event.to_nota()]).to_nota()
            }
            Self::AliasReceipt(receipt) => {
                TextVariantEncoding::new("AliasReceipt", vec![receipt.event.to_nota()]).to_nota()
            }
            Self::View(view) => {
                TextVariantEncoding::new("View", vec![view.items.to_nota(), view.events.to_nota()])
                    .to_nota()
            }
            Self::Rejection(rejection) => {
                TextVariantEncoding::new("Rejection", vec![rejection.reason.to_nota()]).to_nota()
            }
        }
    }
}
