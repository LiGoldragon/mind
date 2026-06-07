# Persona Mind — Agent Instructions

Read `/home/li/primary/AGENTS.md` first, then `/home/li/primary/lore/AGENTS.md`.
This repository follows the primary workspace orchestration protocol.

## Purpose

`mind` is Persona's central state machine. It models memory/work
items, typed thoughts, relations, notes, dependencies, aliases, subscriptions,
and ready-work views without deepening the transitional BEADS dependency.
Ordinary role claims, handoffs, and activity belong to `orchestrate`.

## Local Rules

- Use Jujutsu for version control.
- Keep repositories public unless the human gives a specific reason otherwise.
- Use Nix for build and test entry points.
- BEADS is shared coordination state and is never claimed or exclusively locked.
- No polling. Mind status is pushed through explicit writes and future
  Persona messages.
- Durable mind state uses component-facing `.sema` files through
  `sema-engine` / `sema` with rkyv-typed table values.
