use mind::{MindCommand, Result};

#[tokio::main]
async fn main() -> Result<()> {
    MindCommand::from_env().run(std::io::stdout().lock()).await
}
