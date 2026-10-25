//! v3 (G29, G30): an agent's actions with their input, output and diff, the same for every CLI.
//! Adapters turn their own tool events into `ToolAction`s with `tool_action` (start) and a result action
//! (`result`); `Tracker` (inside `run_blocking`) pairs the two by id, cuts long output and scrubs secrets
//! out of everything a run says before the engine or a chat sees it.
use super::{AgentEvent, Run};
use crate::model::{ActionInput, PlanItem, PlanState, ToolAction};
use serde_json::Value;
use std::collections::HashMap;

/// Output of one action kept in the log and sent to the window (story 230).
pub const OUTPUT_MAX: usize = 64 * 1024;
/// A diff of one action (story 230).
pub const DIFF_MAX: usize = 200 * 1024;
/// Lines of a new file shown under `Write` (story 225).
pub const HEAD_LINES: usize = 20;
/// In place of the diff / head of a secret file (`.env`, `*.key` ...): only its path is shown.
pub const SECRET_HIDDEN: &str = "[secret_file_hidden]";

/// `s` cut to at most `max` bytes on a char boundary, with a line saying how long it was.
pub fn cut(s: &str, max: usize, what: &str) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n[{what}_truncated: {} bytes]", &s[..end], s.len())
}

/// Unified diff of one file (`--- a/<path>`, `+++ b/<path>`, `@@` hunks, 3 lines of context). Not cut here:
/// `finish_action` scrubs first and cuts after, so a cut never splits a secret.
/// `old` = None is a new file, `new` = None a removed one. Empty when nothing changed.
pub fn unified_diff(path: &str, old: Option<&str>, new: Option<&str>) -> String {
    let (a, b) = (old.unwrap_or(""), new.unwrap_or(""));
    if old.is_some() && a == b {
        return String::new();
    }
    let p = path.trim_start_matches('/');
    let from = if old.is_some() { format!("a/{p}") } else { "/dev/null".to_string() };
    let to = if new.is_some() { format!("b/{p}") } else { "/dev/null".to_string() };
    similar::udiff::unified_diff(similar::Algorithm::Myers, a, b, 3, Some((&from, &to)))
}

fn s<'a>(v: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|k| v.get(k).and_then(Value::as_str))
}

const PATH_KEYS: [&str; 6] = ["file_path", "path", "filePath", "absolute_path", "target_file", "notebook_path"];

/// Diff of an edit given as old/new strings (Edit, MultiEdit, opencode `edit`, Gemini `replace`); the
/// line numbers count from the edited piece, the result of the tool gives the real ones.
fn edits_diff(path: &str, input: &Value) -> String {
    let pairs: Vec<(String, String)> = match input.get("edits").and_then(Value::as_array) {
        Some(list) => list
            .iter()
            .map(|e| (s(e, &["old_string", "oldString"]).unwrap_or("").to_string(), s(e, &["new_string", "newString"]).unwrap_or("").to_string()))
            .collect(),
        None => vec![(s(input, &["old_string", "oldString", "old_str"]).unwrap_or("").to_string(), s(input, &["new_string", "newString", "new_str"]).unwrap_or("").to_string())],
    };
    let (old, new): (Vec<String>, Vec<String>) = pairs.into_iter().map(|(o, n)| (with_nl(o), with_nl(n))).unzip();
    unified_diff(path, Some(&old.concat()), Some(&new.concat()))
}

fn with_nl(mut s: String) -> String {
    if !s.is_empty() && !s.ends_with('\n') {
        s.push('\n');
    }
    s
}

/// `Write` of a file: its whole text as `head` and its number of lines. `finish_action` scrubs first and
/// cuts `head` to `HEAD_LINES` after, so a cut never splits a secret of several lines.
pub fn write_input(path: &str, content: &str) -> ActionInput {
    ActionInput::Write { path: path.to_string(), head: content.to_string(), lines: content.lines().count() as u32 }
}

/// The first `HEAD_LINES` lines of `s`.
fn head_lines(s: &str) -> String {
    s.lines().take(HEAD_LINES).collect::<Vec<_>>().join("\n")
}

/// `s` on one line, at most `max` chars (with `…` when cut).
pub fn one_line(s: &str, max: usize) -> String {
    let one: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() > max {
        format!("{}…", one.chars().take(max).collect::<String>())
    } else {
        one
    }
}

/// Length of the one-line summary of a tool the app does not know.
const SUMMARY_MAX: usize = 120;

/// A plan or to-do list (Claude `TodoWrite`, Codex `update_plan` / `todo_list`, opencode `todowrite`), when `v` is one.
pub fn plan_items(v: &Value) -> Option<Vec<PlanItem>> {
    let list = ["todos", "plan", "items"].iter().find_map(|k| v.get(k).and_then(Value::as_array))?;
    let items = list
        .iter()
        .filter_map(|it| {
            let text = s(it, &["content", "step", "text", "title", "description"])?.to_string();
            let state = match (s(it, &["status", "state"]), it.get("completed").and_then(Value::as_bool)) {
                (Some("in_progress" | "active" | "running"), _) => PlanState::Active,
                (Some("completed" | "done"), _) | (None, Some(true)) => PlanState::Done,
                (Some("cancelled" | "canceled" | "dropped" | "skipped"), _) => PlanState::Dropped,
                _ => PlanState::Pending,
            };
            Some(PlanItem { text, state })
        })
        .collect();
    Some(items)
}

/// What a tool works on, by its name (any CLI's spelling) and its input.
pub fn action_input(name: &str, input: &Value) -> ActionInput {
    let n = name.to_ascii_lowercase().replace(['_', '-'], "");
    let path = || s(input, &PATH_KEYS).unwrap_or("").to_string();
    match n.as_str() {
        "read" | "readfile" | "view" | "readmanyfiles" | "notebookread" => ActionInput::Read { path: path() },
        "edit" | "multiedit" | "replace" | "strreplace" | "strreplaceeditor" | "notebookedit" | "searchreplace" => {
            ActionInput::Edit { diff: edits_diff(&path(), input), path: path() }
        }
        "write" | "writefile" | "create" | "createfile" => write_input(&path(), s(input, &["content", "contents", "fileText", "file_text"]).unwrap_or("")),
        "bash" | "shell" | "runshellcommand" | "execcommand" | "localshell" | "terminal" | "runterminalcmd" => ActionInput::Bash {
            command: match input.get("command") {
                Some(Value::Array(a)) => a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" "),
                _ => s(input, &["command", "cmd"]).unwrap_or("").to_string(),
            },
        },
        "grep" | "glob" | "search" | "searchfilecontent" | "find" | "ls" | "listdirectory" | "codebasesearch" | "filesearch" => ActionInput::Search {
            pattern: s(input, &["pattern", "query", "glob", "path", "dir_path"]).unwrap_or("").to_string(),
            hits: None,
        },
        "webfetch" | "websearch" | "googlewebsearch" | "fetch" => ActionInput::Web { target: s(input, &["url", "query", "prompt"]).unwrap_or("").to_string() },
        "todowrite" | "updateplan" | "writetodos" | "todolist" => match plan_items(input) {
            Some(items) => ActionInput::Plan { items },
            None => ActionInput::Other { summary: super::adapters::common::summarize(input) },
        },
        _ => ActionInput::Other { summary: super::adapters::common::summarize(input) },
    }
}

/// The action at its start.
pub fn tool_action(id: &str, name: &str, input: &Value) -> ToolAction {
    ToolAction { id: id.to_string(), name: name.to_string(), input: action_input(name, input), output: None, ok: true, done: false }
}

/// The result of an action, by id; the name and the input come from its start (`Tracker`) unless `input` says more.
pub fn result(id: &str, output: Option<String>, ok: bool, input: Option<ActionInput>) -> ToolAction {
    ToolAction { id: id.to_string(), name: String::new(), input: input.unwrap_or(ActionInput::Other { summary: String::new() }), output, ok, done: true }
}

/// The one-line text of a log line about the action (`<name>: <what>`), the form `Tool` lines always had.
pub fn summary_line(a: &ToolAction) -> String {
    let what = match &a.input {
        ActionInput::Read { path } | ActionInput::Edit { path, .. } | ActionInput::Write { path, .. } => path.clone(),
        ActionInput::Bash { command } => command.clone(),
        ActionInput::Search { pattern, .. } => pattern.clone(),
        ActionInput::Web { target } => target.clone(),
        ActionInput::Plan { items } => items.iter().map(|i| i.text.as_str()).collect::<Vec<_>>().join("; "),
        ActionInput::Other { summary } => summary.clone(),
    };
    format!("{}: {}", a.name, one_line(&what, SUMMARY_MAX))
}

/// Text of a tool result: a string, or the text parts of a list of content blocks.
pub fn content_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Array(a) => a.iter().filter_map(|b| b.get("text").and_then(Value::as_str).or_else(|| b.as_str())).collect::<Vec<_>>().join("\n"),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// Values of secrets replaced by `${NAME}` (story 231): the run's variables (kit secrets from the keyring)
/// and the values of the `.env` files in the run's folder.
#[derive(Clone, Debug, Default)]
pub struct Scrub {
    /// (value, name), longest value first.
    pairs: Vec<(String, String)>,
}

/// Shorter values are not secrets worth hiding and would eat ordinary words.
const MIN_SECRET: usize = 6;

impl Scrub {
    pub fn new(pairs: impl IntoIterator<Item = (String, String)>) -> Scrub {
        let mut v: Vec<(String, String)> = pairs.into_iter().filter(|(_, val)| val.trim().len() >= MIN_SECRET).map(|(n, val)| (val, n)).collect();
        v.sort_by(|a, b| b.0.len().cmp(&a.0.len()).then_with(|| a.1.cmp(&b.1)));
        v.dedup_by(|a, b| a.0 == b.0);
        Scrub { pairs: v }
    }

    /// The secrets of a run: `Run.extra_env` and `<cwd>/.env*` (not `.env.example` and other templates).
    pub fn for_run(run: &Run) -> Scrub {
        let mut pairs = run.extra_env.clone();
        pairs.extend(dotenv_pairs(&run.cwd));
        Scrub::new(pairs)
    }

    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }

    pub fn text(&self, s: &str) -> String {
        let mut out = s.to_string();
        for (val, name) in &self.pairs {
            if out.contains(val.as_str()) {
                out = out.replace(val.as_str(), &format!("${{{name}}}"));
            }
        }
        out
    }

    /// The longest end of `s` that is the start of a secret (held back from a stream until it is whole).
    fn open_tail(&self, s: &str) -> usize {
        let mut best = 0;
        for (val, _) in &self.pairs {
            for (i, _) in s.char_indices().rev().take(val.len()) {
                let tail = &s[i..];
                if tail.len() < val.len() && tail.len() > best && val.starts_with(tail) {
                    best = tail.len();
                }
            }
        }
        best
    }

    pub fn action(&self, a: &mut ToolAction) {
        let f = |x: &mut String| *x = self.text(x);
        f(&mut a.name);
        if let Some(o) = a.output.as_mut() {
            f(o);
        }
        match &mut a.input {
            ActionInput::Read { path } => f(path),
            ActionInput::Edit { path, diff } => {
                f(path);
                f(diff);
            }
            ActionInput::Write { path, head, .. } => {
                f(path);
                f(head);
            }
            ActionInput::Bash { command } => f(command),
            ActionInput::Search { pattern, .. } => f(pattern),
            ActionInput::Web { target } => f(target),
            ActionInput::Plan { items } => items.iter_mut().for_each(|i| f(&mut i.text)),
            ActionInput::Other { summary } => f(summary),
        }
    }
}

/// How deep and how wide the search for `.env*` files in a copy goes.
const ENV_DEPTH: usize = 4;
const ENV_DIRS: usize = 500;
const ENV_FILE_MAX: u64 = 64 * 1024;
/// Folders that hold other people's code or build output, never the project's own `.env`.
const SKIP_DIRS: [&str; 8] = ["node_modules", "target", "vendor", "dist", "build", "venv", "__pycache__", "Pods"];

/// KEY=VALUE pairs of the `.env*` files of `dir` and its folders down to `ENV_DEPTH` (quotes and `export `
/// taken off); hidden folders and `SKIP_DIRS` are not entered.
fn dotenv_pairs(dir: &std::path::Path) -> Vec<(String, String)> {
    let mut out = vec![];
    // wide first: one folder with hundreds of folders must not use up `ENV_DIRS` before its neighbours
    let (mut todo, mut seen) = (std::collections::VecDeque::from([(dir.to_path_buf(), 0usize)]), 0usize);
    while let Some((d, depth)) = todo.pop_front() {
        seen += 1;
        if seen > ENV_DIRS {
            break;
        }
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let Ok(ft) = e.file_type() else { continue };
            // a symlinked folder is not entered (it may lead anywhere, or in a loop)
            if ft.is_dir() {
                if depth < ENV_DEPTH && !name.starts_with('.') && !SKIP_DIRS.contains(&name.as_str()) {
                    todo.push_back((e.path(), depth + 1));
                }
                continue;
            }
            let template = [".env.example", ".env.sample", ".env.template"].contains(&name.as_str());
            if !name.starts_with(".env") || template {
                continue;
            }
            // a symlinked `.env` (git worktrees often have one) is read through the link
            if std::fs::metadata(e.path()).is_ok_and(|m| m.is_file() && m.len() <= ENV_FILE_MAX) {
                env_file_pairs(&e.path(), &mut out);
            }
        }
    }
    out
}

fn env_file_pairs(path: &std::path::Path, out: &mut Vec<(String, String)>) {
    {
        let Ok(text) = std::fs::read_to_string(path) else { return };
        for line in text.lines() {
            let line = line.trim();
            let line = line.strip_prefix("export ").unwrap_or(line);
            if line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                let v = v.trim().trim_matches(|c| c == '"' || c == '\'');
                if !k.trim().is_empty() && !v.is_empty() {
                    out.push((k.trim().to_string(), v.to_string()));
                }
            }
        }
    }
}

/// Makes the final form of an action: secrets out, output and diff cut, no output for a read or an edit that worked.
pub fn finish_action(a: &mut ToolAction, scrub: &Scrub) {
    scrub.action(a);
    // a secret file shows its path only: values written into it now are not known to `scrub`
    match &mut a.input {
        ActionInput::Edit { path, diff } if crate::kits::is_secret_file(path) => *diff = SECRET_HIDDEN.into(),
        ActionInput::Write { path, head, .. } if crate::kits::is_secret_file(path) => *head = SECRET_HIDDEN.into(),
        _ => {}
    }
    if a.ok && a.done && matches!(a.input, ActionInput::Read { .. } | ActionInput::Edit { .. } | ActionInput::Write { .. }) {
        a.output = None;
    }
    if let Some(o) = a.output.take() {
        a.output = Some(cut(&o, OUTPUT_MAX, "output"));
    }
    match &mut a.input {
        ActionInput::Edit { diff, .. } => *diff = cut(diff, DIFF_MAX, "diff"),
        ActionInput::Write { head, .. } => *head = head_lines(head),
        ActionInput::Other { summary } => *summary = one_line(summary, SUMMARY_MAX),
        _ => {}
    }
}

/// Paths of a run's actions as a person reads them: relative to the copy (`calc.py`), outside it with `~`
/// for the home folder, else as they are.
#[derive(Clone, Debug, Default)]
pub struct Paths {
    /// The copy, as given and as resolved (`/var` vs `/private/var`), each ending in `/`.
    roots: Vec<String>,
    home: Option<String>,
}

impl Paths {
    pub fn new(root: &std::path::Path) -> Paths {
        let slash = |p: &std::path::Path| format!("{}/", p.to_string_lossy().trim_end_matches('/'));
        let mut roots = vec![slash(root)];
        if let Ok(c) = root.canonicalize() {
            roots.push(slash(&c));
        }
        roots.dedup();
        let home = std::env::var("HOME").ok().filter(|h| h.len() > 1).map(|h| format!("{}/", h.trim_end_matches('/')));
        Paths { roots, home }
    }

    pub fn show(&self, p: &str) -> String {
        for r in &self.roots {
            if let Some(rel) = p.strip_prefix(r.as_str()) {
                return if rel.is_empty() { ".".into() } else { rel.to_string() };
            }
            if p == r.trim_end_matches('/') {
                return ".".into();
            }
        }
        match &self.home {
            Some(h) => p.strip_prefix(h.as_str()).map_or_else(|| p.to_string(), |rest| format!("~/{rest}")),
            None => p.to_string(),
        }
    }

    /// Every path the action names (the headers of its diff too); free text is left alone.
    pub fn action(&self, a: &mut ToolAction) {
        match &mut a.input {
            ActionInput::Read { path } | ActionInput::Write { path, .. } => *path = self.show(path),
            ActionInput::Edit { path, diff } => {
                *path = self.show(path);
                *diff = self.diff_headers(diff);
            }
            _ => {}
        }
    }

    /// `--- a/<abs>` / `+++ b/<abs>` lines with the copy's path taken off.
    fn diff_headers(&self, diff: &str) -> String {
        let mut out = String::with_capacity(diff.len());
        for line in diff.split_inclusive('\n') {
            let fixed = ["--- a/", "+++ b/"].iter().find_map(|pre| {
                let rest = line.strip_prefix(pre)?;
                let (p, nl) = rest.strip_suffix('\n').map_or((rest, ""), |p| (p, "\n"));
                let shown = self.show(&format!("/{p}"));
                (!shown.starts_with('/') && !shown.starts_with('~')).then(|| format!("{pre}{shown}{nl}"))
            });
            out.push_str(fixed.as_deref().unwrap_or(line));
        }
        out
    }

    /// A free line that lists paths (`Tool` summaries): the copy's path taken off.
    pub fn text(&self, s: &str) -> String {
        self.roots.iter().fold(s.to_string(), |acc, r| acc.replace(r.as_str(), ""))
    }
}

/// The tool by which Claude Code hands over the answer of `--json-schema`: our roles' decision, not work to show.
const STRUCTURED_TOOL: &str = "StructuredOutput";

/// What a run says on the way to the engine: results joined to their starts, secrets out, long text cut.
pub(crate) struct Tracker {
    scrub: Scrub,
    paths: Paths,
    /// The run asked for an answer by schema: its `StructuredOutput` call is a log line, not an action.
    structured: bool,
    /// Ids of those calls, whose results are not passed on.
    hidden: std::collections::HashSet<String>,
    started: HashMap<String, ToolAction>,
    /// Streamed text held back because it may be the start of a secret.
    carry: String,
    /// The same for the pieces of a thought, and the thought so far (cleaned, held-back end included).
    think_carry: String,
    think_seen: String,
}

impl Tracker {
    pub(crate) fn new(scrub: Scrub, paths: Paths, structured: bool) -> Tracker {
        Tracker { scrub, paths, structured, hidden: Default::default(), started: HashMap::new(), carry: String::new(), think_carry: String::new(), think_seen: String::new() }
    }

    fn finish(&self, a: &mut ToolAction) {
        self.paths.action(a);
        finish_action(a, &self.scrub);
    }

    /// One piece of a thought: like `TextDelta`, a secret cut in two pieces is held until it is whole. Claude then sends the
    /// whole thought once more; it is told by being the thought so far, and the held end goes out just before it.
    fn thinking(&mut self, t: String) -> Vec<AgentEvent> {
        if self.scrub.is_empty() {
            return vec![AgentEvent::Thinking(t)];
        }
        let whole = self.scrub.text(&t);
        if !self.think_seen.is_empty() && whole.trim() == self.think_seen.trim() {
            let mut out: Vec<AgentEvent> = self.end_thought().into_iter().collect();
            out.push(AgentEvent::Thinking(whole));
            return out;
        }
        let held = std::mem::take(&mut self.think_carry);
        let all = self.scrub.text(&format!("{held}{t}"));
        let keep = self.scrub.open_tail(&all);
        let (out, rest) = all.split_at(all.len() - keep);
        let sent = self.think_seen.strip_suffix(held.as_str()).unwrap_or(&self.think_seen).to_string();
        self.think_seen = format!("{sent}{all}");
        self.think_carry = rest.to_string();
        if out.is_empty() { vec![] } else { vec![AgentEvent::Thinking(out.to_string())] }
    }

    /// Any other event ends the thought: what was held back goes out first (it is not the start of a secret any more).
    fn end_thought(&mut self) -> Option<AgentEvent> {
        self.think_seen.clear();
        let rest = std::mem::take(&mut self.think_carry);
        (!rest.is_empty()).then_some(AgentEvent::Thinking(rest))
    }

    pub(crate) fn pass(&mut self, e: AgentEvent) -> Vec<AgentEvent> {
        if let AgentEvent::Thinking(t) = e {
            return self.thinking(t);
        }
        let mut out: Vec<AgentEvent> = self.end_thought().into_iter().collect();
        out.extend(self.pass_other(e));
        out
    }

    fn pass_other(&mut self, e: AgentEvent) -> Vec<AgentEvent> {
        let sc = |t: &str| self.scrub.text(t);
        match e {
            AgentEvent::Action(mut a) => {
                if self.hidden.contains(&a.id) {
                    if a.done {
                        self.hidden.remove(&a.id);
                    }
                    return vec![];
                }
                if !a.done && self.structured && a.name == STRUCTURED_TOOL {
                    self.hidden.insert(a.id.clone());
                    self.finish(&mut a);
                    let summary = summary_line(&a).split_once(": ").map(|(_, s)| s.to_string()).unwrap_or_default();
                    return vec![AgentEvent::Tool { name: a.name, summary }];
                }
                if !a.done {
                    self.finish(&mut a);
                    self.started.insert(a.id.clone(), a.clone());
                    return vec![AgentEvent::Action(a)];
                }
                if let Some(st) = self.started.remove(&a.id) {
                    if a.name.is_empty() {
                        a.name = st.name.clone();
                    }
                    a.input = match (a.input, st.input) {
                        (ActionInput::Other { summary }, start) if summary.is_empty() => start,
                        (ActionInput::Search { pattern, hits }, ActionInput::Search { pattern: p0, .. }) => {
                            ActionInput::Search { pattern: if pattern.is_empty() { p0 } else { pattern }, hits }
                        }
                        (ActionInput::Search { .. }, start) => start,
                        (mine, _) => mine,
                    };
                } else if a.name.is_empty() {
                    a.name = "tool".into();
                }
                self.finish(&mut a);
                vec![AgentEvent::Action(a)]
            }
            AgentEvent::TextDelta(t) => {
                if self.scrub.is_empty() {
                    return vec![AgentEvent::TextDelta(t)];
                }
                let all = sc(&format!("{}{t}", std::mem::take(&mut self.carry)));
                let keep = self.scrub.open_tail(&all);
                let (out, rest) = all.split_at(all.len() - keep);
                self.carry = rest.to_string();
                if out.is_empty() {
                    vec![]
                } else {
                    vec![AgentEvent::TextDelta(out.to_string())]
                }
            }
            AgentEvent::Text(t) => {
                let mut out = vec![];
                if !self.carry.is_empty() {
                    out.push(AgentEvent::TextDelta(sc(&std::mem::take(&mut self.carry))));
                }
                out.push(AgentEvent::Text(sc(&t)));
                out
            }
            AgentEvent::Tool { name, summary } => vec![AgentEvent::Tool { name, summary: self.paths.text(&sc(&summary)) }],
            AgentEvent::Done { session, text, is_error } => vec![AgentEvent::Done { session, text: sc(&text), is_error }],
            AgentEvent::ModelRejected(m) => vec![AgentEvent::ModelRejected(sc(&m))],
            AgentEvent::AuthRequired(m) => vec![AgentEvent::AuthRequired(sc(&m))],
            other => vec![other],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tracker(secret: &str) -> Tracker {
        Tracker::new(Scrub::new([("API_TOKEN".to_string(), secret.to_string())]), Paths::new(std::path::Path::new("/nowhere")), false)
    }

    fn thoughts(evs: &[AgentEvent]) -> Vec<String> {
        evs.iter().filter_map(|e| if let AgentEvent::Thinking(t) = e { Some(t.clone()) } else { None }).collect()
    }

    #[test]
    fn a_secret_cut_between_two_thinking_pieces_never_reaches_the_window() {
        let mut tr = tracker("sk-SECRET-1234567890");
        let mut out = vec![];
        for piece in ["the key is sk-SECRET-12", "34567890 ok"] {
            out.extend(tr.pass(AgentEvent::Thinking(piece.into())));
        }
        out.extend(tr.pass(AgentEvent::Text("done".into())));
        let all = thoughts(&out).concat();
        assert!(!all.contains("SECRET"), "{all:?}");
        assert!(all.starts_with("the key is ") && all.ends_with(" ok"), "{all:?}");
    }

    #[test]
    fn the_whole_thought_that_follows_its_pieces_comes_once_and_equal_to_them() {
        let mut tr = tracker("sk-SECRET-1234567890");
        let mut out = vec![];
        for piece in ["the key is sk-SECRET-12", "34567890 ok", "the key is sk-SECRET-1234567890 ok"] {
            out.extend(tr.pass(AgentEvent::Thinking(piece.into())));
        }
        let t = thoughts(&out);
        assert!(!t.concat().contains("SECRET"), "{t:?}");
        // the chat drops the last event because it equals what the pieces made (reply.rs); the pieces are whole by then
        let (last, pieces) = t.split_last().unwrap();
        assert_eq!(pieces.concat().trim(), last.trim(), "{t:?}");
        // the next thought starts afresh
        let next = tr.pass(AgentEvent::Thinking("another".into()));
        assert_eq!(thoughts(&next), vec!["another".to_string()]);
    }

    #[test]
    fn thinking_without_secrets_passes_as_it_comes() {
        let mut tr = Tracker::new(Scrub::default(), Paths::new(std::path::Path::new("/nowhere")), false);
        assert_eq!(tr.pass(AgentEvent::Thinking("a".into())), vec![AgentEvent::Thinking("a".into())]);
    }
}
