//! Task 20: the config file `key = value` (stories 293, 294).
use nightshift_core::config::Config;
use nightshift_core::model::Settings;
use std::time::Duration;

fn cfg(text: &str) -> (tempfile::TempDir, Config) {
    let t = tempfile::tempdir().unwrap();
    let p = t.path().join("nightshift/config");
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(&p, text).unwrap();
    (t, Config::new(p))
}

#[test]
fn a_wrong_value_is_an_error_with_its_line_and_the_rest_applies() {
    let (_t, c) = cfg("# my look\nlook.theme = dark\n\nfont-size = big\nmax-parallel = 5\nno-such-key = 1\nlimits.warn-pct = 120\n");
    let (s, errors) = c.apply(&Settings::default()).unwrap();
    assert_eq!(s.look.theme, "dark");
    assert_eq!(s.max_parallel, 5);
    assert_eq!(s.look.font_px, Settings::default().look.font_px, "the wrong key keeps its default");
    assert_eq!(s.limits.warn_pct, Settings::default().limits.warn_pct);
    let got: Vec<_> = errors.iter().map(|e| (e.line, e.key.as_str(), e.msg.as_str())).collect();
    assert_eq!(got, [(4, "font-size", "need_number"), (6, "no-such-key", "unknown_key"), (7, "limits.warn-pct", "out_of_range: 50..99")]);
}

#[test]
fn writing_a_key_keeps_comments_and_order() {
    let text = "# Nightshift\n# my comment\nlook.theme = dark\n\n# speed\nmax-parallel = 2\nkeys.send = Enter\n";
    let (_t, c) = cfg(text);
    let (mut s, errors) = c.apply(&Settings::default()).unwrap();
    assert!(errors.is_empty(), "{errors:?}");
    c.write_key("max-parallel", "4").unwrap();
    s.look.font_px = 15;
    s.max_parallel = 4;
    c.write_settings(&s).unwrap();
    let out = std::fs::read_to_string(&c.path).unwrap();
    assert_eq!(out, "# Nightshift\n# my comment\nlook.theme = dark\n\n# speed\nmax-parallel = 4\nkeys.send = Enter\nlook.font-px = 15\n");
    let (back, errors) = c.apply(&Settings::default()).unwrap();
    assert!(errors.is_empty());
    assert_eq!((back.max_parallel, back.look.font_px, back.look.theme.as_str()), (4, 15, "dark"));
}

#[test]
fn a_missing_file_is_created_with_commented_defaults() {
    let t = tempfile::tempdir().unwrap();
    let c = Config::new(t.path().join("nightshift/config"));
    assert!(c.apply(&Settings::default()).unwrap().1.is_empty());
    assert!(c.create_default().unwrap());
    let text = std::fs::read_to_string(&c.path).unwrap();
    assert!(text.lines().any(|l| l == "# max-parallel = 3"), "{text}");
    assert!(text.lines().any(|l| l == "# look.font-px = 13"));
    assert!(text.lines().filter(|l| !l.trim().is_empty()).all(|l| l.starts_with('#')));
    let (s, errors) = c.apply(&Settings::default()).unwrap();
    assert!(errors.is_empty());
    assert_eq!(serde_json::to_value(&s).unwrap(), serde_json::to_value(Settings::default()).unwrap());
    assert!(!c.create_default().unwrap(), "an existing file is not touched");
}

#[test]
fn a_change_of_the_file_is_seen() {
    let (_t, c) = cfg("max-parallel = 2\n");
    let (tx, rx) = std::sync::mpsc::channel();
    c.watch(Box::new(move |errors| {
        let _ = tx.send(errors);
    }))
    .unwrap();
    std::thread::sleep(Duration::from_millis(1200));
    std::fs::write(&c.path, "max-parallel = many\n").unwrap();
    let errors = rx.recv_timeout(Duration::from_secs(4)).expect("no config-changed");
    assert_eq!(errors.len(), 1);
    assert_eq!((errors[0].line, errors[0].msg.as_str()), (1, "need_number"));
}
