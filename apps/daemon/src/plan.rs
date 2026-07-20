use lantern_protocol::{AgentIntent, MAX_QUESTION_BYTES};
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
        writeln!(
            file,
            "---\nlantern_plan: 1\nstatus: active\n---\n\n# Active implementation plan\n\n{}",
            brief.text.trim()
        )
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
}
