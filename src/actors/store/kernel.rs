use kameo::actor::{Actor, ActorRef};
use kameo::message::{Context, Message};
use signal_mind::MindReply;

use crate::graph::MindGraphLedger;
use crate::tables::GraphSubscriptionPublisher;
use crate::{MemoryGraph, MindEnvelope, MindTables, StoreLocation};

use super::GraphRecords;
use super::persistence::PersistenceRejection;

pub(super) struct StoreKernel {
    tables: Option<MindTables>,
}

#[derive(Clone)]
pub(super) struct Arguments {
    pub(super) store: StoreLocation,
    pub(super) subscription: ActorRef<crate::actors::subscription::SubscriptionSupervisor>,
}

pub(super) struct CommitMemoryGraph {
    graph: MemoryGraph,
}

pub(super) struct LoadMemoryGraph;

pub(super) struct WriteThought {
    envelope: MindEnvelope,
}

pub(super) struct WriteRelation {
    envelope: MindEnvelope,
}

pub(super) struct ReadThoughts {
    envelope: MindEnvelope,
}

pub(super) struct ReadRelations {
    envelope: MindEnvelope,
}

pub(super) struct SubscribeThoughts {
    envelope: MindEnvelope,
}

pub(super) struct SubscribeRelations {
    envelope: MindEnvelope,
}

pub(super) struct ReadGraphRecords;

pub(super) struct ShutdownKernel;

#[derive(kameo::Reply)]
pub(super) struct KernelReply {
    reply: Option<MindReply>,
}

impl KernelReply {
    fn new(reply: Option<MindReply>) -> Self {
        Self { reply }
    }

    pub(super) fn into_reply(self) -> Option<MindReply> {
        self.reply
    }
}

#[derive(kameo::Reply)]
pub(super) struct KernelCommit {
    rejection: Option<MindReply>,
}

impl KernelCommit {
    fn accepted() -> Self {
        Self { rejection: None }
    }

    fn rejected(error: crate::Error) -> Self {
        Self {
            rejection: Some(PersistenceRejection::reply(error)),
        }
    }

    pub(super) fn into_rejection(self) -> Option<MindReply> {
        self.rejection
    }
}

impl CommitMemoryGraph {
    pub(super) fn new(graph: MemoryGraph) -> Self {
        Self { graph }
    }
}

impl WriteThought {
    pub(super) fn new(envelope: MindEnvelope) -> Self {
        Self { envelope }
    }
}

impl WriteRelation {
    pub(super) fn new(envelope: MindEnvelope) -> Self {
        Self { envelope }
    }
}

impl ReadThoughts {
    pub(super) fn new(envelope: MindEnvelope) -> Self {
        Self { envelope }
    }
}

impl ReadRelations {
    pub(super) fn new(envelope: MindEnvelope) -> Self {
        Self { envelope }
    }
}

impl SubscribeThoughts {
    pub(super) fn new(envelope: MindEnvelope) -> Self {
        Self { envelope }
    }
}

impl SubscribeRelations {
    pub(super) fn new(envelope: MindEnvelope) -> Self {
        Self { envelope }
    }
}

impl StoreKernel {
    fn open(arguments: Arguments) -> crate::Result<Self> {
        Ok(Self {
            tables: Some(MindTables::open(
                &arguments.store,
                GraphSubscriptionPublisher::actor(arguments.subscription),
            )?),
        })
    }

    fn tables(&self) -> crate::Result<&MindTables> {
        self.tables
            .as_ref()
            .ok_or_else(|| crate::Error::ActorCall("store kernel is closed".to_string()))
    }

    fn shutdown(&mut self) {
        let _closed = self.tables.take();
    }

    fn commit_memory_graph(&self, graph: MemoryGraph) -> KernelCommit {
        self.tables()
            .and_then(|tables| tables.replace_memory_graph(&graph))
            .map(|()| KernelCommit::accepted())
            .unwrap_or_else(KernelCommit::rejected)
    }

    fn load_memory_graph(&self) -> Option<MemoryGraph> {
        self.tables()
            .ok()
            .and_then(|tables| tables.memory_graph().ok().flatten())
    }

    fn write_thought(&self, envelope: MindEnvelope) -> KernelReply {
        let reply = self
            .tables()
            .and_then(|tables| MindGraphLedger::new(tables).submit_thought(envelope))
            .map(Some)
            .unwrap_or_else(|error| Some(PersistenceRejection::reply(error)));

        KernelReply::new(reply)
    }

    fn write_relation(&self, envelope: MindEnvelope) -> KernelReply {
        let reply = self
            .tables()
            .and_then(|tables| MindGraphLedger::new(tables).submit_relation(envelope))
            .map(Some)
            .unwrap_or_else(|error| Some(PersistenceRejection::reply(error)));

        KernelReply::new(reply)
    }

    fn read_thoughts(&self, envelope: MindEnvelope) -> KernelReply {
        let reply = self
            .tables()
            .and_then(|tables| MindGraphLedger::new(tables).query_thoughts(envelope))
            .map(Some)
            .unwrap_or_else(|error| Some(PersistenceRejection::reply(error)));

        KernelReply::new(reply)
    }

    fn read_relations(&self, envelope: MindEnvelope) -> KernelReply {
        let reply = self
            .tables()
            .and_then(|tables| MindGraphLedger::new(tables).query_relations(envelope))
            .map(Some)
            .unwrap_or_else(|error| Some(PersistenceRejection::reply(error)));

        KernelReply::new(reply)
    }

    fn subscribe_thoughts(&self, envelope: MindEnvelope) -> KernelReply {
        let reply = self
            .tables()
            .and_then(|tables| MindGraphLedger::new(tables).subscribe_thoughts(envelope))
            .map(Some)
            .unwrap_or_else(|error| Some(PersistenceRejection::reply(error)));

        KernelReply::new(reply)
    }

    fn subscribe_relations(&self, envelope: MindEnvelope) -> KernelReply {
        let reply = self
            .tables()
            .and_then(|tables| MindGraphLedger::new(tables).subscribe_relations(envelope))
            .map(Some)
            .unwrap_or_else(|error| Some(PersistenceRejection::reply(error)));

        KernelReply::new(reply)
    }

    fn read_graph_records(&self) -> GraphRecords {
        GraphRecords {
            relations: self
                .tables()
                .and_then(MindTables::relation_records)
                .unwrap_or_default(),
        }
    }
}

impl Actor for StoreKernel {
    type Args = Arguments;
    type Error = crate::Error;

    async fn on_start(
        arguments: Self::Args,
        _actor_reference: ActorRef<Self>,
    ) -> Result<Self, Self::Error> {
        Self::open(arguments)
    }
}

impl Message<CommitMemoryGraph> for StoreKernel {
    type Reply = KernelCommit;

    async fn handle(
        &mut self,
        message: CommitMemoryGraph,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.commit_memory_graph(message.graph)
    }
}

impl Message<LoadMemoryGraph> for StoreKernel {
    type Reply = Option<MemoryGraph>;

    async fn handle(
        &mut self,
        _message: LoadMemoryGraph,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.load_memory_graph()
    }
}

impl Message<WriteThought> for StoreKernel {
    type Reply = KernelReply;

    async fn handle(
        &mut self,
        message: WriteThought,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.write_thought(message.envelope)
    }
}

impl Message<WriteRelation> for StoreKernel {
    type Reply = KernelReply;

    async fn handle(
        &mut self,
        message: WriteRelation,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.write_relation(message.envelope)
    }
}

impl Message<ReadThoughts> for StoreKernel {
    type Reply = KernelReply;

    async fn handle(
        &mut self,
        message: ReadThoughts,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.read_thoughts(message.envelope)
    }
}

impl Message<ReadRelations> for StoreKernel {
    type Reply = KernelReply;

    async fn handle(
        &mut self,
        message: ReadRelations,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.read_relations(message.envelope)
    }
}

impl Message<SubscribeThoughts> for StoreKernel {
    type Reply = KernelReply;

    async fn handle(
        &mut self,
        message: SubscribeThoughts,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.subscribe_thoughts(message.envelope)
    }
}

impl Message<SubscribeRelations> for StoreKernel {
    type Reply = KernelReply;

    async fn handle(
        &mut self,
        message: SubscribeRelations,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.subscribe_relations(message.envelope)
    }
}

impl Message<ReadGraphRecords> for StoreKernel {
    type Reply = GraphRecords;

    async fn handle(
        &mut self,
        _message: ReadGraphRecords,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.read_graph_records()
    }
}

impl Message<ShutdownKernel> for StoreKernel {
    type Reply = bool;

    async fn handle(
        &mut self,
        _message: ShutdownKernel,
        _context: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        self.shutdown();
        true
    }
}
