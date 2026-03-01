use crate::types::arena_alloc_str;
use crate::types::*;
use regex::Regex;
use std::sync::LazyLock;

static CHECKBOX_TASK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^-\s+\[([x /])\]\s+").unwrap());
static MARKER_TASK_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^-\s+(TODO|DONE)\s+").unwrap());

/// Extract tasks
pub fn extract_tasks<'a>(
    _elements: &[Element<'a>],
    source: &str,
    arena: &'a bumpalo::Bump,
) -> Vec<Task<'a>> {
    let mut tasks = Vec::new();

    // Extract checkbox tasks
    for captures in CHECKBOX_TASK_RE.captures_iter(source) {
        if let Some(state_match) = captures.get(1) {
            let state_str = match state_match.as_str().trim() {
                "" => "unchecked",
                "x" => "checked",
                "/" => "in_progress",
                _ => "unchecked",
            };
            tasks.push(Task::new(TaskState::new(arena_alloc_str(arena, state_str))));
        }
    }

    // Extract marker tasks
    for captures in MARKER_TASK_RE.captures_iter(source) {
        if let Some(marker_match) = captures.get(1) {
            tasks.push(Task::new(TaskState::new(arena_alloc_str(
                arena,
                &marker_match.as_str().to_lowercase(),
            ))));
        }
    }

    tasks
}
