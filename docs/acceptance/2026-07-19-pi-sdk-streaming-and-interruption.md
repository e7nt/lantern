# Pi SDK streaming and interruption spike — 2026-07-19

Status: the streaming and interruption gate in ADR 005 passes. Like-for-like RPC
latency comparison and model-evaluation parity remain before production
promotion.

Lantern now has a deliberately narrow SDK adapter prototype at
`spikes/agent-runtime/pi-sdk-adapter.mjs`. It translates Pi events into five
Lantern concepts: text delta, tool started, tool finished, turn settled, and an
explicit settlement outcome. Settlement is derived from Pi's `agent_settled`,
not the earlier `agent_end`, so automatic continuation cannot make Lantern look
idle prematurely. Tool arguments, raw model messages, Pi retry events, and
other upstream implementation details are not part of the contract.

Deterministic tests prove the event mapping, active-turn guard, interruption,
and interrupted settlement without a provider. A live run then uses the pinned
Pi 0.80.6 SDK, existing subscription authentication, an in-memory conversation,
a disposable Python fixture, and only the `read` tool. It requires a real tool
start and finish, streamed text, and a clean settlement. A second active turn is
interrupted after its first text delta and must settle as interrupted within one
second.

Two measured live runs produced:

- SDK import, authentication/model discovery, and session setup: 1512–1520 ms
- first Lantern activity on the grounded turn: 2427–4277 ms
- first text on the grounded turn: 2427–4307 ms
- active-turn interruption and idle settlement: 6–7 ms

The setup measurement is intentionally broader than the earlier 6 ms
session-only measurement. One provider turn met Lantern's under-three-second
experience goal and one did not, so a single fast run is not accepted as a
latency result. These runs also cannot yet be compared fairly with the
maintained RPC path because the prompt and measurement boundary differ. The
next gate is a same-process benchmark harness that feeds the identical fixture
and prompt to both transports and separates local overhead from provider wait.

No prompt text, answer text, tool arguments, credentials, or fixture contents
are printed by the live runner. Run both checks with:

```bash
node --test spikes/agent-runtime/pi-sdk-adapter.test.mjs
node scripts/spike-pi-sdk-streaming.mjs
```

The live command fails on the wrong Pi version, a missing contract event, an
unclean completion, an interruption that is not observed, an abort over one
second, or a 45-second turn timeout. It always disposes the session and removes
the fixture.
