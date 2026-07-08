#![cfg(any())]
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use mind::{
    ConfigurationError, MindDaemonConfiguration, MindJudgeRequestResponseLog,
    MindKnowledgeJudgeAgentConfiguration,
};
use signal_mind::WirePath;

struct ConfigurationValidationFixture {
    root: PathBuf,
    store_path: PathBuf,
    log_path: PathBuf,
}

impl ConfigurationValidationFixture {
    fn new(test_name: &str) -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "mind-configuration-validation-{test_name}-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("create validation fixture directory");
        Self {
            store_path: root.join("mind.sema"),
            log_path: root.join("judge.jsonl"),
            root,
        }
    }

    fn configuration(&self) -> MindDaemonConfiguration {
        MindDaemonConfiguration::new(
            Self::wire_path(&self.store_path),
            Self::wire_path(&self.root.join("mind.sock")),
            Self::wire_path(&self.root.join("meta-mind.sock")),
        )
        .with_agent_knowledge_judge(
            MindKnowledgeJudgeAgentConfiguration::deepseek_flash(Self::wire_path(
                &self.root.join("agent.sock"),
            ))
            .with_request_response_log(MindJudgeRequestResponseLog::JsonLines(
                Self::wire_path(&self.log_path),
            )),
        )
    }

    fn wire_path(path: &Path) -> WirePath {
        WirePath::from_absolute_path(path.to_string_lossy().to_string())
            .expect("fixture path is absolute")
    }
}

impl Drop for ConfigurationValidationFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[cfg(unix)]
#[test]
fn judge_request_response_log_rejects_existing_hard_link_to_store() {
    let fixture = ConfigurationValidationFixture::new("hard-link");
    std::fs::write(&fixture.store_path, b"mind store").expect("write store fixture");
    std::fs::hard_link(&fixture.store_path, &fixture.log_path).expect("create hard link fixture");

    let error = fixture
        .configuration()
        .validate()
        .expect_err("hard-linked log path must be rejected");

    assert!(
        matches!(
            error,
            ConfigurationError::JudgeRequestResponseLogPathIsStore { .. }
        ),
        "unexpected validation error: {error}"
    );
}

#[cfg(unix)]
#[test]
fn judge_request_response_log_rejects_direct_symlink_to_store() {
    let fixture = ConfigurationValidationFixture::new("symlink");
    std::fs::write(&fixture.store_path, b"mind store").expect("write store fixture");
    std::os::unix::fs::symlink(&fixture.store_path, &fixture.log_path)
        .expect("create symlink fixture");

    let error = fixture
        .configuration()
        .validate()
        .expect_err("symlinked log path must be rejected");

    assert!(
        matches!(
            error,
            ConfigurationError::JudgeRequestResponseLogPathIsStore { .. }
        ),
        "unexpected validation error: {error}"
    );
}
