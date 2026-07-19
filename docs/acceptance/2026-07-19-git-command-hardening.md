# Git command hardening — 2026-07-19

Status: the focused rail's command boundary and responsive network-operation
gate pass. Lazygit remains maintained until accessibility, external-refresh,
and measured startup/RSS gates pass.

Every Git invocation now passes through one typed runner. Local commands have
a five-second deadline; fetch and fast-forward pull have a thirty-second
deadline. Standard output is bounded to 512 KiB and diagnostic capture to 8
KiB. Git prompts and credential-manager interaction are disabled so the narrow
rail cannot disappear behind an invisible prompt. Timeout and cancellation
terminate the command's process group, including credential helpers and Git
children.

Errors distinguish invalid input, repository mismatch, timeout, cancellation,
oversized output, missing authentication, command failure, invalid output, and
failure to start. The interface gives one recovery action without echoing
provider diagnostics, remote URLs, or credentials.

Fetch and behind-only pull now run on a worker thread while the terminal keeps
rendering every 50 ms. The footer identifies the active operation and `Esc`
cancels it. Completion refreshes branch and change state; cancellation and
failure return a typed notice rather than freezing or exiting the rail.

Twenty deterministic tests pass: eight command/parser tests, six renderer-state
tests, and six repository journeys. Dedicated runner tests prove
non-interactive environment variables, output bounds, deadline termination,
sub-500 ms cancellation of a sleeping process group, and private typed
authentication errors. Formatting and Clippy with warnings denied pass.

This checkpoint adds no Git feature and no dependency. Remaining promotion
work is state-preserving external refresh, keyboard/mouse accessibility
verification, and a reproducible startup/RSS comparison with pinned Lazygit.
