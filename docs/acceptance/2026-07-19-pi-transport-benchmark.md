# Pi RPC versus SDK transport benchmark — 2026-07-19

Status: matched-model comparison harness passes; repeated sampling and DeepEval
parity remain before replacing the maintained RPC driver.

`scripts/benchmark-pi-transports.mjs` exercises the current RPC transport and
the proposed SDK adapter against the same disposable repository, Python file,
prompt, read-only tool set, thinking level, and system instruction. It asks RPC
for the fully resolved provider and model, resolves that exact pair through the
SDK's public `AuthStorage`, `ModelRegistry`, and `resolveCliModel` APIs, and
fails if the SDK session reports a different pair.

The first matched run on `openai-codex/gpt-5.4` measured:

| Transport | Initialization | First activity | First text |
| --- | ---: | ---: | ---: |
| RPC | 825 ms | 2487 ms | 3939 ms |
| SDK | 1651 ms | 2008 ms | 3166 ms |

This single pair is directional evidence, not a latency verdict: provider
variance and run order can dominate first-token time. It does show no immediate
streaming regression from the adapter. The SDK's broader cold initialization
cost is acceptable only if Lantern keeps one session warm; it must not recreate
the model registry per developer turn.

The benchmark uses an LF-only byte parser rather than Node's `readline`, as
required by Pi's JSONL framing. It prints only the resolved model and timings,
never prompt text, answer text, tool arguments, credentials, or fixture
contents. Run it with:

```bash
node scripts/benchmark-pi-transports.mjs
```

`LANTERN_PI_MODEL` may select the same model pattern used by the maintained
daemon. The command fails on timeout, invalid JSON framing, process failure,
model mismatch, or missing streaming events, and always removes the fixture.
