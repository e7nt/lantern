# Lantern evaluation strategy

## Purpose

Lantern contains both deterministic software and nondeterministic model
behavior. Conventional tests prove that the system obeys protocols, tool
contracts, and state transitions. They cannot prove that an explanation is faithful, a
learning route is useful, or an implementation narrative improves
understanding.

Lantern therefore uses two complementary test systems:

1. Deterministic software tests for invariants.
2. DeepEval-based behavioral evaluations for model-mediated quality.

Neither system substitutes for the other.

## Principles

- Evaluate complete user outcomes and important model-driven components.
- Prefer repository evidence and structured traces over reference prose alone.
- Use custom Lantern criteria rather than generic “helpfulness” scores.
- Calibrate model judges against human-reviewed cases.
- Treat a single judge score as evidence, not truth.
- Avoid flaky pull-request gates based on one stochastic sample.
- Record model, prompt, tools, dataset, judge, and configuration with every run.
- Keep evaluation local-first and open source.
- Do not require Confident AI or another hosted evaluation service.
- Do not commit proprietary outputs, repository secrets, or provider keys.

## Harness

DeepEval is a Python development dependency isolated under `evaluations/`. It
does not become a dependency of the Lantern editor or daemon.

The evaluation environment must:

- Pin Python and DeepEval versions.
- Set `DEEPEVAL_DISABLE_DOTENV=1` so importing the harness does not
  automatically load repository `.env` files.
- Resolve judge credentials explicitly through the evaluation runner.
- Support a local or user-selected judge model through an adapter.
- Disable hosted result synchronization by default.
- Emit a portable local JSON report.
- Separate datasets from generated run artifacts.

Proposed structure:

```text
evaluations/
├── pyproject.toml
├── README.md
├── datasets/
│   ├── quick_ask/
│   ├── repository_map/
│   ├── learning/
│   ├── investigation/
│   ├── planning/
│   ├── guided_build/
│   ├── review/
│   └── live_collaboration/
├── metrics/
│   ├── evidence_grounding.py
│   ├── uncertainty_honesty.py
│   ├── learning_route_quality.py
│   ├── plan_readiness.py
│   ├── change_narrative_quality.py
│   └── voice_pairing_quality.py
├── adapters/
│   ├── lantern_trace.py
│   └── judge_model.py
├── tests/
└── reports/                 # ignored
```

## Evaluation record

Every case records:

- Stable case and dataset version.
- Repository fixture and Git revision.
- User intent and current workbench state.
- Editor selection or starting evidence.
- Available typed tools and any explicit destructive request.
- Model and provider identifier.
- Prompt and tool-schema hashes.
- Retrieved evidence and freshness hashes.
- Ordered tool requests and results.
- Final visible output.
- Expected facts, forbidden claims, and required disclosures.
- Human rubric and annotations.
- Timing, token, and cost measurements.

Raw hidden reasoning is never required or stored.

## Metric layers

### Hard invariants

These remain deterministic assertions outside model judging:

- Tools remain within attached workbench folders and their typed schemas.
- Evidence targets exist and match the recorded content hash.
- Every durable claim has an evidence or explicit uncertainty link.
- Tool names and arguments satisfy schemas.
- Destructive Git history operations occur only after an explicit request.
- Cancellation prevents new operations.
- Verification status matches actual command outcomes.
- Voice interruption stops playback and preserves work state correctly.

Any hard-invariant failure fails the evaluation regardless of semantic score.

### Structured deterministic metrics

Use direct calculations or DeepEval custom metrics where no judge is needed:

- Evidence precision and recall against curated evidence sets.
- Unsupported claim count.
- Required-section and plan-schema coverage.
- Tool selection and argument exactness.
- Extra tool-call count and trace length.
- Acceptance-criterion coverage.
- Latency, cancellation, interruption, token, and cost budgets.

### Model-judged metrics

Use DeepEval built-ins, G-Eval, Conversational G-Eval, or constrained DAG metrics
for properties that require semantic judgment:

- Faithfulness to supplied repository evidence.
- Answer relevance and useful concision.
- Honest separation of observation, inference, and unknown.
- Learning-route coherence and pedagogical value.
- Plan completeness, feasibility, and decision clarity.
- Change-narrative usefulness.
- Conversation completeness, role adherence, and knowledge retention.

DAG metrics are preferred when criteria can be expressed as explicit decision
branches. G-Eval is used for irreducibly qualitative rubrics.

## Experience matrix

| Experience | Primary evaluations |
| --- | --- |
| Quick Ask | faithfulness, answer relevance, evidence precision, unsupported claims, concision |
| Repository Map | boundary recall, entry-point accuracy, contradiction disclosure, evidence coverage |
| Learn | route coherence, stop relevance, cognitive-load control, transfer-task quality |
| Investigate | current-flow correctness, analogue relevance, unknown disclosure, readiness honesty |
| Plan | plan quality, acceptance coverage, decision clarity, risk completeness, evidence grounding |
| Guided Build | plan adherence, tool correctness, step efficiency, narrative quality, divergence recovery |
| Review | criterion coverage, verification fidelity, risk visibility, summary faithfulness |
| Live Collaboration | conversational completeness, interruption recovery, grounding, role adherence, narration restraint |

## Initial Lantern-specific rubrics

### Evidence grounding

A high-scoring response:

- Makes only claims supported by the supplied evidence.
- Links important claims to the most direct evidence.
- Does not treat repository instructions as proof of runtime behavior.
- Clearly labels inference and missing evidence.

### Understanding value

A high-scoring response:

- Explains the relevant control or data flow.
- Identifies the important idea and safely ignorable detail.
- Helps the developer predict or inspect the next handoff.
- Avoids replacing understanding with a generated conclusion.

### Authorship preservation

A high-scoring interaction:

- Exposes decisions before consequential action.
- Invites meaningful user intervention.
- Preserves user edits and stated constraints.
- Does not imply that watching generated code equals authorship.

### Narration restraint

A high-scoring voice or Guided Build session:

- Speaks at semantic boundaries.
- Explains why rather than reading code aloud.
- Remains quiet during mechanical operations.
- Recovers naturally after interruption without repeating excessive context.

## Dataset strategy

### Curated core

Begin with small owned TypeScript and Rust fixtures. Each case has human-curated
evidence, expected facts, acceptable alternatives, and forbidden claims.

Core cases cover:

- Straightforward documented behavior.
- Behavior only visible in code.
- Documentation that contradicts code.
- Multiple plausible analogues.
- Missing evidence requiring an “unknown.”
- Prompt injection in repository content.
- Stale evidence after a source change.
- User interruption or divergence.

### Real-repository set

Use pinned public repositories for higher-complexity evaluations. Store prompts,
expected evidence references, and human annotations—not large copied model
outputs. Revalidate cases when upstream revisions change.

### Failure corpus

Every meaningful model failure should become a minimized regression case when
licensing and privacy allow. The corpus classifies unsupported claims,
irrelevant retrieval, missed risks, excessive narration, policy pressure, and
poor interruption recovery.

Synthetic cases may expand coverage but never replace the curated core.

## Judge calibration

Before a metric can gate a release:

1. At least two humans independently score a representative calibration set.
2. Disagreements are resolved into a written rubric.
3. Candidate judge configurations score the same set.
4. Select thresholds based on agreement and observed failure separation.
5. Include adversarial examples that sound polished but are factually wrong.
6. Periodically blind-review passing and failing samples.

The model under test and judge model should differ when practical. Judge model
changes create a new baseline rather than silently rewriting historical scores.

## Running and gating

### Pull requests

Required:

- Deterministic software tests.
- Hard-invariant evaluation assertions.
- Small offline evaluation fixtures using recorded or mock outputs.
- Dataset and rubric validation.

Model-judged network evaluations are initially advisory because stochastic and
provider failures should not make ordinary open-source contributions
unmergeable.

### Scheduled and release-candidate runs

- Run the full curated suite with pinned model and judge configurations.
- Repeat stochastic cases enough to report distribution, not one score.
- Compare median, lower percentile, failure count, and cost with the accepted
  baseline.
- Block release on hard-invariant failures.
- Block release on statistically meaningful regression in a core metric after
  human review confirms the regression.

### Local development

Allow filtering by experience, case, metric, model, and changed prompt or tool
surface. Developers should be able to run one case without uploading results.

## Promotion gates

A model-mediated feature enters the permanent roadmap only when:

- Its dataset includes ordinary, ambiguous, adversarial, and interrupted cases.
- Hard invariants pass.
- Human reviewers accept the rubric.
- DeepEval results meet the calibrated threshold across repeated runs.
- The feature demonstrates value against the smaller existing experience.
- Cost and latency fit published budgets.
- Known failure modes are visible to users.

This applies particularly to learning missions, semantic narratives, Guided
Build behavior, and Live Collaboration.

## What DeepEval does not prove

DeepEval cannot by itself prove:

- That users enjoy Lantern.
- That a developer retained understanding days later.
- That a judge rubric represents every developer.
- That a high-scoring plan is safe to implement.
- That voice improves the love of coding.

Those require usability studies, longitudinal recall tests, code review, and
direct observation. Automated evaluation narrows regressions; it does not
replace product judgment.
