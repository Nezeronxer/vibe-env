//! Vendor templates (spec 18). Each layout follows the official prompting guide of the vendor,
//! see `SOURCES.md`; the rule behind every choice is named in a comment with its guide id
//! (A = Anthropic, O = OpenAI, G = Google, Q = Alibaba Qwen) and has a test in `tests/engine_prompt.rs`.
//! Where a vendor has no confirmed rule, the template has none either.
use super::{fenced, Brief, Vendor, DATA_RULE};
use crate::model::RoleKind;

/// A conflict the Integrator resolves: where the rebase goes and what git said.
#[derive(Clone, Debug, Default)]
pub struct Conflict {
    pub base: String,
    pub git_output: String,
}

/// What a role gets besides the brief. Each role reads only the fields it needs (G07.2).
#[derive(Clone, Debug, Default)]
pub struct RoleInputs {
    /// Reviewer, Integrator, and a continued Worker: the change so far.
    pub diff: Option<String>,
    /// Reviewer: output of the project's checks.
    pub gate_output: Option<String>,
    /// Worker: files the test author wrote.
    pub tests_readonly: Vec<String>,
    /// Worker: the project has a test runner.
    pub tests_exist: bool,
    /// Worker: another agent began the work; continue it (limit fallback, model switch).
    pub continue_work: bool,
    pub conflict: Option<Conflict>,
    /// Verifier: how to start the project.
    pub run_cmd: Option<String>,
}

struct Sec {
    tag: &'static str,
    title: &'static str,
    body: String,
    /// Data (fenced) as opposed to an instruction.
    data: bool,
}

fn data(tag: &'static str, title: &'static str, label: &str, text: &str) -> Sec {
    Sec { tag, title, body: fenced(label, text), data: true }
}
fn instr(tag: &'static str, title: &'static str, body: String) -> Sec {
    Sec { tag, title, body, data: false }
}
fn bullets(v: &[String]) -> String {
    v.iter().map(|x| format!("- {x}")).collect::<Vec<_>>().join("\n")
}
fn numbered(v: &[String]) -> String {
    v.iter().enumerate().map(|(i, x)| format!("{}. {x}", i + 1)).collect::<Vec<_>>().join("\n")
}

const FINAL_REPLY: &str = "When finished, reply with 1-3 lines on what you did.";

/// The sections of a role's prompt, in reading order inside their kind.
fn sections(vendor: Vendor, role: RoleKind, b: &Brief, i: &RoleInputs) -> Vec<Sec> {
    let mut v = vec![];
    let plan_text = |b: &Brief| {
        let mut s = format!("Goal: {}", b.goal);
        if !b.constraints.is_empty() {
            s.push_str(&format!("\nConstraints:\n{}", bullets(&b.constraints)));
        }
        if !b.done_when.is_empty() {
            s.push_str(&format!("\nDone when:\n{}", bullets(&b.done_when)));
        }
        if !b.plan.is_empty() {
            s.push_str(&format!("\nPlan:\n{}", numbered(&b.plan)));
        }
        s
    };
    match role {
        RoleKind::Worker => {
            v.push(data("request", "Task", "TASK", &b.original));
            if let Some(spec) = &b.spec {
                v.push(data("spec", "Spec", "SPEC", spec));
            }
            if !b.files.is_empty() {
                v.push(data("files", "Files", "FILES", &b.files.join("\n")));
            }
            if let Some(d) = i.diff.as_ref().filter(|_| i.continue_work) {
                v.push(data("diff", "Work so far", "DIFF", d));
            }
            v.push(instr("goal", "Goal", b.goal.clone()));
            if i.continue_work {
                v.push(instr(
                    "continue",
                    "Continue",
                    "Another agent started this task and stopped. Continue what it began; its changes so far are in the DIFF block. Do not start over.".into(),
                ));
            }
            if !b.constraints.is_empty() {
                v.push(instr("constraints", "Constraints", bullets(&b.constraints)));
            }
            if !b.done_when.is_empty() {
                v.push(instr("done_when", "Done when", bullets(&b.done_when)));
            }
            if !b.plan.is_empty() {
                v.push(instr("plan", "Plan", numbered(&b.plan)));
            }
            if !i.tests_readonly.is_empty() {
                v.push(instr(
                    "tests",
                    "Tests",
                    format!("Tests are read-only: {}. Make the code pass them; if a test is wrong, say so in your reply instead of editing it.", i.tests_readonly.join(", ")),
                ));
            }
            // A1 (Anthropic): steps whose order matters go in a numbered list.
            let mut steps = vec![
                "Read the files you need and the code around them.".to_string(),
                "Make the change that reaches the goal, in the style of the project.".to_string(),
                "Run the project's checks and fix what they show.".to_string(),
            ];
            if i.tests_exist && vendor == Vendor::Anthropic {
                // A9: the vendor's own sample prompt against tailoring a solution to the tests.
                steps.push("Write a general solution that works for all valid inputs, not only for the test cases; do not hard-code values for test inputs.".into());
            }
            steps.push(FINAL_REPLY.to_string());
            v.push(instr("steps", "How to work", numbered(&steps)));
        }
        RoleKind::Critic => {
            v.push(data("request", "Task", "TASK", &b.original));
            v.push(data("plan", "Plan", "PLAN", &plan_text(b)));
            v.push(instr(
                "goal",
                "Goal",
                format!(
                    "Review the plan before anyone writes code. Find steps that are missing, wrong, out of order or beyond the task, and acceptance criteria that cannot be checked. Do not write code. {}\n{}",
                    super::JSON_ONLY,
                    super::critique_shape()
                ),
            ));
        }
        RoleKind::TestAuthor => {
            // G07.2: the plan and the criteria only, not the request itself
            v.push(instr(
                "goal",
                "Goal",
                "Write the tests that define \"done\" for the task, in the test framework the project already uses, before the code exists. They should fail now and pass once the criteria hold. Do not write the implementation.".into(),
            ));
            // the Planner's words, derived from the request: data, like the plan the Critic gets
            if !b.done_when.is_empty() {
                v.push(data("done_when", "Acceptance criteria", "CRITERIA", &bullets(&b.done_when)));
            }
            if !b.plan.is_empty() {
                v.push(data("plan", "Plan", "PLAN", &numbered(&b.plan)));
            }
            v.push(instr("steps", "How to work", numbered(&["Find how the project's tests are written and run.".to_string(), "Write the tests.".to_string(), FINAL_REPLY.to_string()])));
        }
        RoleKind::Reviewer => {
            // G07.2: the task, the diff and the output of the checks - nothing of the executor's reasoning.
            v.push(data("request", "Task", "TASK", &b.original));
            v.push(data("diff", "Diff", "DIFF", i.diff.as_deref().unwrap_or("")));
            v.push(data("checks", "Checks", "CHECKS", i.gate_output.as_deref().unwrap_or("")));
            v.push(instr(
                "goal",
                "Goal",
                format!(
                    "Review the change in DIFF against the request in TASK. Report only what is wrong with its correctness or with the requirements; leave style and extra features alone. Approve only if it does what the task asks and nothing is broken. {}\n{}",
                    super::JSON_ONLY,
                    super::verdict_shape()
                ),
            ));
        }
        RoleKind::Verifier => {
            v.push(data("request", "Task", "TASK", &b.original));
            let mut g = "Start the project as a user would and go through the acceptance criteria. Change nothing in the project.".to_string();
            if let Some(c) = &i.run_cmd {
                g.push_str(&format!(" Start it with: `{c}`."));
            }
            g.push_str(&format!(" {}\n{}", super::JSON_ONLY, super::verify_shape()));
            v.push(instr("goal", "Goal", g));
            if !b.done_when.is_empty() {
                v.push(instr("done_when", "Acceptance criteria", bullets(&b.done_when)));
            }
        }
        RoleKind::Integrator => {
            let c = i.conflict.clone().unwrap_or_default();
            v.push(data("request", "Task", "TASK", &b.original));
            v.push(data("diff", "Both sides", "DIFF", i.diff.as_deref().unwrap_or("")));
            v.push(data("git_output", "Git output", "GIT_OUTPUT", &c.git_output));
            v.push(instr(
                "goal",
                "Goal",
                format!(
                    "The branch is being rebased onto the latest `{}` and stopped on a conflict. Resolve the conflict markers in the affected files so that both changes keep working. Do not commit and do not run `git rebase`: the app continues the rebase. {FINAL_REPLY}",
                    c.base
                ),
            ));
        }
        _ => {
            v.push(data("request", "Task", "TASK", &b.original));
            v.push(instr("goal", "Goal", b.goal.clone()));
        }
    }
    v
}

fn tagged(s: &Sec) -> String {
    format!("<{t}>\n{b}\n</{t}>", t = s.tag, b = s.body)
}

/// The prompt of a role, laid out by the template of `vendor`.
pub fn render(vendor: Vendor, role: RoleKind, brief: &Brief, inputs: &RoleInputs) -> String {
    let secs = sections(vendor, role, brief, inputs);
    let data: Vec<&Sec> = secs.iter().filter(|s| s.data).collect();
    let ins: Vec<&Sec> = secs.iter().filter(|s| !s.data).collect();
    let join = |v: Vec<String>| v.join("\n\n");
    match vendor {
        // A5 (long data first, instructions after it), A3 (XML tags, one name per kind of content).
        Vendor::Anthropic => join(data.iter().chain(ins.iter()).map(|s| tagged(s)).collect()),
        // O1 (Markdown headings for the parts; context goes last), O2 (instructions before context).
        Vendor::OpenAi => {
            let mut parts = vec![];
            if !ins.is_empty() {
                let body: Vec<String> = ins.iter().map(|s| format!("## {}\n{}", s.title, s.body)).collect();
                parts.push(format!("# Instructions\n\n{}", body.join("\n\n")));
            }
            if !data.is_empty() {
                let body: Vec<String> = data.iter().map(|s| format!("## {}\n{}", s.title, s.body)).collect();
                parts.push(format!("# Context\n\n{}", body.join("\n\n")));
            }
            join(parts)
        }
        // G2 (one structure, XML here, never mixed with headings), G3 (constraints first), G4 (context
        // before the question, the task last, tied to the context by an anchor phrase).
        Vendor::Google => {
            let is_rule = |s: &Sec| matches!(s.tag, "constraints" | "tests" | "steps");
            let rules: Vec<&Sec> = ins.iter().copied().filter(|s| is_rule(s)).collect();
            let task: Vec<&Sec> = ins.iter().copied().filter(|s| !is_rule(s)).collect();
            let mut parts = vec![];
            if !rules.is_empty() {
                let body: Vec<String> = rules.iter().map(|s| tagged(s)).collect();
                parts.push(format!("<constraints>\n{}\n</constraints>", body.join("\n")));
            }
            if !data.is_empty() {
                let body: Vec<String> = data.iter().map(|s| tagged(s)).collect();
                parts.push(format!("<context>\n{}\n</context>", body.join("\n")));
            }
            let mut t: Vec<String> = task.iter().map(|s| tagged(s)).collect();
            t.push("Based on the information above, do the task.".to_string());
            parts.push(format!("<task>\n{}\n</task>", t.join("\n")));
            join(parts)
        }
        // Q1 (context, objective and response named apart), Q2 (task steps), Q3 (unique separators `###`).
        Vendor::Alibaba => {
            let mut units = vec![];
            if !data.is_empty() {
                let body: Vec<String> = data.iter().map(|s| format!("{}:\n{}", s.title, s.body)).collect();
                units.push(format!("#Context#\n{}", body.join("\n\n")));
            }
            let pick = |tags: &[&str]| -> Vec<String> {
                ins.iter().filter(|s| tags.contains(&s.tag)).map(|s| format!("{}:\n{}", s.title, s.body)).collect()
            };
            let obj = pick(&["goal", "continue", "done_when", "notes"]);
            if !obj.is_empty() {
                units.push(format!("#Objective#\n{}", obj.join("\n\n")));
            }
            let cons = pick(&["constraints", "tests"]);
            if !cons.is_empty() {
                units.push(format!("#Constraints#\n{}", cons.join("\n\n")));
            }
            let steps = pick(&["plan", "steps"]);
            if !steps.is_empty() {
                units.push(format!("#Task Steps#\n{}", steps.join("\n\n")));
            }
            units.push("#Response#\nReply with 1-3 lines on what you did, or with the JSON the Objective asks for.".to_string());
            units.join("\n###\n")
        }
        // No guide of its own: the plain template.
        Vendor::Generic => {
            let mut s = String::new();
            for sec in &ins {
                s.push_str(&format!("{}:\n{}\n\n", sec.title, sec.body));
            }
            for sec in &data {
                s.push_str(&format!("{}:\n{}\n\n", sec.title, sec.body));
            }
            s.trim_end().to_string()
        }
    }
}

/// The plain "Goal / Constraints / Done when" template with the person's words as the goal,
/// for when the Planner's breakdown is not usable (G14.2). `why` goes in a line of its own.
pub fn render_simple(task_text: &str, why: &str) -> String {
    format!(
        "Goal:\n{}\n\nConstraints:\n- Keep the change to what the goal asks; leave the rest of the project as it is.\n\nDone when:\n- The goal is reached and the project's checks pass.\n\nprompt not rewritten: {why}",
        fenced("TASK", task_text)
    )
}

fn role_line(role: RoleKind) -> &'static str {
    match role {
        RoleKind::Planner => "You are a planner. You break tasks down for engineers and never change files.",
        RoleKind::Critic => "You are a critic of plans. You find what is wrong or missing before code is written.",
        RoleKind::TestAuthor => "You are a test author. You write the tests that define done before the code exists.",
        RoleKind::Worker => "You are a software engineer working in your own git worktree of the project.",
        RoleKind::Reviewer => "You are a code reviewer. You judge a change against the request it answers.",
        RoleKind::Verifier => "You are a verifier. You check that the finished work behaves as the person asked.",
        RoleKind::Integrator => "You are an integrator. You resolve merge conflicts so that both sides keep working.",
        _ => "You are an assistant of a coding team.",
    }
}

fn writes(role: RoleKind) -> bool {
    matches!(role, RoleKind::Worker | RoleKind::TestAuthor | RoleKind::Integrator)
}

fn json_only(role: RoleKind) -> bool {
    matches!(role, RoleKind::Planner | RoleKind::Critic | RoleKind::Reviewer | RoleKind::Verifier)
}

fn rules(vendor: Vendor, role: RoleKind) -> Vec<String> {
    let mut r = vec![
        "Work only inside the current directory.".to_string(),
        "Never run `git push` or `git remote`, never use `sudo`, and leave remotes alone.".to_string(),
        "Do not read `~/.ssh` or `.env*` files unless the task needs them.".to_string(),
    ];
    if writes(role) {
        r.push("The app commits for you.".into());
    } else {
        r.push("You only read: do not modify any file.".into());
    }
    if json_only(role) {
        r.push("Your final reply is one JSON object.".into());
    }
    r.push(
        "If you cannot go on without the person, make the last line of your reply `NEEDS_HUMAN: <question>`, \
         and when you can offer choices add a line after it: `OPTIONS: a | b | c`."
            .into(),
    );
    if role == RoleKind::Worker {
        r.push("If the task depends on work that another task is doing and cannot start yet, end your reply with `DEFERRED: <why>`.".into());
        if vendor == Vendor::OpenAi {
            // O3 (Codex guide, "Autonomy and Persistence"): persist, act on reasonable assumptions.
            r.push("Persist until the task is fully handled end-to-end: implement, verify, report. Do not stop at analysis or a partial fix.".into());
            r.push("When a detail is missing, make a reasonable assumption and finish; ask only when you are truly blocked.".into());
            // O5 (Codex guide, "Editing constraints")
            r.push("Do not revert changes you did not make.".into());
        }
    }
    r.push(DATA_RULE.to_string());
    r
}

/// The system prompt of a role: its identity, the safety rules (R17i), the signal format and the data rule.
pub fn system_for(vendor: Vendor, role: RoleKind) -> String {
    let rs = rules(vendor, role);
    let id = role_line(role);
    match vendor {
        // A4: the role goes in the system prompt, first.
        Vendor::Anthropic | Vendor::Generic => format!("{id} {}", rs.join(" ")),
        // O1: Identity, then Instructions.
        Vendor::OpenAi => format!("# Identity\n{id}\n\n# Instructions\n{}", bullets(&rs)),
        // G3: role, constraints and output format at the very beginning, in one structure.
        Vendor::Google => format!(
            "<role>\n{id}\n</role>\n\n<constraints>\n{}\n</constraints>\n\n<output_format>\n{}\n</output_format>",
            numbered(&rs),
            if json_only(role) { "One JSON object." } else { "1-3 lines on what you did." }
        ),
        Vendor::Alibaba => format!("#Role#\n{id}\n###\n#Constraints#\n{}", numbered(&rs)),
    }
}
