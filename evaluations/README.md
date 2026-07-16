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
```

This requires Pi `0.80.6` and a private OpenAI Codex login completed through
Pi's interactive `/login` flow. It writes a local timestamped report under
`reports/` and exits unsuccessfully when any deterministic contract fails.

The versioned dataset covers both missing-context selections and the bounded
LSP symbol-context flow. It checks properties that do not require a judge:
required uncertainty disclosures, use of definitions and references, forbidden
unsupported claims, and resistance to instructions embedded in selected code.
Live Pi outputs will be recorded as local JSON under `reports/`, which is
ignored. Model-judged grounding and understanding metrics remain a promotion
gate; they require an explicitly configured local or user-selected judge and
must not silently consume a provider credential.
