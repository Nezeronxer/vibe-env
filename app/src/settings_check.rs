//! What `set_settings` checks before anything is applied (G16.2): a number out of range, not a number, a taken key,
//! a path that is not there, quotes that do not close. The error is `<key>: <field>`; the field is the dotted path of
//! the value in the settings, so the window can put the hint under the right field. Nothing is applied on an error.
use crate::hub::{lk, Hub, Res};
use nightshift_core::model::Settings;
use serde_json::Value;
use tauri_plugin_global_shortcut::Shortcut;

const TIMES: [&str; 4] = ["limits.night_from", "limits.night_to", "notify.quiet_from", "notify.quiet_to"];

fn at<'a>(v: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.').try_fold(v, |v, k| v.get(k))
}

fn hhmm(s: &str) -> bool {
    let Some((h, m)) = s.split_once(':') else { return false };
    let ok = |x: &str, max: u32| x.len() == 2 && x.parse::<u32>().is_ok_and(|n| n <= max);
    ok(h, 23) && ok(m, 59)
}

/// Words of a line as separate arguments, the way a shell reads quotes but without running anything:
/// whitespace splits, `'...'` is literal, `"..."` takes `\"` and `\\`, a backslash outside quotes takes the next
/// character. Quotes that do not close are an error (`bad_quotes`).
pub fn split_args(line: &str) -> Result<Vec<String>, String> {
    let (mut out, mut cur, mut have) = (vec![], String::new(), false);
    let mut it = line.chars();
    while let Some(c) = it.next() {
        match c {
            c if c.is_whitespace() => {
                if have {
                    out.push(std::mem::take(&mut cur));
                    have = false;
                }
            }
            '\'' => {
                have = true;
                loop {
                    match it.next() {
                        Some('\'') => break,
                        Some(x) => cur.push(x),
                        None => return Err("bad_quotes".into()),
                    }
                }
            }
            '"' => {
                have = true;
                loop {
                    match it.next() {
                        Some('"') => break,
                        Some('\\') => match it.next() {
                            Some(x @ ('"' | '\\')) => cur.push(x),
                            Some(x) => {
                                cur.push('\\');
                                cur.push(x)
                            }
                            None => return Err("bad_quotes".into()),
                        },
                        Some(x) => cur.push(x),
                        None => return Err("bad_quotes".into()),
                    }
                }
            }
            '\\' => {
                have = true;
                cur.push(it.next().ok_or("bad_quotes")?);
            }
            x => {
                have = true;
                cur.push(x);
            }
        }
    }
    if have {
        out.push(cur);
    }
    Ok(out)
}

/// A shell template of an own command: every quote must close (the words are checked by the adapter when it runs).
fn closes(template: &str) -> bool {
    split_args(&template.replace("{prompt}", "x").replace("{cwd}", "x").replace("{session}", "x")).is_ok()
}

/// Checks `new` against the old settings and returns the typed result.
pub fn check(old: &Settings, mut new: Value) -> Res<Settings> {
    for (path, min, max, whole) in nightshift_core::model::SETTING_RANGES {
        let Some(v) = at(&new, path) else { continue };
        let n = v.as_f64().ok_or_else(|| format!("not_a_number: {path}"))?;
        if whole && n.fract() != 0.0 {
            return Err(format!("not_a_number: {path}"));
        }
        if n < min || n > max {
            return Err(format!("out_of_range: {path} {min}..{max}"));
        }
    }
    for path in TIMES {
        if let Some(v) = at(&new, path) {
            if !v.as_str().is_some_and(hhmm) {
                return Err(format!("bad_time: {path}"));
            }
        }
    }
    // extra arguments may arrive as the line the person typed: split them here, without a shell
    if let Some(envs) = new.get_mut("envs").and_then(Value::as_object_mut) {
        for (id, e) in envs.iter_mut() {
            if let Some(line) = e.get("extra_args").and_then(Value::as_str).map(str::to_string) {
                let words = split_args(&line).map_err(|k| format!("{k}: envs.{id}.extra_args"))?;
                e["extra_args"] = Value::from(words);
            }
        }
    }
    let s: Settings = serde_json::from_value(new).map_err(|e| format!("bad_settings: {e}"))?;
    // keys: both global ones must be readable and different from each other and from the env switch
    let sc = |f: &str, v: &str| v.replace("{n}", "1").parse::<Shortcut>().map_err(|_| format!("bad_key: keys.{f}"));
    let (nc, fr, sw) = (sc("new_chat", &s.keys.new_chat)?, sc("freeze", &s.keys.freeze)?, sc("switch_env", &s.keys.switch_env)?);
    if nc == fr {
        return Err("key_busy: keys.freeze".into());
    }
    if sw == nc || sw == fr {
        return Err("key_busy: keys.switch_env".into());
    }
    for (id, e) in &s.envs {
        let before = old.envs.get(id).and_then(|o| o.cli_path.as_ref());
        if let Some(p) = e.cli_path.as_ref().filter(|p| Some(*p) != before) {
            if !p.is_file() {
                return Err(format!("path_missing: envs.{id}.cli_path"));
            }
        }
    }
    for c in &s.custom_agents {
        for (field, t) in [("run", Some(&c.run)), ("resume", c.resume.as_ref())] {
            if t.is_some_and(|t| !closes(t)) {
                return Err(format!("bad_quotes: custom_agents.{}.{field}", c.id));
            }
        }
    }
    Ok(s)
}

impl Hub {
    /// Validates, lets `keys` register the global keys (new ones first), stores, and gives runs that start later the
    /// new commands; runs that are going keep theirs (G16.3). Returns the settings that were replaced.
    pub fn set_settings(&self, new: Value, keys: &dyn Fn(&Settings, &Settings) -> Res<()>) -> Res<Settings> {
        let old = self.settings();
        let s = check(&old, new)?;
        keys(&old, &s)?;
        if let Err(e) = lk(self.engine.store()).set_settings(s.clone()) {
            let _ = keys(&s, &old);
            return Err(e.to_string());
        }
        if serde_json::json!(s.custom_agents) != serde_json::json!(old.custom_agents) || serde_json::json!(s.envs) != serde_json::json!(old.envs) {
            self.reload_adapters();
            self.refresh_envs();
        }
        for f in lk(&self.settings_hooks).iter() {
            f(&s);
        }
        Ok(old)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::*;
    use nightshift_core::agent::FakeAdapter;
    use serde_json::json;

    fn with(r: &Rig, f: impl FnOnce(&mut Value)) -> Value {
        let mut v = serde_json::to_value(r.hub.settings()).unwrap();
        f(&mut v);
        v
    }

    #[test]
    fn a_bad_value_is_refused_with_its_field_and_nothing_is_applied() {
        let r = Rig::new(FakeAdapter::new(vec![]));
        let keep = |_: &Settings, _: &Settings| -> Res<()> { Ok(()) };
        let before = serde_json::to_value(r.hub.settings()).unwrap();
        let cases: Vec<(Value, &str)> = vec![
            (with(&r, |v| v["max_parallel"] = json!(0)), "out_of_range: max_parallel 1..10"),
            (with(&r, |v| v["limits"]["task_usd"] = json!(0.1)), "out_of_range: limits.task_usd 0.5..1000"),
            (with(&r, |v| v["limits"]["warn_pct"] = json!(120)), "out_of_range: limits.warn_pct 50..99"),
            (with(&r, |v| v["look"]["font_px"] = json!("big")), "not_a_number: look.font_px"),
            (with(&r, |v| v["attempts"] = json!(2.5)), "not_a_number: attempts"),
            (with(&r, |v| v["limits"]["night_from"] = json!("25:00")), "bad_time: limits.night_from"),
            (with(&r, |v| v["notify"]["quiet_to"] = json!("8:00")), "bad_time: notify.quiet_to"),
            (with(&r, |v| v["keys"]["freeze"] = serde_json::to_value(&r.hub.settings().keys.new_chat).unwrap()), "key_busy: keys.freeze"),
            (with(&r, |v| v["keys"]["new_chat"] = json!("Banana+Q")), "bad_key: keys.new_chat"),
            (with(&r, |v| v["envs"]["fake"]["cli_path"] = json!("/no/such/claude")), "path_missing: envs.fake.cli_path"),
            (with(&r, |v| v["envs"]["fake"]["extra_args"] = json!("--model 'opus")), "bad_quotes: envs.fake.extra_args"),
            (with(&r, |v| v["custom_agents"] = json!([{"id": "custom:x", "name": "X", "run": "tool \"{prompt}", "resume": null}])), "bad_quotes: custom_agents.custom:x.run"),
        ];
        for (v, want) in cases {
            assert_eq!(r.hub.set_settings(v, &keep).unwrap_err(), want);
        }
        assert_eq!(serde_json::to_value(r.hub.settings()).unwrap(), before, "no refusal changed anything");
    }

    #[test]
    fn a_good_change_is_stored_extra_arguments_become_separate_words_and_an_own_command_is_seen_by_new_runs() {
        let r = Rig::new(FakeAdapter::new(vec![]));
        let keys_seen = std::cell::RefCell::new(vec![]);
        let keys = |a: &Settings, b: &Settings| -> Res<()> {
            keys_seen.borrow_mut().push((a.keys.new_chat.clone(), b.keys.new_chat.clone()));
            Ok(())
        };
        let v = with(&r, |v| {
            v["max_parallel"] = json!(5);
            v["keys"]["new_chat"] = json!("Alt+Shift+K");
            v["envs"]["fake"]["extra_args"] = json!("--model \"my model\" -v  'a b'");
            v["custom_agents"] = json!([{"id": "custom:x", "name": "X", "run": "tool {prompt}", "resume": null}]);
        });
        let heard = std::sync::Arc::new(std::sync::Mutex::new(vec![]));
        let h2 = heard.clone();
        r.hub.on_settings(Box::new(move |s| h2.lock().unwrap().push(s.max_parallel)));
        r.hub.set_settings(v, &keys).unwrap();
        assert_eq!(*heard.lock().unwrap(), vec![5], "what follows a save is told the new settings once");
        let s = r.hub.settings();
        assert_eq!(s.max_parallel, 5);
        assert_eq!(s.envs["fake"].extra_args, ["--model", "my model", "-v", "a b"]);
        assert_eq!(keys_seen.borrow().len(), 1);
        assert_eq!(keys_seen.borrow()[0].1, "Alt+Shift+K");
        assert!(r.hub.adapter("custom:x").is_some(), "the own command is in the list of the next runs");
    }

    #[test]
    fn arguments_are_split_like_words_with_quotes_and_never_run() {
        assert_eq!(split_args("a  b").unwrap(), ["a", "b"]);
        assert_eq!(split_args(r#"--x "a b" 'c d' e\ f"#).unwrap(), ["--x", "a b", "c d", "e f"]);
        assert_eq!(split_args(r#"$(id) `x` ; |"#).unwrap(), ["$(id)", "`x`", ";", "|"]);
        assert_eq!(split_args("''").unwrap(), [""]);
        assert!(split_args("a 'b").is_err() && split_args("a \"b").is_err() && split_args("a\\").is_err());
    }
}
