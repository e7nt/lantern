# Live Collaboration

## Status

- **Status:** Proposed evaluation track
- **Release commitment:** Not yet committed to v0.1
- **Working name:** Live Collaboration
- **Provider spike:** OpenAI Realtime API

## Product idea

Live Collaboration makes Lantern feel like a developer sharing a screen with a
thoughtful collaborator. The user can speak naturally while navigating,
learning, investigating, planning, or implementing. Lantern responds in voice,
points to the code it is discussing, narrates the intent of meaningful actions,
and can be interrupted at any moment.

The goal is not continuous synthetic narration. The goal is a low-latency,
interruptible conversation grounded in the exact editor, plan, evidence, and
implementation state visible to the user.

## Example interaction

```text
User: Walk me through where this request is validated.

Lantern: It enters through handle_request here, then delegates to—

User: Stop. Why is validation not in the domain layer?

Lantern: The current evidence suggests this check depends on transport metadata.
I have not yet confirmed whether the domain type can represent that metadata.
Would you like me to inspect the constructor and its tests?

User: Yes, but do not change anything.

Lantern: Understood. I am switching to a read-only investigation branch.
```

During Guided Build:

```text
Lantern: I am about to add the repository contract required by plan task T3.
This changes a public trait but does not yet alter persistence.

User: Pause. Show me the analogous contract first.

Lantern: Paused before applying the chapter. The closest observed pattern is—
```

## Design principles

1. Voice is another control and explanation surface over the same Lantern
   session, not a separate source of truth.
2. Interrupting speech stops audio immediately and prevents unstarted actions.
3. Spoken intent does not bypass normal capability checks or approval gates.
4. Lantern narrates intent, evidence, decisions, and results—not hidden model
   reasoning.
5. Important decisions, approvals, commands, and edits remain visible and
   durable in the editor.
6. Silence is a feature. Mechanical work and obvious navigation should not be
   narrated unless requested.
7. The user can switch between voice, keyboard, and direct editing without
   losing conversational or implementation state.

## Interaction modes

### Conversational Ask

- Ask about the selection, symbol, diagnostic, diff, or plan.
- Receive a concise spoken answer while evidence is highlighted.
- Interrupt, ask for a simpler explanation, or request deeper inspection.

### Guided Tour

- Lantern moves through a repository learning mission.
- The user controls pacing with voice: next, back, pause, explain, skip, or
  branch.
- The editor shows the active location and return path.

### Investigation Pair

- The user and Lantern jointly trace behavior.
- Lantern announces what it will inspect next and why.
- Read-only policy remains structurally enforced.

### Planning Partner

- Discuss objectives, alternatives, risks, and acceptance criteria.
- Proposed decisions appear as editable plan changes.
- Spoken agreement is not sufficient for irreversible or high-impact approval;
  the editor presents a confirmable artifact.

### Guided Build Pair

- Lantern states the next chapter before staging it.
- The user can interrupt before or during narration.
- Code application remains a separately controlled transition.
- Questions create a pause, not a parallel mutation stream.

### Review Companion

- Walk acceptance criteria and logical changes aloud.
- Navigate to evidence, diffs, and verification results.
- Mark review questions without publishing or changing code automatically.

## State model

```text
idle
  -> listening
  -> interpreting
  -> responding
  -> listening

responding
  -> interrupted
  -> listening

interpreting
  -> proposes_action
  -> awaiting_confirmation
  -> action_running
  -> action_completed

any active state
  -> muted | disconnected | failed
```

Speech state and work state are separate. Audio interruption cancels the spoken
response. It cancels or pauses work only according to the action's declared
interruption policy:

| Work state | Voice interruption behavior |
| --- | --- |
| Read/search not yet started | Cancel |
| Bounded read/search running | Cancel and discard late results |
| Plan proposal generation | Cancel generation; retain last durable revision |
| Staged edit generation | Cancel generation; do not apply |
| Chapter awaiting application | Remain paused |
| Editor transaction applying | Finish or roll back atomically; never stop mid-operation |
| Approved command running | Request process cancellation and report actual outcome |
| Verification running | Ask whether to cancel or continue silently |

## Architecture hypothesis

```text
Lantern renderer/workbench
  microphone permission and capture
  audio playback
  visible transcript and voice controls
  editor presence events
          |
          | WebRTC media + realtime control events
          v
OpenAI Realtime session
          |
          | constrained function calls
          v
Lantern voice coordinator in daemon
  session binding
  tool mediation
  policy checks
  action/approval bridge
  durable transcript projection
          |
          v
Existing Lantern services
```

The evaluation should compare two designs:

1. Direct WebRTC media from the workbench using a short-lived client secret,
   with all function calls mediated by the daemon.
2. Daemon-managed WebSocket audio and events.

The WebRTC design is the default hypothesis because it provides low-latency
media handling and server-managed truncation of unplayed audio. The WebSocket
design may provide simpler centralized policy and logging but makes Lantern
responsible for playback synchronization and interruption truncation.

Long-lived provider credentials must never be exposed to the renderer.

## Tool boundary

The realtime model receives a small voice-specific tool surface:

- `editor.describe_presence`
- `editor.reveal_evidence`
- `session.ask`
- `session.pause`
- `session.resume`
- `investigation.inspect`
- `plan.propose_change`
- `build.describe_next_chapter`
- `build.stage_next_chapter`
- `review.open_item`

These tools express intent. They do not directly read arbitrary files, execute
commands, or apply edits. The daemon maps them to existing Lantern operations
after policy and state validation.

## Interruption contract

- Voice activity detection may provide natural barge-in.
- Push-to-talk must be available as a deterministic user-selected interaction
  mode.
- Playback stops within 150 ms p95 after interruption is detected locally.
- No new tool call starts after an interruption terminal event.
- Unplayed model audio is removed from conversational state.
- The visible transcript distinguishes heard output from truncated output.
- A keyboard Escape action always stops speech and requests cancellation.
- A dedicated mute control disables microphone capture at the source.

## Narration policy

Narrate:

- The next meaningful intention.
- Why a decision matters.
- Evidence supporting a claim.
- A change in plan or risk.
- The result of a requested action.
- Failures, uncertainty, and verification status.

Normally collapse:

- Formatting and import organization.
- Repetitive searches.
- Generated output.
- Token-by-token code production.
- Internal retries that do not change the result.
- Hidden model reasoning.

User-selectable levels:

| Level | Behavior |
| --- | --- |
| Quiet | Speak only direct answers, blockers, and requested summaries |
| Pair | Speak intentions, decisions, and results; recommended default |
| Teaching | Add sparse predictions and transferable explanations |
| Detailed | Narrate most semantic operations for accessibility or close study |

## Privacy and security requirements

- Microphone use is opt-in per session and visibly indicated.
- The user can inspect whether audio, transcripts, code, and tool results are
  transmitted.
- Audio retention behavior is documented separately from Lantern's local
  transcript storage.
- Raw audio is not stored locally by default.
- Durable transcripts are opt-in and redact configured sensitive terms.
- Repository content remains subject to normal transmission policy.
- Voice cannot grant itself stronger capabilities.
- High-impact approvals require an editor confirmation by default.
- Function-call arguments are treated as untrusted model output.
- Prompt injection encountered in source or tool output cannot alter voice
  permissions.

## Evaluation spike

### Scope

Build a disposable prototype that can:

1. Start and stop a realtime voice session.
2. Answer a question about a mocked editor selection.
3. Highlight one evidence range while speaking.
4. Be interrupted naturally and through push-to-talk.
5. Invoke a read-only mediated tool.
6. Pause before a mocked Guided Build chapter.
7. Produce a visible transcript with interrupted output marked.

It must not edit repository files or execute commands.

### Metrics

- Time from end of user speech to first audible response.
- Barge-in detection to stopped playback.
- False interruption and missed interruption rate.
- Tool-call round-trip latency.
- Percentage of spoken claims linked to visible evidence.
- Frequency of narration judged excessive or insufficient.
- Conversation recovery after interruption.
- Audio and text cost per 30-minute session.
- User recall and task-completion comparison with text-only Guided Build.
- CPU, memory, and battery impact.

### Test scenarios

- User interrupts mid-sentence with a new question.
- User says “stop” while a tool is pending.
- Background noise triggers voice activity detection.
- Network drops during model speech.
- The realtime model requests an unavailable or forbidden tool.
- The editor selection changes during an explanation.
- A staged chapter becomes stale while being discussed.
- The user switches from voice to keyboard and back.
- Microphone permission is revoked during the session.
- Provider limits or authentication expire.

## Evaluation gate

Promote Live Collaboration into the release roadmap only if:

- Interruption feels immediate and preserves correct session state.
- Voice improves understanding or control in user testing.
- Spoken narration stays grounded in visible evidence.
- The same policy engine governs voice and text actions.
- Costs and resource use are visible and acceptable.
- Microphone and transmission behavior are understandable.
- The product remains fully usable without voice or OpenAI.

If the gate fails, retain push-to-talk questions and optional text-to-speech as
smaller accessibility features without committing to a continuous voice agent.
