# Persistent Pi workbench — 2026-07-18

Status: passed deterministic lifecycle coverage and an authenticated live
follow-up through Lantern's Protocol v6 daemon.

## User outcome

The first agent question lazily starts one Pi RPC process. Sequential questions
reuse that process and its in-memory conversation until the workbench closes.
Pi still runs with `--no-session`, so Lantern does not enable Pi session-file
persistence.

Cancellation sends Pi's RPC `abort` command without destroying a healthy
driver. A later question can continue through the same process. Malformed output
or process failure stops the driver and fails subsequent work visibly; Lantern
does not silently restart or select another provider.

## Live measurement

The initial fixture explanation performed five tools, began tool activity in
5,608 ms, began response text in 15,515 ms, and settled in 19,804 ms. This was a
repository-discovery turn and is not under the follow-up latency target.

The next question asked for a fact from the code Pi had just inspected:

- Tool calls: 0
- First response text: 1,518 ms
- Settled: 2,486 ms
- Grounding contract: passed
- Repository state: unchanged

A third turn was interrupted during tool activity. Cancellation was reported in
41 ms and the operation settled 42 ms after the cancel request.

An isolated direct Pi diagnostic established the provider/process floor before
implementation: a cold trivial turn produced text in 2,834 ms and the second
turn on the same process in 1,775 ms. The product follow-up result is consistent
with that warm measurement.

## Deterministic proof

Protocol tests verify:

- two settled operations write two prompts through one Pi PID;
- conversational process state advances across the turns;
- cancellation is followed by successful reuse of the same process;
- daemon shutdown terminates and reaps the Pi child;
- malformed Pi output terminates the child and the next operation fails visibly
  without a hidden restart;
- the existing edit/test journey, bounded event stream, privacy, and
  cancellation contracts remain green.

## Remaining latency boundary

Under three seconds is now the enforced target for a warm, context-grounded
follow-up. A first question that requires repository discovery still depends on
provider latency and necessary tools. Lantern should reduce redundant discovery
and use supplied LSP evidence before considering an index or cache.

No raw answer, prompt, source dump, credential, provider diagnostic, PID, or
machine-specific path is committed.
