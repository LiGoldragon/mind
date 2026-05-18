use std::time::{SystemTime, UNIX_EPOCH};

use nota_codec::{Encoder, NotaEncode};
use persona_mind::{MindCommand, MindDaemon, MindDaemonEndpoint, StoreLocation};
use signal_persona_mind::{
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
            "persona-mind-cli-{test_name}-{}-{stamp}",
            std::process::id()
        ));
        Self {
            endpoint: MindDaemonEndpoint::new(root.with_extension("sock")),
            store: StoreLocation::new(root.with_extension("redb").to_string_lossy().to_string()),
        }
    }

    async fn bind(&self) -> persona_mind::transport::BoundMindDaemon {
        MindDaemon::new(self.endpoint.clone(), self.store.clone())
            .bind()
            .await
            .expect("daemon binds")
    }
}

#[test]
fn nota_opening_text_maps_to_signal_request() {
    let request =
        persona_mind::MindTextRequest::from_nota("(Opening Task High \"Open work\" \"body\")")
            .expect("text decodes")
            .into_request()
            .expect("request maps to signal");

    let MindRequest::Opening(opening) = request else {
        panic!("expected opening");
    };

    assert_eq!(opening.kind, signal_persona_mind::ItemKind::Task);
    assert_eq!(opening.priority, signal_persona_mind::ItemPriority::High);
}

#[test]
fn nota_query_text_maps_to_signal_request() {
    let request = persona_mind::MindTextRequest::from_nota("(Query (Open) 10)")
        .expect("text decodes")
        .into_request()
        .expect("request maps to signal");

    let MindRequest::Query(query) = request else {
        panic!("expected query");
    };

    assert_eq!(query.kind, signal_persona_mind::QueryKind::Open);
    assert_eq!(query.limit.into_u16(), 10);
}

#[test]
fn nota_work_mutation_text_maps_to_signal_requests() {
    let item_display = "aab";

    let note = persona_mind::MindTextRequest::from_nota(&format!(
        "(NoteSubmission (Display {item_display}) \"note body\")"
    ))
    .expect("note text decodes")
    .into_request()
    .expect("note maps to signal");

    let MindRequest::NoteSubmission(note) = note else {
        panic!("expected note submission");
    };
    assert_eq!(
        note.item,
        signal_persona_mind::ItemReference::Display(signal_persona_mind::DisplayId::new(
            item_display
        ))
    );

    let link = persona_mind::MindTextRequest::from_nota(&format!(
        "(Link (Display {item_display}) References (Report \"reports/operator/105-command-line-mind-architecture-survey.md\") None)"
    ))
    .expect("link text decodes")
    .into_request()
    .expect("link maps to signal");

    let MindRequest::Link(link) = link else {
        panic!("expected link");
    };
    assert_eq!(link.kind, signal_persona_mind::EdgeKind::References);

    let status = persona_mind::MindTextRequest::from_nota(&format!(
        "(StatusChange (Display {item_display}) InProgress \"started\")"
    ))
    .expect("status text decodes")
    .into_request()
    .expect("status maps to signal");

    let MindRequest::StatusChange(status) = status else {
        panic!("expected status change");
    };
    assert_eq!(status.status, signal_persona_mind::ItemStatus::InProgress);

    let alias = persona_mind::MindTextRequest::from_nota(&format!(
        "(AliasAssignment (Display {item_display}) primary-test)"
    ))
    .expect("alias text decodes")
    .into_request()
    .expect("alias maps to signal");

    let MindRequest::AliasAssignment(alias) = alias else {
        panic!("expected alias assignment");
    };
    assert_eq!(
        alias.alias,
        signal_persona_mind::ExternalAlias::new("primary-test")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mind_cli_opens_and_queries_work_item_through_daemon() {
    let fixture = CliFixture::new("opening-query");
    let daemon = fixture.bind().await;
    let endpoint = daemon.endpoint().clone();
    let server = tokio::spawn(async move { daemon.serve_count(2).await });

    let mut opening_output = Vec::new();
    MindCommand::from_arguments([
        "--socket",
        endpoint.as_path().to_str().expect("socket path utf8"),
        "--actor",
        "operator",
        "(Opening Task High \"Open CLI-visible work\" \"created through mind text\")",
    ])
    .run(&mut opening_output)
    .await
    .expect("cli opens work item");

    let mut query_output = Vec::new();
    MindCommand::from_arguments([
        "--socket",
        endpoint.as_path().to_str().expect("socket path utf8"),
        "--actor",
        "operator",
        "(Query (Open) 10)",
    ])
    .run(&mut query_output)
    .await
    .expect("cli queries work items");

    server
        .await
        .expect("daemon task joins")
        .expect("daemon serves requests");

    let opening = String::from_utf8(opening_output).expect("opening output utf8");
    assert!(opening.contains("(OpeningReceipt"));
    assert!(opening.contains("\"Open CLI-visible work\""));

    let query = String::from_utf8(query_output).expect("query output utf8");
    assert!(query.contains("(View [(Item"));
    assert!(query.contains("\"Open CLI-visible work\""));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mind_cli_mutates_work_item_through_daemon() {
    let fixture = CliFixture::new("mutate-work-item");
    let daemon = fixture.bind().await;
    let endpoint = daemon.endpoint().clone();
    let server = tokio::spawn(async move { daemon.serve_count(6).await });
    let socket = endpoint.as_path().to_str().expect("socket path utf8");
    let item_display = "aab";

    let mut opening_output = Vec::new();
    MindCommand::from_arguments([
        "--socket",
        socket,
        "--actor",
        "operator",
        "(Opening Task High \"Mutate CLI-visible work\" \"created through mind text\")",
    ])
    .run(&mut opening_output)
    .await
    .expect("cli opens work item");

    let mut note_output = Vec::new();
    MindCommand::from_arguments([
        "--socket",
        socket,
        "--actor",
        "designer",
        &format!("(NoteSubmission (Display {item_display}) \"designer note\")"),
    ])
    .run(&mut note_output)
    .await
    .expect("cli adds note");

    let mut alias_output = Vec::new();
    MindCommand::from_arguments([
        "--socket",
        socket,
        "--actor",
        "operator",
        &format!("(AliasAssignment (Display {item_display}) primary-mind-text)"),
    ])
    .run(&mut alias_output)
    .await
    .expect("cli adds alias");

    let mut link_output = Vec::new();
    MindCommand::from_arguments([
        "--socket",
        socket,
        "--actor",
        "operator",
        &format!(
            "(Link (Display {item_display}) References (Report \"reports/operator/105-command-line-mind-architecture-survey.md\") \"source report\")"
        ),
    ])
    .run(&mut link_output)
    .await
    .expect("cli adds report link");

    let mut status_output = Vec::new();
    MindCommand::from_arguments([
        "--socket",
        socket,
        "--actor",
        "operator",
        &format!("(StatusChange (Display {item_display}) InProgress \"implementation started\")"),
    ])
    .run(&mut status_output)
    .await
    .expect("cli changes status");

    let mut query_output = Vec::new();
    MindCommand::from_arguments([
        "--socket",
        socket,
        "--actor",
        "operator",
        &format!("(Query (ByItem (Display {item_display})) 20)"),
    ])
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
    assert!(query.contains("\"designer note\""));
    assert!(query.contains("reports/operator/105-command-line-mind-architecture-survey.md"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mind_cli_accepts_full_signal_mind_request_for_typed_graph() {
    let fixture = CliFixture::new("typed-graph");
    let daemon = fixture.bind().await;
    let endpoint = daemon.endpoint().clone();
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
    let mut encoder = Encoder::new();
    request.encode(&mut encoder).expect("request encodes");

    let mut output = Vec::new();
    MindCommand::from_arguments([
        "--socket",
        endpoint.as_path().to_str().expect("socket path utf8"),
        "--actor",
        "operator",
        &encoder.into_string(),
    ])
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
