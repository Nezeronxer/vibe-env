//! The rules of the swarm (spec 23): which models the variants get, whether the parts of a crew
//! overlap, who wins a vote, how variants are ranked, which failed attempt is the best. Models only
//! propose; everything that picks or counts is a pure function here, with no process and no I/O.
pub mod parse;
pub mod prompts;
pub mod view;

use crate::model::{Catalog, ModelChoice};

/// "A", "B", ... : the name of a variant (and of a voter of the council).
pub type VariantId = String;

/// The names of `n` variants: A, B, C ...
pub fn letters(n: usize) -> Vec<VariantId> {
    (0..n).map(|i| ((b'A' + (i % 26) as u8) as char).to_string()).collect()
}

/// Models of `n` variants: the executor's model first, then the models of the env in the order of
/// its catalog, round the circle. Only the executor's own choice keeps its pinned version; the
/// others take the newest one of their family.
pub fn assign_models(cat: &Catalog, worker: &ModelChoice, n: u8) -> Vec<ModelChoice> {
    let ids: Vec<&str> = cat.families.iter().map(|f| f.id.as_str()).collect();
    if ids.is_empty() {
        return vec![worker.clone(); n as usize];
    }
    let start = ids.iter().position(|id| *id == worker.family).unwrap_or(0);
    (0..n as usize)
        .map(|i| {
            if i == 0 {
                worker.clone()
            } else {
                ModelChoice { family: ids[(start + i) % ids.len()].to_string(), version: None }
            }
        })
        .collect()
}

/// One part of a crew's split: who does it and which files it may touch.
#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    pub id: String,
    pub files: Vec<String>,
}

/// Two parts that touch the same file (or a folder and a file inside it).
#[derive(Clone, Debug, PartialEq)]
pub struct Overlap {
    pub a: String,
    pub b: String,
    pub path_a: String,
    pub path_b: String,
}

/// Relative path in a comparable form: no `./`, no doubled or trailing `/`, lower case (the file
/// system of the person may not tell `A.rs` from `a.rs`; a false overlap only costs a re-split).
/// The whole project (`.`, empty) is the empty string.
pub fn norm_path(p: &str) -> String {
    p.trim()
        .split('/')
        .filter(|c| !c.is_empty() && *c != ".")
        .collect::<Vec<_>>()
        .join("/")
        .to_lowercase()
}

/// `inner` is `outer` or lies inside the folder `outer` (both normalised; "" is everything).
fn inside(outer: &str, inner: &str) -> bool {
    outer.is_empty() || inner == outer || inner.strip_prefix(outer).is_some_and(|r| r.starts_with('/'))
}

/// Where a crew's split does not hold: the first pair of parts (in the order given) that share a
/// file. A path that is a folder covers everything inside it. The app never takes the split of the
/// Coordinator on its word.
pub fn check_split(parts: &[Part]) -> Result<(), Overlap> {
    for (i, a) in parts.iter().enumerate() {
        for b in &parts[i + 1..] {
            for pa in &a.files {
                for pb in &b.files {
                    let (na, nb) = (norm_path(pa), norm_path(pb));
                    if inside(&na, &nb) || inside(&nb, &na) {
                        return Err(Overlap { a: a.id.clone(), b: b.id.clone(), path_a: pa.clone(), path_b: pb.clone() });
                    }
                }
            }
        }
    }
    Ok(())
}

/// One voter's choice (a proposal id, or `approve` / `changes` on a review).
#[derive(Clone, Debug, PartialEq)]
pub struct Vote {
    pub voter: String,
    pub choice: String,
}

/// The count of a vote.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Tally {
    /// Votes per choice, only choices that got any.
    pub counts: std::collections::BTreeMap<String, u32>,
    /// The choice that has more votes than every other one.
    pub winner: Option<String>,
    /// When no choice does: the choices that share the top (sorted). Empty without votes.
    pub tied: Vec<String>,
    /// Votes counted (one per voter).
    pub total: u32,
}

/// Counts a vote. The most votes win (not necessarily more than half); a shared top has no winner,
/// somebody else decides (the Reviewer). A voter counts once, the last word counts.
pub fn tally(votes: &[Vote]) -> Tally {
    let mut last: std::collections::BTreeMap<&str, &str> = std::collections::BTreeMap::new();
    for v in votes {
        last.insert(&v.voter, &v.choice);
    }
    let mut t = Tally::default();
    for choice in last.values() {
        *t.counts.entry(choice.to_string()).or_default() += 1;
        t.total += 1;
    }
    let top = t.counts.values().copied().max().unwrap_or(0);
    let leaders: Vec<String> = t.counts.iter().filter(|(_, n)| **n == top).map(|(k, _)| k.clone()).collect();
    match leaders.len() {
        0 => {}
        1 => t.winner = leaders.into_iter().next(),
        _ => t.tied = leaders,
    }
    t
}

/// The reviewer's marks of a variant, 0..=10 each; for all four a higher mark is better
/// (`risk`: 10 = the safest).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scores {
    pub correctness: u8,
    pub coverage: u8,
    pub simplicity: u8,
    pub risk: u8,
}

impl Scores {
    pub fn total(&self) -> u32 {
        self.correctness as u32 + self.coverage as u32 + self.simplicity as u32 + self.risk as u32
    }
}

/// What the rules know of one variant.
#[derive(Clone, Debug, PartialEq)]
pub struct VariantResult {
    pub id: VariantId,
    /// The gates are green.
    pub passed: bool,
    /// Green tests and all tests the gates ran (counts from their output, else green gates of all gates).
    pub green: u32,
    pub total: u32,
    pub scores: Option<Scores>,
    /// The reviewer recommended it.
    pub recommended: bool,
}

/// The variants that passed, best first: by the total of the four marks, then the one the reviewer
/// recommends, then the more correct one, then the order given. Variants with no marks come after
/// the marked ones. Failed variants are not ranked (see `best_failed`).
pub fn rank(results: &[VariantResult]) -> Vec<VariantId> {
    let mut passed: Vec<&VariantResult> = results.iter().filter(|r| r.passed).collect();
    // stable: equal keys keep the order given
    passed.sort_by_key(|r| {
        let s = r.scores.as_ref();
        (
            std::cmp::Reverse(s.map_or(0, Scores::total)),
            std::cmp::Reverse(r.recommended),
            std::cmp::Reverse(s.map_or(0, |s| s.correctness)),
        )
    });
    passed.into_iter().map(|r| r.id.clone()).collect()
}

/// When nothing passed: the attempt with most green tests; on a tie the one with fewer red tests,
/// then the first.
pub fn best_failed(results: &[VariantResult]) -> Option<VariantId> {
    results
        .iter()
        .filter(|r| !r.passed)
        .min_by_key(|r| (std::cmp::Reverse(r.green), r.total.saturating_sub(r.green)))
        .map(|r| r.id.clone())
}

/// `path` is one of the `declared` files or lies inside a declared folder.
pub fn covers(declared: &[String], path: &str) -> bool {
    let p = norm_path(path);
    declared.iter().any(|d| inside(&norm_path(d), &p))
}
