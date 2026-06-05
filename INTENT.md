# INTENT — mind

`mind` owns Persona's central workspace state: work items, typed Thought and Relation
records, notes, dependencies, decisions, aliases, event history, subscriptions, channel
choreography policy, and ready/blocked views. Lock files are transitional; `mind`
replaces them.

The authority principle is load-bearing: `mind` is the authority root of the Persona
control plane. It *receives inbound* observations (Assert, Match, Subscribe) from peers
and *issues outbound* orders (Mutate, Retract) to downstream components. Authority
direction is "observe up-tree, order down-tree": mind subscribes to router/harness/
orchestrate events and decides; then issues Mutate orders (ChannelGrant, ChannelExtend,
ChannelRetract, AdjudicationDeny) that peer components obey and confirm.

The CLI is a thin client boundary. The daemon owns `MindRoot` for its process lifetime.
Requests enter through `MindEnvelope` (caller identity + typed `MindRequest`). The
database is workspace-local `mind.redb` opened only by `StoreKernel`. Typed Thought/
Relation graph records write through `sema-engine` Assert on registered table families.
Graph IDs are compact sequence-derived tokens minted from engine snapshot; they are
not content hashes, timestamps, or embedded type prefixes. Queries are read-only;
writes append typed events. Work/memory mutations replace the typed memory_graph
snapshot in `mind.redb` before success replies. Typed graph subscriptions register
through `sema-engine` Subscribe and persist durable Persona-specific filters.

Key constraints: the CLI accepts exactly one NOTA request and prints exactly one
reply. All public operations enter as one MindEnvelope. Caller identity, time, event
sequence, operation IDs, and display IDs are minted by infrastructure/store actors,
not by request payloads. State-bearing phases are actors or reducers owned by actors—
no shared Arc<Mutex<T>>. Typed Thought and Relation records are immutable; correction
is a new record plus a relation like Supersedes. Durable truth is mind.redb; lock
files are outside this implementation, and BEADS is import/history only.
