use signal_mind::{
    ComponentNode, ContractNode, ContractSurface, CrateNode, ReportNode, RepositoryNode,
    SchemaFamilyNode, SourceArtifactNode, StorageResourceNode, SubmitTechnicalNode,
    SubmitTechnicalRelation, TableNode, TaskToken, TechnicalClaimNode, TechnicalNodeBody,
    TechnicalNodeKey, TechnicalNodeKind, TechnicalRelationKind, TechnicalSourceLocator, TextBody,
    WirePath, WitnessNode, WorkItemNode,
};
use signal_persona::ComponentName;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TechnicalSeedDataset {
    nodes: Vec<SubmitTechnicalNode>,
    relations: Vec<SubmitTechnicalRelation>,
}

impl TechnicalSeedDataset {
    pub fn public_first_slice() -> Self {
        Self {
            nodes: Self::public_first_slice_nodes(),
            relations: Self::public_first_slice_relations(),
        }
    }

    pub fn nodes(&self) -> &[SubmitTechnicalNode] {
        &self.nodes
    }

    pub fn relations(&self) -> &[SubmitTechnicalRelation] {
        &self.relations
    }

    pub fn all_node_keys(&self) -> Vec<TechnicalNodeKey> {
        self.nodes
            .iter()
            .map(|node| node.stable_key.clone())
            .collect()
    }

    pub fn all_relation_triples(
        &self,
    ) -> Vec<(TechnicalRelationKind, TechnicalNodeKey, TechnicalNodeKey)> {
        self.relations
            .iter()
            .map(|relation| {
                (
                    relation.kind,
                    relation.source.clone(),
                    relation.target.clone(),
                )
            })
            .collect()
    }

    pub fn mind_component_key(&self) -> TechnicalNodeKey {
        Self::key("component:mind")
    }

    pub fn signal_mind_contract_key(&self) -> TechnicalNodeKey {
        Self::key("contract:signal-mind:ordinary")
    }

    pub fn durable_storage_claim_key(&self) -> TechnicalNodeKey {
        Self::key("storage:mind:mind.sema")
    }

    pub fn primary_irfi_epic_key(&self) -> TechnicalNodeKey {
        Self::key("task:primary-irfi")
    }

    pub fn primary_irfi_slice_claim_key(&self) -> TechnicalNodeKey {
        Self::key("claim:primary-irfi-completed-first-technical-memory-slice")
    }

    pub fn mind_nix_check_witness_key(&self) -> TechnicalNodeKey {
        Self::key("witness:mind-nix-flake-check-2026-06-27")
    }

    pub fn signal_mind_nix_check_witness_key(&self) -> TechnicalNodeKey {
        Self::key("witness:signal-mind-nix-flake-check-2026-06-27")
    }

    fn public_first_slice_nodes() -> Vec<SubmitTechnicalNode> {
        vec![
            Self::component("component:mind", "mind", "Persona central mind daemon"),
            Self::repository(
                "repo:mind",
                "/git/github.com/LiGoldragon/mind",
                "https://github.com/LiGoldragon/mind",
            ),
            Self::repository(
                "repo:signal-mind",
                "/git/github.com/LiGoldragon/signal-mind",
                "https://github.com/LiGoldragon/signal-mind",
            ),
            Self::repository(
                "repo:meta-signal-mind",
                "/git/github.com/LiGoldragon/meta-signal-mind",
                "https://github.com/LiGoldragon/meta-signal-mind",
            ),
            Self::repository(
                "repo:sema-engine",
                "/git/github.com/LiGoldragon/sema-engine",
                "https://github.com/LiGoldragon/sema-engine",
            ),
            Self::contract(
                "contract:signal-mind:ordinary",
                "signal-mind ordinary contract",
                ContractSurface::Ordinary,
            ),
            Self::contract(
                "contract:meta-signal-mind:meta-policy",
                "meta-signal-mind meta policy contract",
                ContractSurface::Meta,
            ),
            Self::crate_node("crate:mind", "mind", "repo:mind"),
            Self::crate_node("crate:signal-mind", "signal-mind", "repo:signal-mind"),
            Self::crate_node("crate:sema-engine", "sema-engine", "repo:sema-engine"),
            Self::storage_resource(
                "storage:mind:mind.sema",
                "component:mind",
                "mind.sema",
                "/home/li/.local/state/mind/mind.sema",
            ),
            Self::schema_family("schema:mind:technical-v2", "component:mind", "technical-v2"),
            Self::table(
                "table:mind:technical_nodes",
                "storage:mind:mind.sema",
                "technical_nodes",
                Some("schema:mind:technical-v2"),
            ),
            Self::table(
                "table:mind:technical_relations",
                "storage:mind:mind.sema",
                "technical_relations",
                Some("schema:mind:technical-v2"),
            ),
            Self::source_artifact(
                "artifact:signal-mind-src-graph.rs",
                "/git/github.com/LiGoldragon/signal-mind/src/graph.rs",
                "signal-mind graph records and subscription event shapes",
            ),
            Self::source_artifact(
                "artifact:signal-mind-src-lib.rs",
                "/git/github.com/LiGoldragon/signal-mind/src/lib.rs",
                "signal-mind channel declaration and contract-local heads",
            ),
            Self::source_artifact(
                "artifact:mind-src-tables.rs",
                "/git/github.com/LiGoldragon/mind/src/tables.rs",
                "mind technical node and relation storage families",
            ),
            Self::source_artifact(
                "artifact:mind-src-actors-dispatch.rs",
                "/git/github.com/LiGoldragon/mind/src/actors/dispatch.rs",
                "mind dispatch routing for technical memory operations",
            ),
            Self::source_artifact(
                "artifact:sema-engine-src-engine.rs",
                "/git/github.com/LiGoldragon/sema-engine/src/engine.rs",
                "sema-engine durable engine implementation",
            ),
            Self::report(
                "report:system-operator-218-signal-mind-contract-modernization-stop-point-2026-06-13",
                "/home/li/primary/reports/system-operator/218-signal-mind-contract-modernization-stop-point-2026-06-13.md",
                "signal-mind contract modernization stop point and schema gap",
            ),
            Self::report(
                "report:system-operator-210-sema-versioned-state-engine-and-mind-implementation-2026-06-11",
                "/home/li/primary/reports/system-operator/210-sema-versioned-state-engine-and-mind-implementation-2026-06-11.md",
                "sema-engine versioned state substrate and mind integration provenance",
            ),
            Self::work_item(
                "task:primary-irfi",
                "primary-irfi",
                "Mind typed technical dependency memory production slice",
            ),
            Self::work_item(
                "task:primary-irfi.1",
                "primary-irfi.1",
                "Add signal-mind technical contract types and round-trip tests",
            ),
            Self::work_item(
                "task:primary-irfi.2",
                "primary-irfi.2",
                "Update mind to consume the new signal-mind contract",
            ),
            Self::work_item(
                "task:primary-irfi.3",
                "primary-irfi.3",
                "Add MindTables schema v9 technical storage families",
            ),
            Self::work_item(
                "task:primary-irfi.4",
                "primary-irfi.4",
                "Implement Mind technical append and query handling",
            ),
            Self::work_item(
                "task:primary-irfi.5",
                "primary-irfi.5",
                "Extend Mind subscriptions and events for technical records",
            ),
            Self::work_item(
                "task:primary-irfi.6",
                "primary-irfi.6",
                "Add public technical seed dataset and end-to-end witnesses",
            ),
            Self::work_item(
                "task:primary-irfi.7",
                "primary-irfi.7",
                "Bump versions and land Nix/check witnesses for Mind technical memory",
            ),
            Self::source_artifact_with_locator(
                "artifact:signal-mind-commit-5a5a4fb4",
                TechnicalSourceLocator::Url(TextBody::new(
                    "https://github.com/LiGoldragon/signal-mind/commit/5a5a4fb43e92da301e018853330eb08288e7b6ac",
                )),
                "signal-mind 0.2.0 technical memory contract commit",
            ),
            Self::source_artifact_with_locator(
                "artifact:mind-commit-c546b29f",
                TechnicalSourceLocator::Url(TextBody::new(
                    "https://github.com/LiGoldragon/mind/commit/c546b29f66b0edf6820217e4610ed872fb00d3f6",
                )),
                "mind consumes signal-mind 0.2.0 technical contract commit",
            ),
            Self::source_artifact_with_locator(
                "artifact:mind-commit-ad12bfc4",
                TechnicalSourceLocator::Url(TextBody::new(
                    "https://github.com/LiGoldragon/mind/commit/ad12bfc487f7142390922a2d2bee234c3f982f6d",
                )),
                "MindTables schema v9 technical storage family commit",
            ),
            Self::source_artifact_with_locator(
                "artifact:mind-commit-4c705095",
                TechnicalSourceLocator::Url(TextBody::new(
                    "https://github.com/LiGoldragon/mind/commit/4c705095e7f1c96ca02478f3053f538c402a5a03",
                )),
                "mind 0.5.0 technical append and query commit",
            ),
            Self::source_artifact_with_locator(
                "artifact:mind-commit-9fd00cf3",
                TechnicalSourceLocator::Url(TextBody::new(
                    "https://github.com/LiGoldragon/mind/commit/9fd00cf385d4317cdf3df582fdd3a0334e36984b",
                )),
                "technical subscription event delivery commit",
            ),
            Self::source_artifact_with_locator(
                "artifact:mind-commit-888ddd51",
                TechnicalSourceLocator::Url(TextBody::new(
                    "https://github.com/LiGoldragon/mind/commit/888ddd518f90ec705df2ffff0d4af92a62e87279",
                )),
                "public technical seed witness commit",
            ),
            Self::source_artifact_with_locator(
                "artifact:beads-primary-irfi-public-epic",
                TechnicalSourceLocator::Task(
                    TaskToken::try_new("primary-irfi".to_string())
                        .expect("primary-irfi is a valid task token"),
                ),
                "public BEADS epic that completed the first Mind technical memory slice",
            ),
            Self::claim(
                "claim:signal-mind-public-wire-vocabulary",
                "signal-mind owns public Mind wire vocabulary",
            ),
            Self::claim(
                "claim:mind-durable-mind-sema-through-sema-engine",
                "mind owns durable mind.sema through sema-engine",
            ),
            Self::claim(
                "claim:first-technical-memory-slice-explicit-technical-nodes-relations",
                "first technical memory slice adds explicit technical nodes and relations, not generic Thought conventions",
            ),
            Self::claim(
                "claim:primary-irfi-completed-first-technical-memory-slice",
                "primary-irfi completed the first public Mind typed technical dependency memory production slice",
            ),
            Self::claim(
                "claim:mind-technical-memory-nix-witnesses-passed-2026-06-27",
                "Mind technical memory Nix witnesses passed on 2026-06-27",
            ),
            Self::witness(
                "witness:persistence",
                "technical memory persistence survives daemon restart",
                "Symbol:mind_typed_technical_seed_survives_daemon_restart_and_queries_back",
            ),
            Self::witness(
                "witness:wire-round-trip",
                "technical seed submissions cross the daemon Signal-frame path",
                "Symbol:mind_typed_technical_seed_survives_daemon_restart_and_queries_back",
            ),
            Self::witness(
                "witness:relation-round-trip",
                "technical relations round trip with committed endpoint identifiers",
                "Symbol:mind_typed_technical_seed_survives_daemon_restart_and_queries_back",
            ),
            Self::witness(
                "witness:contract-local-heads",
                "signal-mind contract-local heads cover technical operations",
                "Symbol:mind_request_variants_declare_contract_local_operation_heads",
            ),
            Self::witness(
                "witness:technical-subscription-delivery",
                "technical node and relation subscriptions deliver post-commit deltas",
                "Symbol:public_technical_seed_delivers_subscription_deltas",
            ),
            Self::witness(
                "witness:mind-nix-flake-check-2026-06-27",
                "mind nix flake check -L passed on 2026-06-27 with technical storage, append/query, subscription, seed, daemon, CLI, fmt, doc, clippy, and actor-truth checks",
                "Nix:mind:nix-flake-check-L:2026-06-27",
            ),
            Self::witness(
                "witness:signal-mind-nix-flake-check-2026-06-27",
                "signal-mind nix flake check -L passed on 2026-06-27 with round-trip, validation, docs, fmt, and clippy checks",
                "Nix:signal-mind:nix-flake-check-L:2026-06-27",
            ),
        ]
    }

    fn public_first_slice_relations() -> Vec<SubmitTechnicalRelation> {
        vec![
            Self::relation(
                TechnicalRelationKind::OwnsRepository,
                "component:mind",
                "repo:mind",
                "mind owns its public implementation repository",
            ),
            Self::relation(
                TechnicalRelationKind::WireDependency,
                "component:mind",
                "contract:signal-mind:ordinary",
                "mind consumes the ordinary signal-mind contract",
            ),
            Self::relation(
                TechnicalRelationKind::WireDependency,
                "component:mind",
                "contract:meta-signal-mind:meta-policy",
                "mind consumes the owner-side meta policy contract",
            ),
            Self::relation(
                TechnicalRelationKind::StorageDependency,
                "component:mind",
                "storage:mind:mind.sema",
                "mind owns durable mind.sema through sema-engine",
            ),
            Self::relation(
                TechnicalRelationKind::BuildDependency,
                "component:mind",
                "crate:sema-engine",
                "mind builds against sema-engine",
            ),
            Self::relation(
                TechnicalRelationKind::RuntimeDependency,
                "component:mind",
                "crate:sema-engine",
                "mind uses sema-engine at runtime",
            ),
            Self::relation(
                TechnicalRelationKind::StorageDependency,
                "storage:mind:mind.sema",
                "schema:mind:technical-v2",
                "mind.sema stores the technical vocabulary v2 schema family",
            ),
            Self::relation(
                TechnicalRelationKind::StorageDependency,
                "schema:mind:technical-v2",
                "table:mind:technical_nodes",
                "technical vocabulary v2 includes the technical_nodes table",
            ),
            Self::relation(
                TechnicalRelationKind::StorageDependency,
                "schema:mind:technical-v2",
                "table:mind:technical_relations",
                "technical vocabulary v2 includes the technical_relations table",
            ),
            Self::relation(
                TechnicalRelationKind::DefinesContract,
                "repo:signal-mind",
                "contract:signal-mind:ordinary",
                "signal-mind defines the ordinary Mind channel",
            ),
            Self::relation(
                TechnicalRelationKind::DefinesContract,
                "repo:meta-signal-mind",
                "contract:meta-signal-mind:meta-policy",
                "meta-signal-mind defines the meta Mind policy channel",
            ),
            Self::relation(
                TechnicalRelationKind::DefinesCrate,
                "repo:sema-engine",
                "crate:sema-engine",
                "sema-engine repository defines the sema-engine crate",
            ),
            Self::relation(
                TechnicalRelationKind::Documents,
                "report:system-operator-218-signal-mind-contract-modernization-stop-point-2026-06-13",
                "contract:signal-mind:ordinary",
                "report 218 documents the signal-mind schema gap and modernization stop point",
            ),
            Self::relation(
                TechnicalRelationKind::Documents,
                "report:system-operator-210-sema-versioned-state-engine-and-mind-implementation-2026-06-11",
                "crate:sema-engine",
                "report 210 documents sema-engine versioned state provenance used by mind",
            ),
            Self::relation(
                TechnicalRelationKind::ClaimsAbout,
                "claim:signal-mind-public-wire-vocabulary",
                "contract:signal-mind:ordinary",
                "the claim is about the ordinary Mind wire vocabulary",
            ),
            Self::relation(
                TechnicalRelationKind::ClaimsAbout,
                "claim:mind-durable-mind-sema-through-sema-engine",
                "crate:sema-engine",
                "the durable mind.sema claim is about the sema-engine crate",
            ),
            Self::relation(
                TechnicalRelationKind::ClaimsAbout,
                "claim:first-technical-memory-slice-explicit-technical-nodes-relations",
                "component:mind",
                "the first technical memory slice is a mind production slice",
            ),
            Self::relation(
                TechnicalRelationKind::ClaimsAbout,
                "claim:primary-irfi-completed-first-technical-memory-slice",
                "task:primary-irfi",
                "the completed-slice claim is about the primary-irfi epic",
            ),
            Self::relation(
                TechnicalRelationKind::ClaimsAbout,
                "claim:mind-technical-memory-nix-witnesses-passed-2026-06-27",
                "task:primary-irfi.7",
                "the Nix witness claim is about final version/check reconciliation",
            ),
            Self::relation(
                TechnicalRelationKind::LocatedAt,
                "contract:signal-mind:ordinary",
                "artifact:signal-mind-src-lib.rs",
                "the ordinary Mind channel is declared in signal-mind/src/lib.rs",
            ),
            Self::relation(
                TechnicalRelationKind::LocatedAt,
                "contract:signal-mind:ordinary",
                "artifact:signal-mind-src-graph.rs",
                "the graph event vocabulary is implemented in signal-mind/src/graph.rs",
            ),
            Self::relation(
                TechnicalRelationKind::LocatedAt,
                "claim:mind-durable-mind-sema-through-sema-engine",
                "artifact:mind-src-tables.rs",
                "mind/src/tables.rs owns the technical Sema families",
            ),
            Self::relation(
                TechnicalRelationKind::LocatedAt,
                "claim:first-technical-memory-slice-explicit-technical-nodes-relations",
                "artifact:mind-src-tables.rs",
                "technical node and relation records are durable table families",
            ),
            Self::relation(
                TechnicalRelationKind::LocatedAt,
                "claim:first-technical-memory-slice-explicit-technical-nodes-relations",
                "artifact:mind-src-actors-dispatch.rs",
                "technical requests route through mind dispatch",
            ),
            Self::relation(
                TechnicalRelationKind::LocatedAt,
                "crate:sema-engine",
                "artifact:sema-engine-src-engine.rs",
                "sema-engine crate behavior is located in sema-engine/src/engine.rs",
            ),
            Self::relation(
                TechnicalRelationKind::LocatedAt,
                "task:primary-irfi",
                "artifact:beads-primary-irfi-public-epic",
                "the primary-irfi epic source is the public BEADS task",
            ),
            Self::relation(
                TechnicalRelationKind::Implements,
                "artifact:signal-mind-src-lib.rs",
                "contract:signal-mind:ordinary",
                "signal-mind/src/lib.rs implements the ordinary Mind contract declaration",
            ),
            Self::relation(
                TechnicalRelationKind::Implements,
                "artifact:mind-src-tables.rs",
                "claim:first-technical-memory-slice-explicit-technical-nodes-relations",
                "mind/src/tables.rs implements the first technical memory table families",
            ),
            Self::relation(
                TechnicalRelationKind::Implements,
                "artifact:mind-src-actors-dispatch.rs",
                "claim:first-technical-memory-slice-explicit-technical-nodes-relations",
                "mind dispatch implements technical memory request routing",
            ),
            Self::relation(
                TechnicalRelationKind::Implements,
                "artifact:sema-engine-src-engine.rs",
                "claim:mind-durable-mind-sema-through-sema-engine",
                "sema-engine implements the durable engine mind uses",
            ),
            Self::relation(
                TechnicalRelationKind::Implements,
                "task:primary-irfi.1",
                "contract:signal-mind:ordinary",
                "primary-irfi.1 implemented the signal-mind technical contract surface",
            ),
            Self::relation(
                TechnicalRelationKind::Implements,
                "task:primary-irfi.2",
                "claim:signal-mind-public-wire-vocabulary",
                "primary-irfi.2 made mind consume the new technical wire vocabulary",
            ),
            Self::relation(
                TechnicalRelationKind::Implements,
                "task:primary-irfi.3",
                "claim:mind-durable-mind-sema-through-sema-engine",
                "primary-irfi.3 added the technical storage families under mind.sema",
            ),
            Self::relation(
                TechnicalRelationKind::Implements,
                "task:primary-irfi.4",
                "claim:first-technical-memory-slice-explicit-technical-nodes-relations",
                "primary-irfi.4 implemented technical append and query handling",
            ),
            Self::relation(
                TechnicalRelationKind::Implements,
                "task:primary-irfi.5",
                "claim:first-technical-memory-slice-explicit-technical-nodes-relations",
                "primary-irfi.5 implemented technical subscription delivery",
            ),
            Self::relation(
                TechnicalRelationKind::Implements,
                "task:primary-irfi.6",
                "claim:first-technical-memory-slice-explicit-technical-nodes-relations",
                "primary-irfi.6 added the public seed dataset and witnesses",
            ),
            Self::relation(
                TechnicalRelationKind::Implements,
                "task:primary-irfi.7",
                "claim:mind-technical-memory-nix-witnesses-passed-2026-06-27",
                "primary-irfi.7 landed final version and Nix/check witness evidence",
            ),
            Self::relation(
                TechnicalRelationKind::TaskDependency,
                "task:primary-irfi",
                "task:primary-irfi.1",
                "the primary-irfi epic completion depended on child task primary-irfi.1",
            ),
            Self::relation(
                TechnicalRelationKind::TaskDependency,
                "task:primary-irfi",
                "task:primary-irfi.2",
                "the primary-irfi epic completion depended on child task primary-irfi.2",
            ),
            Self::relation(
                TechnicalRelationKind::TaskDependency,
                "task:primary-irfi",
                "task:primary-irfi.3",
                "the primary-irfi epic completion depended on child task primary-irfi.3",
            ),
            Self::relation(
                TechnicalRelationKind::TaskDependency,
                "task:primary-irfi",
                "task:primary-irfi.4",
                "the primary-irfi epic completion depended on child task primary-irfi.4",
            ),
            Self::relation(
                TechnicalRelationKind::TaskDependency,
                "task:primary-irfi",
                "task:primary-irfi.5",
                "the primary-irfi epic completion depended on child task primary-irfi.5",
            ),
            Self::relation(
                TechnicalRelationKind::TaskDependency,
                "task:primary-irfi",
                "task:primary-irfi.6",
                "the primary-irfi epic completion depended on child task primary-irfi.6",
            ),
            Self::relation(
                TechnicalRelationKind::TaskDependency,
                "task:primary-irfi",
                "task:primary-irfi.7",
                "the primary-irfi epic completion depended on child task primary-irfi.7",
            ),
            Self::relation(
                TechnicalRelationKind::TaskDependency,
                "task:primary-irfi.1",
                "task:primary-irfi.2",
                "contract work unblocked mind consumption",
            ),
            Self::relation(
                TechnicalRelationKind::TaskDependency,
                "task:primary-irfi.2",
                "task:primary-irfi.3",
                "contract consumption unblocked storage family work",
            ),
            Self::relation(
                TechnicalRelationKind::TaskDependency,
                "task:primary-irfi.3",
                "task:primary-irfi.4",
                "storage families unblocked append and query handling",
            ),
            Self::relation(
                TechnicalRelationKind::TaskDependency,
                "task:primary-irfi.4",
                "task:primary-irfi.5",
                "append handling unblocked technical subscription delivery",
            ),
            Self::relation(
                TechnicalRelationKind::TaskDependency,
                "task:primary-irfi.4",
                "task:primary-irfi.6",
                "append and query handling unblocked seed witnesses",
            ),
            Self::relation(
                TechnicalRelationKind::TaskDependency,
                "task:primary-irfi.5",
                "task:primary-irfi.6",
                "subscription delivery unblocked seed subscription witnesses",
            ),
            Self::relation(
                TechnicalRelationKind::TaskDependency,
                "task:primary-irfi.6",
                "task:primary-irfi.7",
                "seed witnesses unblocked final version/check reconciliation",
            ),
            Self::relation(
                TechnicalRelationKind::Blocks,
                "task:primary-irfi.1",
                "task:primary-irfi.2",
                "primary-irfi.1 blocked primary-irfi.2",
            ),
            Self::relation(
                TechnicalRelationKind::Blocks,
                "task:primary-irfi.6",
                "task:primary-irfi.7",
                "primary-irfi.6 blocked final version/check reconciliation",
            ),
            Self::relation(
                TechnicalRelationKind::ProvenanceDependency,
                "claim:primary-irfi-completed-first-technical-memory-slice",
                "task:primary-irfi",
                "the completed first-slice claim is grounded in the public BEADS epic",
            ),
            Self::relation(
                TechnicalRelationKind::ProvenanceDependency,
                "claim:primary-irfi-completed-first-technical-memory-slice",
                "artifact:beads-primary-irfi-public-epic",
                "the public BEADS epic is the source record for the completed slice",
            ),
            Self::relation(
                TechnicalRelationKind::ProvenanceDependency,
                "task:primary-irfi.1",
                "artifact:signal-mind-commit-5a5a4fb4",
                "primary-irfi.1 shipped in signal-mind commit 5a5a4fb4",
            ),
            Self::relation(
                TechnicalRelationKind::ProvenanceDependency,
                "task:primary-irfi.2",
                "artifact:mind-commit-c546b29f",
                "primary-irfi.2 shipped in mind commit c546b29f",
            ),
            Self::relation(
                TechnicalRelationKind::ProvenanceDependency,
                "task:primary-irfi.3",
                "artifact:mind-commit-ad12bfc4",
                "primary-irfi.3 shipped in mind commit ad12bfc4",
            ),
            Self::relation(
                TechnicalRelationKind::ProvenanceDependency,
                "task:primary-irfi.4",
                "artifact:mind-commit-4c705095",
                "primary-irfi.4 shipped in mind commit 4c705095",
            ),
            Self::relation(
                TechnicalRelationKind::ProvenanceDependency,
                "task:primary-irfi.5",
                "artifact:mind-commit-9fd00cf3",
                "primary-irfi.5 shipped in mind commit 9fd00cf3",
            ),
            Self::relation(
                TechnicalRelationKind::ProvenanceDependency,
                "task:primary-irfi.6",
                "artifact:mind-commit-888ddd51",
                "primary-irfi.6 shipped in mind commit 888ddd51",
            ),
            Self::relation(
                TechnicalRelationKind::ProvenanceDependency,
                "task:primary-irfi.7",
                "witness:mind-nix-flake-check-2026-06-27",
                "primary-irfi.7 recorded the passing mind Nix flake check",
            ),
            Self::relation(
                TechnicalRelationKind::ProvenanceDependency,
                "task:primary-irfi.7",
                "witness:signal-mind-nix-flake-check-2026-06-27",
                "primary-irfi.7 recorded the passing signal-mind Nix flake check",
            ),
            Self::relation(
                TechnicalRelationKind::ProvenBy,
                "claim:primary-irfi-completed-first-technical-memory-slice",
                "witness:mind-nix-flake-check-2026-06-27",
                "mind Nix checks prove the first technical memory slice",
            ),
            Self::relation(
                TechnicalRelationKind::ProvenBy,
                "claim:primary-irfi-completed-first-technical-memory-slice",
                "witness:signal-mind-nix-flake-check-2026-06-27",
                "signal-mind Nix checks prove the contract side of the first technical memory slice",
            ),
            Self::relation(
                TechnicalRelationKind::ProvenBy,
                "claim:mind-technical-memory-nix-witnesses-passed-2026-06-27",
                "witness:mind-nix-flake-check-2026-06-27",
                "the mind Nix witness proves the final mind checks passed",
            ),
            Self::relation(
                TechnicalRelationKind::ProvenBy,
                "claim:mind-technical-memory-nix-witnesses-passed-2026-06-27",
                "witness:signal-mind-nix-flake-check-2026-06-27",
                "the signal-mind Nix witness proves the final contract checks passed",
            ),
            Self::relation(
                TechnicalRelationKind::ProvenBy,
                "claim:mind-durable-mind-sema-through-sema-engine",
                "witness:persistence",
                "persistence witness proves durable technical memory",
            ),
            Self::relation(
                TechnicalRelationKind::ProvenBy,
                "claim:mind-durable-mind-sema-through-sema-engine",
                "witness:wire-round-trip",
                "wire witness proves daemon path submission",
            ),
            Self::relation(
                TechnicalRelationKind::ProvenBy,
                "claim:first-technical-memory-slice-explicit-technical-nodes-relations",
                "witness:relation-round-trip",
                "relation witness proves explicit technical relations round trip",
            ),
            Self::relation(
                TechnicalRelationKind::ProvenBy,
                "claim:signal-mind-public-wire-vocabulary",
                "witness:contract-local-heads",
                "contract-local heads prove public operation vocabulary",
            ),
            Self::relation(
                TechnicalRelationKind::ProvenBy,
                "claim:first-technical-memory-slice-explicit-technical-nodes-relations",
                "witness:technical-subscription-delivery",
                "subscription witness proves technical post-commit deltas",
            ),
        ]
    }

    fn component(stable_key: &str, component: &str, summary: &str) -> SubmitTechnicalNode {
        SubmitTechnicalNode {
            stable_key: Self::key(stable_key),
            kind: TechnicalNodeKind::Component,
            body: TechnicalNodeBody::Component(ComponentNode {
                component: ComponentName::new(component),
                summary: Some(TextBody::new(summary)),
            }),
        }
    }

    fn repository(stable_key: &str, path: &str, remote: &str) -> SubmitTechnicalNode {
        SubmitTechnicalNode {
            stable_key: Self::key(stable_key),
            kind: TechnicalNodeKind::Repository,
            body: TechnicalNodeBody::Repository(RepositoryNode {
                path: Self::wire_path(path),
                remote: Some(TextBody::new(remote)),
            }),
        }
    }

    fn crate_node(stable_key: &str, name: &str, repository: &str) -> SubmitTechnicalNode {
        SubmitTechnicalNode {
            stable_key: Self::key(stable_key),
            kind: TechnicalNodeKind::Crate,
            body: TechnicalNodeBody::Crate(CrateNode {
                name: TextBody::new(name),
                repository: Self::key(repository),
            }),
        }
    }

    fn contract(stable_key: &str, name: &str, surface: ContractSurface) -> SubmitTechnicalNode {
        SubmitTechnicalNode {
            stable_key: Self::key(stable_key),
            kind: TechnicalNodeKind::Contract,
            body: TechnicalNodeBody::Contract(ContractNode {
                name: TextBody::new(name),
                surface,
            }),
        }
    }

    fn work_item(stable_key: &str, task: &str, title: &str) -> SubmitTechnicalNode {
        SubmitTechnicalNode {
            stable_key: Self::key(stable_key),
            kind: TechnicalNodeKind::WorkItem,
            body: TechnicalNodeBody::WorkItem(WorkItemNode {
                task: TaskToken::try_new(task.to_string()).expect("seed task token is valid"),
                title: TextBody::new(title),
            }),
        }
    }

    fn storage_resource(
        stable_key: &str,
        owner: &str,
        name: &str,
        path: &str,
    ) -> SubmitTechnicalNode {
        SubmitTechnicalNode {
            stable_key: Self::key(stable_key),
            kind: TechnicalNodeKind::StorageResource,
            body: TechnicalNodeBody::StorageResource(StorageResourceNode {
                owner: Self::key(owner),
                name: TextBody::new(name),
                path: Some(Self::wire_path(path)),
            }),
        }
    }

    fn schema_family(stable_key: &str, owner: &str, name: &str) -> SubmitTechnicalNode {
        SubmitTechnicalNode {
            stable_key: Self::key(stable_key),
            kind: TechnicalNodeKind::SchemaFamily,
            body: TechnicalNodeBody::SchemaFamily(SchemaFamilyNode {
                owner: Self::key(owner),
                name: TextBody::new(name),
                version: Some(TextBody::new("2")),
            }),
        }
    }

    fn table(
        stable_key: &str,
        storage: &str,
        name: &str,
        schema_family: Option<&str>,
    ) -> SubmitTechnicalNode {
        SubmitTechnicalNode {
            stable_key: Self::key(stable_key),
            kind: TechnicalNodeKind::Table,
            body: TechnicalNodeBody::Table(TableNode {
                storage: Self::key(storage),
                name: TextBody::new(name),
                schema_family: schema_family.map(Self::key),
            }),
        }
    }

    fn source_artifact(stable_key: &str, path: &str, summary: &str) -> SubmitTechnicalNode {
        Self::source_artifact_with_locator(
            stable_key,
            TechnicalSourceLocator::Path(Self::wire_path(path)),
            summary,
        )
    }

    fn source_artifact_with_locator(
        stable_key: &str,
        locator: TechnicalSourceLocator,
        summary: &str,
    ) -> SubmitTechnicalNode {
        SubmitTechnicalNode {
            stable_key: Self::key(stable_key),
            kind: TechnicalNodeKind::SourceArtifact,
            body: TechnicalNodeBody::SourceArtifact(SourceArtifactNode {
                locator,
                summary: Some(TextBody::new(summary)),
            }),
        }
    }

    fn report(stable_key: &str, path: &str, summary: &str) -> SubmitTechnicalNode {
        SubmitTechnicalNode {
            stable_key: Self::key(stable_key),
            kind: TechnicalNodeKind::Report,
            body: TechnicalNodeBody::Report(ReportNode {
                path: Self::wire_path(path),
                summary: Some(TextBody::new(summary)),
            }),
        }
    }

    fn claim(stable_key: &str, claim: &str) -> SubmitTechnicalNode {
        SubmitTechnicalNode {
            stable_key: Self::key(stable_key),
            kind: TechnicalNodeKind::TechnicalClaim,
            body: TechnicalNodeBody::TechnicalClaim(TechnicalClaimNode {
                claim: TextBody::new(claim),
            }),
        }
    }

    fn witness(stable_key: &str, summary: &str, symbol: &str) -> SubmitTechnicalNode {
        SubmitTechnicalNode {
            stable_key: Self::key(stable_key),
            kind: TechnicalNodeKind::Witness,
            body: TechnicalNodeBody::Witness(WitnessNode {
                summary: TextBody::new(summary),
                locator: Some(TechnicalSourceLocator::Symbol(TextBody::new(symbol))),
            }),
        }
    }

    fn relation(
        kind: TechnicalRelationKind,
        source: &str,
        target: &str,
        note: &str,
    ) -> SubmitTechnicalRelation {
        SubmitTechnicalRelation {
            kind,
            source: Self::key(source),
            target: Self::key(target),
            note: Some(TextBody::new(note)),
        }
    }

    fn key(value: &str) -> TechnicalNodeKey {
        TechnicalNodeKey::from_canonical(value).expect("seed technical keys are canonical")
    }

    fn wire_path(value: &str) -> WirePath {
        WirePath::from_absolute_path(value).expect("seed paths are absolute")
    }
}
