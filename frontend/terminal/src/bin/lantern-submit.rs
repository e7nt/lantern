use lantern_protocol::{ControlRequest, MAX_PLAN_COMMENT_BYTES, MAX_QUESTION_BYTES};
use std::env;
use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

fn main() -> io::Result<()> {
    let add_plan_comment = env::args_os()
        .nth(1)
        .is_some_and(|argument| argument == "--plan-comment");
    let socket = env::var_os("LANTERN_CONTROL_SOCKET")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::other("Lantern control socket is not configured"))?;
    let (label, limit) = if add_plan_comment {
        ("plan comment", MAX_PLAN_COMMENT_BYTES)
    } else {
        ("question", MAX_QUESTION_BYTES)
    };
    let mut bytes = Vec::new();
    io::stdin()
        .take((limit + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.is_empty() || bytes.len() > limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{label} must contain 1 to {limit} bytes"),
        ));
    }
    let question = String::from_utf8(bytes).map_err(|_| {
        io::Error::new(io::ErrorKind::InvalidInput, format!("{label} is not UTF-8"))
    })?;
    if question.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{label} is empty"),
        ));
    }

    let mut stream = UnixStream::connect(socket)?;
    let request = if add_plan_comment {
        ControlRequest::AddPlanComment { comment: question }
    } else {
        ControlRequest::SubmitQuestion { question }
    };
    serde_json::to_writer(&mut stream, &request)?;
    stream.write_all(b"\n")?;
    stream.flush()
}
