use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use mind::{
    MindCommand, MindCommandEnvironment, MindDaemon, MindDaemonConfiguration, MindDaemonEndpoint,
    MindJudgeRequestResponseLog, MindKnowledgeJudgeAgentConfiguration,
    MindKnowledgeJudgeConfiguration, MindKnowledgeJudgeTrainingSource, StoreLocation,
};
use nota_next::NotaEncode;
use signal_mind::{
    GoalBody, GoalScope, MindRequest, SubmitThought, TextBody, ThoughtBody, ThoughtKind,
    WorkspaceGoal,
};

struct CliFixture {
    endpoint: MindDaemonEndpoint,
    store: StoreLocation,
}

impl CliFixture {
    fn new(test_name: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "mind-cli-{test_name}-{}-{stamp}",
            std::process::id()
        ));
        Self {
            endpoint: MindDaemonEndpoint::new(root.with_extension("sock")),
            store: StoreLocation::new(root.with_extension("sema").to_string_lossy().to_string()),
        }
    }

    async fn bind(&self) -> mind::transport::BoundMindDaemon {
        MindDaemon::new(self.endpoint.clone(), self.store.clone())
            .bind()
            .await
            .expect("daemon binds")
    }

    fn environment(&self, actor: &str) -> MindCommandEnvironment {
        MindCommandEnvironment::new(
            self.endpoint.as_path().to_str().expect("socket path utf8"),
            actor,
        )
    }
}

struct ConfigurationWriterFixture {
    root: PathBuf,
    socket_path: PathBuf,
    meta_socket_path: PathBuf,
    store_path: PathBuf,
    output_path: PathBuf,
    agent_socket_path: PathBuf,
    judge_log_path: PathBuf,
}

impl ConfigurationWriterFixture {
    fn new(test_name: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "mind-configuration-writer-{test_name}-{}-{stamp}",
            std::process::id()
        ));
        Self {
            socket_path: root.with_extension("sock"),
            meta_socket_path: root.with_extension("meta.sock"),
            store_path: root.with_extension("sema"),
            output_path: root.with_extension("rkyv"),
            agent_socket_path: root.with_extension("agent.sock"),
            judge_log_path: root.with_extension("judge.jsonl"),
            root,
        }
    }

    fn old_agent_judge_request(&self) -> String {
        format!(
            "(ConfigurationWriteRequest {} {} {} {} (AgentKnowledgeJudge {} deepseek deepseek-v4-flash 180000 2048))",
            self.socket_path.display(),
            self.meta_socket_path.display(),
            self.store_path.display(),
            self.output_path.display(),
            self.agent_socket_path.display(),
        )
    }

    fn explicit_default_agent_judge_request(&self) -> String {
        format!(
            "(ConfigurationWriteRequest {} {} {} {} (AgentKnowledgeJudge {} deepseek deepseek-v4-flash 180000 2048 (DefaultJudgeTraining)))",
            self.socket_path.display(),
            self.meta_socket_path.display(),
            self.store_path.display(),
            self.output_path.display(),
            self.agent_socket_path.display(),
        )
    }

    fn local_openai_agent_judge_request(&self) -> String {
        format!(
            "(ConfigurationWriteRequest {} {} {} {} (AgentKnowledgeJudge {} {} {} 180000 2048))",
            self.socket_path.display(),
            self.meta_socket_path.display(),
            self.store_path.display(),
            self.output_path.display(),
            self.agent_socket_path.display(),
            MindKnowledgeJudgeAgentConfiguration::LOCAL_OPENAI_COMPATIBLE_PROVIDER,
            MindKnowledgeJudgeAgentConfiguration::LOCAL_OPENAI_COMPATIBLE_MODEL,
        )
    }

    fn override_agent_judge_request(&self, training_path: &std::path::Path) -> String {
        format!(
            "(ConfigurationWriteRequest {} {} {} {} (AgentKnowledgeJudge {} deepseek deepseek-v4-flash 180000 2048 (JudgeTrainingFile {})))",
            self.socket_path.display(),
            self.meta_socket_path.display(),
            self.store_path.display(),
            self.output_path.display(),
            self.agent_socket_path.display(),
            training_path.display(),
        )
    }

    fn multi_source_agent_judge_request(&self, training_path: &std::path::Path) -> String {
        format!(
            "(ConfigurationWriteRequest {} {} {} {} (AgentKnowledgeJudge {} deepseek deepseek-v4-flash 180000 2048 (JudgeTrainingSources (DefaultJudgeTraining) (JudgeTrainingFile {}) (DiagnosticJudgeTraining))))",
            self.socket_path.display(),
            self.meta_socket_path.display(),
            self.store_path.display(),
            self.output_path.display(),
            self.agent_socket_path.display(),
            training_path.display(),
        )
    }

    fn diagnostic_agent_judge_request(&self) -> String {
        format!(
            "(ConfigurationWriteRequest {} {} {} {} (AgentKnowledgeJudge {} deepseek deepseek-v4-flash 180000 2048 (DiagnosticJudgeTraining)))",
            self.socket_path.display(),
            self.meta_socket_path.display(),
            self.store_path.display(),
            self.output_path.display(),
            self.agent_socket_path.display(),
        )
    }

    fn agent_judge_request_response_log_request(&self) -> String {
        format!(
            "(ConfigurationWriteRequest {} {} {} {} (AgentKnowledgeJudge {} deepseek deepseek-v4-flash 180000 2048 (DefaultJudgeTraining) (JudgeRequestResponseLog (JsonLines {}))))",
            self.socket_path.display(),
            self.meta_socket_path.display(),
            self.store_path.display(),
            self.output_path.display(),
            self.agent_socket_path.display(),
            self.judge_log_path.display(),
        )
    }

    fn agent_judge_request_response_log_store_path_request(&self) -> String {
        format!(
            "(ConfigurationWriteRequest {} {} {} {} (AgentKnowledgeJudge {} deepseek deepseek-v4-flash 180000 2048 (JudgeRequestResponseLog (JsonLines {}))))",
            self.socket_path.display(),
            self.meta_socket_path.display(),
            self.store_path.display(),
            self.output_path.display(),
            self.agent_socket_path.display(),
            self.store_path.display(),
        )
    }

    fn request_path(&self) -> PathBuf {
        self.root.with_extension("nota")
    }

    fn training_path(&self) -> PathBuf {
        self.root.with_extension("training.md")
    }

    fn read_configuration(&self) -> MindDaemonConfiguration {
        MindDaemonConfiguration::from_signal_file(&self.output_path)
            .expect("written configuration decodes")
    }
}

impl Drop for ConfigurationWriterFixture {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.output_path);
        let _ = fs::remove_file(&self.judge_log_path);
        let _ = fs::remove_file(self.request_path());
        let _ = fs::remove_file(self.training_path());
    }
}

fn run_configuration_writer(argument: impl AsRef<std::ffi::OsStr>) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_mind-write-configuration"))
        .arg(argument)
        .output()
        .expect("configuration writer process runs");
    assert!(
        output.status.success(),
        "configuration writer should succeed; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("configuration writer stdout utf8")
}

fn run_configuration_writer_failure(argument: impl AsRef<std::ffi::OsStr>) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_mind-write-configuration"))
        .arg(argument)
        .output()
        .expect("configuration writer process runs");
    assert!(!output.status.success(), "configuration writer should fail");
    String::from_utf8(output.stderr).expect("configuration writer stderr utf8")
}

fn assert_agent_training_source(
    configuration: &MindDaemonConfiguration,
) -> &MindKnowledgeJudgeTrainingSource {
    let MindKnowledgeJudgeConfiguration::Agent(agent) = &configuration.knowledge_judge else {
        panic!("expected agent knowledge judge configuration");
    };
    &agent.training_source
}

fn assert_agent_configuration(
    configuration: &MindDaemonConfiguration,
) -> &MindKnowledgeJudgeAgentConfiguration {
    let MindKnowledgeJudgeConfiguration::Agent(agent) = &configuration.knowledge_judge else {
        panic!("expected agent knowledge judge configuration");
    };
    agent
}

fn assert_agent_request_response_log(
    configuration: &MindDaemonConfiguration,
) -> &MindJudgeRequestResponseLog {
    &assert_agent_configuration(configuration).request_response_log
}

#[test]
fn nota_opening_text_maps_to_signal_request() {
    let request = mind::MindTextRequest::from_nota("(Opening Task High [Open work] body)")
        .expect("text decodes")
        .into_request()
        .expect("request maps to signal");

    let MindRequest::Opening(opening) = request else {
        panic!("expected opening");
    };

    assert_eq!(opening.kind, signal_mind::ItemKind::Task);
    assert_eq!(opening.priority, signal_mind::Magnitude::High);
}

#[test]
fn configuration_writer_preserves_old_agent_judge_default_training_shape() {
    let fixture = ConfigurationWriterFixture::new("old-agent-default");
    let stdout = run_configuration_writer(fixture.old_agent_judge_request());
    assert!(stdout.contains("(ConfigurationWritten"));

    let configuration = fixture.read_configuration();
    assert_eq!(
        assert_agent_training_source(&configuration),
        &MindKnowledgeJudgeTrainingSource::CompiledDefault
    );
}

#[test]
fn configuration_writer_accepts_explicit_default_training_shape() {
    let fixture = ConfigurationWriterFixture::new("explicit-agent-default");
    let stdout = run_configuration_writer(fixture.explicit_default_agent_judge_request());
    assert!(stdout.contains("(ConfigurationWritten"));

    let configuration = fixture.read_configuration();
    assert_eq!(
        assert_agent_training_source(&configuration),
        &MindKnowledgeJudgeTrainingSource::CompiledDefault
    );
}

#[test]
fn configuration_writer_accepts_local_openai_compatible_judge_shape() {
    let fixture = ConfigurationWriterFixture::new("local-openai-agent");
    let stdout = run_configuration_writer(fixture.local_openai_agent_judge_request());
    assert!(stdout.contains("(ConfigurationWritten"));

    let configuration = fixture.read_configuration();
    let agent = assert_agent_configuration(&configuration);
    assert_eq!(
        agent.provider_name.as_deref(),
        Some(MindKnowledgeJudgeAgentConfiguration::LOCAL_OPENAI_COMPATIBLE_PROVIDER)
    );
    assert_eq!(
        agent.model_name.as_deref(),
        Some(MindKnowledgeJudgeAgentConfiguration::LOCAL_OPENAI_COMPATIBLE_MODEL)
    );
    assert_eq!(
        agent.training_source,
        MindKnowledgeJudgeTrainingSource::CompiledDefault
    );
}

#[test]
fn configuration_writer_accepts_judge_request_response_log_shape() {
    let fixture = ConfigurationWriterFixture::new("judge-request-response-log");
    let stdout = run_configuration_writer(fixture.agent_judge_request_response_log_request());
    assert!(stdout.contains("(ConfigurationWritten"));

    let configuration = fixture.read_configuration();
    match assert_agent_request_response_log(&configuration) {
        MindJudgeRequestResponseLog::JsonLines(path) => {
            assert_eq!(path.as_str(), fixture.judge_log_path.display().to_string());
            assert_ne!(path.as_str(), configuration.store_path.as_str());
        }
        other => panic!("expected judge request/response JSONL log, got {other:?}"),
    }
}

#[test]
fn configuration_writer_rejects_judge_request_response_log_at_store_path() {
    let fixture = ConfigurationWriterFixture::new("judge-request-response-log-store-path");
    let stderr = run_configuration_writer_failure(
        fixture.agent_judge_request_response_log_store_path_request(),
    );
    assert!(
        stderr.contains("judge request/response log path must differ from store path"),
        "unexpected stderr: {stderr}"
    );
}

#[test]
fn local_openai_compatible_helper_uses_subscription_server_defaults() {
    let agent_socket_path =
        signal_mind::WirePath::from_absolute_path("/tmp/agent.sock").expect("absolute agent path");
    let configuration =
        MindKnowledgeJudgeAgentConfiguration::local_openai_compatible(agent_socket_path);

    assert_eq!(
        configuration.provider_name.as_deref(),
        Some(MindKnowledgeJudgeAgentConfiguration::LOCAL_OPENAI_COMPATIBLE_PROVIDER)
    );
    assert_eq!(
        configuration.model_name.as_deref(),
        Some(MindKnowledgeJudgeAgentConfiguration::LOCAL_OPENAI_COMPATIBLE_MODEL)
    );
    assert_eq!(
        MindKnowledgeJudgeAgentConfiguration::LOCAL_OPENAI_COMPATIBLE_ENDPOINT,
        "http://127.0.0.1:18080/v1"
    );
}

#[test]
fn configuration_writer_reads_override_training_file_into_archive() {
    let fixture = ConfigurationWriterFixture::new("override-training");
    let training_path = fixture.training_path();
    fs::write(
        &training_path,
        "Override writer training marker reaches binary configuration.\n",
    )
    .expect("write training override fixture");
    let request_path = fixture.request_path();
    fs::write(
        &request_path,
        fixture.override_agent_judge_request(&training_path),
    )
    .expect("write configuration writer request file");

    let stdout = run_configuration_writer(&request_path);
    assert!(stdout.contains("(ConfigurationWritten"));

    let configuration = fixture.read_configuration();
    assert_eq!(
        assert_agent_training_source(&configuration),
        &MindKnowledgeJudgeTrainingSource::OverrideText(
            "Override writer training marker reaches binary configuration.\n".to_owned()
        )
    );
}

#[test]
fn configuration_writer_composes_multiple_training_sources_into_archive() {
    let fixture = ConfigurationWriterFixture::new("multi-source-training");
    let training_path = fixture.training_path();
    fs::write(
        &training_path,
        "# External NOTA literacy marker\n\nUse positional records.\n",
    )
    .expect("write training source fixture");
    let request_path = fixture.request_path();
    fs::write(
        &request_path,
        fixture.multi_source_agent_judge_request(&training_path),
    )
    .expect("write multi-source configuration writer request file");

    let stdout = run_configuration_writer(&request_path);
    assert!(stdout.contains("(ConfigurationWritten"));

    let configuration = fixture.read_configuration();
    let MindKnowledgeJudgeTrainingSource::OverrideText(text) =
        assert_agent_training_source(&configuration)
    else {
        panic!("multi-source training should be archived as composed text");
    };
    assert!(text.contains("# Mind accepted-knowledge judge training"));
    assert!(text.contains("# External NOTA literacy marker"));
    assert!(text.contains("Use positional records."));
    assert!(text.contains("Debug-only Mind judge diagnostic prose escape hatch"));
}

#[test]
fn configuration_writer_accepts_optional_diagnostic_training_source() {
    let fixture = ConfigurationWriterFixture::new("diagnostic-training");
    let stdout = run_configuration_writer(fixture.diagnostic_agent_judge_request());
    assert!(stdout.contains("(ConfigurationWritten"));

    let configuration = fixture.read_configuration();
    let MindKnowledgeJudgeTrainingSource::OverrideText(text) =
        assert_agent_training_source(&configuration)
    else {
        panic!("diagnostic training should be archived as override text");
    };
    assert!(text.contains("Debug-only Mind judge diagnostic prose escape hatch"));
    assert!(text.contains("Normal response path"));
    assert!(text.contains("Do not use prose for ordinary semantic uncertainty"));
}

#[test]
fn nota_query_text_maps_to_signal_request() {
    let request = mind::MindTextRequest::from_nota("(Query (Open) 10)")
        .expect("text decodes")
        .into_request()
        .expect("request maps to signal");

    let MindRequest::Query(query) = request else {
        panic!("expected query");
    };

    assert_eq!(query.kind, signal_mind::QueryKind::Open);
    assert_eq!(query.limit.into_u16(), 10);
}

#[test]
fn nota_work_mutation_text_maps_to_signal_requests() {
    let item_display = "aab";

    let note = mind::MindTextRequest::from_nota(&format!(
        "(NoteSubmission (Display {item_display}) [note body])"
    ))
    .expect("note text decodes")
    .into_request()
    .expect("note maps to signal");

    let MindRequest::NoteSubmission(note) = note else {
        panic!("expected note submission");
    };
    assert_eq!(
        note.item,
        signal_mind::ItemReference::Display(signal_mind::DisplayIdentifier::new(item_display))
    );

    let link = mind::MindTextRequest::from_nota(&format!(
        "(Link (Display {item_display}) References (Report reports/operator/105-command-line-mind-architecture-survey.md) None)"
    ))
    .expect("link text decodes")
    .into_request()
    .expect("link maps to signal");

    let MindRequest::Link(link) = link else {
        panic!("expected link");
    };
    assert_eq!(link.kind, signal_mind::EdgeKind::References);

    let status = mind::MindTextRequest::from_nota(&format!(
        "(StatusChange (Display {item_display}) InProgress started)"
    ))
    .expect("status text decodes")
    .into_request()
    .expect("status maps to signal");

    let MindRequest::StatusChange(status) = status else {
        panic!("expected status change");
    };
    assert_eq!(status.status, signal_mind::ItemStatus::InProgress);

    let alias = mind::MindTextRequest::from_nota(&format!(
        "(AliasAssignment (Display {item_display}) primary-test)"
    ))
    .expect("alias text decodes")
    .into_request()
    .expect("alias maps to signal");

    let MindRequest::AliasAssignment(alias) = alias else {
        panic!("expected alias assignment");
    };
    assert_eq!(alias.alias, signal_mind::ExternalAlias::new("primary-test"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mind_cli_opens_and_queries_work_item_through_daemon() {
    let fixture = CliFixture::new("opening-query");
    let daemon = fixture.bind().await;
    let server = tokio::spawn(async move { daemon.serve_count(2).await });

    let mut opening_output = Vec::new();
    MindCommand::from_arguments_with_environment(
        ["(Opening Task High [Open CLI-visible work] [created through mind text])"],
        fixture.environment("operator"),
    )
    .run(&mut opening_output)
    .await
    .expect("cli opens work item");

    let mut query_output = Vec::new();
    MindCommand::from_arguments_with_environment(
        ["(Query (Open) 10)"],
        fixture.environment("operator"),
    )
    .run(&mut query_output)
    .await
    .expect("cli queries work items");

    server
        .await
        .expect("daemon task joins")
        .expect("daemon serves requests");

    let opening = String::from_utf8(opening_output).expect("opening output utf8");
    assert!(opening.contains("(OpeningReceipt"));
    assert!(opening.contains("[Open CLI-visible work]"));

    let query = String::from_utf8(query_output).expect("query output utf8");
    assert!(query.contains("(View ["));
    assert!(query.contains("[Open CLI-visible work]"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mind_cli_mutates_work_item_through_daemon() {
    let fixture = CliFixture::new("mutate-work-item");
    let daemon = fixture.bind().await;
    let server = tokio::spawn(async move { daemon.serve_count(6).await });
    let item_display = "aab";

    let mut opening_output = Vec::new();
    MindCommand::from_arguments_with_environment(
        ["(Opening Task High [Mutate CLI-visible work] [created through mind text])"],
        fixture.environment("operator"),
    )
    .run(&mut opening_output)
    .await
    .expect("cli opens work item");

    let mut note_output = Vec::new();
    MindCommand::from_arguments_with_environment(
        [format!(
            "(NoteSubmission (Display {item_display}) [designer note])"
        )],
        fixture.environment("designer"),
    )
    .run(&mut note_output)
    .await
    .expect("cli adds note");

    let mut alias_output = Vec::new();
    MindCommand::from_arguments_with_environment(
        [format!(
            "(AliasAssignment (Display {item_display}) primary-mind-text)"
        )],
        fixture.environment("operator"),
    )
    .run(&mut alias_output)
    .await
    .expect("cli adds alias");

    let mut link_output = Vec::new();
    MindCommand::from_arguments_with_environment(
        [format!(
            "(Link (Display {item_display}) References (Report reports/operator/105-command-line-mind-architecture-survey.md) [source report])"
        )],
        fixture.environment("operator"),
    )
    .run(&mut link_output)
    .await
    .expect("cli adds report link");

    let mut status_output = Vec::new();
    MindCommand::from_arguments_with_environment(
        [format!(
            "(StatusChange (Display {item_display}) InProgress [implementation started])"
        )],
        fixture.environment("operator"),
    )
    .run(&mut status_output)
    .await
    .expect("cli changes status");

    let mut query_output = Vec::new();
    MindCommand::from_arguments_with_environment(
        [format!("(Query (ByItem (Display {item_display})) 20)")],
        fixture.environment("operator"),
    )
    .run(&mut query_output)
    .await
    .expect("cli queries work item");

    server
        .await
        .expect("daemon task joins")
        .expect("daemon serves mutation requests");

    assert!(
        String::from_utf8(note_output)
            .expect("note output utf8")
            .contains("(NoteReceipt")
    );
    assert!(
        String::from_utf8(alias_output)
            .expect("alias output utf8")
            .contains("(AliasReceipt")
    );
    assert!(
        String::from_utf8(link_output)
            .expect("link output utf8")
            .contains("(LinkReceipt")
    );
    assert!(
        String::from_utf8(status_output)
            .expect("status output utf8")
            .contains("(StatusReceipt")
    );

    let query = String::from_utf8(query_output).expect("query output utf8");
    assert!(query.contains("InProgress"));
    assert!(query.contains("primary-mind-text"));
    assert!(query.contains("[designer note]"));
    assert!(query.contains("reports/operator/105-command-line-mind-architecture-survey.md"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mind_cli_accepts_full_signal_mind_request_for_typed_graph() {
    let fixture = CliFixture::new("typed-graph");
    let daemon = fixture.bind().await;
    let server = tokio::spawn(async move { daemon.serve_one().await });
    let request = MindRequest::SubmitThought(SubmitThought {
        kind: ThoughtKind::Goal,
        body: ThoughtBody::Goal(GoalBody {
            description: TextBody::new("CLI accepts full signal request"),
            scope: GoalScope::Workspace(WorkspaceGoal {
                workspace: TextBody::new("primary"),
            }),
        }),
    });
    let encoded_request = request.to_nota();

    let mut output = Vec::new();
    MindCommand::from_arguments_with_environment(
        [encoded_request],
        fixture.environment("operator"),
    )
    .run(&mut output)
    .await
    .expect("cli sends typed graph request");

    server
        .await
        .expect("daemon task joins")
        .expect("daemon serves request");
    let text = String::from_utf8(output).expect("cli output utf8");

    assert!(text.starts_with("(ThoughtCommitted "));
    assert!(text.contains("aaa"));
    assert!(!text.contains("item-"));
}
