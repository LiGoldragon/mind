use std::io::{Read, Write};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use mind::{
    MindCommand, MindCommandEnvironment, MindDaemon, MindDaemonEndpoint,
    MindJudgeSocketKnowledgeJudge, MindKnowledgeJudgeSocketConfiguration, StoreLocation,
};
use signal_frame::{NonEmpty, Reply, SubReply};
use signal_mind::{KnowledgeRejectionReason, MindReply, WirePath};

struct CliFixture {
    mind_socket: PathBuf,
    store: PathBuf,
    judge_socket: PathBuf,
    judge_server: thread::JoinHandle<signal_mind_judge::KnowledgeJudgePacket>,
}

impl CliFixture {
    fn new() -> Self {
        let root = temporary_path("mind-cli-judge", "root");
        let mind_socket = root.with_extension("sock");
        let store = root.with_extension("sema");
        let judge_socket = root.with_extension("judge.sock");
        let _ = std::fs::remove_file(&judge_socket);
        let listener = UnixListener::bind(&judge_socket).expect("fake judge binds");
        let judge_server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept judge request");
            let (exchange, packet) = read_judge_request(&mut stream);
            let reply = signal_mind_judge::MindJudgeFrame::new(
                signal_mind_judge::MindJudgeFrameBody::Reply {
                    exchange,
                    reply: Reply::committed(NonEmpty::single(SubReply::Ok(
                        signal_mind_judge::MindJudgeReply::KnowledgeJudged(
                            signal_mind_judge::KnowledgeJudgeResponse::new(
                                signal_mind_judge::KnowledgeJudgeVerdict::Reject(
                                    signal_mind_judge::KnowledgeRejectionReason::NotKnowledge,
                                ),
                                None,
                            ),
                        ),
                    ))),
                },
            );
            stream
                .write_all(&reply.encode_length_prefixed().expect("encode judge reply"))
                .expect("write judge reply");
            packet
        });
        Self {
            mind_socket,
            store,
            judge_socket,
            judge_server,
        }
    }

    fn daemon(&self) -> MindDaemon {
        let judge = MindJudgeSocketKnowledgeJudge::new(MindKnowledgeJudgeSocketConfiguration::new(
            WirePath::from_absolute_path(self.judge_socket.to_string_lossy().into_owned())
                .expect("judge socket path is absolute"),
            500,
        ));
        MindDaemon::new(
            MindDaemonEndpoint::new(&self.mind_socket),
            StoreLocation::new(self.store.to_string_lossy().to_string()),
        )
        .with_knowledge_judge(Arc::new(judge))
    }

    fn environment(&self) -> MindCommandEnvironment {
        MindCommandEnvironment::new(self.mind_socket.to_string_lossy().to_string(), "cli-tester")
    }

    fn cleanup(self) -> signal_mind_judge::KnowledgeJudgePacket {
        let packet = self.judge_server.join().expect("fake judge joins");
        let _ = std::fs::remove_file(self.mind_socket);
        let _ = std::fs::remove_file(self.judge_socket);
        let _ = std::fs::remove_file(&self.store);
        let _ = std::fs::remove_dir_all(&self.store);
        packet
    }
}

fn read_judge_request(
    stream: &mut std::os::unix::net::UnixStream,
) -> (
    signal_frame::ExchangeIdentifier,
    signal_mind_judge::KnowledgeJudgePacket,
) {
    let mut length = [0_u8; 4];
    stream.read_exact(&mut length).expect("read frame length");
    let length = u32::from_be_bytes(length) as usize;
    let mut payload = vec![0_u8; length];
    stream.read_exact(&mut payload).expect("read frame payload");
    let mut frame_bytes = Vec::with_capacity(4 + payload.len());
    frame_bytes.extend_from_slice(&(length as u32).to_be_bytes());
    frame_bytes.extend_from_slice(&payload);
    let frame = signal_mind_judge::MindJudgeFrame::decode_length_prefixed(&frame_bytes)
        .expect("decode judge request");
    let signal_mind_judge::MindJudgeFrameBody::Request { exchange, request } = frame.into_body()
    else {
        panic!("expected request frame");
    };
    let signal_mind_judge::MindJudgeRequest::JudgeKnowledge(packet) = request.payloads.into_head();
    (exchange, packet)
}

fn temporary_path(name: &str, extension: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    std::env::temp_dir().join(format!("{name}-{}-{stamp}.{extension}", std::process::id()))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mind_cli_routes_submit_through_configured_typed_judge_socket() {
    let fixture = CliFixture::new();
    let daemon = fixture.daemon().bind().await.expect("daemon binds");
    let server = tokio::spawn(async move { daemon.serve_one().await });
    let request = "(Submit ((Technology (Software (Engineering Architecture))) TaskText))";
    let mut output = Vec::new();

    MindCommand::from_arguments_with_environment([request], fixture.environment())
        .run(&mut output)
        .await
        .expect("mind command succeeds");
    let daemon_reply = server
        .await
        .expect("daemon task joins")
        .expect("daemon serves one request");

    assert!(matches!(
        daemon_reply,
        MindReply::Rejected(KnowledgeRejectionReason::NotKnowledge)
    ));
    let output = String::from_utf8(output).expect("CLI output is UTF-8");
    assert!(output.contains("NotKnowledge"));
    let packet = fixture.cleanup();
    assert_eq!(packet.statement.as_str(), "TaskText");
}
