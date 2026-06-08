use mind::MindProcessDaemon;
use mind::schema::daemon::DaemonEntry;

fn main() -> std::process::ExitCode {
    <MindProcessDaemon as DaemonEntry>::run_to_exit_code()
}
