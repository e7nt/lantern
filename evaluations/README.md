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
DEEPEVAL_DISABLE_DOTENV=1 uv run python run_retrieval_baseline.py
DEEPEVAL_DISABLE_DOTENV=1 uv run python run_external_edit_journey.py
```

This requires Pi `0.80.6` and a private OpenAI Codex login completed through
Pi's interactive `/login` flow. It writes a local timestamped report under
`reports/` and exits unsuccessfully when any deterministic contract fails.
Build Lantern first with `cargo build`; `run_live_trace.py` then exercises the
real daemon through Protocol v7. It measures a grounded repository explanation,
repository-relative evidence use, tool efficiency, time to first tool and text,
an under-three-second warm grounded follow-up, settling time, and cancellation
while a tool-driven turn is active. Override
the binaries explicitly with `LANTERN_DAEMON_BIN` or `LANTERN_PI_BIN`; the
runner never chooses a fallback binary or provider.

`run_retrieval_baseline.py` compares repository-only exact discovery with typed
Helix/LSP selection, definition, reference, and bounded call context on pinned
external checkouts under `.lantern/upstream`. It fails if a checkout or revision
does not match, runs both modes through the same daemon and Pi adapter, verifies
read-only repository state, and reports LSP-minus-exact latency and tool-count
deltas. Dataset v2 also includes an intentionally incomplete symbol question:
it requires bounded tool escalation and measures first useful activity instead
of pretending a direct answer is possible. Prepare the pinned upstream
repositories using the normal Lantern setup before running it. Dataset v3 adds
the measured two-hop call evidence and requires the same answer with zero tools.
Dataset v4 retains that Rust regression and adds a Go/Lazygit case captured from
`gopls`, so the generic LSP boundary is exercised across languages.
Dataset v5 adds real Pyright and TypeScript-language-server evidence for Python,
JavaScript, and TypeScript. Each evaluation checkout's upstream URL and exact
revision are declared in the dataset; the runner rejects missing or mismatched
revisions with an explicit preparation instruction.

`run_external_edit_journey.py` creates disposable Git repositories outside the
Lantern checkout. It submits a Protocol v7 symbol-grounded change, verifies the
exact implementation and test files, runs the focused repository test, requires
an unstaged reviewable diff, and separately interrupts a tool-driven read. Its
report contains only bounded tool metadata and outcome measurements.

The versioned datasets cover missing-context selections, bounded LSP symbol
context, and efficient coding-tool journeys. They check properties that do not require a judge:
required uncertainty disclosures, use of definitions and references, forbidden
unsupported claims, resistance to instructions embedded in selected code, and
ordered inspect/read/edit/verify behavior without unnecessary mutations.
Live Pi outputs will be recorded as local JSON under `reports/`, which is
ignored. Model-judged grounding and understanding metrics remain a promotion
gate; they require an explicitly configured local or user-selected judge and
must not silently consume a provider credential.
