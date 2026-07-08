use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use mind::{
    MindDaemonConfiguration, MindKnowledgeJudgeConfiguration, MindKnowledgeJudgeSocketConfiguration,
};
use signal_mind::WirePath;

struct ConfigurationFixture {
    root: PathBuf,
}

impl ConfigurationFixture {
    fn new(test_name: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "mind-configuration-{test_name}-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("create fixture root");
        Self { root }
    }

    fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    fn wire(&self, name: &str) -> WirePath {
        wire_path(&self.path(name))
    }
}

impl Drop for ConfigurationFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn wire_path(path: &Path) -> WirePath {
    WirePath::from_absolute_path(path.to_string_lossy().to_string())
        .expect("fixture path is absolute")
}

#[test]
fn mind_judge_socket_configuration_default_timeout_is_short() {
    assert_eq!(
        MindKnowledgeJudgeSocketConfiguration::DEFAULT_TIMEOUT_MILLISECONDS,
        5_000
    );
}

#[test]
fn daemon_configuration_round_trips_mind_judge_socket_selection() {
    let fixture = ConfigurationFixture::new("round-trip");
    let configuration = MindDaemonConfiguration::new(
        fixture.wire("mind.sema"),
        fixture.wire("mind.sock"),
        fixture.wire("meta-mind.sock"),
    )
    .with_mind_judge(MindKnowledgeJudgeSocketConfiguration::new(
        fixture.wire("mind-judge.sock"),
        750,
    ));

    let decoded = MindDaemonConfiguration::from_signal_bytes(
        &configuration
            .to_signal_bytes()
            .expect("configuration encodes"),
    )
    .expect("configuration decodes");

    assert!(matches!(
        decoded.knowledge_judge,
        MindKnowledgeJudgeConfiguration::MindJudge(ref judge)
            if judge.socket_path == fixture.wire("mind-judge.sock")
                && judge.timeout_milliseconds == 750
    ));
}

#[test]
fn configuration_writer_accepts_mind_judge_socket_record() {
    let fixture = ConfigurationFixture::new("writer");
    let output = fixture.path("daemon.rkyv");
    let request = format!(
        "(ConfigurationWriteRequest {} {} {} {} (MindJudge {} 750))",
        fixture.path("mind.sock").display(),
        fixture.path("meta-mind.sock").display(),
        fixture.path("mind.sema").display(),
        output.display(),
        fixture.path("mind-judge.sock").display(),
    );

    let command_output = Command::new(env!("CARGO_BIN_EXE_mind-write-configuration"))
        .arg(request)
        .output()
        .expect("configuration writer runs");

    assert!(
        command_output.status.success(),
        "configuration writer stderr: {}",
        String::from_utf8_lossy(&command_output.stderr)
    );
    let configuration =
        MindDaemonConfiguration::from_signal_file(&output).expect("written configuration decodes");
    assert!(matches!(
        configuration.knowledge_judge,
        MindKnowledgeJudgeConfiguration::MindJudge(ref judge)
            if judge.socket_path == fixture.wire("mind-judge.sock")
                && judge.timeout_milliseconds == 750
    ));
}
