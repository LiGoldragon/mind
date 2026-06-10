use std::{
    fs,
    path::{Path, PathBuf},
};

use mind::{ConfigurationError, MindDaemonConfiguration};
use nota_next::{Delimiter, NotaBlock, NotaDecode, NotaDecodeError, NotaEncode, NotaSource};
use signal_mind::WirePath;
use thiserror::Error;
use triad_runtime::{ArgumentError, ComponentArgument, ComponentCommand};

fn main() {
    if let Err(error) = ConfigurationWriterCommand::from_environment().run() {
        eprintln!("mind-write-configuration: {error}");
        std::process::exit(1);
    }
}

struct ConfigurationWriterCommand {
    command: ComponentCommand,
}

struct ConfigurationWriterInput {
    text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ConfigurationWriteRequest {
    socket_path: ConfigurationWriterPath,
    meta_socket_path: ConfigurationWriterPath,
    store_path: ConfigurationWriterPath,
    output_path: ConfigurationWriterPath,
}

#[derive(Debug, Clone, PartialEq, Eq, NotaDecode, NotaEncode)]
struct ConfigurationWriterPath(String);

#[derive(Debug, Clone, PartialEq, Eq)]
struct ConfigurationWriteOutput {
    output_path: ConfigurationWriterPath,
}

impl ConfigurationWriterCommand {
    fn from_environment() -> Self {
        Self {
            command: ComponentCommand::from_environment(),
        }
    }

    fn run(&self) -> Result<(), ConfigurationWriterError> {
        let source = self.source()?;
        let request = source.parse_request()?;
        let output = request.write()?;
        println!("{}", output.to_nota());
        Ok(())
    }

    fn source(&self) -> Result<ConfigurationWriterInput, ConfigurationWriterError> {
        match self.command.nota_argument()? {
            ComponentArgument::InlineNota(argument) => {
                Ok(ConfigurationWriterInput::new(argument.into_string()))
            }
            ComponentArgument::NotaFile(file) => {
                let path = file.into_path();
                fs::read_to_string(&path)
                    .map(ConfigurationWriterInput::new)
                    .map_err(|source| ConfigurationWriterError::ReadNotaFile { path, source })
            }
            ComponentArgument::SignalFile(file) => Err(ConfigurationWriterError::SignalInput {
                path: file.into_path(),
            }),
        }
    }
}

impl ConfigurationWriterInput {
    fn new(text: String) -> Self {
        Self { text }
    }

    fn parse_request(&self) -> Result<ConfigurationWriteRequest, NotaDecodeError> {
        NotaSource::new(&self.text).parse()
    }
}

impl ConfigurationWriteRequest {
    fn write(self) -> Result<ConfigurationWriteOutput, ConfigurationWriterError> {
        let output_path = self.output_path.clone();
        fs::write(
            output_path.as_path(),
            self.configuration()?.to_signal_bytes()?,
        )
        .map_err(|source| ConfigurationWriterError::WriteArchive {
            path: output_path.path_buf(),
            source,
        })?;
        Ok(ConfigurationWriteOutput { output_path })
    }

    fn configuration(self) -> Result<MindDaemonConfiguration, ConfigurationWriterError> {
        Ok(MindDaemonConfiguration::new(
            self.store_path.into_wire_path()?,
            self.socket_path.into_wire_path()?,
            self.meta_socket_path.into_wire_path()?,
        ))
    }
}

impl NotaDecode for ConfigurationWriteRequest {
    fn from_nota_block(block: &nota_next::Block) -> Result<Self, NotaDecodeError> {
        let body = NotaBlock::new(block)
            .expect_body(Delimiter::Parenthesis, "ConfigurationWriteRequest")?;
        let objects = body.root_objects();
        if objects.len() != 5 {
            return Err(NotaDecodeError::ExpectedRootCount {
                type_name: "ConfigurationWriteRequest",
                expected: 5,
                found: objects.len(),
            });
        }
        match objects[0].demote_to_string() {
            Some("ConfigurationWriteRequest") => {}
            Some(variant) => {
                return Err(NotaDecodeError::UnknownVariant {
                    enum_name: "ConfigurationWriteRequest",
                    variant: variant.to_owned(),
                });
            }
            None => {
                return Err(NotaDecodeError::ExpectedAtom {
                    type_name: "ConfigurationWriteRequest",
                });
            }
        }
        Ok(Self {
            socket_path: ConfigurationWriterPath::from_nota_block(&objects[1])?,
            meta_socket_path: ConfigurationWriterPath::from_nota_block(&objects[2])?,
            store_path: ConfigurationWriterPath::from_nota_block(&objects[3])?,
            output_path: ConfigurationWriterPath::from_nota_block(&objects[4])?,
        })
    }
}

impl NotaEncode for ConfigurationWriteOutput {
    fn to_nota(&self) -> String {
        Delimiter::Parenthesis.wrap([
            String::from("ConfigurationWritten"),
            self.output_path.to_nota(),
        ])
    }
}

impl ConfigurationWriterPath {
    fn as_path(&self) -> &Path {
        Path::new(&self.0)
    }

    fn path_buf(&self) -> PathBuf {
        self.as_path().to_path_buf()
    }

    fn into_wire_path(self) -> Result<WirePath, ConfigurationWriterError> {
        WirePath::from_absolute_path(self.0).map_err(ConfigurationWriterError::WirePath)
    }
}

#[derive(Debug, Error)]
enum ConfigurationWriterError {
    #[error(transparent)]
    Argument(#[from] ArgumentError),

    #[error("read NOTA file {}: {source}", path.display())]
    ReadNotaFile {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("signal-encoded configuration writer input is unsupported: {}", path.display())]
    SignalInput { path: PathBuf },

    #[error("write binary archive {}: {source}", path.display())]
    WriteArchive {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error(transparent)]
    Configuration(#[from] ConfigurationError),

    #[error(transparent)]
    WirePath(#[from] signal_mind::Error),

    #[error(transparent)]
    Nota(#[from] NotaDecodeError),
}
