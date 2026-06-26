use std::fs;
use std::io::Write;
use std::path::PathBuf;

use signal_mind::{ActorName, MindReply, MindRequest};
use triad_runtime::{ComponentArgument, ComponentCommand};

use crate::nota::{NotaEncode, NotaSource};
use crate::{Error, MindClient, MindDaemonEndpoint, MindTextReply, MindTextRequest, Result};

const DEFAULT_MIND_SOCKET: &str = "/tmp/mind.sock";
const DEFAULT_MIND_ACTOR: &str = "operator";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MindCommand {
    command: ComponentCommand,
    environment: MindCommandEnvironment,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MindCommandEnvironment {
    socket: String,
    actor: String,
}

impl MindCommand {
    pub fn from_env() -> Self {
        Self {
            command: ComponentCommand::from_environment(),
            environment: MindCommandEnvironment::from_process(),
        }
    }

    pub fn from_arguments<Arguments, Argument>(arguments: Arguments) -> Self
    where
        Arguments: IntoIterator<Item = Argument>,
        Argument: Into<String>,
    {
        Self::from_arguments_with_environment(arguments, MindCommandEnvironment::from_process())
    }

    pub fn from_arguments_with_environment<Arguments, Argument>(
        arguments: Arguments,
        environment: MindCommandEnvironment,
    ) -> Self
    where
        Arguments: IntoIterator<Item = Argument>,
        Argument: Into<String>,
    {
        Self {
            command: ComponentCommand::from_arguments(arguments),
            environment,
        }
    }

    /// The `mind` CLI is a daemon client: it decodes exactly one NOTA request,
    /// sends one binary Signal frame to the long-lived daemon, and prints one
    /// NOTA reply. Socket and caller defaults come from the process environment,
    /// not from extra argv flags.
    pub async fn run(self, output: impl Write) -> Result<()> {
        SubmissionCommand::from_command(self.command, self.environment)?
            .run(output)
            .await
    }
}

impl MindCommandEnvironment {
    pub fn new(socket: impl Into<String>, actor: impl Into<String>) -> Self {
        Self {
            socket: socket.into(),
            actor: actor.into(),
        }
    }

    fn from_process() -> Self {
        Self {
            socket: std::env::var("MIND_SOCKET")
                .unwrap_or_else(|_| String::from(DEFAULT_MIND_SOCKET)),
            actor: std::env::var("MIND_ACTOR").unwrap_or_else(|_| String::from(DEFAULT_MIND_ACTOR)),
        }
    }

    fn endpoint(&self) -> MindDaemonEndpoint {
        MindDaemonEndpoint::new(PathBuf::from(&self.socket))
    }

    fn actor(&self) -> ActorName {
        ActorName::new(self.actor.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SubmissionCommand {
    endpoint: MindDaemonEndpoint,
    actor: ActorName,
    request: MindRequest,
}

impl SubmissionCommand {
    fn from_command(
        command: ComponentCommand,
        environment: MindCommandEnvironment,
    ) -> Result<Self> {
        let request =
            CommandRequest::from_source(CommandInputSource::from_command(command)?)?.into_request();
        Ok(Self {
            endpoint: environment.endpoint(),
            actor: environment.actor(),
            request,
        })
    }

    async fn run(self, mut output: impl Write) -> Result<()> {
        let reply = MindClient::new(self.endpoint, self.actor)
            .submit(self.request)
            .await?;
        writeln!(output, "{}", CommandReply::new(reply).to_nota()?)?;
        Ok(())
    }
}

struct CommandInputSource {
    text: String,
}

impl CommandInputSource {
    fn from_command(command: ComponentCommand) -> Result<Self> {
        match command.nota_argument()? {
            ComponentArgument::InlineNota(argument) => Ok(Self::new(argument.into_string())),
            ComponentArgument::NotaFile(file) => {
                let path = file.into_path();
                fs::read_to_string(&path)
                    .map(Self::new)
                    .map_err(|source| Error::ReadNotaFile { path, source })
            }
            ComponentArgument::SignalFile(file) => {
                let path = file.into_path();
                fs::read_to_string(&path)
                    .map(Self::new)
                    .map_err(|source| Error::ReadNotaFile { path, source })
            }
        }
    }

    fn new(text: String) -> Self {
        Self { text }
    }

    fn text(&self) -> &str {
        &self.text
    }
}

struct CommandRequest {
    request: MindRequest,
}

impl CommandRequest {
    fn from_source(source: CommandInputSource) -> Result<Self> {
        if let Ok(text_request) = MindTextRequest::from_nota(source.text()) {
            return Ok(Self {
                request: text_request.into_request()?,
            });
        }

        let request = NotaSource::new(source.text()).parse::<MindRequest>()?;
        Ok(Self { request })
    }

    fn into_request(self) -> MindRequest {
        self.request
    }
}

struct CommandReply {
    reply: MindReply,
}

impl CommandReply {
    fn new(reply: MindReply) -> Self {
        Self { reply }
    }

    fn to_nota(&self) -> Result<String> {
        if let Ok(text_reply) = MindTextReply::from_reply(self.reply.clone()) {
            return text_reply.to_nota();
        }

        Ok(self.reply.to_nota())
    }
}
