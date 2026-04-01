use std::io::{BufRead, BufReader, Write};
use serde_json::{json, Value};

// Unix: Unix Domain Socket
#[cfg(unix)]
fn connect_and_send(req_line: &str) -> Result<Value, String> {
    use std::os::unix::net::UnixStream;
    use std::time::Duration;

    const SOCKET_PATH: &str = "/tmp/filemaker_mcp.sock";

    let mut stream = UnixStream::connect(SOCKET_PATH)
        .map_err(|_| "FileMaker is not running".to_string())?;

    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .map_err(|e| e.to_string())?;

    eprintln!("[filemaker-mcp] ipc: sending {} bytes", req_line.len());

    stream
        .write_all(req_line.as_bytes())
        .map_err(|e| format!("send error: {e}"))?;

    eprintln!("[filemaker-mcp] ipc: sent, waiting for response");

    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .map_err(|e| format!("receive error: {e}"))?;

    eprintln!("[filemaker-mcp] ipc: received {} bytes", line.len());

    serde_json::from_str(line.trim()).map_err(|e| format!("JSON parse error: {e}"))
}

// Windows: Named Pipe
// The client connects to \\.\pipe\filemaker_mcp via OpenOptions (no extra crates needed).
#[cfg(windows)]
fn connect_and_send(req_line: &str) -> Result<Value, String> {
    use std::fs::OpenOptions;

    const PIPE_PATH: &str = r"\\.\pipe\filemaker_mcp";

    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(PIPE_PATH)
        .map_err(|_| "FileMaker is not running".to_string())?;

    file.write_all(req_line.as_bytes())
        .map_err(|e| format!("send error: {e}"))?;

    let mut reader = BufReader::new(file);
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .map_err(|e| format!("receive error: {e}"))?;

    serde_json::from_str(line.trim()).map_err(|e| format!("JSON parse error: {e}"))
}

#[cfg(not(any(unix, windows)))]
fn connect_and_send(_req_line: &str) -> Result<Value, String> {
    Err("unsupported platform".to_string())
}

/// Send a JSON command to the plugin and receive the response.
/// Returns an error string if the plugin is not running.
pub fn send_to_plugin(command: &str, args: &Value) -> Result<Value, String> {
    let req = json!({ "command": command, "args": args });
    let req_line = req.to_string() + "\n";
    connect_and_send(&req_line)
}
