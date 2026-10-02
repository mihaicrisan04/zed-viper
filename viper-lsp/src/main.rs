//! Bridges an editor speaking LSP over stdio to ViperServer, which only speaks LSP over TCP.
//!
//! Besides passing messages through, it fills the gaps a plain LSP client leaves:
//! - sends ViperServer's custom `Verify` notification when a file is opened or saved
//! - answers ViperServer's custom requests that only the VS Code extension knows
//! - turns ViperServer's custom `StateChange` notifications into standard `$/progress`

use serde_json::{Value, json};
use std::io::{self, BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;

const SHIM_REQUEST_PREFIX: &str = "viper-lsp/";

type Sink = Arc<Mutex<Box<dyn Write + Send>>>;

fn log(message: impl AsRef<str>) {
    eprintln!("[viper-lsp] {}", message.as_ref());
}

struct Config {
    java: String,
    viper_home: PathBuf,
    backend: String,
    server_log: PathBuf,
}

impl Config {
    fn from_env() -> Config {
        let home = std::env::var("HOME").unwrap_or_default();
        Config {
            java: std::env::var("VIPER_JAVA").unwrap_or_else(|_| "java".into()),
            viper_home: std::env::var_os("VIPER_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| Path::new(&home).join("opt/ViperTools")),
            backend: std::env::var("VIPER_BACKEND").unwrap_or_else(|_| "silicon".into()),
            server_log: std::env::temp_dir().join("viper-lsp-server.log"),
        }
    }

    fn tool(&self, relative: &str) -> String {
        self.viper_home.join(relative).display().to_string()
    }

    fn server_command(&self) -> Command {
        let mut command = Command::new(&self.java);
        command
            .args(["-Xmx2048m", "-Xss128m", "-cp"])
            .arg(self.tool("backends/viperserver.jar"))
            .args(["-server", "viper.server.ViperServerRunner"])
            .args(["--serverMode", "LSP", "--singleClient", "--logLevel", "INFO", "--logFile"])
            .arg(&self.server_log);
        command
    }

    /// Same arguments the VS Code extension passes for its built-in backends.
    fn backend_arguments(&self) -> String {
        let exe = std::env::consts::EXE_SUFFIX;
        let z3 = format!("--z3Exe \"{}\"", self.tool(&format!("z3/bin/z3{exe}")));
        match self.backend.as_str() {
            "carbon" => format!("{z3} --boogieExe \"{}\"", self.tool(&format!("boogie/Binaries/Boogie{exe}"))),
            _ => z3,
        }
    }
}

/// Starts ViperServer and waits for the `<ViperServerPort:N>` line it prints once it listens.
fn start_server(config: &Config) -> io::Result<(Child, u16)> {
    let mut server = config.server_command().stdout(Stdio::piped()).spawn().map_err(|error| {
        io::Error::new(error.kind(), format!("could not run `{}`: {error}", config.java))
    })?;
    let mut stdout = BufReader::new(server.stdout.take().expect("stdout is piped"));
    let mut line = String::new();
    while stdout.read_line(&mut line)? > 0 {
        if let Some(port) = parse_port(&line) {
            // keep draining stdout so ViperServer never blocks on a full pipe
            thread::spawn(move || {
                for line in stdout.lines().map_while(Result::ok) {
                    log(format!("server: {line}"));
                }
            });
            return Ok((server, port));
        }
        log(format!("server: {}", line.trim_end()));
        line.clear();
    }
    Err(io::Error::other(format!(
        "ViperServer exited before reporting its port, see {}",
        config.server_log.display()
    )))
}

fn parse_port(line: &str) -> Option<u16> {
    let start = line.find("<ViperServerPort:")? + "<ViperServerPort:".len();
    let end = start + line[start..].find('>')?;
    line[start..end].parse().ok()
}

fn read_message(reader: &mut impl BufRead) -> io::Result<Option<Value>> {
    let mut length = None;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        let line = line.trim_end();
        if line.is_empty() {
            if length.is_some() {
                break;
            }
            continue;
        }
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            length = value.trim().parse::<usize>().ok();
        }
    }
    let mut body = vec![0; length.expect("loop only exits with a length")];
    reader.read_exact(&mut body)?;
    Ok(Some(serde_json::from_slice(&body)?))
}

fn send(sink: &Sink, message: &Value) {
    let body = message.to_string();
    let mut writer = sink.lock().unwrap();
    let result = write!(writer, "Content-Length: {}\r\n\r\n{body}", body.len()).and_then(|_| writer.flush());
    if let Err(error) = result {
        log(format!("write failed: {error}"));
    }
}

fn notification(method: &str, params: Value) -> Value {
    json!({ "jsonrpc": "2.0", "method": method, "params": params })
}

fn response(id: &Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

/// Standard LSP methods are namespaced (`textDocument/...`, `$/...`); ViperServer's custom ones are not.
fn is_custom_method(method: &str) -> bool {
    !method.contains('/')
}

fn uri_to_path(uri: &str) -> PathBuf {
    let encoded = uri.strip_prefix("file://").unwrap_or(uri);
    let mut bytes = Vec::with_capacity(encoded.len());
    let mut rest = encoded.as_bytes();
    while let Some((&byte, tail)) = rest.split_first() {
        let escaped = (byte == b'%')
            .then(|| tail.get(..2))
            .flatten()
            .and_then(|hex| u8::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok());
        match escaped {
            Some(decoded) => {
                bytes.push(decoded);
                rest = &tail[2..];
            }
            None => {
                bytes.push(byte);
                rest = tail;
            }
        }
    }
    let decoded = String::from_utf8_lossy(&bytes).into_owned();
    // `file:///C:/x` on Windows: drop the slash in front of the drive letter
    let is_drive_path = decoded.as_bytes().get(2) == Some(&b':') && decoded.starts_with('/');
    PathBuf::from(if is_drive_path { &decoded[1..] } else { &decoded })
}

/// One `$/progress` token per verification, shown by the editor as a status bar spinner.
#[derive(Default)]
struct Progress {
    requests_sent: u64,
    token: Option<String>,
}

impl Progress {
    fn begin(&mut self, editor: &Sink, title: &str) {
        self.end(editor, None);
        self.requests_sent += 1;
        let token = format!("{SHIM_REQUEST_PREFIX}verification-{}", self.requests_sent);
        send(editor, &json!({
            "jsonrpc": "2.0",
            "id": format!("{SHIM_REQUEST_PREFIX}{}", self.requests_sent),
            "method": "window/workDoneProgress/create",
            "params": { "token": token },
        }));
        send(editor, &notification("$/progress", json!({
            "token": token,
            "value": { "kind": "begin", "title": title, "cancellable": false, "percentage": 0 },
        })));
        self.token = Some(token);
    }

    fn report(&mut self, editor: &Sink, percentage: f64) {
        if let Some(token) = &self.token {
            send(editor, &notification("$/progress", json!({
                "token": token,
                "value": { "kind": "report", "percentage": percentage.clamp(0.0, 100.0) as u32 },
            })));
        }
    }

    fn end(&mut self, editor: &Sink, message: Option<String>) {
        if let Some(token) = self.token.take() {
            send(editor, &notification("$/progress", json!({
                "token": token,
                "value": { "kind": "end", "message": message },
            })));
        }
    }
}

// ViperServer's `VerificationState` and `VerificationSuccess` enums (DataProtocol.scala)
const STATE_STOPPED: i64 = 0;
const STATE_VERIFICATION_RUNNING: i64 = 2;
const STATE_READY: i64 = 6;
const STATE_STOPPING: i64 = 7;

fn outcome(success: i64, seconds: f64) -> Option<String> {
    let outcome = match success {
        1 => "verified",
        2 => "parsing failed",
        3 => "type checking failed",
        4 => "verification failed",
        5 => "aborted",
        6 => "internal error",
        7 => "timed out",
        _ => return None,
    };
    Some(if seconds >= 0.0 { format!("{outcome} in {seconds:.1}s") } else { outcome.into() })
}

struct Bridge {
    config: Config,
    editor: Sink,
    server: Sink,
    progress: Mutex<Progress>,
}

impl Bridge {
    fn verify(&self, uri: &str, content: String) {
        let path = uri_to_path(uri);
        let workspace = path.parent().unwrap_or(Path::new("/")).display().to_string();
        let file_name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
        self.progress.lock().unwrap().begin(&self.editor, &format!("{} {file_name}", self.config.backend));
        send(&self.server, &notification("Verify", json!({
            "uri": uri,
            "content": content,
            "manuallyTriggered": true,
            "workspace": workspace,
            "backend": self.config.backend,
            "customArgs": self.config.backend_arguments(),
        })));
    }

    fn handle_editor_message(&self, message: Value) {
        let is_reply_to_shim = message.get("method").is_none()
            && message["id"].as_str().is_some_and(|id| id.starts_with(SHIM_REQUEST_PREFIX));
        if is_reply_to_shim {
            return;
        }
        send(&self.server, &message);

        let params = &message["params"];
        let uri = params["textDocument"]["uri"].as_str().unwrap_or_default();
        match message["method"].as_str() {
            Some("textDocument/didOpen") => {
                self.verify(uri, params["textDocument"]["text"].as_str().unwrap_or_default().into())
            }
            Some("textDocument/didSave") => {
                let content = params["text"].as_str().map(String::from)
                    .or_else(|| std::fs::read_to_string(uri_to_path(uri)).ok())
                    .unwrap_or_default();
                self.verify(uri, content)
            }
            _ => {}
        }
    }

    fn handle_server_message(&self, message: Value) {
        let Some(method) = message["method"].as_str() else {
            send(&self.editor, &message);
            return;
        };
        if !is_custom_method(method) {
            send(&self.editor, &message);
            return;
        }

        let params = &message["params"];
        match (method, message.get("id")) {
            ("GetViperFileEndings", Some(id)) => {
                send(&self.server, &response(id, json!({ "fileEndings": [".vpr"] })))
            }
            // GetIdentifier, GetRange and SetupProject: VS Code-only conveniences, safe to decline
            (_, Some(id)) => send(&self.server, &response(id, Value::Null)),
            ("StateChange", None) => self.handle_state_change(params),
            ("VerificationNotStarted", None) => {
                self.progress.lock().unwrap().end(&self.editor, Some("verification did not start".into()))
            }
            ("Hint", None) => send(&self.editor, &notification("window/showMessage", json!({
                "type": 2,
                "message": params["message"],
            }))),
            // Log, UnhandledViperServerMessageType: VS Code output channel chatter
            _ => {}
        }
    }

    fn handle_state_change(&self, params: &Value) {
        let mut progress = self.progress.lock().unwrap();
        match params["newState"].as_i64().unwrap_or(-1) {
            STATE_VERIFICATION_RUNNING => {
                let percentage = params["progress"].as_f64().unwrap_or(-1.0);
                if percentage >= 0.0 {
                    progress.report(&self.editor, percentage);
                }
            }
            STATE_READY => {
                let success = params["success"].as_i64().unwrap_or(0);
                let seconds = params["time"].as_f64().unwrap_or(-1.0);
                progress.end(&self.editor, outcome(success, seconds));
            }
            STATE_STOPPED | STATE_STOPPING => progress.end(&self.editor, None),
            _ => {}
        }
    }
}

fn run() -> io::Result<()> {
    let config = Config::from_env();
    let (mut server_process, port) = start_server(&config)?;
    log(format!("ViperServer listening on port {port}, backend {}", config.backend));
    let tcp = TcpStream::connect(("127.0.0.1", port))?;

    let bridge = Arc::new(Bridge {
        config,
        editor: Arc::new(Mutex::new(Box::new(io::stdout()))),
        server: Arc::new(Mutex::new(Box::new(tcp.try_clone()?))),
        progress: Mutex::new(Progress::default()),
    });

    let from_server = Arc::clone(&bridge);
    thread::spawn(move || {
        let mut reader = BufReader::new(tcp);
        loop {
            match read_message(&mut reader) {
                Ok(Some(message)) => from_server.handle_server_message(message),
                Ok(None) => break,
                Err(error) => {
                    log(format!("reading from ViperServer failed: {error}"));
                    break;
                }
            }
        }
        log("ViperServer closed the connection");
        std::process::exit(0);
    });

    let mut stdin = io::stdin().lock();
    while let Some(message) = read_message(&mut stdin)? {
        bridge.handle_editor_message(message);
    }
    log("editor closed stdin, stopping ViperServer");
    server_process.kill().ok();
    Ok(())
}

fn main() {
    if std::env::args().any(|arg| arg == "--version") {
        println!("viper-lsp {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if let Err(error) = run() {
        log(format!("fatal: {error}"));
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_port_line() {
        assert_eq!(parse_port("<ViperServerPort:54321>\n"), Some(54321));
        assert_eq!(parse_port("ViperServer online"), None);
    }

    #[test]
    fn decodes_file_uris() {
        assert_eq!(uri_to_path("file:///tmp/with%20space/a.vpr"), PathBuf::from("/tmp/with space/a.vpr"));
        assert_eq!(uri_to_path("file:///c%3A/Users/a.vpr"), PathBuf::from("c:/Users/a.vpr"));
    }

    #[test]
    fn reads_framed_messages() {
        let body = r#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#;
        let framed = format!("Content-Length: {}\r\n\r\n{body}", body.len());
        let message = read_message(&mut framed.as_bytes()).unwrap().unwrap();
        assert_eq!(message["method"], "initialized");
    }
}
