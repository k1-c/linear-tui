//! The control channel: how an agent works a running linear-tui the way a
//! person does — reading the screen, pressing keys, running commands.
//!
//! A running instance listens on a local TCP port (loopback only) and
//! writes where, with a token, to `<pid>.control` beside its view
//! snapshot: a file only the user can read. `linear-tui tui …` finds the
//! instance of the current repository (`usecase::instance::to_report`),
//! reads the file, and sends one command per connection as a line of JSON;
//! the answer is a line of JSON back. TCP rather than a Unix socket, so the
//! same code runs on Windows.
//!
//! The main loop carries a command out between frames, and answers once
//! Linear has answered what the command asked for, so what comes back is
//! the screen a person would read once the spinner stops.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{mpsc, oneshot};

use crate::interface::screen::ScreenReport;

/// What an agent asks of a running linear-tui.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum Command {
    /// Report the screen.
    Screen,
    /// Press keys, in `interface::notation`.
    Press { keys: String },
    /// Type text into the field that has focus.
    Type { text: String },
    /// Run a palette command by its title.
    Run { title: String },
    /// Open an issue by its identifier.
    Open { issue: String },
    /// Quit linear-tui.
    Quit,
}

/// The answer: the screen after the command, or why it was not carried out.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reply {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub screen: Option<serde_json::Value>,
}

impl Reply {
    pub fn screen(report: &ScreenReport) -> Self {
        Self {
            error: None,
            screen: serde_json::to_value(report).ok(),
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            error: Some(message.into()),
            screen: None,
        }
    }

    pub fn done() -> Self {
        Self {
            error: None,
            screen: None,
        }
    }
}

/// One line on the wire: a command and the token that proves who sent it.
#[derive(Debug, Serialize, Deserialize)]
struct Envelope {
    token: String,
    #[serde(flatten)]
    command: Command,
}

/// Where a running instance listens, as its control file says.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Endpoint {
    pub port: u16,
    pub token: String,
}

/// A command waiting for the main loop, and where its answer goes.
pub type Asked = (Command, oneshot::Sender<Reply>);

/// The control file of instance `pid`, beside its snapshot.
pub fn endpoint_path(snapshot: &Path) -> PathBuf {
    snapshot.with_extension("control")
}

/// Listen for commands, and say where in `path`. The commands arrive on
/// the returned channel for the main loop to carry out; the file goes when
/// the returned guard is dropped.
pub async fn listen(path: PathBuf) -> Result<(mpsc::UnboundedReceiver<Asked>, EndpointFile)> {
    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .context("listening for agents on the loopback interface")?;
    let endpoint = Endpoint {
        port: listener.local_addr()?.port(),
        token: token(),
    };
    crate::adapter::private_file::write(&path, serde_json::to_string(&endpoint)?.as_bytes())?;
    let (tx, rx) = mpsc::unbounded_channel();
    let token = endpoint.token.clone();
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let tx = tx.clone();
            let token = token.clone();
            tokio::spawn(async move {
                if let Err(e) = serve(stream, &token, tx).await {
                    tracing::debug!("control connection: {e:#}");
                }
            });
        }
    });
    Ok((rx, EndpointFile(path)))
}

/// Removes the control file when the instance stops listening.
pub struct EndpointFile(PathBuf);

impl Drop for EndpointFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// A fresh random token.
fn token() -> String {
    use rand::Rng;
    let bytes: [u8; 16] = rand::rng().random();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Answer one connection: a command in, the main loop's reply out.
async fn serve(stream: TcpStream, token: &str, tx: mpsc::UnboundedSender<Asked>) -> Result<()> {
    let (read, mut write) = stream.into_split();
    let mut line = String::new();
    BufReader::new(read).read_line(&mut line).await?;
    let reply = match serde_json::from_str::<Envelope>(&line) {
        Err(e) => Reply::error(format!("not a command: {e}")),
        Ok(envelope) if envelope.token != token => Reply::error("wrong token"),
        Ok(envelope) => {
            let (answer, answered) = oneshot::channel();
            tx.send((envelope.command, answer))
                .map_err(|_| anyhow::anyhow!("linear-tui is quitting"))?;
            answered
                .await
                .unwrap_or_else(|_| Reply::error("linear-tui quit before answering"))
        }
    };
    let mut out = serde_json::to_string(&reply)?;
    out.push('\n');
    write.write_all(out.as_bytes()).await?;
    Ok(())
}

/// How long `send` waits for an answer. The main loop answers within its
/// own wait for Linear, so this only trips when the instance is stuck.
const SEND_TIMEOUT: Duration = Duration::from_secs(60);

/// Send `command` to the instance whose control file is `path`.
pub async fn send(path: &Path, command: Command) -> Result<Reply> {
    let text = std::fs::read_to_string(path).with_context(|| {
        format!(
            "linear-tui is not accepting commands here ({} is missing)",
            path.display()
        )
    })?;
    let endpoint: Endpoint = serde_json::from_str(&text)?;
    let stream = TcpStream::connect(SocketAddr::from(([127, 0, 0, 1], endpoint.port)))
        .await
        .context("linear-tui did not answer on its control port")?;
    let (read, mut write) = stream.into_split();
    let mut line = serde_json::to_string(&Envelope {
        token: endpoint.token,
        command,
    })?;
    line.push('\n');
    write.write_all(line.as_bytes()).await?;
    let mut answer = String::new();
    tokio::time::timeout(SEND_TIMEOUT, BufReader::new(read).read_line(&mut answer))
        .await
        .context("linear-tui did not answer in time")??;
    if answer.is_empty() {
        bail!("linear-tui closed the connection without answering");
    }
    Ok(serde_json::from_str(&answer)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("linear-tui-control-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("1.json")
    }

    #[test]
    fn a_command_is_one_line_of_json_with_its_token() {
        let line = serde_json::to_string(&Envelope {
            token: "t".into(),
            command: Command::Press { keys: "g m".into() },
        })
        .unwrap();
        assert_eq!(line, r#"{"token":"t","command":"press","keys":"g m"}"#);
    }

    #[tokio::test]
    async fn a_command_reaches_the_main_loop_and_its_answer_comes_back() {
        let path = endpoint_path(&scratch("roundtrip"));
        let (mut rx, file) = listen(path.clone()).await.unwrap();
        let loop_side = tokio::spawn(async move {
            let (command, answer) = rx.recv().await.unwrap();
            assert_eq!(
                command,
                Command::Run {
                    title: "Refresh".into()
                }
            );
            answer.send(Reply::error("no view yet")).unwrap();
        });
        let reply = send(
            &path,
            Command::Run {
                title: "Refresh".into(),
            },
        )
        .await
        .unwrap();
        assert_eq!(reply.error.as_deref(), Some("no view yet"));
        loop_side.await.unwrap();
        drop(file);
        assert!(!path.exists(), "the control file goes with the listener");
    }

    #[tokio::test]
    async fn a_wrong_token_is_refused_before_the_main_loop_sees_it() {
        let path = endpoint_path(&scratch("token"));
        let (mut rx, _file) = listen(path.clone()).await.unwrap();
        let mut endpoint: Endpoint =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        endpoint.token = "forged".into();
        std::fs::write(&path, serde_json::to_string(&endpoint).unwrap()).unwrap();
        let reply = send(&path, Command::Screen).await.unwrap();
        assert_eq!(reply.error.as_deref(), Some("wrong token"));
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn no_control_file_says_linear_tui_is_not_listening() {
        let error = send(Path::new("/nonexistent/1.control"), Command::Screen)
            .await
            .unwrap_err();
        assert!(
            format!("{error:#}").contains("not accepting commands"),
            "{error:#}"
        );
    }
}
