# Lantern Protocol v10

Protocol v10 retains v9's strict trusted-workbench lifecycle and grounding
states. It adds one `investigate_agent` request for an explicitly read-only
feature investigation. The daemon runs that request through Pi with only
`read`, `grep`, `find`, and `ls`; coding turns retain their existing tools.

Investigation text uses the ordinary streamed lifecycle and terminal surface.
There is no separate report event, hidden persistence, or protocol-level model
output parser. The readiness structure is a model-behavior contract covered by
the versioned DeepEval dataset, while tool restriction is deterministic.

Requests are limited to 1 MiB and events to 256 KiB. Unknown methods, event
types, fields, grounding states, and evidence provenance are hard errors.
Lantern does not negotiate or fall back to an older protocol.
