# Lantern Protocol v4

This directory defines the executable wire contract between the Lantern pane
and the local agent daemon. The Rust `Request` and `Event` types are canonical;
the JSONL files are golden examples that the test suite deserializes on every
run.

## Framing

- Each frame is one UTF-8 JSON value terminated by LF (`0x0a`).
- A CR immediately before LF is ignored, so CRLF clients are accepted.
- Unicode line and paragraph separators are ordinary string content, not frame
  boundaries.
- An inbound request frame is at most 1 MiB. An oversized or malformed frame
  produces an `error` event; the next LF-delimited request can still be
  processed.
- An outbound event frame is at most 256 KiB. The daemon serializes and writes
  one event at a time, so the operating-system pipe supplies back-pressure
  without an additional unbounded application queue.
- Unknown methods, event types, fields, evidence sources, and capabilities are
  protocol errors. There is no compatibility fallback.

## Workspace trust

Initialization does not grant repository access. `configure_workspace` binds
the daemon to one canonical repository for its lifetime and replaces the
session-local capability set. A later request cannot retarget the daemon to a
different repository.

The capability vocabulary is `repository_read`, `repository_write`,
`process_execution`, and `network_access`. The read-only Quick Ask profile
accepts only repository read and network access. Network access represents the
visible transmission of selected evidence to the configured model and requires
read access. Repository write and process execution are hard denials: they do
not create a pending approval or silently downgrade.

`process_execution` means commands or tools acting on behalf of repository
content. It does not describe Lantern's pinned, no-tools provider adapter
process, which is runtime plumbing governed by `network_access` and receives
only the explicitly transmitted evidence.

Trust cannot change during an active operation. An operational request is
authorized against every capability it needs before it is accepted. A denial
therefore emits `error` without `accepted` or `settled`.

## Evidence provenance

Every evidence record carries exactly one typed `source`:

- `selection`: exact code selected in Helix;
- `definition`: the bounded definition resolved by Helix language intelligence;
- `reference`: a bounded reference resolved by Helix language intelligence; or
- `literal_match`: an exact match found by Lantern's bounded local search.

The source is determined locally while assembling evidence. It adds no model
request, repository scan, or free-form explanation payload. Clients may derive
short presentation labels from the enum and navigate using the existing exact
range.

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
Lantern admits one active operation across the daemon, matching the single
visible agent surface and bounding concurrent producers. `completed`,
`cancelled`, and `error` describe the outcome, but the client must remain busy
until `settled` confirms that subprocesses, listeners, and registry state have
been released. An execution-setup failure may emit `error` before
`operation_started`.

An admission failure emits `error` without `accepted` or `settled`. After
acceptance, every outcome is followed by exactly one `settled`. A cancellation
request targets an existing operation and does not create a second lifecycle.
Cancellation is idempotent: if the target has already settled, the request is
an intentional no-op.

`shutdown` requests cancellation of every active operation and waits for the
daemon-owned workers to exit before the daemon process exits.

## Compatibility

Initialization must use protocol version `4`. A version mismatch is a hard
error with an explicit rebuild recovery; Lantern never silently downgrades.
Operational requests sent before successful initialization are rejected. The
client treats initialization as the ready boundary and applies a bounded
startup deadline. Protocol stdout contains JSONL only; process diagnostics stay
on stderr and are continuously drained into a bounded tail.

Protocol v4 replaces v3 by adding typed evidence provenance. The v3 fixtures
remain in the repository as historical contract evidence; the maintained
client and daemon do not negotiate it.
