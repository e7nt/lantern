# Lantern Protocol v7

Protocol v7 is Lantern's strict LF-delimited JSON contract. It retains v6's
trusted-workbench and operation lifecycle and adds bounded typed outgoing-call
evidence to `ask_agent_symbol`.

Each call has an untrusted bounded symbol name, depth one or two, and a validated
repository-relative source range. A symbol context contains at most eight calls.
Call evidence is optional enrichment from the active language server; ordinary
repository questions remain immediately available without editor context.

Requests are limited to 1 MiB and events to 256 KiB. Unknown methods, event
types, fields, and evidence provenance are hard errors. Lantern does not
negotiate or fall back to an older protocol. Accepted operations have exactly
one terminal outcome and then `settled`.
