# Lantern Protocol v9

Protocol v9 retains v8's strict trusted-workbench lifecycle and verified local
semantic evidence. It adds one typed `grounding_state` event so the terminal can
distinguish a background semantic build from repository-tool-only grounding
before Pi starts.

The only states are `preparing_index` and `repository_search_only`. They are
transient operation status, contain no source or paths, and do not promise
progress percentages. Ready semantic evidence continues to use the existing
`evidence` event. Questions never wait for index preparation.

Requests are limited to 1 MiB and events to 256 KiB. Unknown methods, event
types, fields, grounding states, and evidence provenance are hard errors.
Lantern does not negotiate or fall back to an older protocol.
