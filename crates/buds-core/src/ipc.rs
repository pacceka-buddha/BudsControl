//! Talking to a running tray app from `budsctl`: a local socket (a named pipe on Windows, a Unix socket
//! elsewhere) carrying the same commands as the command line.
//!
//! Protocol: the client sends one JSON line `{"args":["status"]}`; the server answers with JSON lines
//! `{"line":"..."}` for each line of output and finally `{"exit":0}`. The server runs the very same
//! [`cli::execute`] against its live controller, so the output is identical to a direct run.
//!
//! Security: the socket belongs to the user who runs the app. On Windows the name contains the user name
//! and the pipe's default access rights let only that user, administrators and SYSTEM write to it. On
//! Unix it is a socket file (mode 0600) in `$XDG_RUNTIME_DIR` (a private directory) or, failing that, in
//! a private (0700) directory under the temp folder; abstract sockets are avoided because they have no
//! access control.

use std::io::{self, Write};
use std::time::Duration;

#[cfg(unix)]
use interprocess::local_socket::GenericFilePath;
#[cfg(not(unix))]
use interprocess::local_socket::GenericNamespaced;
use interprocess::local_socket::tokio::{Stream, prelude::*};
use interprocess::local_socket::{ListenerOptions, Name};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;

use crate::cli;
use crate::controller::ControllerHandle;

/// A request longer than this is refused.
const MAX_REQUEST: usize = 64 * 1024;
/// How long the client waits to connect before deciding no app is running.
const CONNECT_TIMEOUT: Duration = Duration::from_millis(800);

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
struct Request {
    args: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum Message {
    Line(String),
    Exit(i32),
}

/// The default socket name for this user.
pub fn default_name() -> String {
    let user = std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "user".into());
    let safe: String = user
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("budscontrol-{safe}")
}

#[cfg(not(unix))]
fn socket_name(name: &str) -> io::Result<Name<'static>> {
    name.to_owned().to_ns_name::<GenericNamespaced>()
}

/// The socket file for `name`: in `$XDG_RUNTIME_DIR` (already private to the user), else in a private
/// directory below the temp folder.
#[cfg(unix)]
fn socket_path(name: &str) -> io::Result<std::path::PathBuf> {
    use std::os::unix::fs::DirBuilderExt;
    let dir = match std::env::var_os("XDG_RUNTIME_DIR").filter(|d| !d.is_empty()) {
        Some(dir) => std::path::PathBuf::from(dir),
        None => {
            let dir = std::env::temp_dir().join(format!("{}-private", default_name()));
            std::fs::DirBuilder::new()
                .mode(0o700)
                .recursive(true)
                .create(&dir)?;
            dir
        }
    };
    Ok(dir.join(format!("{name}.sock")))
}

#[cfg(unix)]
fn socket_name(name: &str) -> io::Result<Name<'static>> {
    socket_path(name)?
        .into_os_string()
        .to_fs_name::<GenericFilePath>()
}

/// Serves commands for `handle` on the default socket until the task is dropped.
pub async fn serve(handle: ControllerHandle) -> io::Result<()> {
    serve_on(&default_name(), handle).await
}

pub async fn serve_on(name: &str, handle: ControllerHandle) -> io::Result<()> {
    let listener = ListenerOptions::new()
        .name(socket_name(name)?)
        .try_overwrite(true)
        .create_tokio()?;
    // Only the user may talk to the app (the directory is private already; this is the second lock).
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ =
            std::fs::set_permissions(socket_path(name)?, std::fs::Permissions::from_mode(0o600));
    }
    loop {
        let connection = match listener.accept().await {
            Ok(c) => c,
            Err(_) => continue,
        };
        let handle = handle.clone();
        tokio::spawn(async move {
            let _ = handle_connection(connection, handle).await;
        });
    }
}

async fn handle_connection(connection: Stream, handle: ControllerHandle) -> io::Result<()> {
    let mut reader = BufReader::new(&connection);
    let mut sender = &connection;

    let mut line = String::new();
    let read = (&mut reader)
        .take(MAX_REQUEST as u64)
        .read_line(&mut line)
        .await;
    let request: Request = match read {
        Ok(n) if n > 0 => match serde_json::from_str(line.trim()) {
            Ok(r) => r,
            Err(e) => {
                return send(
                    &mut sender,
                    &Message::Line(format!("bad request: {e}")),
                    Some(2),
                )
                .await;
            }
        },
        _ => return Ok(()),
    };

    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    let task = tokio::spawn(async move {
        let mut writer = LineWriter::new(tx);
        let code = cli::execute(&handle, &request.args, &mut writer).await;
        writer.finish();
        code
    });
    while let Some(text) = rx.recv().await {
        send(&mut sender, &Message::Line(text), None).await?;
    }
    let code = task.await.unwrap_or(1);
    send(&mut sender, &Message::Exit(code), None).await
}

async fn send(sender: &mut &Stream, message: &Message, then_exit: Option<i32>) -> io::Result<()> {
    let mut bytes = serde_json::to_vec(message).map_err(io::Error::other)?;
    bytes.push(b'\n');
    sender.write_all(&bytes).await?;
    if let Some(code) = then_exit {
        let mut bytes = serde_json::to_vec(&Message::Exit(code)).map_err(io::Error::other)?;
        bytes.push(b'\n');
        sender.write_all(&bytes).await?;
    }
    sender.flush().await
}

/// Collects output and sends it line by line.
struct LineWriter {
    tx: mpsc::UnboundedSender<String>,
    pending: Vec<u8>,
}

impl LineWriter {
    fn new(tx: mpsc::UnboundedSender<String>) -> LineWriter {
        LineWriter {
            tx,
            pending: Vec::new(),
        }
    }

    /// Sends a trailing line that had no newline.
    fn finish(&mut self) {
        if !self.pending.is_empty() {
            let _ = self
                .tx
                .send(String::from_utf8_lossy(&self.pending).into_owned());
            self.pending.clear();
        }
    }
}

impl Write for LineWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.pending.extend_from_slice(buf);
        while let Some(end) = self.pending.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = self.pending.drain(..=end).collect();
            let text = String::from_utf8_lossy(&line[..line.len() - 1]).into_owned();
            let _ = self.tx.send(text);
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Runs `args` in the app listening on the default socket, writing its output to `out`.
/// `None` means no app is listening (run the command yourself).
pub async fn forward(args: &[String], out: &mut dyn Write) -> Option<io::Result<i32>> {
    forward_on(&default_name(), args, out).await
}

pub async fn forward_on(
    name: &str,
    args: &[String],
    out: &mut dyn Write,
) -> Option<io::Result<i32>> {
    let name = socket_name(name).ok()?;
    let connection = tokio::time::timeout(CONNECT_TIMEOUT, Stream::connect(name))
        .await
        .ok()?
        .ok()?;
    Some(exchange(connection, args, out).await)
}

async fn exchange(connection: Stream, args: &[String], out: &mut dyn Write) -> io::Result<i32> {
    let mut reader = BufReader::new(&connection);
    let mut sender = &connection;
    let mut request = serde_json::to_vec(&Request {
        args: args.to_vec(),
    })
    .map_err(io::Error::other)?;
    request.push(b'\n');
    sender.write_all(&request).await?;
    sender.flush().await?;

    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line).await? == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "the app closed the connection",
            ));
        }
        match serde_json::from_str::<Message>(line.trim()).map_err(io::Error::other)? {
            Message::Line(text) => {
                writeln!(out, "{text}")?;
                out.flush()?;
            }
            Message::Exit(code) => return Ok(code),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_use_the_documented_json_shape() {
        assert_eq!(
            serde_json::to_string(&Message::Line("hi".into())).unwrap(),
            r#"{"line":"hi"}"#
        );
        assert_eq!(
            serde_json::to_string(&Message::Exit(3)).unwrap(),
            r#"{"exit":3}"#
        );
        let r: Request = serde_json::from_str(r#"{"args":["status","x"]}"#).unwrap();
        assert_eq!(r.args, ["status", "x"]);
    }

    #[test]
    fn the_default_name_is_per_user_and_safe() {
        let name = default_name();
        assert!(name.starts_with("budscontrol-"));
        assert!(
            name.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        );
    }

    #[test]
    fn the_line_writer_splits_on_newlines() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut w = LineWriter::new(tx);
        w.write_all(b"one\ntwo").unwrap();
        w.write_all(b" more\nthree").unwrap();
        w.finish();
        drop(w);
        let mut lines = Vec::new();
        while let Ok(l) = rx.try_recv() {
            lines.push(l);
        }
        assert_eq!(lines, ["one", "two more", "three"]);
    }
}
