# Lantern Protocol v8

Protocol v8 retains v7's strict LF-delimited JSON contract, trusted-workbench
lifecycle, and bounded LSP call evidence. It adds `semantic` as typed evidence
provenance for local embedding candidates that the daemon has reopened and
verified against current repository source.

Semantic matches are optional evidence for ordinary `ask_agent` requests. The
index is disposable and revision-bound. A building or unavailable index is an
explicit state, never stale evidence; Pi's repository tools remain an
independent complementary source.

Requests are limited to 1 MiB and events to 256 KiB. Unknown methods, event
types, fields, and evidence provenance are hard errors. Lantern does not
negotiate or fall back to an older protocol. Accepted operations have exactly
one terminal outcome and then `settled`.
