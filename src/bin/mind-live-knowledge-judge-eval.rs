//! Compile-only live-eval placeholder.
//!
//! The old in-Mind live eval runner depended on the removed AgentKnowledgeJudge
//! path. Provider-backed accepted-knowledge evals now belong at the `mind-judge`
//! text/model edge. This binary remains feature-gated so `--features live-eval`
//! has a truthful compile target without performing provider calls.

fn main() {
    eprintln!(
        "mind-live-knowledge-judge-eval is retired; run live accepted-knowledge evals through mind-judge"
    );
}
