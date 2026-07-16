# Diagnostic privacy contract

Lantern diagnostics exist to explain lifecycle and boundary failures without
turning developer work into telemetry. Diagnostics are local, bounded, and
metadata-only. Lantern does not create, upload, or share a diagnostic bundle
unless the developer explicitly enters `/diagnostics`.

## Structured record

The daemon writes one JSON record per line to its private stderr pipe. Schema
version 1 permits only:

- timestamp;
- severity;
- component and typed event code;
- optional numeric operation identifier.

The schema has no arbitrary message, attributes, or payload field. It therefore
cannot contain prompts, answers, source, evidence excerpts, repository paths,
file names, environment values, credentials, provider stderr, command output,
or model reasoning. Unknown fields and unknown event codes are rejected.

The pane continuously drains stderr into the existing 8 KiB tail so the daemon
cannot block on diagnostics. Only valid versioned records are summarized or
exported. Unstructured stderr is counted and discarded; Lantern does not guess
which substrings might be secret and copy the rest.

## Explicit export

`/diagnostics` creates a new JSON file in the system temporary directory. It
contains:

- diagnostic schema and Lantern protocol versions;
- generation time;
- operating system and architecture;
- visible daemon state;
- at most the latest 128 valid structured records;
- a count of excluded unstructured stderr lines.

On Unix, the file is created with mode `0600`. Existing files are never
overwritten. The bundle contains no repository identifier or path. The command
remains available when the daemon is unavailable, and the resulting path and
exclusions are shown to the developer.

Diagnostic export is not a network capability and performs no transmission.
Sharing the resulting file is a separate, manual developer action.

Provider authentication follows the separate
[credential contract](CREDENTIALS.md); diagnostics never become a credential
transport or inspection mechanism.
