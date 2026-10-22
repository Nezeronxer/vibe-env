//! The window checks the numbers of the settings before it sends them; the core checks them again with
//! `model::SETTING_RANGES`. The two copies must say the same, or the window lets through what the core refuses
//! (or refuses what the core takes). The window's copy lives in `ui/js/screens/settings.js` (the rule of each number
//! field) and `ui/logic.js` (the swarm size).
use nightshift_core::model::SETTING_RANGES;
use std::collections::BTreeMap;

fn ui(file: &str) -> String {
    std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../ui").join(file)).unwrap()
}

/// `setting path -> (min, max)` of every number field of the settings screen: the rule and the call that saves it.
fn screen_ranges() -> BTreeMap<String, Vec<(f64, f64)>> {
    let rule = regex::Regex::new(r"\{ min: ([\d.]+), max: ([\d.]+)").unwrap();
    let save = regex::Regex::new(r"\b(commit|setL|setLook|setQ)\(\{ (\w+):").unwrap();
    let text = ui("js/screens/settings.js");
    let lines: Vec<&str> = text.lines().collect();
    let mut out: BTreeMap<String, Vec<(f64, f64)>> = BTreeMap::new();
    for (i, l) in lines.iter().enumerate() {
        for c in rule.captures_iter(l) {
            // the save call is on the line of the rule or a few lines below it (a field with its own handler)
            let s = lines[i..(i + 4).min(lines.len())].iter().find_map(|l| save.captures(l)).unwrap_or_else(|| panic!("no save call for line {}: {l}", i + 1));
            let prefix = match &s[1] { "setL" => "limits.", "setLook" => "look.", "setQ" => "quality.", _ => "" };
            let r = (c[1].parse().unwrap(), c[2].parse().unwrap());
            let v = out.entry(format!("{prefix}{}", &s[2])).or_default();
            if !v.contains(&r) {
                v.push(r);
            }
        }
    }
    out
}

#[test]
fn the_window_checks_the_settings_with_the_same_ranges_as_the_core() {
    let core: BTreeMap<&str, (f64, f64)> = SETTING_RANGES.iter().map(|(p, min, max, _)| (*p, (*min, *max))).collect();
    let screen = screen_ranges();
    assert!(screen.len() >= 8, "the settings screen lost its number fields? {screen:?}");
    for (path, ranges) in &screen {
        let Some(&(min, max)) = core.get(path.as_str()) else {
            // Best-of-N is not built in the core (interfaces: «Best-of-N для рискованных задач не построен»)
            assert_eq!(path, "quality.best_of_n", "the window checks a setting the core does not: {path}");
            continue;
        };
        // "auto-pick": 0 = off comes from its switch, the number field takes 1 and up
        let min = if path == "autopick_min" && min == 0.0 { 1.0 } else { min };
        assert_eq!(ranges, &vec![(min, max)], "{path}: window {ranges:?}, core {min}..{max}");
    }
    let logic = ui("logic.js");
    let n = regex::Regex::new(r"SWARM_MIN = (\d+), SWARM_MAX = (\d+)").unwrap().captures(&logic).expect("SWARM_MIN / SWARM_MAX in logic.js");
    assert_eq!((n[1].parse::<f64>().unwrap(), n[2].parse::<f64>().unwrap()), core["swarm_default.n"], "swarm size");
    // the words under the swarm size name the range too, in both languages
    let words = ui("i18n-v2.js");
    let said: Vec<(f64, f64)> = regex::Regex::new(r"'set\.sw\.n_d': '(?:От|From) (\d+) (?:до|to) (\d+)")
        .unwrap()
        .captures_iter(&words)
        .map(|c| (c[1].parse().unwrap(), c[2].parse().unwrap()))
        .collect();
    assert_eq!(said, vec![core["swarm_default.n"]; 2], "set.sw.n_d in ru and en");
}
