use mind::MetaMindCommand;

#[tokio::main]
async fn main() {
    if let Err(error) = MetaMindCommand::from_env()
        .run(std::io::stdout().lock())
        .await
    {
        eprintln!("meta-mind: {error}");
        std::process::exit(1);
    }
}
