# mind skill

Work here when the change concerns Persona's central typed state:
memory/work items, notes, dependencies, aliases, ready-work views, typed
Thought/Relation records, graph subscriptions, channel choreography policy, or
the `mind` CLI.

Rules for work here:

- Never model BEADS as exclusively locked. Any agent may write BEADS while it
  remains the transitional task substrate.
- Keep runtime message delivery in `router`.
- Keep harness lifecycle in `harness`.
- Keep ordinary role claims, handoffs, and activity in `orchestrate`.
- This component owns **its own** mind Sema layer over the `sema` kernel and
  writes one `mind.sema`. The mind state actor sequences writes through that
  database; no shared cross-component DB.
- Typed Thought/Relation graph records use `sema-engine` for Assert/Match,
  operation-log snapshots, subscription registration, and post-commit
  subscription delta delivery. Unmigrated work tables use
  `Engine::storage_kernel()`;
  do not open a second `sema::Sema` handle to the same `mind.sema`.
- Graph subscription deltas must become typed
  `signal-mind::SubscriptionEvent` values through
  `SubscriptionSupervisor`; do not leave delivery as a table-level callback.
- Memory/work mutations append typed events; item state and ready-work lists are
  projections.
- Typed mind graph mutations append immutable `Thought` / `Relation` records;
  corrections are new records plus relations, not in-place edits.
- Thought and relation IDs are typed contract values minted by mind. Do not
  encode type prefixes into ID strings.
- Current graph IDs are compact sequence-derived tokens minted from the
  `sema-engine` snapshot sequence. Do not replace them with content hashes or
  timestamp strings without a new architecture decision.
- The convenience CLI projection may be smaller than the full contract, but the
  CLI must still accept a full `signal-mind::MindRequest` NOTA record.
- Lock files are outside the implementation target. They are temporary
  workspace coordination artifacts and should not be regenerated or projected
  by `mind`.
- Runtime actors use direct `kameo`; do not add a second actor abstraction as a
  prerequisite for mind work.
