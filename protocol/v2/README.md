# Lantern Protocol v2

This directory defines the executable wire contract between the Lantern pane and
the local agent daemon. The Rust `Request` and `Event` types are canonical; the
JSONL files are golden examples that the test suite deserializes on every run.

## Framing

- Each frame is one UTF-8 JSON value terminated by LF (`0x0a`).
- A CR immediately before LF is ignored, so CRLF clients are accepted.
- Unicode line and paragraph separators are ordinary string content, not frame
  boundaries.
- A frame payload is at most 1 MiB. An oversized or malformed frame produces an
  `error` event; the next LF-delimited request can still be processed.
- Unknown methods, event types, and fields required by a known variant are
  protocol errors. There is no compatibility fallback.

## Operation lifecycle

An accepted operation ID has one legal lifecycle:

```text
request
  -> accepted
  -> [operation_started -> progress | evidence | change_proposal | text_delta ...]
  -> completed | cancelled | error
  -> settled
```

`accepted` means the request passed admission checks and owns its operation ID.
Only one active operation may own an ID. `completed`, `cancelled`, and `error`
describe the outcome, but the client must remain busy until `settled` confirms
that subprocesses, listeners, and registry state have been released.
An execution-setup failure may emit `error` before `operation_started`.

An admission failure emits `error` without `accepted` or `settled`. After
acceptance, every outcome is followed by exactly one `settled`. A cancellation
request targets an existing operation and does not create a second lifecycle.
Cancellation is idempotent: if the target has already settled, the request is
an intentional no-op.

`shutdown` requests cancellation of every active operation and waits for the
daemon-owned workers to exit before the daemon process exits.

## Compatibility

Initialization must use protocol version `2`. A version mismatch is a hard
error with an explicit rebuild recovery; Lantern never silently downgrades.
Operational requests sent before successful initialization are rejected.
