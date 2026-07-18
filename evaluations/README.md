# Lantern behavioral evaluations

This isolated harness checks nondeterministic model behavior without adding
Python or DeepEval to the editor or daemon. DeepEval and Python are pinned; no
hosted evaluation service is required.

Run the deterministic contract checks:

```bash
cd evaluations
DEEPEVAL_DISABLE_DOTENV=1 uv run pytest
uv run ruff format --check .
uv run ruff check .
```

Run the live, subscription-authenticated Pi driver against the versioned cases:

```bash
cd evaluations
DEEPEVAL_DISABLE_DOTENV=1 uv run python run_pi_quick_ask.py
DEEPEVAL_DISABLE_DOTENV=1 uv run python run_live_trace.py
```

This requires Pi `0.80.6` and a private OpenAI Codex login completed through
Pi's interactive `/login` flow. It writes a local timestamped report under
`reports/` and exits unsuccessfully when any deterministic contract fails.
Build Lantern first with `cargo build`; `run_live_trace.py` then exercises the
real daemon through Protocol v6. It measures a grounded repository explanation,
repository-relative evidence use, tool efficiency, time to first tool and text,
settling time, and cancellation while a tool-driven turn is active. Override
the binaries explicitly with `LANTERN_DAEMON_BIN` or `LANTERN_PI_BIN`; the
runner never chooses a fallback binary or provider.

The versioned datasets cover missing-context selections, bounded LSP symbol
context, and efficient coding-tool journeys. They check properties that do not require a judge:
required uncertainty disclosures, use of definitions and references, forbidden
unsupported claims, resistance to instructions embedded in selected code, and
ordered inspect/read/edit/verify behavior without unnecessary mutations.
Live Pi outputs will be recorded as local JSON under `reports/`, which is
ignored. Model-judged grounding and understanding metrics remain a promotion
gate; they require an explicitly configured local or user-selected judge and
must not silently consume a provider credential.
