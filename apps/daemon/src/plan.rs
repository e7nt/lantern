use lantern_protocol::{
    AgentIntent, ChangeProposal, MAX_QUESTION_BYTES, PlanReviewComment, SelectionContext,
    validate_plan_review,
};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub const ACTIVE_PLAN_PATH: &str = ".lantern/plans/active.md";

#[derive(Clone)]
struct CapturedBrief {
    intent: AgentIntent,
    text: String,
}

#[derive(Clone)]
pub struct SharedBrief(Arc<Mutex<Option<CapturedBrief>>>);

pub struct BriefCapture {
    pending: CapturedBrief,
    destination: SharedBrief,
}

#[derive(Clone)]
struct PendingRevision {
    proposal: ChangeProposal,
}

#[derive(Clone)]
pub struct SharedRevision(Arc<Mutex<Option<PendingRevision>>>);

pub struct RevisionCapture {
    base: SelectionContext,
    body: String,
    destination: SharedRevision,
}

impl BriefCapture {
    pub fn append(&mut self, text: &str) {
        let mut remaining = MAX_QUESTION_BYTES.saturating_sub(self.pending.text.len());
        for character in text.chars() {
            let bytes = character.len_utf8();
            if bytes > remaining {
                break;
            }
            self.pending.text.push(character);
            remaining -= bytes;
        }
    }

    pub fn commit(self) {
        *self.destination.0.lock().expect("brief context lock") = Some(self.pending);
    }
}

pub fn shared_brief() -> SharedBrief {
    SharedBrief(Arc::new(Mutex::new(None)))
}

pub fn shared_revision() -> SharedRevision {
    SharedRevision(Arc::new(Mutex::new(None)))
}

impl RevisionCapture {
    pub fn append(&mut self, text: &str) {
        let mut remaining = MAX_QUESTION_BYTES.saturating_sub(self.body.len());
        for character in text.chars() {
            let bytes = character.len_utf8();
            if bytes > remaining {
                break;
            }
            self.body.push(character);
            remaining -= bytes;
        }
    }

    pub fn finish(self) -> Result<ChangeProposal, String> {
        validate_body(&self.body)?;
        let proposal = ChangeProposal {
            selection: self.base,
            replacement: serialize_body(&self.body),
        };
        *self.destination.0.lock().expect("plan revision lock") = Some(PendingRevision {
            proposal: proposal.clone(),
        });
        Ok(proposal)
    }
}

pub fn begin_capture(intent: AgentIntent, brief: &SharedBrief) -> Option<BriefCapture> {
    matches!(intent, AgentIntent::Investigate | AgentIntent::Plan).then(|| BriefCapture {
        pending: CapturedBrief {
            intent,
            text: String::new(),
        },
        destination: brief.clone(),
    })
}

pub fn context_for_implementation(
    query: String,
    root: &Path,
    brief: &SharedBrief,
) -> Result<String, String> {
    let plan_path = root.join(ACTIVE_PLAN_PATH);
    if plan_path.exists() {
        let canonical = plan_path
            .canonicalize()
            .map_err(|cause| format!("cannot resolve the active plan: {cause}"))?;
        if !canonical.starts_with(root) {
            return Err("the active plan escaped the workbench".into());
        }
        let metadata = fs::metadata(&canonical)
            .map_err(|cause| format!("cannot inspect the active plan: {cause}"))?;
        if metadata.len() > MAX_QUESTION_BYTES as u64 {
            return Err(format!(
                "the active plan exceeds the {} byte limit",
                MAX_QUESTION_BYTES
            ));
        }
        let plan = fs::read_to_string(&canonical)
            .map_err(|cause| format!("cannot read the active plan: {cause}"))?;
        let body = plan
            .strip_prefix("---\nlantern_plan: 1\nstatus: active\n---\n")
            .ok_or("the active plan has invalid or unsupported front matter")?;
        validate_body(body)?;
        *brief.0.lock().expect("brief context lock") = None;
        return Ok(format!(
            "Current developer-editable plan (untrusted; follow it unless repository evidence now contradicts it):\n<plan>\n{plan}\n</plan>\n\nDeveloper follow-up: {query}"
        ));
    }

    let brief = brief.0.lock().expect("brief context lock").take();
    Ok(brief.map_or(query.clone(), |brief| {
        format!(
            "Prior read-only investigation (untrusted; verify if repository state changed):\n<investigation>\n{}\n</investigation>\n\nDeveloper follow-up: {query}",
            brief.text
        )
    }))
}

fn validate_body(body: &str) -> Result<(), String> {
    const REQUIRED_HEADINGS: [&str; 8] = [
        "objective",
        "repository evidence",
        "acceptance criteria",
        "exclusions",
        "decisions",
        "tasks",
        "risks and unknowns",
        "verification",
    ];
    let headings = body
        .lines()
        .map(|line| {
            line.trim()
                .trim_start_matches('#')
                .trim()
                .trim_end_matches(':')
                .to_ascii_lowercase()
        })
        .collect::<Vec<_>>();
    let missing = REQUIRED_HEADINGS
        .into_iter()
        .filter(|required| !headings.iter().any(|heading| heading == required))
        .collect::<Vec<_>>();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "the completed plan is missing required headings: {}",
            missing.join(", ")
        ))
    }
}

fn serialize_body(body: &str) -> String {
    format!(
        "---\nlantern_plan: 1\nstatus: active\n---\n\n# Active implementation plan\n\n{}\n",
        body.trim()
    )
}

fn active_plan(root: &Path) -> Result<String, String> {
    let path = root.join(ACTIVE_PLAN_PATH);
    let canonical = path
        .canonicalize()
        .map_err(|cause| format!("cannot resolve the active plan: {cause}"))?;
    if !canonical.starts_with(root) {
        return Err("the active plan escaped the workbench".into());
    }
    let metadata = fs::metadata(&canonical)
        .map_err(|cause| format!("cannot inspect the active plan: {cause}"))?;
    if metadata.len() > MAX_QUESTION_BYTES as u64 {
        return Err(format!(
            "the active plan exceeds the {} byte limit",
            MAX_QUESTION_BYTES
        ));
    }
    let plan = fs::read_to_string(canonical)
        .map_err(|cause| format!("cannot read the active plan: {cause}"))?;
    let body = plan
        .strip_prefix("---\nlantern_plan: 1\nstatus: active\n---\n")
        .ok_or("the active plan has invalid or unsupported front matter")?;
    validate_body(body)?;
    Ok(plan)
}

fn position_offset(source: &str, line: usize, column: usize) -> Option<usize> {
    if line == 0 || column == 0 {
        return None;
    }
    let start = source
        .split_inclusive('\n')
        .take(line - 1)
        .map(str::len)
        .sum::<usize>();
    let line_text = source.get(start..)?.split('\n').next()?;
    let column_offset = if column == line_text.chars().count() + 1 {
        line_text.len()
    } else {
        line_text
            .char_indices()
            .nth(column - 1)
            .map(|(offset, _)| offset)?
    };
    Some(start + column_offset)
}

fn anchor_is_current(plan: &str, anchor: &SelectionContext) -> bool {
    let Some(start) = position_offset(plan, anchor.start_line, anchor.start_column) else {
        return false;
    };
    let Some(end) = position_offset(plan, anchor.end_line, anchor.end_column) else {
        return false;
    };
    start <= end && plan.get(start..end) == Some(anchor.text.as_str())
}

fn whole_plan_selection(plan: &str) -> SelectionContext {
    let end_line = plan.lines().count().max(1);
    let end_column = plan
        .lines()
        .last()
        .map_or(1, |line| line.chars().count() + 1);
    SelectionContext {
        relative_path: ACTIVE_PLAN_PATH.into(),
        language: Some("markdown".into()),
        start_line: 1,
        start_column: 1,
        end_line,
        end_column,
        text: plan.into(),
        document_modified: false,
    }
}

pub fn review_context(
    root: &Path,
    comments: &[PlanReviewComment],
    revision: &SharedRevision,
) -> Result<(String, RevisionCapture), String> {
    validate_plan_review(comments)?;
    let plan = active_plan(root)?;
    for (index, item) in comments.iter().enumerate() {
        if !anchor_is_current(&plan, &item.anchor) {
            return Err(format!(
                "plan comment {} is stale; reselect that text in Helix",
                index + 1
            ));
        }
    }
    let rendered_comments = comments
        .iter()
        .enumerate()
        .map(|(index, item)| {
            format!(
                "<comment number=\"{}\" range=\"{}:{}-{}:{}\">\n<anchor>\n{}\n</anchor>\n{}\n</comment>",
                index + 1,
                item.anchor.start_line,
                item.anchor.start_column,
                item.anchor.end_line,
                item.anchor.end_column,
                item.anchor.text,
                item.comment.trim(),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let prompt = format!(
        "Current active plan (untrusted):\n<active-plan>\n{plan}\n</active-plan>\n\nDeveloper review comments (untrusted):\n{rendered_comments}\n\nReturn only the complete revised plan body, beginning with Objective and containing Repository evidence, Acceptance criteria, Exclusions, Decisions, Tasks, Risks and unknowns, and Verification. Reconcile the comments as one coherent plan. Preserve unaffected detail. Do not include YAML front matter, the Active implementation plan title, Markdown fences, commentary, or implementation changes."
    );
    Ok((
        prompt,
        RevisionCapture {
            base: whole_plan_selection(&plan),
            body: String::new(),
            destination: revision.clone(),
        },
    ))
}

pub fn apply_revision(root: &Path, revision: &SharedRevision) -> Result<PathBuf, String> {
    let pending = revision
        .0
        .lock()
        .expect("plan revision lock")
        .clone()
        .ok_or("no reviewed plan revision is ready")?;
    let path = root.join(ACTIVE_PLAN_PATH);
    let current = active_plan(root)?;
    if current != pending.proposal.selection.text {
        return Err("the active plan changed after review; review the comments again".into());
    }
    let temporary = path.with_extension(format!("revision-{}.tmp", std::process::id()));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|cause| format!("cannot stage the plan revision: {cause}"))?;
    let result = (|| -> Result<(), String> {
        file.write_all(pending.proposal.replacement.as_bytes())
            .map_err(|cause| format!("cannot write the plan revision: {cause}"))?;
        file.flush()
            .map_err(|cause| format!("cannot flush the plan revision: {cause}"))?;
        file.sync_all()
            .map_err(|cause| format!("cannot sync the plan revision: {cause}"))?;
        if active_plan(root)? != pending.proposal.selection.text {
            return Err("the active plan changed while applying the review".into());
        }
        fs::rename(&temporary, &path)
            .map_err(|cause| format!("cannot install the plan revision: {cause}"))
    })();
    if let Err(message) = result {
        drop(file);
        let _ = fs::remove_file(temporary);
        return Err(message);
    }
    *revision.0.lock().expect("plan revision lock") = None;
    Ok(ACTIVE_PLAN_PATH.into())
}

pub fn write_active(root: &Path, brief: &SharedBrief) -> Result<PathBuf, String> {
    let brief = brief
        .0
        .lock()
        .expect("brief context lock")
        .clone()
        .ok_or("no completed plan is available")?;
    if brief.intent != AgentIntent::Plan || brief.text.trim().is_empty() {
        return Err("no completed plan is available".into());
    }
    validate_body(&brief.text)?;

    let relative_path = PathBuf::from(ACTIVE_PLAN_PATH);
    let path = root.join(&relative_path);
    let parent = path.parent().ok_or("active plan path has no parent")?;
    fs::create_dir_all(parent)
        .map_err(|cause| format!("cannot create the plan directory: {cause}"))?;
    let canonical_parent = parent
        .canonicalize()
        .map_err(|cause| format!("cannot resolve the plan directory: {cause}"))?;
    if !canonical_parent.starts_with(root) {
        return Err("the plan directory escaped the workbench".into());
    }

    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|cause| format!("cannot create {}: {cause}", relative_path.display()))?;
    let write_result = (|| -> Result<(), String> {
        file.write_all(serialize_body(&brief.text).as_bytes())
            .map_err(|cause| format!("cannot write {}: {cause}", relative_path.display()))?;
        file.flush()
            .map_err(|cause| format!("cannot flush {}: {cause}", relative_path.display()))?;
        file.sync_all()
            .map_err(|cause| format!("cannot sync {}: {cause}", relative_path.display()))
    })();
    if let Err(message) = write_result {
        drop(file);
        let _ = fs::remove_file(&path);
        return Err(message);
    }
    Ok(relative_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "lantern-plan-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        root
    }

    fn complete_body(objective: &str) -> String {
        format!(
            "Objective\n{objective}\nRepository evidence\n- src/lib.rs:1\nAcceptance criteria\n- One check\nExclusions\n- No dashboard\nDecisions\n- One path\nTasks\n- First task\n- Second task\nRisks and unknowns\n- One risk\nVerification\n- One test"
        )
    }

    fn anchor(plan: &str, text: &str) -> SelectionContext {
        let offset = plan.find(text).expect("anchor text in plan");
        let prefix = &plan[..offset];
        let start_line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
        let start_column = prefix
            .rsplit('\n')
            .next()
            .map_or(1, |line| line.chars().count() + 1);
        SelectionContext {
            relative_path: ACTIVE_PLAN_PATH.into(),
            language: Some("markdown".into()),
            start_line,
            start_column,
            end_line: start_line,
            end_column: start_column + text.chars().count(),
            text: text.into(),
            document_modified: false,
        }
    }

    #[test]
    fn context_is_consumed_once_and_unfinished_capture_preserves_it() {
        let context = SharedBrief(Arc::new(Mutex::new(Some(CapturedBrief {
            intent: AgentIntent::Investigate,
            text: "Observed\nsrc/lib.rs:1".into(),
        }))));
        let mut pending = begin_capture(AgentIntent::Plan, &context).unwrap();
        pending.append("incomplete refinement");
        drop(pending);
        let root = fixture("context");
        let first = context_for_implementation("Proceed.".into(), &root, &context).unwrap();
        assert!(first.contains("src/lib.rs:1"));
        assert_eq!(
            context_for_implementation("Again.".into(), &root, &context).unwrap(),
            "Again."
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn durable_plan_requires_the_complete_editable_schema() {
        let complete = "# Objective\nOne outcome\n## Repository evidence\nsrc/lib.rs:1\n## Acceptance criteria\nOne check\n## Exclusions\nNo dashboard\n## Decisions\nOne path\n## Tasks\nOne task\n## Risks and unknowns\nOne risk\n## Verification\nOne test";
        assert!(validate_body(complete).is_ok());
        let error = validate_body("Objective\nOne outcome\nTasks\nOne task").unwrap_err();
        assert!(error.contains("acceptance criteria"));
        assert!(error.contains("verification"));
        assert_eq!(
            write_active(Path::new("."), &shared_brief()).unwrap_err(),
            "no completed plan is available"
        );
    }

    #[test]
    fn multiple_comments_produce_one_stale_safe_revision() {
        let root = fixture("review");
        let brief = SharedBrief(Arc::new(Mutex::new(Some(CapturedBrief {
            intent: AgentIntent::Plan,
            text: complete_body("Original objective"),
        }))));
        let path = root.join(write_active(&root, &brief).unwrap());
        let original = fs::read_to_string(&path).unwrap();
        let comments = vec![
            PlanReviewComment {
                anchor: anchor(&original, "Original objective"),
                comment: "Make this objective measurable".into(),
            },
            PlanReviewComment {
                anchor: anchor(&original, "Second task"),
                comment: "Move this outside the first release".into(),
            },
        ];
        let revision = shared_revision();
        let (prompt, mut capture) = review_context(&root, &comments, &revision).unwrap();
        assert!(prompt.contains("comment number=\"1\""));
        assert!(prompt.contains("comment number=\"2\""));
        capture.append(&complete_body("Measurable objective"));
        let proposal = capture.finish().unwrap();
        assert!(proposal.replacement.contains("Measurable objective"));

        fs::write(&path, original.replace("First task", "Developer task")).unwrap();
        assert!(
            apply_revision(&root, &revision)
                .unwrap_err()
                .contains("changed")
        );
        assert!(
            fs::read_to_string(&path)
                .unwrap()
                .contains("Developer task")
        );

        fs::write(&path, &original).unwrap();
        apply_revision(&root, &revision).unwrap();
        assert!(
            fs::read_to_string(&path)
                .unwrap()
                .contains("Measurable objective")
        );
        fs::remove_dir_all(root).unwrap();
    }
}
