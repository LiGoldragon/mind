use signal_mind::{
    ComponentNode, ContractNode, ContractSurface, CrateNode, ReportNode, RepositoryNode,
    SchemaFamilyNode, SourceArtifactNode, StorageResourceNode, SubmitTechnicalNode,
    SubmitTechnicalRelation, TableNode, TechnicalClaimNode, TechnicalNodeBody, TechnicalNodeKey,
    TechnicalNodeKind, TechnicalRelationKind, TechnicalSourceLocator, TextBody, WirePath,
    WitnessNode,
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
        SubmitTechnicalNode {
            stable_key: Self::key(stable_key),
            kind: TechnicalNodeKind::SourceArtifact,
            body: TechnicalNodeBody::SourceArtifact(SourceArtifactNode {
                locator: TechnicalSourceLocator::Path(Self::wire_path(path)),
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
