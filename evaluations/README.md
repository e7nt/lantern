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
DEEPEVAL_DISABLE_DOTENV=1 uv run python run_semantic_retrieval_spike.py
DEEPEVAL_DISABLE_DOTENV=1 uv run python run_semantic_refresh.py
DEEPEVAL_DISABLE_DOTENV=1 uv run python run_cold_grounding_status.py
DEEPEVAL_DISABLE_DOTENV=1 uv run python run_external_edit_journey.py
```

This requires Pi `0.80.6` and a private OpenAI Codex login completed through
Pi's interactive `/login` flow. It writes a local timestamped report under
`reports/` and exits unsuccessfully when any deterministic contract fails.
Build Lantern first with `cargo build`; `run_live_trace.py` then exercises the
real daemon through Protocol v17. It measures a grounded repository explanation,
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

`run_semantic_retrieval_spike.py` measures repository-only questions whose
natural wording deliberately differs from the implementation identifiers. It
records grounding, observed paths, tool count, first verified evidence, first
model text, provider wait after evidence, timeout, and read-only state. Set
`LANTERN_EVAL_CASE` to an exact case id to isolate one expensive live turn;
unknown ids fail explicitly.

`run_semantic_refresh.py` needs no provider credential. It clones the pinned
p-limit fixture into a disposable directory, runs the real local embedding
worker, changes indexed source without committing it, and requires the
automatically refreshed index within three seconds. It also verifies that only
the changed symbol is embedded, unchanged vectors are reused, and a query sees
the new ready revision.

`run_cold_grounding_status.py` needs no provider credential. It opens a
disposable pinned Requests clone with a fresh real local index and requires the
typed `preparing_index` state within one second. It does not wait for an answer;
the gate protects truthful cold-start feedback before provider latency.

`run_external_edit_journey.py` creates disposable Git repositories outside the
Lantern checkout. It submits a Protocol v17 symbol-grounded change, verifies the
exact implementation and test files, runs the focused repository test, requires
an unstaged reviewable diff, and separately interrupts a tool-driven read. Its
report contains only bounded tool metadata and outcome measurements.

The versioned datasets cover missing-context selections, bounded LSP symbol
context, and efficient coding-tool journeys. They check properties that do not require a judge:
required uncertainty disclosures, use of definitions and references, forbidden
unsupported claims, resistance to instructions embedded in selected code, and
ordered inspect/read/edit/verify behavior without unnecessary mutations.
The planning dataset additionally requires every persisted-plan section and
repository evidence across the protocol, daemon, and terminal boundaries. The
plan-review dataset requires one complete revision to address every queued
anchor while rejecting Markdown wrappers, frontmatter, and false implementation
claims. The plan-progress dataset requires a complete checkpoint to mark only
diff-supported work complete, retain untouched tasks, and avoid inventing
broader verification.
The code-review dataset requires one concise correction result to acknowledge
every submitted concern and its focused verification without inventing broader
test coverage or API changes.
Live Pi outputs will be recorded as local JSON under `reports/`, which is
ignored. Model-judged grounding and understanding metrics remain a promotion
gate; they require an explicitly configured local or user-selected judge and
must not silently consume a provider credential.
