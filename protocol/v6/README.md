# Lantern Protocol v6

Protocol v6 is Lantern's strict LF-delimited JSON contract. It retains the v5
trusted-workbench and operation lifecycle, and adds `ask_agent` for a Pi turn
that begins with repository tools rather than requiring editor context.

Editor selection, definition, and reference context are optional enrichment.
The terminal uses them when Helix has exported a valid saved-file selection;
otherwise a normal question is admitted immediately as `ask_agent`. Missing
selection or LSP context must never prevent a developer from talking to Pi.

Requests are limited to 1 MiB and events to 256 KiB. Unknown methods, event
types, and fields are hard errors. Lantern does not negotiate or fall back to
an older protocol. Accepted operations have exactly one terminal outcome and
then `settled`.
