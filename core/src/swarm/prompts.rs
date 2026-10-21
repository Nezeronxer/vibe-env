//! Requests of the roles of the swarm. Free text of the person and the code of the project always go
//! in `fenced` blocks (data, not instructions).
use crate::prompt::{fenced, Brief};

pub const SPLIT_SHAPE: &str = r#"{"parts":[{"id":"p1","title":"short name","files":["exact/relative/path.ext"],"brief":"what this part does"}]}"#;
pub const CONVOY_SHAPE: &str = r#"{"tasks":[{"id":"t1","title":"short name","text":"what to do, enough to do it alone","depends_on":[]}]}"#;
pub const PROPOSAL_SHAPE: &str = r#"{"approach":"how you would do the task, in a few sentences"}"#;
pub const VOTE_SHAPE: &str = r#"{"vote":"<id of the proposal>","why":"one sentence"}"#;
pub const COMPARE_SHAPE: &str = r#"{"scores":[{"variant":"A","correctness":0,"coverage":0,"simplicity":0,"risk":0,"why":"one sentence"}],"recommend":"A","advice":"one or two sentences for the person"}"#;
pub const REVIEW_SHAPE: &str = r#"{"verdict":"approve","issues":[]}"#;
pub const CHOICE_SHAPE: &str = r#"{"choice":"<one of the options>"}"#;

fn schema(props: serde_json::Value, required: &[&str]) -> String {
    serde_json::json!({"type": "object", "properties": props, "required": required, "additionalProperties": true}).to_string()
}
pub fn split_schema() -> String {
    schema(serde_json::json!({"parts": {"type": "array", "items": {"type": "object"}}}), &["parts"])
}
pub fn convoy_schema() -> String {
    schema(serde_json::json!({"tasks": {"type": "array", "items": {"type": "object"}}}), &["tasks"])
}
pub fn proposal_schema() -> String {
    schema(serde_json::json!({"approach": {"type": "string"}}), &["approach"])
}
pub fn vote_schema() -> String {
    schema(serde_json::json!({"vote": {"type": "string"}, "why": {"type": "string"}}), &["vote"])
}
pub fn compare_schema() -> String {
    schema(serde_json::json!({"scores": {"type": "array", "items": {"type": "object"}}, "recommend": {"type": "string"}, "advice": {"type": "string"}}), &["scores", "recommend"])
}
pub fn choice_schema() -> String {
    schema(serde_json::json!({"choice": {"type": "string"}}), &["choice"])
}

fn plan_of(b: &Brief) -> String {
    if b.plan.is_empty() { String::new() } else { format!("\n\nPlan so far:\n{}", b.plan.iter().map(|x| format!("- {x}")).collect::<Vec<_>>().join("\n")) }
}

/// Coordinator of a crew: split the task into parts with their own files.
pub fn split_request(task: &str, b: &Brief, max: usize, fix: Option<&str>) -> String {
    let again = fix.map(|f| format!("\n\nYour previous split could not be used: {f}. Split again.")).unwrap_or_default();
    format!(
        "Split the task below into at most {max} parts that different engineers do at the same time, each in a copy of the project \
         of its own. The parts must not touch the same files: list for every part every file it creates or edits, as exact relative \
         paths (a folder means everything inside it; no wildcards). A file belongs to one part only. Add shared glue to one part. \
         Reply with one JSON object of this shape and nothing after it: {SPLIT_SHAPE}{again}{}\n\n{}",
        plan_of(b),
        fenced("TASK", task)
    )
}

/// Coordinator of a convoy: tasks with dependencies out of one phrase.
pub fn convoy_request(text: &str, project_hint: &str, fix: Option<&str>) -> String {
    let again = fix.map(|f| format!("\n\nYour previous answer could not be used: {f}. Answer again.")).unwrap_or_default();
    format!(
        "Break the request below into tasks for coding agents, at most 12. Each task must be doable alone in a copy of the project. \
         `depends_on` lists the ids of the tasks that must be merged before this one can start. If the request is too unclear \
         to break down, ask the person instead of guessing. Reply with one JSON object of this shape and nothing after it: \
         {CONVOY_SHAPE}{again}\n\n{}\n\n{}",
        fenced("PROJECT", project_hint),
        fenced("REQUEST", text)
    )
}

/// A member of the council proposes an approach.
pub fn proposal_request(task: &str, b: &Brief) -> String {
    format!(
        "Propose how the task below should be done. Do not change any file. Think about the design, the risks and what to test. \
         Reply with one JSON object of this shape and nothing after it: {PROPOSAL_SHAPE}{}\n\n{}",
        plan_of(b),
        fenced("TASK", task)
    )
}

/// The one round of the vote: every member sees all the proposals.
pub fn vote_request(task: &str, proposals: &[(String, String)]) -> String {
    let list: String = proposals.iter().map(|(id, t)| format!("Proposal {id}:\n{t}\n\n")).collect();
    format!(
        "Several engineers proposed an approach to the task below. Vote for the one that should be built (yours too, if it is the best). \
         Reply with one JSON object of this shape and nothing after it: {VOTE_SHAPE}\n\n{}\n\n{}",
        fenced("TASK", task),
        fenced("PROPOSALS", list.trim_end())
    )
}

/// A member of the council votes on the finished work.
pub fn council_review_request(task: &str, diff: &str, checks: &str) -> String {
    format!(
        "Judge the change below against the request it answers. Do not change any file. Reply with one JSON object of this shape and \
         nothing after it: {REVIEW_SHAPE} (verdict `approve` or `changes`; list what must change).\n\n{}\n\n{}\n\n{}",
        fenced("TASK", task),
        fenced("CHECKS", checks),
        fenced("DIFF", diff)
    )
}

/// Reviewer of a race: marks and a recommendation among the variants that passed.
pub fn compare_request(task: &str, variants: &[(String, String, String)]) -> String {
    let mut blocks = String::new();
    for (id, summary, diff) in variants {
        blocks.push_str(&format!("{}\n\n", fenced(&format!("VARIANT_{id}"), &format!("What the executor said:\n{summary}\n\nChange:\n{diff}"))));
    }
    format!(
        "Several engineers did the same task, each in a copy of their own; all of them pass the project's checks. Compare the variants. \
         Mark each from 0 to 10 on correctness, coverage (tests, edge cases), simplicity and risk (10 = the safest), recommend one and \
         tell the person why in a sentence or two. Do not change any file. Reply with one JSON object of this shape and nothing after \
         it: {COMPARE_SHAPE}\n\n{}\n\n{}",
        fenced("TASK", task),
        blocks.trim_end()
    )
}

/// The Reviewer decides a tie of the council.
pub fn tie_request(question: &str, options: &[String], context: &str) -> String {
    format!(
        "The council is split between {}. Decide. Do not change any file. Reply with one JSON object of this shape and nothing after it: \
         {CHOICE_SHAPE}\n\n{question}\n\n{}",
        options.join(", "),
        fenced("CONTEXT", context)
    )
}

/// A part of a crew.
pub fn part_prompt(task: &str, title: &str, brief: &str, files: &[String], others: &[(String, Vec<String>)]) -> String {
    let others: String = others.iter().map(|(t, f)| format!("- {t}: {}\n", f.join(", "))).collect();
    format!(
        "You do one part of a larger task; other engineers do the other parts at the same time in copies of their own.\n\n\
         Your part: {title}\n{brief}\n\nTouch only these files: {}. Anything else you change is discarded.\n\nThe other parts (not yours):\n{}\n\
         Run the project's checks and fix what they show. When finished, reply with 1-3 lines on what you did.\n\n{}",
        files.join(", "),
        others.trim_end(),
        fenced("TASK", task)
    )
}
