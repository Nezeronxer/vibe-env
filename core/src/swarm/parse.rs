//! What the models of the swarm answer, and what the app accepts of it: every answer is JSON and
//! none is taken on its word (paths are checked, counts are capped, references must exist).
use crate::prompt::last_json_object;
use serde_json::Value;

/// The most tasks a convoy may have (G08.1).
pub const MAX_CONVOY_TASKS: usize = 12;

fn text(v: &Value) -> String {
    v.as_str().map(|s| s.trim().to_string()).unwrap_or_default()
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array().map(|a| a.iter().filter_map(|x| x.as_str()).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()).unwrap_or_default()
}

fn obj(s: &str) -> Result<Value, String> {
    last_json_object(s).ok_or_else(|| "no JSON object in the answer".to_string())
}

fn id_ok(id: &str) -> bool {
    !id.is_empty() && id.len() <= 20 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// A path of the project the way a part may name it: relative, inside the project, no globs.
fn path_ok(p: &str) -> bool {
    let p = p.trim();
    !p.is_empty()
        && !p.starts_with('/')
        && !p.contains(['\\', '*', '?', '[', ']', '\0', '\n'])
        && !p.split('/').any(|c| c == "..")
        && !(p.len() > 1 && p.as_bytes()[1] == b':')
}

/// A part of the Coordinator's split.
#[derive(Clone, Debug, PartialEq)]
pub struct SplitPart {
    pub id: String,
    pub title: String,
    pub files: Vec<String>,
    pub brief: String,
}

/// `{"parts":[{"id","title","files","brief"}]}`: 1..=`max` parts, unique ids, every part names its
/// files (exact relative paths inside the project).
pub fn parse_split(s: &str, max: usize) -> Result<Vec<SplitPart>, String> {
    let v = obj(s)?;
    let arr = v["parts"].as_array().ok_or("the answer has no \"parts\" list")?;
    if arr.is_empty() {
        return Err("the split has no parts".into());
    }
    if arr.len() > max {
        return Err(format!("the split has {} parts, at most {max} are allowed", arr.len()));
    }
    let mut out: Vec<SplitPart> = vec![];
    for (i, p) in arr.iter().enumerate() {
        let id = if p["id"].is_null() { format!("p{}", i + 1) } else { text(&p["id"]) };
        if !id_ok(&id) || out.iter().any(|x| x.id == id) {
            return Err(format!("part id {id:?} is empty, unusual or used twice"));
        }
        let files = strings(&p["files"]);
        if files.is_empty() {
            return Err(format!("part {id} names no files"));
        }
        if let Some(bad) = files.iter().find(|f| !path_ok(f)) {
            return Err(format!("part {id}: {bad:?} is not a relative path inside the project"));
        }
        let title = text(&p["title"]);
        out.push(SplitPart { title: if title.is_empty() { id.clone() } else { title }, brief: text(&p["brief"]), id, files });
    }
    Ok(out)
}

/// A task of a convoy.
#[derive(Clone, Debug, PartialEq)]
pub struct ConvoyTask {
    pub id: String,
    pub title: String,
    pub text: String,
    pub depends_on: Vec<String>,
}

/// `{"tasks":[{"id","title","text","depends_on":[ids]}]}`: 1..=12 tasks, every dependency is a task
/// of the list, no task waits for itself, no circle.
pub fn parse_convoy(s: &str) -> Result<Vec<ConvoyTask>, String> {
    let v = obj(s)?;
    let arr = v["tasks"].as_array().ok_or("the answer has no \"tasks\" list")?;
    if arr.is_empty() {
        return Err("the convoy has no tasks".into());
    }
    if arr.len() > MAX_CONVOY_TASKS {
        return Err(format!("more than {MAX_CONVOY_TASKS} tasks ({})", arr.len()));
    }
    let mut out: Vec<ConvoyTask> = vec![];
    for (i, t) in arr.iter().enumerate() {
        let id = if t["id"].is_null() { format!("t{}", i + 1) } else { text(&t["id"]) };
        if !id_ok(&id) || out.iter().any(|x| x.id == id) {
            return Err(format!("task id {id:?} is empty, unusual or used twice"));
        }
        let body = text(&t["text"]);
        let title = text(&t["title"]);
        let (title, body) = match (title.is_empty(), body.is_empty()) {
            (true, true) => return Err(format!("task {id} has neither a title nor a text")),
            (true, false) => (body.lines().next().unwrap_or("").chars().take(80).collect(), body),
            (false, true) => (title.clone(), title),
            (false, false) => (title, body),
        };
        out.push(ConvoyTask { id, title, text: body, depends_on: strings(&t["depends_on"]) });
    }
    for t in &out {
        for d in &t.depends_on {
            if d == &t.id {
                return Err(format!("task {} waits for itself", t.id));
            }
            if !out.iter().any(|x| &x.id == d) {
                return Err(format!("task {} depends on {d}, which is not in the list", t.id));
            }
        }
    }
    // a circle: peel off tasks whose dependencies are all peeled
    let mut left: Vec<&ConvoyTask> = out.iter().collect();
    while !left.is_empty() {
        let before = left.len();
        let ids: Vec<&str> = left.iter().map(|t| t.id.as_str()).collect();
        left.retain(|t| t.depends_on.iter().any(|d| ids.contains(&d.as_str())));
        if left.len() == before {
            return Err("the tasks depend on each other in a circle".into());
        }
    }
    Ok(out)
}

/// `{"approach":"..."}` of a member of the council.
pub fn parse_proposal(s: &str) -> Result<String, String> {
    let t = text(&obj(s)?["approach"]);
    if t.is_empty() {
        return Err("the answer has no approach".into());
    }
    Ok(t)
}

/// `{"vote":"B","why":"..."}`: the choice must be one of `options`.
pub fn parse_vote(s: &str, options: &[String]) -> Result<(String, Option<String>), String> {
    let v = obj(s)?;
    let c = text(&v["vote"]);
    if !options.contains(&c) {
        return Err(format!("{c:?} is not one of {}", options.join(", ")));
    }
    let why = text(&v["why"]);
    Ok((c, (!why.is_empty()).then_some(why)))
}

/// The reviewer's comparison of variants.
#[derive(Clone, Debug, PartialEq)]
pub struct Comparison {
    /// variant, [correctness, coverage, simplicity, risk], why
    pub scores: Vec<(String, [u8; 4], Option<String>)>,
    pub recommend: String,
    pub advice: String,
}

/// `{"scores":[{"variant","correctness","coverage","simplicity","risk","why"}],"recommend":"A","advice":"..."}`:
/// marks are 0..=10, variants must be among `ids`, and the recommended one too.
pub fn parse_comparison(s: &str, ids: &[String]) -> Result<Comparison, String> {
    let v = obj(s)?;
    let arr = v["scores"].as_array().ok_or("the answer has no \"scores\" list")?;
    let mut scores = vec![];
    for x in arr {
        let id = text(&x["variant"]);
        if !ids.contains(&id) || scores.iter().any(|(i, _, _): &(String, [u8; 4], Option<String>)| *i == id) {
            continue; // a variant that was not shown, or marked twice: the first mark counts
        }
        let mut m = [0u8; 4];
        for (k, key) in ["correctness", "coverage", "simplicity", "risk"].iter().enumerate() {
            let n = x[key].as_f64().ok_or_else(|| format!("variant {id}: no mark for {key}"))?;
            if !(0.0..=10.0).contains(&n) {
                return Err(format!("variant {id}: {key} is {n}, marks go from 0 to 10"));
            }
            m[k] = n.round() as u8;
        }
        let why = text(&x["why"]);
        scores.push((id, m, (!why.is_empty()).then_some(why)));
    }
    let recommend = text(&v["recommend"]);
    if !ids.contains(&recommend) {
        return Err(format!("recommended variant {recommend:?} is not one of {}", ids.join(", ")));
    }
    Ok(Comparison { scores, recommend, advice: text(&v["advice"]) })
}

/// `{"choice":"B"}` of the Reviewer deciding a tie.
pub fn parse_choice(s: &str, options: &[String]) -> Result<String, String> {
    let c = text(&obj(s)?["choice"]);
    if options.contains(&c) { Ok(c) } else { Err(format!("{c:?} is not one of {}", options.join(", "))) }
}

/// Tests that passed and failed in the output of a check: `12 passed`, `3 failed` (pytest, jest, cargo ...),
/// summed over all matches. None when the output says neither.
pub fn count_tests(out: &str) -> Option<(u32, u32)> {
    let sum = |re: &regex::Regex| re.captures_iter(out).filter_map(|c| c[1].parse::<u32>().ok()).fold(None, |a: Option<u32>, n| Some(a.unwrap_or(0) + n));
    let passed = sum(&regex::Regex::new(r"(?i)(\d+)\s+passed").ok()?);
    let failed = sum(&regex::Regex::new(r"(?i)(\d+)\s+failed").ok()?);
    (passed.is_some() || failed.is_some()).then(|| (passed.unwrap_or(0), failed.unwrap_or(0)))
}
