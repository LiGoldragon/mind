use mind::MindCommand;

#[tokio::main]
async fn main() {
    if let Err(error) = MindCommand::from_env().run(std::io::stdout().lock()).await {
        eprintln!("mind: {error}");
        std::process::exit(1);
    }
}
