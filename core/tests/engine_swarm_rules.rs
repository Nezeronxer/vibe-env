//! The rules of the swarm (`core::swarm`) on tables: expected values are worked out by hand.
use chrono::Local;
use nightshift_core::model::*;
use nightshift_core::swarm;

fn catalog(ids: &[&str]) -> Catalog {
    Catalog {
        env: "fake".into(),
        families: ids
            .iter()
            .map(|id| ModelFamily { id: id.to_string(), label: id.to_string(), vendor: "x".into(), blurb: None, versions: vec![], default: false, exact: Default::default() })
            .collect(),
        source: CatalogSource::BuiltIn,
        at: Local::now(),
    }
}

fn choice(f: &str) -> ModelChoice {
    ModelChoice { family: f.into(), version: None }
}

fn fams(v: Vec<ModelChoice>) -> Vec<String> {
    v.into_iter().map(|c| c.family).collect()
}

#[test]
fn variants_take_the_models_of_the_env_round_the_circle_from_the_executors_model() {
    let cat = catalog(&["default", "opus", "sonnet", "haiku"]);
    // table: (executor, n) -> models, by hand: the circle is default, opus, sonnet, haiku
    let table: &[(&str, u8, &[&str])] = &[
        ("opus", 2, &["opus", "sonnet"]),
        ("opus", 3, &["opus", "sonnet", "haiku"]),
        ("sonnet", 3, &["sonnet", "haiku", "default"]),
        ("haiku", 5, &["haiku", "default", "opus", "sonnet", "haiku"]),
        ("default", 4, &["default", "opus", "sonnet", "haiku"]),
    ];
    for (worker, n, want) in table {
        assert_eq!(fams(swarm::assign_models(&cat, &choice(worker), *n)), *want, "executor {worker}, n={n}");
    }
    // a model that is not in the catalog (the person typed it): the circle starts from the first entry
    assert_eq!(fams(swarm::assign_models(&cat, &choice("mine"), 3)), ["mine", "opus", "sonnet"]);
    // the executor keeps its pinned version, the others take the newest
    let pinned = ModelChoice { family: "opus".into(), version: Some("4.5".into()) };
    let got = swarm::assign_models(&cat, &pinned, 2);
    assert_eq!((got[0].version.as_deref(), got[1].version.clone()), (Some("4.5"), None));
    // one model in the env: every variant gets it
    assert_eq!(fams(swarm::assign_models(&catalog(&["only"]), &choice("only"), 3)), ["only", "only", "only"]);
}

fn part(id: &str, files: &[&str]) -> swarm::Part {
    swarm::Part { id: id.into(), files: files.iter().map(|f| f.to_string()).collect() }
}

#[test]
fn a_split_whose_parts_share_a_file_or_a_folder_is_refused() {
    // table by hand: (parts, the overlap or None)
    let ok: Vec<Vec<swarm::Part>> = vec![
        vec![part("p1", &["src/a.rs"]), part("p2", &["src/b.rs"])],
        vec![part("p1", &["src/a.rs", "src/a_test.rs"]), part("p2", &["docs/x.md"]), part("p3", &["README.md"])],
        // a name that only starts like the other is not inside it
        vec![part("p1", &["src"]), part("p2", &["src2/a.rs"])],
        vec![part("p1", &["a.txt"])],
        vec![],
    ];
    for parts in &ok {
        assert!(swarm::check_split(parts).is_ok(), "{parts:?}");
    }
    let bad: Vec<(Vec<swarm::Part>, (&str, &str))> = vec![
        (vec![part("p1", &["src/a.rs"]), part("p2", &["lib/x.rs", "src/a.rs"])], ("p1", "p2")),
        // nested: a folder and a file inside it
        (vec![part("p1", &["src/"]), part("p2", &["src/deep/a.rs"])], ("p1", "p2")),
        (vec![part("p1", &["src/deep/a.rs"]), part("p2", &["src"])], ("p1", "p2")),
        // the same file written another way
        (vec![part("p1", &["./src//a.rs"]), part("p2", &["src/a.rs"])], ("p1", "p2")),
        // the third part collides with the first
        (vec![part("p1", &["a.txt"]), part("p2", &["b.txt"]), part("p3", &["a.txt"])], ("p1", "p3")),
        // the whole project overlaps with everything
        (vec![part("p1", &["."]), part("p2", &["a.txt"])], ("p1", "p2")),
        // the file system may not tell the case apart
        (vec![part("p1", &["Src/A.rs"]), part("p2", &["src/a.rs"])], ("p1", "p2")),
    ];
    for (parts, (a, b)) in bad {
        let o = swarm::check_split(&parts).unwrap_err();
        assert_eq!((o.a.as_str(), o.b.as_str()), (a, b), "{parts:?}");
    }
    // a part that lists the same file twice does not overlap with itself
    assert!(swarm::check_split(&[part("p1", &["a.txt", "a.txt"]), part("p2", &["b.txt"])]).is_ok());
}

fn vote(who: &str, choice: &str) -> swarm::Vote {
    swarm::Vote { voter: who.into(), choice: choice.into() }
}

#[test]
fn the_most_votes_win_and_a_shared_top_is_a_tie() {
    // table by hand: (votes, winner, tied, counts)
    type Case<'a> = (Vec<swarm::Vote>, Option<&'a str>, Vec<&'a str>, Vec<(&'a str, u32)>);
    let cases: Vec<Case> = vec![
        (vec![vote("A", "B"), vote("B", "B"), vote("C", "A")], Some("B"), vec![], vec![("A", 1), ("B", 2)]),
        // 2 of 5 is not the majority of the voters, but nobody has more: it wins
        (vec![vote("A", "A"), vote("B", "A"), vote("C", "B"), vote("D", "C"), vote("E", "D")], Some("A"), vec![], vec![("A", 2), ("B", 1), ("C", 1), ("D", 1)]),
        (vec![vote("A", "A"), vote("B", "B")], None, vec!["A", "B"], vec![("A", 1), ("B", 1)]),
        (vec![vote("A", "C"), vote("B", "A"), vote("C", "B")], None, vec!["A", "B", "C"], vec![("A", 1), ("B", 1), ("C", 1)]),
        // the tie is only at the top: the one below it is not in it
        (vec![vote("A", "A"), vote("B", "A"), vote("C", "B"), vote("D", "B"), vote("E", "C")], None, vec!["A", "B"], vec![("A", 2), ("B", 2), ("C", 1)]),
        (vec![vote("A", "A")], Some("A"), vec![], vec![("A", 1)]),
        (vec![], None, vec![], vec![]),
        // a voter who votes twice counts once: the last word counts
        (vec![vote("A", "A"), vote("A", "B"), vote("B", "B")], Some("B"), vec![], vec![("B", 2)]),
    ];
    for (votes, winner, tied, counts) in cases {
        let t = swarm::tally(&votes);
        assert_eq!(t.winner.as_deref(), winner, "{votes:?}");
        assert_eq!(t.tied, tied, "{votes:?}");
        assert_eq!(t.counts.iter().map(|(k, v)| (k.as_str(), *v)).collect::<Vec<_>>(), counts, "{votes:?}");
        assert_eq!(t.total as usize, counts.iter().map(|(_, v)| *v as usize).sum::<usize>());
    }
}

fn res(id: &str, passed: bool, green: u32, total: u32, scores: Option<[u8; 4]>, recommended: bool) -> swarm::VariantResult {
    swarm::VariantResult {
        id: id.into(),
        passed,
        green,
        total,
        scores: scores.map(|s| swarm::Scores { correctness: s[0], coverage: s[1], simplicity: s[2], risk: s[3] }),
        recommended,
    }
}

#[test]
fn passed_variants_are_ranked_by_their_four_scores() {
    let ids = |v: Vec<String>| v.join("");
    // totals by hand: A = 9+8+8+9 = 34, B = 6+8+9+5 = 28, D = 7+7+7+7 = 28, E = 8+6+7+7 = 28
    let a = res("A", true, 14, 14, Some([9, 8, 8, 9]), false);
    let b = res("B", true, 14, 14, Some([6, 8, 9, 5]), false);
    assert_eq!(ids(swarm::rank(&[a.clone(), b.clone()])), "AB");
    assert_eq!(ids(swarm::rank(&[b.clone(), a.clone()])), "AB", "the order of the input does not matter when the scores differ");
    // a failed variant is not ranked at all
    let c = res("C", false, 12, 14, Some([10, 10, 10, 10]), false);
    assert_eq!(ids(swarm::rank(&[c.clone(), b.clone(), a.clone()])), "AB");
    // the same total: the one the reviewer recommends goes first, then the more correct one, then the order given
    let d = res("D", true, 3, 3, Some([7, 7, 7, 7]), false);
    let e = res("E", true, 3, 3, Some([8, 6, 7, 7]), false);
    // by correctness (8, 7, 6): E, D, B
    assert_eq!(ids(swarm::rank(&[b.clone(), d.clone(), e.clone()])), "EDB");
    // the recommendation of the reviewer breaks the tie first
    let d_rec = res("D", true, 3, 3, Some([7, 7, 7, 7]), true);
    assert_eq!(ids(swarm::rank(&[b.clone(), d_rec, e.clone()])), "DEB");
    // the recommendation does not beat a higher total
    let b_rec = res("B", true, 14, 14, Some([6, 8, 9, 5]), true);
    assert_eq!(ids(swarm::rank(&[b_rec, a.clone()])), "AB");
    // no scores (the reviewer's answer was unusable): the ones with scores first, the rest in the order given
    let x = res("X", true, 1, 1, None, false);
    let y = res("Y", true, 1, 1, None, false);
    assert_eq!(ids(swarm::rank(&[x.clone(), y.clone(), b.clone()])), "BXY");
    assert_eq!(ids(swarm::rank(&[x, y])), "XY");
    assert!(swarm::rank(&[c]).is_empty());
    assert!(swarm::rank(&[]).is_empty());
}

#[test]
fn the_best_failed_attempt_is_the_one_with_most_green_tests() {
    let id = |v: Vec<swarm::VariantResult>| swarm::best_failed(&v);
    // by hand: 12 green beats 9
    let a = res("A", false, 12, 14, None, false);
    let b = res("B", false, 9, 14, None, false);
    let ok = res("C", true, 14, 14, None, false);
    assert_eq!(id(vec![b.clone(), a.clone(), ok.clone()]).as_deref(), Some("A"), "a passed variant is not a failed attempt");
    // the same number of green: the one with fewer red (5 of 6 against 5 of 10)
    let x = res("X", false, 5, 10, None, false);
    let y = res("Y", false, 5, 6, None, false);
    assert_eq!(id(vec![x.clone(), y.clone()]).as_deref(), Some("Y"));
    // everything the same: the first
    let z = res("Z", false, 5, 6, None, false);
    assert_eq!(id(vec![y, z]).as_deref(), Some("Y"));
    // nothing green anywhere: still a best attempt (the first)
    assert_eq!(id(vec![res("P", false, 0, 0, None, false), res("Q", false, 0, 0, None, false)]).as_deref(), Some("P"));
    // nobody failed
    assert_eq!(id(vec![ok]), None);
    assert_eq!(id(vec![]), None);
}

#[test]
fn the_app_accepts_only_a_split_whose_paths_are_exact_and_inside_the_project() {
    use nightshift_core::swarm::parse::parse_split;
    let j = |parts: &str| format!(r#"{{"parts":[{parts}]}}"#);
    let ok = parse_split(&j(r#"{"id":"p1","title":"a","files":["src/a.rs","docs/"],"brief":"x"},{"id":"p2","files":["b.rs"]}"#), 3).unwrap();
    assert_eq!((ok[0].files.clone(), ok[1].title.as_str()), (vec!["src/a.rs".to_string(), "docs/".to_string()], "p2"));
    // by hand: each of these is refused, and the reason names the problem
    let bad: &[(&str, &str)] = &[
        (r#"{"id":"p1","files":["../outside.txt"]}"#, "relative path inside"),
        (r#"{"id":"p1","files":["/etc/passwd"]}"#, "relative path inside"),
        (r#"{"id":"p1","files":["src/*.rs"]}"#, "relative path inside"),
        (r#"{"id":"p1","files":["C:\\x"]}"#, "relative path inside"),
        (r#"{"id":"p1","files":[]}"#, "names no files"),
        (r#"{"id":"p1","files":["a"]},{"id":"p1","files":["b"]}"#, "used twice"),
        (r#"{"id":"p1","files":["a"]},{"id":"p2","files":["b"]},{"id":"p3","files":["c"]},{"id":"p4","files":["d"]}"#, "at most 3"),
    ];
    for (parts, why) in bad {
        let e = parse_split(&j(parts), 3).unwrap_err();
        assert!(e.contains(why), "{parts}: {e}");
    }
    assert!(parse_split("no json here", 3).is_err());
}

#[test]
fn a_convoy_of_more_than_twelve_tasks_or_with_a_broken_chain_of_dependencies_is_refused() {
    use nightshift_core::swarm::parse::parse_convoy;
    let task = |id: &str, deps: &[&str]| serde_json::json!({"id": id, "title": format!("task {id}"), "text": "do it", "depends_on": deps});
    let j = |ts: Vec<serde_json::Value>| serde_json::json!({ "tasks": ts }).to_string();
    let ok = parse_convoy(&j(vec![task("t1", &[]), task("t2", &["t1"]), task("t3", &["t1", "t2"])])).unwrap();
    assert_eq!(ok[2].depends_on, ["t1", "t2"]);
    // 12 is fine, 13 is not
    let many = |n: usize| j((0..n).map(|i| task(&format!("t{i}"), &[])).collect());
    assert_eq!(parse_convoy(&many(12)).unwrap().len(), 12);
    assert!(parse_convoy(&many(13)).unwrap_err().contains("more than 12"));
    assert!(parse_convoy(&j(vec![task("t1", &["t9"])])).unwrap_err().contains("not in the list"));
    assert!(parse_convoy(&j(vec![task("t1", &["t1"])])).unwrap_err().contains("waits for itself"));
    assert!(parse_convoy(&j(vec![task("t1", &["t2"]), task("t2", &["t1"])])).unwrap_err().contains("circle"));
    assert!(parse_convoy(&j(vec![task("t1", &[]), task("t1", &[])])).unwrap_err().contains("twice"));
    assert!(parse_convoy(&j(vec![])).is_err());
}
