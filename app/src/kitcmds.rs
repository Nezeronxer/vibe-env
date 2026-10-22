//! `kits_*` and `set_secret`: thin over `Kits`. Errors are the core's `KitError` as JSON (`{kind, ...}`), shown by the
//! window with its own words. A secret's value goes to the keyring and nowhere else: not to a log, not to an event, not back.
use crate::hub::{lk, Hub};
use nightshift_core::kits::{Choices, ExportDest, HeldRef, ImportPlan, Installed, KitError, KitSource, Recovered, Scope, Selection};
use serde_json::{json, Value};

/// A refusal as the window gets it: the `KitError` JSON.
pub type KRes<T> = Result<T, Value>;

fn kerr(e: KitError) -> Value {
    serde_json::to_value(&e).unwrap_or_else(|_| json!({ "kind": e.kind() }))
}
fn ok(v: serde_json::Result<Value>) -> KRes<Value> {
    v.map_err(|e| json!({ "kind": "io", "0": e.to_string() }))
}

/// Previews kept for "Install": about as many as the window can have open.
const KEEP_PLANS: usize = 16;

impl Hub {
    pub fn kits_inventory(&self, env: &str, scope: &Scope) -> KRes<Value> {
        ok(serde_json::to_value(self.kits.inventory(env, scope)))
    }

    pub fn kits_export(&self, env: &str, sel: &Selection, dest: ExportDest) -> KRes<Value> {
        ok(serde_json::to_value(self.kits.export(env, sel, dest).map_err(kerr)?))
    }

    fn remember(&self, plan: ImportPlan) -> KRes<Value> {
        let v = ok(serde_json::to_value(&plan))?;
        let mut plans = lk(&self.plans);
        plans.retain(|p| p.id != plan.id);
        plans.push(plan);
        if plans.len() > KEEP_PLANS {
            plans.remove(0);
        }
        Ok(v)
    }

    pub fn kits_preview(&self, env: &str, source: KitSource, scope: &Scope) -> KRes<Value> {
        self.remember(self.kits.preview(env, source, scope).map_err(kerr)?)
    }

    pub fn kits_check_update(&self, installed: &Installed) -> KRes<Value> {
        self.remember(self.kits.check_update(installed).map_err(kerr)?)
    }

    /// Installs the preview with this id (the window sends the plan back; only its id counts, so a plan that was
    /// edited on the way is never trusted). A kit "only into the project" puts the files it wrote into the project's
    /// "Copy into the task's copy" list.
    /// Returns the snapshot id and the id of the project whose list was changed, if the kit was for a project.
    pub fn kits_install(&self, plan: &Value, choices: &Choices) -> KRes<(String, Option<String>)> {
        let id = plan.get("id").and_then(Value::as_str).ok_or_else(|| kerr(KitError::Changed))?;
        let plan = lk(&self.plans).iter().find(|p| p.id == id).cloned().ok_or_else(|| kerr(KitError::Changed))?;
        let snap = self.kits.install(&plan, choices).map_err(kerr)?;
        let mut changed = None;
        if let Scope::Project(dir) = &plan.scope {
            let files = self.kits.snapshots(&plan.env).into_iter().find(|s| s.id == snap).map(|s| s.project_files).unwrap_or_default();
            changed = self.add_copy_files(dir, &files);
        }
        lk(&self.plans).retain(|p| p.id != id);
        Ok((snap, changed))
    }

    /// Adds `files` to `copy_files` of the project in `dir`, without duplicates.
    fn add_copy_files(&self, dir: &std::path::Path, files: &[String]) -> Option<String> {
        let real = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
        let mut st = lk(self.engine.store());
        let mut p = st.projects().into_iter().find(|p| std::fs::canonicalize(&p.path).unwrap_or_else(|_| p.path.clone()) == real)?;
        let before = p.copy_files.len();
        for f in files {
            if !p.copy_files.contains(f) {
                p.copy_files.push(f.clone());
            }
        }
        let id = p.id.clone();
        if p.copy_files.len() != before {
            let _ = st.upsert_project(p);
        }
        Some(id)
    }

    pub fn kits_enable(&self, env: &str, item: &HeldRef) -> KRes<()> {
        self.kits.enable_item(env, item).map_err(kerr)
    }

    pub fn kits_snapshots(&self, env: &str) -> KRes<Value> {
        ok(serde_json::to_value(self.kits.snapshots(env)))
    }

    pub fn kits_rollback(&self, id: &str) -> KRes<Value> {
        ok(serde_json::to_value(self.kits.rollback(id).map_err(kerr)?))
    }

    /// At start: installs that were cut short are rolled back; the window gets a line for each.
    pub fn kits_recover(&self) -> Vec<Recovered> {
        self.kits.recover()
    }

    /// The answer is where the value went (`keyring`, or a file with a warning), never the value.
    pub fn set_secret(&self, env: &str, name: &str, value: &str) -> KRes<Value> {
        ok(serde_json::to_value(self.kits.set_secret(env, name, value).map_err(kerr)?))
    }
}

#[cfg(test)]
mod tests {
    use crate::testkit::*;
    use nightshift_core::agent::{FakeAdapter, FakeStep};
    use nightshift_core::kits::{Choices, ExportDest, KitSource, Scope, Selection};
    use nightshift_core::model::{RoleKind, Status};
    use serde_json::json;

    const SECRET: &str = "tok-9f8e7d6c5b4a-never-in-a-file";

    fn kit_rig() -> Rig {
        let r = Rig::new(FakeAdapter::new(vec![]));
        let skill = r.s.0.join("home/.claude/skills/notes/SKILL.md");
        std::fs::create_dir_all(skill.parent().unwrap()).unwrap();
        std::fs::write(skill, "---\nname: notes\n---\nKeep notes short.\n").unwrap();
        r
    }

    #[test]
    fn a_kit_for_the_project_only_adds_its_files_to_the_copy_list_and_the_window_cannot_forge_the_plan() {
        let r = kit_rig();
        let sel = Selection { name: "mine".into(), groups: vec![nightshift_core::kits::Group::Skills], scope: Scope::Global };
        let out = r.s.0.join("kit-out");
        r.hub.kits_export("claude", &sel, ExportDest::Folder(out.clone())).unwrap();

        let plan = r.hub.kits_preview("claude", KitSource::Folder(out), &Scope::Project(r.repo.clone())).unwrap();
        let wanted = plan["project_files"].as_array().unwrap().iter().map(|f| f.as_str().unwrap().to_string()).collect::<Vec<_>>();
        assert!(wanted.iter().any(|f| f.ends_with("skills/notes/SKILL.md")), "{wanted:?}");

        // a plan the app never showed is refused, whatever the window says about it
        assert_eq!(r.hub.kits_install(&json!({"id": "forged"}), &Choices::default()).unwrap_err()["kind"], "changed");
        let (_, changed) = r.hub.kits_install(&plan, &Choices::default()).unwrap();
        assert_eq!(changed.as_deref(), Some(r.project.id.as_str()), "the app learns which project changed from the core, not from the window's plan");
        let copy = r.hub.project(&r.project.id).unwrap().copy_files;
        for f in &wanted {
            assert!(copy.contains(f), "{f} missing from {copy:?}");
        }
        assert!(copy.contains(&".env*".to_string()), "what was there stays");
        assert!(r.repo.join(&wanted[0]).is_file(), "the file is in the project");
        // the preview is used up
        assert_eq!(r.hub.kits_install(&plan, &Choices::default()).unwrap_err()["kind"], "changed");
    }

    #[test]
    fn a_secret_goes_to_the_keyring_only_and_reaches_the_runs_of_its_env_as_a_variable() {
        let fake = FakeAdapter::new(vec![])
            .with_id("custom:f")
            .with_role(RoleKind::Worker, vec![vec![FakeStep::Write("f.txt".into(), "f".into()), FakeStep::Done("f".into())]])
            .with_role(RoleKind::Reviewer, vec![vec![FakeStep::Done(r#"{"verdict":"approve","issues":[]}"#.into())]]);
        let r = Rig::new(fake);
        let answer = r.hub.set_secret("custom:f", "FAKE_TOKEN", SECRET).unwrap();
        assert_eq!(answer, json!({ "kind": "keyring" }), "the answer says where it went, not what it is");
        assert_eq!(r.hub.set_secret("custom:f", "bad name", SECRET).unwrap_err()["kind"], "bad_name");
        assert_eq!(r.hub.set_secret("custom:f", "HOME", "/home/x").unwrap_err(), json!({"kind": "bad_name", "detail": "HOME"}));
        assert_eq!(r.hub.set_secret("custom:f", "LANG", "C;x").unwrap_err(), json!({"kind": "bad_value", "detail": "LANG"}));
        assert_eq!(r.hub.set_secret("nope", "X", SECRET).unwrap_err()["kind"], "unknown_env");

        let id = r.hub.submit_task(&r.project.id, "Add f", None).unwrap();
        r.wait_task(&id, "done", |t| t.status == Status::Done);
        let env: Vec<(String, String)> = r.fake.calls().iter().flat_map(|c| c.extra_env.clone()).collect();
        assert!(env.contains(&("FAKE_TOKEN".to_string(), SECRET.to_string())), "the run of the env got the variable");

        // nothing the app writes holds the value: not the state, not the logs, not the chats
        fn walk(d: &std::path::Path, out: &mut String) {
            for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out);
                } else if let Ok(b) = std::fs::read(&p) {
                    out.push_str(&String::from_utf8_lossy(&b));
                }
            }
        }
        let mut all = String::new();
        walk(&r.s.0.join("data"), &mut all);
        assert!(all.contains("FAKE_TOKEN") && !all.contains(SECRET), "only the name is kept on disk");
    }
}
