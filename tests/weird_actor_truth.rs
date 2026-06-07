use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use mind::actors::{ActorManifest, ActorResidency};
use mind::{
    ActorRef, MindEnvelope, MindRoot, MindRootArguments, MindRootReply, StoreLocation,
    SubmitEnvelope,
};
use signal_mind::{
    ActorName, ItemKind, Magnitude, MindReply, MindRequest, Opening, Query, QueryKind, QueryLimit,
    TextBody, Title,
};

struct SourceTree {
    root: PathBuf,
}

struct SourceFile {
    path: PathBuf,
    text: String,
}

struct RustFileCollector {
    pending_paths: Vec<PathBuf>,
    files: Vec<SourceFile>,
}

struct GuardedFileCollector {
    pending_paths: Vec<PathBuf>,
    files: Vec<SourceFile>,
}

struct ForbiddenFragment {
    text: &'static str,
    reason: &'static str,
}

struct SourceViolation {
    path: PathBuf,
    line_number: usize,
    reason: String,
}

struct ActorRuntimeFixture {
    root: ActorRef<MindRoot>,
    actor: ActorName,
    store: PathBuf,
}

impl SourceTree {
    fn new() -> Self {
        Self {
            root: PathBuf::from(env!("CARGO_MANIFEST_DIR")),
        }
    }

    fn source_files(&self) -> Vec<SourceFile> {
        RustFileCollector::new(self.root.join("src")).into_files()
    }

    fn guarded_files(&self) -> Vec<SourceFile> {
        GuardedFileCollector::new(self.root.clone()).into_files()
    }

    fn file(&self, relative_name: &str) -> SourceFile {
        let path = self.root.join(relative_name);
        let text = fs::read_to_string(&path).expect("named source file is readable");
        SourceFile { path, text }
    }
}

impl RustFileCollector {
    fn new(root: PathBuf) -> Self {
        Self {
            pending_paths: vec![root],
            files: Vec::new(),
        }
    }

    fn into_files(mut self) -> Vec<SourceFile> {
        while let Some(path) = self.pending_paths.pop() {
            self.visit_path(path);
        }
        self.files
    }

    fn visit_path(&mut self, path: PathBuf) {
        if path.is_dir() {
            let mut child_paths = fs::read_dir(&path)
                .expect("source directory is readable")
                .map(|entry| entry.expect("source entry is readable").path())
                .collect::<Vec<_>>();
            child_paths.sort();
            child_paths.reverse();
            self.pending_paths.extend(child_paths);
            return;
        }

        if path.extension().is_some_and(|extension| extension == "rs") {
            let text = fs::read_to_string(&path).expect("source file is readable");
            self.files.push(SourceFile { path, text });
        }
    }
}

impl GuardedFileCollector {
    fn new(root: PathBuf) -> Self {
        Self {
            pending_paths: vec![
                root.join("src"),
                root.join("tests"),
                root.join("Cargo.toml"),
                root.join("Cargo.lock"),
                root.join("flake.nix"),
                root.join("README.md"),
                root.join("ARCHITECTURE.md"),
            ],
            files: Vec::new(),
        }
    }

    fn into_files(mut self) -> Vec<SourceFile> {
        while let Some(path) = self.pending_paths.pop() {
            self.visit_path(path);
        }
        self.files
    }

    fn visit_path(&mut self, path: PathBuf) {
        if path.is_dir() {
            let mut child_paths = fs::read_dir(&path)
                .expect("guarded directory is readable")
                .map(|entry| entry.expect("guarded entry is readable").path())
                .collect::<Vec<_>>();
            child_paths.sort();
            child_paths.reverse();
            self.pending_paths.extend(child_paths);
            return;
        }

        if !path.is_file() {
            return;
        }

        let text = fs::read_to_string(&path).expect("guarded file is readable");
        self.files.push(SourceFile { path, text });
    }
}

impl SourceFile {
    fn relative_name(&self) -> String {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        self.path
            .strip_prefix(root)
            .expect("source file lives under manifest directory")
            .display()
            .to_string()
    }

    fn is_root_actor_source(&self) -> bool {
        self.relative_name() == "src/actors/root.rs"
    }

    fn is_store_kernel_source(&self) -> bool {
        self.relative_name() == "src/actors/store/kernel.rs"
    }

    fn is_tables_source(&self) -> bool {
        self.relative_name() == "src/tables.rs"
    }

    fn is_this_guard_source(&self) -> bool {
        self.relative_name() == "tests/weird_actor_truth.rs"
    }

    fn violations_for(&self, fragment: &ForbiddenFragment) -> Vec<SourceViolation> {
        self.text
            .lines()
            .enumerate()
            .filter(|(_index, line)| line.contains(fragment.text))
            .map(|(index, _line)| SourceViolation {
                path: self.path.clone(),
                line_number: index + 1,
                reason: fragment.reason.to_string(),
            })
            .collect()
    }

    fn public_zst_actor_violations(&self) -> Vec<SourceViolation> {
        self.text
            .lines()
            .enumerate()
            .filter(|(_index, line)| {
                let trimmed = line.trim();
                trimmed.starts_with("pub struct ") && trimmed.ends_with(';')
            })
            .map(|(index, line)| SourceViolation {
                path: self.path.clone(),
                line_number: index + 1,
                reason: format!("public ZST actor marker: {line}"),
            })
            .collect()
    }
}

impl SourceViolation {
    fn summary(&self) -> String {
        format!(
            "{}:{}: {}",
            self.path.display(),
            self.line_number,
            self.reason
        )
    }
}

impl ActorRuntimeFixture {
    async fn new(actor: ActorName) -> Self {
        let store = Self::store_path();
        Self {
            root: MindRoot::start(MindRootArguments::new(StoreLocation::new(
                store.to_string_lossy().to_string(),
            )))
            .await
            .expect("mind root starts"),
            actor,
            store,
        }
    }

    fn store_path() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "mind-weird-actor-{}-{stamp}.sema",
            std::process::id()
        ))
    }

    fn envelope(&self, request: MindRequest) -> MindEnvelope {
        MindEnvelope::new(self.actor.clone(), request)
    }

    async fn submit(&self, request: MindRequest) -> MindRootReply {
        self.root
            .ask(SubmitEnvelope {
                envelope: self.envelope(request),
            })
            .await
            .expect("actor request succeeds")
    }

    async fn stop(self) {
        MindRoot::stop(self.root).await.expect("mind root stops");
        let _ = fs::remove_file(self.store);
    }
}

#[test]
fn raw_actor_spawn_cannot_escape_mind_root() {
    let violations = SourceTree::new()
        .source_files()
        .into_iter()
        .filter(|file| !file.is_root_actor_source())
        .flat_map(|file| {
            file.violations_for(&ForbiddenFragment {
                text: "Actor::spawn",
                reason: "raw Kameo spawn outside MindRoot",
            })
        })
        .collect::<Vec<_>>();

    assert!(
        violations.is_empty(),
        "raw spawn violations:\n{}",
        violations
            .iter()
            .map(SourceViolation::summary)
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn actor_source_cannot_hide_shared_locks_or_polling_waits() {
    let forbidden_fragments = [
        ForbiddenFragment {
            text: "Arc<Mutex",
            reason: "shared mutex state between actors",
        },
        ForbiddenFragment {
            text: "Arc < Mutex",
            reason: "shared mutex state between actors",
        },
        ForbiddenFragment {
            text: "RwLock",
            reason: "shared read-write lock state between actors",
        },
        ForbiddenFragment {
            text: "std::thread::sleep",
            reason: "blocking sleep in actor source",
        },
        ForbiddenFragment {
            text: "tokio::time::sleep",
            reason: "timer sleep in actor source",
        },
        ForbiddenFragment {
            text: "tokio::time::interval",
            reason: "polling interval in actor source",
        },
    ];

    let violations = SourceTree::new()
        .source_files()
        .into_iter()
        .flat_map(|file| {
            forbidden_fragments
                .iter()
                .flat_map(|fragment| file.violations_for(fragment))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    assert!(
        violations.is_empty(),
        "actor source discipline violations:\n{}",
        violations
            .iter()
            .map(SourceViolation::summary)
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn actor_adapter_markers_cannot_be_public_zst_nouns() {
    let violations = SourceTree::new()
        .source_files()
        .into_iter()
        .filter(|file| file.relative_name().starts_with("src/actors/"))
        .flat_map(|file| file.public_zst_actor_violations())
        .collect::<Vec<_>>();

    assert!(
        violations.is_empty(),
        "public actor ZST violations:\n{}",
        violations
            .iter()
            .map(SourceViolation::summary)
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn kameo_is_the_only_actor_library_boundary() {
    let forbidden_fragments = [
        ForbiddenFragment {
            text: "persona-actor",
            reason: "invented actor abstraction name",
        },
        ForbiddenFragment {
            text: "workspace-actor",
            reason: "invented actor abstraction name",
        },
        ForbiddenFragment {
            text: "workspace_actor",
            reason: "invented actor abstraction namespace",
        },
        ForbiddenFragment {
            text: "PersonaActor",
            reason: "invented actor abstraction type",
        },
        ForbiddenFragment {
            text: "WorkspaceActor",
            reason: "invented actor abstraction type",
        },
        ForbiddenFragment {
            text: "actix =",
            reason: "non-kameo actor dependency",
        },
        ForbiddenFragment {
            text: "name = \"actix\"",
            reason: "non-kameo actor dependency",
        },
        ForbiddenFragment {
            text: "xtra =",
            reason: "non-kameo actor dependency",
        },
        ForbiddenFragment {
            text: "name = \"xtra\"",
            reason: "non-kameo actor dependency",
        },
        ForbiddenFragment {
            text: "bastion =",
            reason: "non-kameo actor dependency",
        },
        ForbiddenFragment {
            text: "name = \"bastion\"",
            reason: "non-kameo actor dependency",
        },
        ForbiddenFragment {
            text: "ractor =",
            reason: "non-kameo actor dependency",
        },
        ForbiddenFragment {
            text: "name = \"ractor\"",
            reason: "non-kameo actor dependency",
        },
        ForbiddenFragment {
            text: "use ractor",
            reason: "non-kameo actor dependency",
        },
        ForbiddenFragment {
            text: "ractor::",
            reason: "non-kameo actor dependency",
        },
        ForbiddenFragment {
            text: "coerce =",
            reason: "non-kameo actor dependency",
        },
        ForbiddenFragment {
            text: "name = \"coerce\"",
            reason: "non-kameo actor dependency",
        },
        ForbiddenFragment {
            text: "kompact =",
            reason: "non-kameo actor dependency",
        },
        ForbiddenFragment {
            text: "name = \"kompact\"",
            reason: "non-kameo actor dependency",
        },
        ForbiddenFragment {
            text: "stakker =",
            reason: "non-kameo actor dependency",
        },
        ForbiddenFragment {
            text: "name = \"stakker\"",
            reason: "non-kameo actor dependency",
        },
    ];

    let violations = SourceTree::new()
        .guarded_files()
        .into_iter()
        .filter(|file| !file.is_this_guard_source())
        .flat_map(|file| {
            forbidden_fragments
                .iter()
                .flat_map(|fragment| file.violations_for(fragment))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    assert!(
        violations.is_empty(),
        "non-kameo actor boundary violations:\n{}",
        violations
            .iter()
            .map(SourceViolation::summary)
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn mind_cli_cannot_open_the_mind_database() {
    let main = SourceTree::new().file("src/main.rs");
    let forbidden_fragments = [
        ForbiddenFragment {
            text: "StoreLocation",
            reason: "CLI must not construct a store location",
        },
        ForbiddenFragment {
            text: "mind.sema",
            reason: "CLI must not name the durable database",
        },
        ForbiddenFragment {
            text: "redb",
            reason: "CLI must not open the database kernel",
        },
        ForbiddenFragment {
            text: "MindRoot::start",
            reason: "CLI must not own the root actor runtime",
        },
    ];

    let violations = forbidden_fragments
        .iter()
        .flat_map(|fragment| main.violations_for(fragment))
        .collect::<Vec<_>>();

    assert!(
        violations.is_empty(),
        "mind CLI database ownership violations:\n{}",
        violations
            .iter()
            .map(SourceViolation::summary)
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn mind_source_cannot_depend_on_persona_sema() {
    let forbidden_fragments = [
        ForbiddenFragment {
            text: "persona-sema",
            reason: "mind owns a local Sema layer; no shared persona-sema component",
        },
        ForbiddenFragment {
            text: "persona_sema",
            reason: "mind owns a local Sema layer; no shared persona-sema crate",
        },
    ];

    let violations = SourceTree::new()
        .source_files()
        .into_iter()
        .flat_map(|file| {
            forbidden_fragments
                .iter()
                .flat_map(|fragment| file.violations_for(fragment))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    assert!(
        violations.is_empty(),
        "persona-sema dependency violations:\n{}",
        violations
            .iter()
            .map(SourceViolation::summary)
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn mind_source_cannot_project_lock_files_or_live_beads_backend() {
    let forbidden_fragments = [
        ForbiddenFragment {
            text: "operator.lock",
            reason: "mind replaces lock files instead of projecting them",
        },
        ForbiddenFragment {
            text: "designer.lock",
            reason: "mind replaces lock files instead of projecting them",
        },
        ForbiddenFragment {
            text: ".beads",
            reason: "mind may import BEADS aliases, not use BEADS as a live backend",
        },
        ForbiddenFragment {
            text: "bd ",
            reason: "mind may import BEADS aliases, not shell out to bd",
        },
    ];

    let violations = SourceTree::new()
        .source_files()
        .into_iter()
        .flat_map(|file| {
            forbidden_fragments
                .iter()
                .flat_map(|fragment| file.violations_for(fragment))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    assert!(
        violations.is_empty(),
        "legacy coordination backend violations:\n{}",
        violations
            .iter()
            .map(SourceViolation::summary)
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn dead_config_actor_cannot_return_without_real_mailbox_use() {
    let source_tree = SourceTree::new();
    let source_files = source_tree.source_files();
    let forbidden_fragments = [
        ForbiddenFragment {
            text: "ReadStoreLocation",
            reason: "Config mailbox was dead code; store location flows through root arguments",
        },
        ForbiddenFragment {
            text: "StoreLocationProbe",
            reason: "Config probe was a fake witness for an unused actor",
        },
        ForbiddenFragment {
            text: "TraceNode::CONFIG",
            reason: "dead Config actor must not remain in topology",
        },
        ForbiddenFragment {
            text: "pub(crate) mod config",
            reason: "dead Config actor module must not be re-exported",
        },
    ];

    let has_config_source = source_files
        .iter()
        .any(|file| file.relative_name() == "src/actors/config.rs");
    let violations = forbidden_fragments
        .iter()
        .flat_map(|fragment| {
            source_files
                .iter()
                .flat_map(|file| file.violations_for(fragment))
        })
        .collect::<Vec<_>>();

    assert!(
        !has_config_source && violations.is_empty(),
        "dead Config actor evidence remains:\n{}",
        violations
            .iter()
            .map(SourceViolation::summary)
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn mind_tables_open_stays_inside_the_store_kernel() {
    let violations = SourceTree::new()
        .source_files()
        .into_iter()
        .filter(|file| !file.is_store_kernel_source() && !file.is_tables_source())
        .flat_map(|file| {
            file.violations_for(&ForbiddenFragment {
                text: "MindTables::open",
                reason: "mind.sema must be opened only by the store kernel",
            })
        })
        .collect::<Vec<_>>();

    assert!(
        violations.is_empty(),
        "mind table open boundary violations:\n{}",
        violations
            .iter()
            .map(SourceViolation::summary)
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn typed_graph_records_cannot_bypass_sema_engine() {
    let tables = SourceTree::new().file("src/tables.rs");
    let forbidden_fragments = [
        ForbiddenFragment {
            text: "Table<&'static str, Thought>",
            reason: "typed graph records must be registered in sema-engine",
        },
        ForbiddenFragment {
            text: "Table<&'static str, Relation>",
            reason: "typed graph records must be registered in sema-engine",
        },
        ForbiddenFragment {
            text: "THOUGHTS.insert",
            reason: "thought assertions must pass through sema-engine operation logging",
        },
        ForbiddenFragment {
            text: "RELATIONS.insert",
            reason: "relation assertions must pass through sema-engine operation logging",
        },
    ];

    let violations = forbidden_fragments
        .iter()
        .flat_map(|fragment| tables.violations_for(fragment))
        .collect::<Vec<_>>();

    assert!(
        violations.is_empty(),
        "typed graph sema-engine bypass violations:\n{}",
        violations
            .iter()
            .map(SourceViolation::summary)
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn graph_subscriptions_cannot_bypass_sema_engine_subscribe() {
    let tables = SourceTree::new().file("src/tables.rs");
    let forbidden_fragments = [
        ForbiddenFragment {
            text: "SUBSCRIPTION_NEXT_SLOT",
            reason: "graph subscription identity must come from sema-engine",
        },
        ForbiddenFragment {
            text: "next_subscription_slot",
            reason: "graph subscription identity must come from sema-engine",
        },
        ForbiddenFragment {
            text: "GraphSlot",
            reason: "graph subscription identity must come from sema-engine",
        },
        ForbiddenFragment {
            text: "thought_subscription_count",
            reason: "subscription registration must not scan local tables to mint ids",
        },
        ForbiddenFragment {
            text: "relation_subscription_count",
            reason: "subscription registration must not scan local tables to mint ids",
        },
    ];

    let violations = forbidden_fragments
        .iter()
        .flat_map(|fragment| tables.violations_for(fragment))
        .collect::<Vec<_>>();

    assert!(
        violations.is_empty(),
        "graph subscription sema-engine bypass violations:\n{}",
        violations
            .iter()
            .map(SourceViolation::summary)
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert_eq!(
        tables.text.matches(".engine.subscribe(").count(),
        2,
        "thought and relation subscriptions must both register through sema-engine"
    );
}

#[test]
fn graph_subscription_deltas_cannot_stop_at_table_sink() {
    let tables = SourceTree::new().file("src/tables.rs");
    let subscription_actor = SourceTree::new().file("src/actors/subscription.rs");

    assert!(
        tables.text.contains("SubscriptionDeliveryMode::Inline"),
        "mind subscription sinks must use inline sema-engine delivery so actor tests can observe post-commit deltas without polling"
    );
    assert!(
        tables.text.contains("PublishThoughtDelta") && tables.text.contains("PublishRelationDelta"),
        "sema-engine deltas must be translated into typed mind subscription actor messages"
    );
    assert!(
        subscription_actor
            .text
            .contains("MindDelta::ThoughtCommitted")
            && subscription_actor
                .text
                .contains("MindDelta::RelationCommitted"),
        "subscription actor must publish typed signal-mind delta variants, not raw table notifications"
    );
}

#[test]
fn mind_lockfile_cannot_resolve_duplicate_storage_or_retired_signal_core() {
    let manifest = SourceTree::new().file("Cargo.toml");
    let lock = SourceTree::new().file("Cargo.lock");

    assert!(
        !manifest.text.contains("\nsema "),
        "Cargo.toml must not depend on the storage kernel directly; use sema-engine"
    );
    assert_eq!(
        lock.text.matches("name = \"sema\"\n").count(),
        1,
        "Cargo.lock must contain one transitive sema storage-kernel package"
    );
    assert_eq!(
        lock.text.matches("name = \"signal-core\"\n").count(),
        0,
        "Cargo.lock must not contain the retired signal-core package"
    );
    assert!(
        !lock.text.contains("signal-core"),
        "signal-core must stay absent after the signal-frame migration"
    );
}

#[test]
fn memory_state_cannot_hide_mutation_behind_refcell() {
    let memory = SourceTree::new().file("src/memory.rs");
    let forbidden_fragments = [
        ForbiddenFragment {
            text: "RefCell",
            reason: "memory reducer is actor-owned mutable state, not interior mutability",
        },
        ForbiddenFragment {
            text: "borrow_mut",
            reason: "memory reducer is actor-owned mutable state, not interior mutability",
        },
    ];

    let violations = forbidden_fragments
        .iter()
        .flat_map(|fragment| memory.violations_for(fragment))
        .collect::<Vec<_>>();

    assert!(
        violations.is_empty(),
        "memory interior mutability violations:\n{}",
        violations
            .iter()
            .map(SourceViolation::summary)
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn trace_phase_actor_cannot_float_without_parent_edge() {
    let manifest = ActorManifest::mind_phase_one();
    let missing_parents = manifest
        .actors()
        .iter()
        .filter(|actor| actor.residency() != ActorResidency::Root)
        .filter(|actor| {
            !manifest
                .edges()
                .iter()
                .any(|edge| edge.child() == actor.kind())
        })
        .map(|actor| actor.kind().label())
        .collect::<Vec<_>>();

    assert!(
        missing_parents.is_empty(),
        "manifest actors without parent edges: {}",
        missing_parents.join(", ")
    );
}

#[test]
fn trace_node_labels_cannot_collide() {
    let manifest = ActorManifest::mind_phase_one();
    let mut labels = HashSet::new();
    let duplicate_labels = manifest
        .actors()
        .iter()
        .map(|actor| actor.kind().label())
        .filter(|label| !labels.insert(*label))
        .collect::<Vec<_>>();

    assert!(
        duplicate_labels.is_empty(),
        "duplicate actor labels: {}",
        duplicate_labels.join(", ")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn parallel_runtimes_cannot_share_registry_names_or_memory() {
    let first_runtime = ActorRuntimeFixture::new(ActorName::new("operator")).await;
    let second_runtime = ActorRuntimeFixture::new(ActorName::new("designer")).await;

    let first_reply = first_runtime
        .submit(MindRequest::Opening(Opening {
            kind: ItemKind::Task,
            priority: Magnitude::High,
            title: Title::new("First runtime item"),
            body: TextBody::new("only the first runtime sees this"),
        }))
        .await;
    let second_reply = second_runtime
        .submit(MindRequest::Opening(Opening {
            kind: ItemKind::Task,
            priority: Magnitude::Low,
            title: Title::new("Second runtime item"),
            body: TextBody::new("only the second runtime sees this"),
        }))
        .await;

    let MindReply::OpeningReceipt(first_receipt) = first_reply.reply().expect("first reply exists")
    else {
        panic!("expected first opened reply");
    };
    let MindReply::OpeningReceipt(second_receipt) =
        second_reply.reply().expect("second reply exists")
    else {
        panic!("expected second opened reply");
    };

    assert_eq!(first_receipt.event.header.actor, ActorName::new("operator"));
    assert_eq!(
        second_receipt.event.header.actor,
        ActorName::new("designer")
    );

    let first_view = first_runtime
        .submit(MindRequest::Query(Query {
            kind: QueryKind::Open,
            limit: QueryLimit::new(10),
        }))
        .await;
    let second_view = second_runtime
        .submit(MindRequest::Query(Query {
            kind: QueryKind::Open,
            limit: QueryLimit::new(10),
        }))
        .await;

    let MindReply::View(first_items) = first_view.reply().expect("first view exists") else {
        panic!("expected first view");
    };
    let MindReply::View(second_items) = second_view.reply().expect("second view exists") else {
        panic!("expected second view");
    };

    assert_eq!(first_items.items.len(), 1);
    assert_eq!(second_items.items.len(), 1);
    assert_eq!(first_items.items[0].title, Title::new("First runtime item"));
    assert_eq!(
        second_items.items[0].title,
        Title::new("Second runtime item")
    );

    first_runtime.stop().await;
    second_runtime.stop().await;
}
