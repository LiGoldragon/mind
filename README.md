# mind

Central typed mind state for Persona agents.

This crate models central mind state: memory/work items, typed thoughts,
relations, notes, dependencies, aliases, subscriptions, and ready-work views.
Ordinary role claims, handoffs, and activity live in `persona-orchestrate`.

It is not Persona's runtime router or harness adapter. The current runtime is
`kameo`-backed and in-process; durable `mind.sema` storage through
`sema-engine` is the storage target. Typed Thought/Relation graph
records now pass through `sema-engine` assertions, match queries, and
subscription registration plus post-commit subscription delta delivery through
the `SubscriptionSupervisor` actor. Older work tables still use the same
underlying storage-kernel handle exposed by `sema-engine` while they await
migration. Lock files are compatibility debris, not durable truth for this
crate.
