{
  description = "Typed mind state for Persona agents.";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      fenix,
      crane,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs { inherit system; };
        toolchain = fenix.packages.${system}.stable.withComponents [
          "cargo"
          "rustc"
          "rustfmt"
          "clippy"
          "rust-analyzer"
          "rust-src"
        ];
        craneLib = (crane.mkLib pkgs).overrideToolchain toolchain;
        src = pkgs.lib.cleanSourceWith {
          src = ./.;
          filter =
            path: type:
            let
              pathString = toString path;
              schemaRoot = "${toString ./.}/schema";
            in
            type == "directory"
            || craneLib.filterCargoSources path type
            || pathString == schemaRoot
            || pkgs.lib.hasPrefix "${schemaRoot}/" pathString;
          name = "source";
        };
        commonArgs = {
          inherit src;
          strictDeps = true;
        };
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;
        mindConstraintCheck =
          name: script:
          pkgs.runCommand name { } ''
            set -euo pipefail

            export MIND_BIN=${self.packages.${system}.default}/bin/mind
            export MIND_META_BIN=${self.packages.${system}.default}/bin/meta-mind
            export MIND_DAEMON_BIN=${self.packages.${system}.default}/bin/mind-daemon
            export MIND_CONFIGURATION_WRITER_BIN=${self.packages.${system}.default}/bin/mind-write-configuration
            ${pkgs.bash}/bin/bash ${script}

            touch "$out"
          '';
      in
      {
        packages.default = craneLib.buildPackage (
          commonArgs
          // {
            inherit cargoArtifacts;
            meta.mainProgram = "mind";
          }
        );
        checks = {
          default = craneLib.cargoTest (commonArgs // { inherit cargoArtifacts; });
          build = craneLib.cargoBuild (commonArgs // { inherit cargoArtifacts; });
          test = craneLib.cargoTest (commonArgs // { inherit cargoArtifacts; });
          weird-actor-truth = craneLib.cargoTest (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoTestExtraArgs = "--test weird_actor_truth";
            }
          );
          mind-dead-config-actor-cannot-return-without-real-mailbox-use = craneLib.cargoTest (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoTestExtraArgs = "--test weird_actor_truth dead_config_actor_cannot_return_without_real_mailbox_use";
            }
          );
          daemon-wire = craneLib.cargoTest (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoTestExtraArgs = "--test daemon_wire";
            }
          );
          mind-daemon-applies-spawn-envelope-socket-mode = craneLib.cargoTest (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoTestExtraArgs = "--test daemon_wire constraint_mind_daemon_applies_spawn_envelope_socket_mode -- --exact";
            }
          );
          mind-daemon-answers-component-supervision-relation = craneLib.cargoTest (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoTestExtraArgs = "--test daemon_wire mind_daemon_answers_component_supervision_relation -- --exact";
            }
          );
          mind-typed-graph-uses-graph-actor-lane = craneLib.cargoTest (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoTestExtraArgs = "--test actor_topology typed_thought_runs_through_graph_actor_lane_and_store_mints_id";
            }
          );
          mind-store-kernel-supervised-thread-restart-reopens-same-database = craneLib.cargoTest (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoTestExtraArgs = "--test actor_topology store_kernel_supervised_thread_restart_reopens_same_database -- --exact";
            }
          );
          mind-typed-thought-append-uses-sema-engine-operation-log = craneLib.cargoTest (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoTestExtraArgs = "typed_thought_append_uses_sema_engine_operation_log";
            }
          );
          mind-graph-id-policy-mints-compact-typed-sequence-ids-without-prefixes = craneLib.cargoTest (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoTestExtraArgs = "graph_id_policy_mints_compact_typed_sequence_ids_without_prefixes";
            }
          );
          mind-graph-id-policy-continues-after-reopen-without-collision = craneLib.cargoTest (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoTestExtraArgs = "graph_id_policy_continues_after_reopen_without_collision";
            }
          );
          mind-typed-graph-records-cannot-bypass-sema-engine = craneLib.cargoTest (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoTestExtraArgs = "--test weird_actor_truth typed_graph_records_cannot_bypass_sema_engine";
            }
          );
          mind-lockfile-cannot-resolve-two-sema-kernels = craneLib.cargoTest (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoTestExtraArgs = "--test weird_actor_truth mind_lockfile_cannot_resolve_two_sema_kernels";
            }
          );
          mind-typed-thought-graph-survives-process-restart = craneLib.cargoTest (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoTestExtraArgs = "--test daemon_wire mind_typed_thought_graph_survives_process_restart";
            }
          );
          mind-superseded-thought-excluded-from-current-query = craneLib.cargoTest (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoTestExtraArgs = "--test actor_topology superseded_thought_excluded_from_current_query";
            }
          );
          mind-supersedes-rejects-different-thought-kinds = craneLib.cargoTest (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoTestExtraArgs = "--test actor_topology supersedes_relation_rejects_different_thought_kinds";
            }
          );
          mind-relation-kind-rejects-wrong-domain = craneLib.cargoTest (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoTestExtraArgs = "--test actor_topology relation_kind_rejects_wrong_domain";
            }
          );
          mind-authored-rejects-non-identity-reference-source = craneLib.cargoTest (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoTestExtraArgs = "--test actor_topology authored_relation_rejects_non_identity_reference_source";
            }
          );
          mind-typed-thought-subscription-registers-and-returns-initial-snapshot = craneLib.cargoTest (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoTestExtraArgs = "--test actor_topology typed_thought_subscription_registers_and_returns_initial_snapshot";
            }
          );
          mind-typed-relation-subscription-registers-and-returns-initial-snapshot = craneLib.cargoTest (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoTestExtraArgs = "--test actor_topology typed_relation_subscription_registers_and_returns_initial_snapshot";
            }
          );
          mind-typed-thought-subscription-delivers-live-delta-through-subscription-actor = craneLib.cargoTest (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoTestExtraArgs = "--test actor_topology typed_thought_subscription_delivers_live_delta_through_subscription_actor";
            }
          );
          mind-typed-relation-subscription-delivers-live-delta-through-subscription-actor = craneLib.cargoTest (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoTestExtraArgs = "--test actor_topology typed_relation_subscription_delivers_live_delta_through_subscription_actor";
            }
          );
          mind-graph-subscription-deltas-cannot-stop-at-table-sink = craneLib.cargoTest (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoTestExtraArgs = "--test weird_actor_truth graph_subscription_deltas_cannot_stop_at_table_sink";
            }
          );
          mind-thought-subscription-is-durable-table-data = craneLib.cargoTest (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoTestExtraArgs = "thought_subscription_is_durable_table_data";
            }
          );
          cli = craneLib.cargoTest (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoTestExtraArgs = "--test cli";
            }
          );
          mind-cli-accepts-full-signal-mind-request-for-typed-graph = craneLib.cargoTest (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoTestExtraArgs = "--test cli mind_cli_accepts_full_signal_mind_request_for_typed_graph";
            }
          );
          cli-binary = pkgs.runCommand "mind-cli-binary" { } ''
            set -euo pipefail

            workspace="$(mktemp -d)"
            socket="$workspace/mind.sock"
            meta_socket="$workspace/mind.meta.sock"
            store="$workspace/mind.redb"
            configuration="$workspace/mind.rkyv"

            ${self.packages.${system}.default}/bin/mind-write-configuration \
              "(ConfigurationWriteRequest $socket $meta_socket $store $configuration)"

            ${self.packages.${system}.default}/bin/mind-daemon "$configuration" &
            daemon="$!"
            trap 'kill "$daemon" 2>/dev/null || true' EXIT

            for attempt in $(seq 1 100); do
              if [ -S "$socket" ]; then
                break
              fi
              sleep 0.05
            done
            test -S "$socket"

            MIND_SOCKET="$socket" \
            MIND_ACTOR=operator \
            ${self.packages.${system}.default}/bin/mind \
              '(Opening Task High [Binary check work] [opened by the binary check])' \
              > "$workspace/opening.out"
            grep -F '(OpeningReceipt' "$workspace/opening.out"

            MIND_SOCKET="$socket" \
            MIND_ACTOR=operator \
            ${self.packages.${system}.default}/bin/mind \
              '(Query (Open) 10)' \
              > "$workspace/query.out"
            grep -F '[Binary check work]' "$workspace/query.out"

            touch "$out"
          '';
          mind-cli-accepts-one-nota-record-and-prints-one-nota-reply = mindConstraintCheck "mind-cli-accepts-one-nota-record-and-prints-one-nota-reply" ./scripts/mind-cli-accepts-one-nota-record-and-prints-one-nota-reply;
          mind-cli-sends-signal-frames-to-long-lived-daemon = mindConstraintCheck "mind-cli-sends-signal-frames-to-long-lived-daemon" ./scripts/mind-cli-sends-signal-frames-to-long-lived-daemon;
          mind-cli-opens-and-queries-work-item-through-daemon = mindConstraintCheck "mind-cli-opens-and-queries-work-item-through-daemon" ./scripts/mind-cli-opens-and-queries-work-item-through-daemon;
          mind-store-survives-process-restart = mindConstraintCheck "mind-store-survives-process-restart" ./scripts/mind-store-survives-process-restart;
          mind-meta-cli-reaches-owner-policy-socket = mindConstraintCheck "mind-meta-cli-reaches-owner-policy-socket" ./scripts/mind-meta-cli-reaches-owner-policy-socket;
          test-doc = craneLib.cargoTest (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoTestExtraArgs = "--doc";
            }
          );
          doc = craneLib.cargoDoc (
            commonArgs
            // {
              inherit cargoArtifacts;
              RUSTDOCFLAGS = "-D warnings";
            }
          );
          fmt = craneLib.cargoFmt { inherit src; };
          clippy = craneLib.cargoClippy (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoClippyExtraArgs = "--all-targets -- -D warnings";
            }
          );
        };
        apps.default = {
          type = "app";
          program = "${self.packages.${system}.default}/bin/mind";
        };
        devShells.default = pkgs.mkShell {
          name = "mind";
          packages = [
            pkgs.jujutsu
            pkgs.pkg-config
            toolchain
          ];
        };
        formatter = pkgs.nixfmt;
      }
    );
}
