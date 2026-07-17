# Lantern Protocol v5

Protocol v5 is the strict JSONL contract between the Lantern terminal and its
local daemon. The Rust `Request` and `Event` types are canonical; the golden
fixtures in this directory are deserialized by the test suite.

Each frame is one UTF-8 JSON value terminated by LF. Requests are limited to
1 MiB and events to 256 KiB. Unknown methods, event types, and fields are hard
errors. Lantern does not negotiate or fall back to an older protocol.

## Trusted workbench

After initialization, `open_workbench` binds the daemon to one canonical
repository. Lantern is an open-source coding agent for developers who want to
understand and write code, so the agent receives full access to that workbench.
There is no capability ceremony or reduced-function fallback. Every operation
must name the open repository; requests cannot silently escape its boundary.

The workbench cannot change while an operation is active. A failed open emits
`workbench_open_failed` before any work is admitted.

## Operation lifecycle

An admitted operation follows exactly this lifecycle:

```text
request -> accepted -> activity* -> completed | cancelled | error -> settled
```

The client stays busy through `settled`, which confirms that subprocesses and
daemon state have been released. Admission errors do not emit `accepted` or
`settled`. Cancellation is idempotent, and shutdown cancels and joins active
workers before the daemon exits.

Evidence retains its typed provenance (`selection`, `definition`, `reference`,
or `literal_match`) and exact range so the terminal can explain an answer and
navigate Helix to the supporting code.

Pi tool payloads remain inside the daemon. The terminal receives only bounded,
typed `tool_started` and `tool_finished` events, an optional validated
repository-relative path, and success state. Command text and tool output are
not copied into the application protocol.
