use lantern_protocol::{ControlRequest, MAX_QUESTION_BYTES};
use std::env;
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

fn main() -> io::Result<()> {
    let socket = env::var_os("LANTERN_CONTROL_SOCKET")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::other("Lantern control socket is not configured"))?;
    let mut bytes = Vec::new();
    io::stdin()
        .take((MAX_QUESTION_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.is_empty() || bytes.len() > MAX_QUESTION_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("question must contain 1 to {MAX_QUESTION_BYTES} bytes"),
        ));
    }
    let question = String::from_utf8(bytes)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "question is not UTF-8"))?;
    if question.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "question is empty",
        ));
    }

    let mut stream = UnixStream::connect(socket)?;
    serde_json::to_writer(&mut stream, &ControlRequest::SubmitQuestion { question })?;
    stream.write_all(b"\n")?;
    stream.flush()
}
