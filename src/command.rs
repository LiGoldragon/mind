use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;

use nota_next::{NotaEncode, NotaSource};
use signal_mind::{ActorName, MindReply, MindRequest};

use crate::{Error, MindClient, MindDaemonEndpoint, MindTextReply, MindTextRequest, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MindCommand {
    arguments: Vec<OsString>,
}

impl MindCommand {
    pub fn from_env() -> Self {
        Self::from_arguments(std::env::args_os().skip(1))
    }

    pub fn from_arguments<I, S>(arguments: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        Self {
            arguments: arguments.into_iter().map(Into::into).collect(),
        }
    }

    /// The `mind` CLI is purely a daemon *client*: it encodes one NOTA request,
    /// sends it to a long-lived `mind-daemon` over the working socket, and prints
    /// the reply. The daemon process itself is the schema-emitted `mind-daemon`
    /// binary (single binary rkyv startup, no flags) — the CLI never launches it.
    pub async fn run(self, output: impl Write) -> Result<()> {
        match self.into_action()? {
            MindAction::Submit(submission) => submission.run(output).await,
        }
    }

    fn into_action(self) -> Result<MindAction> {
        if self.arguments.is_empty() {
            return Err(Error::MissingCommandInput);
        }
        Ok(MindAction::Submit(SubmissionCommand::from_arguments(
            self.arguments,
        )?))
    }
}

enum MindAction {
    Submit(SubmissionCommand),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmissionCommand {
    endpoint: MindDaemonEndpoint,
    actor: ActorName,
    request: MindRequest,
}

impl SubmissionCommand {
    fn from_arguments<I>(arguments: I) -> Result<Self>
    where
        I: IntoIterator<Item = OsString>,
    {
        let options = ParsedOptions::from_arguments(arguments)?;
        let request = CommandRequest::from_nota(&options.request()?)?.into_request();
        Ok(Self {
            endpoint: options.endpoint()?,
            actor: options.actor()?,
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

struct CommandRequest {
    request: MindRequest,
}

impl CommandRequest {
    fn from_nota(text: &str) -> Result<Self> {
        if let Ok(text_request) = MindTextRequest::from_nota(text) {
            return Ok(Self {
                request: text_request.into_request()?,
            });
        }

        let request = NotaSource::new(text).parse::<MindRequest>()?;
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedOptions {
    socket: Option<PathBuf>,
    actor: Option<ActorName>,
    requests: Vec<String>,
}

impl ParsedOptions {
    fn from_arguments<I>(arguments: I) -> Result<Self>
    where
        I: IntoIterator<Item = OsString>,
    {
        let mut parser = OptionParser::new(arguments);
        parser.parse()
    }

    fn endpoint(&self) -> Result<MindDaemonEndpoint> {
        self.socket
            .clone()
            .map(MindDaemonEndpoint::new)
            .ok_or(Error::MissingSocketPath)
    }

    fn actor(&self) -> Result<ActorName> {
        self.actor.clone().ok_or(Error::MissingActorName)
    }

    fn request(&self) -> Result<String> {
        match self.requests.as_slice() {
            [request] => Ok(request.clone()),
            _ => Err(Error::WrongRequestArgumentCount {
                count: self.requests.len(),
            }),
        }
    }
}

struct OptionParser {
    arguments: Vec<OsString>,
}

impl OptionParser {
    fn new<I>(arguments: I) -> Self
    where
        I: IntoIterator<Item = OsString>,
    {
        Self {
            arguments: arguments.into_iter().collect(),
        }
    }

    fn parse(&mut self) -> Result<ParsedOptions> {
        let mut socket = None;
        let mut actor = None;
        let mut requests = Vec::new();
        let mut index = 0;

        while index < self.arguments.len() {
            let argument = CommandArgument::new(&self.arguments[index]);
            if argument.matches("--socket") {
                socket = Some(PathBuf::from(self.option_value(index, "--socket")?));
                index += 2;
            } else if argument.matches("--actor") {
                actor = Some(ActorName::new(self.option_value(index, "--actor")?));
                index += 2;
            } else if argument.starts_with_option() {
                return Err(Error::UnknownCommandLineOption {
                    option: argument.into_string()?,
                });
            } else {
                requests.push(argument.into_string()?);
                index += 1;
            }
        }

        Ok(ParsedOptions {
            socket,
            actor,
            requests,
        })
    }

    fn option_value(&self, option_index: usize, option: &str) -> Result<String> {
        let value_index = option_index + 1;
        let Some(value) = self.arguments.get(value_index) else {
            return Err(Error::MissingCommandLineOptionValue {
                option: option.to_string(),
            });
        };
        CommandArgument::new(value).into_string()
    }
}

struct CommandArgument<'argument> {
    value: &'argument OsString,
}

impl<'argument> CommandArgument<'argument> {
    fn new(value: &'argument OsString) -> Self {
        Self { value }
    }

    fn matches(&self, expected: &str) -> bool {
        self.value.to_str() == Some(expected)
    }

    fn starts_with_option(&self) -> bool {
        self.value
            .to_str()
            .is_some_and(|argument| argument.starts_with("--"))
    }

    fn into_string(self) -> Result<String> {
        self.value.to_str().map(ToOwned::to_owned).ok_or_else(|| {
            Error::InvalidCommandLineArgument {
                argument: format!("{:?}", self.value),
            }
        })
    }
}
