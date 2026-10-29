//! Projects, GitHub and the diff of one file: the commands that look at git (R11.10, G01.6, G01.7).
use crate::hub::{e2s, lk, Hub, Res};
use nightshift_core::git;
use nightshift_core::model::{Project, Status, Task};
use serde_json::{json, Value};
use std::path::Path;

/// Longest diff of one file shown, in lines.
const DIFF_LINES: usize = 2000;

/// A unified diff as the rows the window draws: `hunk`, `add`, `del`, `ctx`. The file header lines are dropped.
pub fn diff_lines(text: &str) -> Vec<Value> {
    let mut in_hunk = false;
    let mut out = vec![];
    for l in text.lines() {
        let kind = if l.starts_with("@@") {
            in_hunk = true;
            "hunk"
        } else if !in_hunk {
            continue;
        } else if l.starts_with('+') {
            "add"
        } else if l.starts_with('-') {
            "del"
        } else {
            "ctx"
        };
        out.push(json!({ "kind": kind, "text": l }));
    }
    out
}

fn safe_rel(p: &str) -> bool {
    !p.is_empty() && !p.contains('\0') && !Path::new(p).is_absolute() && !Path::new(p).components().any(|c| matches!(c, std::path::Component::ParentDir))
}

impl Hub {
    /// "Добавить проект": a folder that is a git repository, once (the same folder through a link or `..` is the same project).
    pub fn add_project(&self, path: &str) -> Res<Project> {
        let p = std::fs::canonicalize(path).map_err(|e| format!("{path}: {e}"))?;
        let info = git::repo_info(&p);
        if !info.is_repo {
            return Err("not a git repo".into());
        }
        let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "project".into());
        let mut proj = Project::new(&name, p.clone(), info.base_branch.as_deref().unwrap_or("main"));
        let (check, setup) = git::detect_commands(&p);
        proj.check_cmd = check;
        proj.setup_cmd = setup;
        let mut st = lk(self.engine.store());
        if st.projects().iter().any(|x| std::fs::canonicalize(&x.path).unwrap_or_else(|_| x.path.clone()) == p) {
            return Err("already added".to_string());
        }
        st.upsert_project(proj.clone()).map_err(e2s)?;
        Ok(proj)
    }

    /// "Создать проект": a new empty folder `name` inside `parent`, `git init` and one empty commit (worktrees need a commit), then added.
    /// Never overwrites: an existing folder is refused. Errors are short keys the window translates.
    pub fn create_project(&self, parent: &str, name: &str) -> Res<Project> {
        let name = name.trim();
        if name.is_empty() || name == "." || name == ".." || name.contains(['/', '\\', '\0']) {
            return Err("bad_project_name".into());
        }
        let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
        let parent = parent_folder(parent, home.as_deref())?;
        let dir = parent.join(name);
        // a dangling link counts as existing too
        if dir.symlink_metadata().is_ok() {
            return Err(format!("folder_exists: {}", dir.display()));
        }
        std::fs::create_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        git::init_repo(&dir).map_err(e2s)?;
        #[cfg(not(test))] // tests make projects too: they must not touch the real ~/.claude.json
        if let Some(h) = std::env::var_os("HOME") {
            crate::claudetrust::created(Path::new(&h), &dir);   // the human just made this empty folder: Claude need not ask
        }
        self.add_project(&dir.to_string_lossy())
    }

    /// "Убрать проект": refused while the project has tasks that are not finished.
    /// v13 "С GitHub…": `owner/name` cloned by gh into `<parent>/<name>` (never over an existing folder), then added.
    pub fn clone_project(&self, parent: &str, full: &str) -> Res<Project> {
        let full = git::gh_full_name(full).ok_or("bad_repo_name")?;
        let name = full.split('/').nth(1).unwrap_or_default();
        let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
        let dir = parent_folder(parent, home.as_deref())?.join(name);
        if dir.symlink_metadata().is_ok() {
            return Err(format!("folder_exists: {}", dir.display()));
        }
        git::gh_clone(&full, &dir).map_err(e2s)?;
        self.add_project(&dir.to_string_lossy())
    }

    pub fn remove_project(&self, id: &str) -> Res<()> {
        let mut st = lk(self.engine.store());
        let open = |t: &Task| !matches!(t.status, Status::Done | Status::DoneNotPushed | Status::Failed | Status::Cancelled | Status::Reverted);
        if st.tasks().iter().any(|t| t.project_id == id && open(t)) {
            return Err("tasks_running".into());
        }
        st.remove_project(id).map_err(e2s)
    }

    pub fn delete_merged_branches(&self, project_id: &str) -> Res<u32> {
        git::delete_merged_ns_branches(&self.project_ready(project_id)?.path).map_err(e2s)
    }

    pub fn repo_info(&self, id: &str) -> Res<Value> {
        let p = self.project_ready(id)?;
        let i = git::repo_info(&p.path);
        Ok(json!({ "is_repo": i.is_repo, "base_branch": i.base_branch, "current_branch": i.current_branch, "dirty": i.dirty,
            "origin_url": i.origin_url, "web_url": i.web_url, "ahead": i.ahead, "behind": i.behind }))
    }

    /// Saves a project. A new folder ("Указать заново") must be a git repository, and when the old folder is still
    /// there, the very same one (same first commit); a moved folder is accepted without that proof, it is gone.
    pub fn update_project(&self, mut project: Project) -> Res<()> {
        let old = self.project(&project.id)?;
        if project.path != old.path {
            let new = std::fs::canonicalize(&project.path).map_err(|_| format!("path_missing: {}", project.path.display()))?;
            if !git::repo_info(&new).is_repo {
                return Err("not a git repo".into());
            }
            if old.path.is_dir() && git::root_commits(&old.path) != git::root_commits(&new) {
                return Err(format!("not_same_repo: {}", new.display()));
            }
            project.path = new;
        }
        lk(self.engine.store()).upsert_project(project).map_err(e2s)
    }

    pub fn github_info(&self, project_id: &str) -> Res<Value> {
        let p = self.project_ready(project_id)?;
        let mut out = match git::gh_status(&p.path) {
            Ok(u) => json!({ "ghUser": u }),
            Err(e) => json!({ "ghHint": e.to_string() }),
        };
        out["repoUrl"] = json!(git::repo_info(&p.path).web_url);
        out["branches"] = json!(git::list_ns_branches(&p.path).into_iter().map(|(name, merged)| json!({ "name": name, "merged": merged })).collect::<Vec<_>>());
        // the pull requests of the project's tasks; the state of one comes from `pr_state`
        let tasks = lk(self.engine.store()).tasks();
        out["prs"] = json!(tasks.iter().filter(|t| t.project_id == project_id).filter_map(|t| Some(json!({ "task_id": t.id, "branch": t.branch, "url": t.outcome.pr_url.as_ref()?, "title": t.title }))).collect::<Vec<_>>());
        Ok(out)
    }

    pub fn pr_state(&self, task_id: &str) -> Res<String> {
        let t = self.task_of(task_id)?;
        let url = t.outcome.pr_url.clone().ok_or("no PR for this task")?;
        git::pr_state(&self.project_ready(&t.project_id)?.path, &url).map_err(e2s)
    }

    pub fn task_of(&self, id: &str) -> Res<Task> {
        lk(self.engine.store()).task(id).ok_or_else(|| "task not found".to_string())
    }

    pub fn get_diffstat(&self, task_id: &str) -> Res<Vec<Value>> {
        let t = self.task_of(task_id)?;
        let p = self.project_ready(&t.project_id)?;
        if let Some(wt) = t.worktree.as_ref().filter(|w| w.exists()) {
            return Ok(git::diffstat(wt, &p.base_branch).into_iter().map(|f| json!({ "path": f.path, "insertions": f.insertions, "deletions": f.deletions })).collect());
        }
        // merged task: its copy is gone, the squash commit on base has the changes
        let Some(sha) = t.outcome.commit.as_deref() else { return Ok(vec![]) };
        Ok(git::commit_stat(&p.path, sha).into_iter().map(|f| json!({ "path": f.path, "insertions": f.insertions, "deletions": f.deletions })).collect())
    }

    /// The diff of one file of a task, or of one swarm variant's copy: at most 2000 lines, a flag for a binary file.
    pub fn diff_file(&self, task_id: &str, path: &str, variant: Option<&str>) -> Res<Value> {
        if !safe_rel(path) {
            return Err(format!("bad_path: {path}"));
        }
        let t = self.task_of(task_id)?;
        let p = self.project_ready(&t.project_id)?;
        let wt = match variant {
            Some(v) => Some(self.engine.swarm_copy(task_id, v).ok_or("variant_not_found")?),
            None => t.worktree.clone().filter(|w| w.exists()),
        };
        let d = match wt {
            Some(wt) => git::diff_file(&wt, &p.base_branch, path, DIFF_LINES),
            None => git::commit_file_diff(&p.path, t.outcome.commit.as_deref().filter(|c| !c.is_empty()).ok_or("no_copy")?, path, DIFF_LINES),
        };
        Ok(json!({ "lines": diff_lines(&d.text), "binary": d.binary, "size_kb": d.size.div_ceil(1024), "truncated": d.truncated }))
    }
}

/// The folder a new project goes into. The window offers "~/Projects" before any choice: `~` and `~/…` are the home,
/// and a missing folder there is made - only inside the home, after links are resolved. Anything else must be an
/// absolute folder that exists (`~user`, a relative path or a missing folder elsewhere is refused, never made).
fn parent_folder(parent: &str, home: Option<&Path>) -> Res<std::path::PathBuf> {
    let missing = || format!("path_missing: {parent}");
    let under_home = parent == "~" || parent.starts_with("~/");
    let wanted = match (under_home, home) {
        (true, Some(h)) if h.is_absolute() => h.join(parent.trim_start_matches('~').trim_start_matches('/')),
        (true, _) => return Err(missing()),
        (false, _) if Path::new(parent).is_absolute() => std::path::PathBuf::from(parent),
        _ => return Err(missing()),
    };
    if under_home && !wanted.exists() {
        let h = home.and_then(|h| std::fs::canonicalize(h).ok()).ok_or_else(missing)?;
        if wanted.components().any(|c| c == std::path::Component::ParentDir) {
            return Err(missing());
        }
        // the deepest folder that already exists must resolve inside the home: a link there out of it makes nothing
        let base = wanted.ancestors().find(|a| a.exists()).and_then(|a| std::fs::canonicalize(a).ok()).ok_or_else(missing)?;
        if !base.starts_with(&h) {
            return Err(missing());
        }
        std::fs::create_dir_all(&wanted).map_err(|e| format!("{}: {e}", wanted.display()))?;
    }
    let real = std::fs::canonicalize(&wanted).map_err(|_| missing())?;
    if under_home && !home.and_then(|h| std::fs::canonicalize(h).ok()).is_some_and(|h| real.starts_with(h)) {
        return Err(missing());
    }
    Ok(real)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testkit::*;
    use nightshift_core::agent::{FakeAdapter, FakeStep};
    use nightshift_core::model::{RoleKind, Status};

    #[test]
    fn a_unified_diff_becomes_rows_without_the_file_header() {
        let d = "diff --git a/a.txt b/a.txt\nindex 1..2 100644\n--- a/a.txt\n+++ b/a.txt\n@@ -1,2 +1,2 @@\n one\n-two\n+three\n";
        let kinds: Vec<(String, String)> = diff_lines(d).iter().map(|r| (r["kind"].as_str().unwrap().into(), r["text"].as_str().unwrap().into())).collect();
        assert_eq!(kinds, [("hunk", "@@ -1,2 +1,2 @@"), ("ctx", " one"), ("del", "-two"), ("add", "+three")].map(|(a, b)| (a.to_string(), b.to_string())));
    }

    #[test]
    fn the_diff_of_one_file_comes_from_the_copy_while_it_exists_and_from_the_commit_after_the_merge_and_a_bad_path_is_refused() {
        let fake = FakeAdapter::new(vec![])
            .with_role(RoleKind::Worker, vec![vec![FakeStep::Write("a.txt".into(), "one\ntwo\n".into()), FakeStep::Done("x".into())]])
            .with_role(RoleKind::Reviewer, vec![vec![FakeStep::Done(r#"{"verdict":"approve","issues":[]}"#.into())]]);
        let r = Rig::new(fake);
        let id = r.hub.submit_task(&r.project.id, "Add two", None).unwrap();
        r.wait_task(&id, "done", |t| t.status == Status::Done);
        let d = r.hub.diff_file(&id, "a.txt", None).unwrap();
        let rows: Vec<String> = d["lines"].as_array().unwrap().iter().map(|x| x["kind"].as_str().unwrap().to_string()).collect();
        assert_eq!(rows, ["hunk", "ctx", "add"], "{d}");
        assert_eq!(d["binary"], false);
        for bad in ["../x", "/etc/passwd", "", "a\0b"] {
            assert!(r.hub.diff_file(&id, bad, None).unwrap_err().starts_with("bad_path"), "{bad:?}");
        }
        assert_eq!(r.hub.diff_file(&id, "a.txt", Some("A")).unwrap_err(), "variant_not_found");
        let stat = r.hub.get_diffstat(&id).unwrap();
        assert_eq!(stat, vec![json!({"path": "a.txt", "insertions": 1, "deletions": 0})]);
    }

    #[test]
    fn a_moved_folder_is_reported_and_blocks_new_work_and_the_new_place_of_the_same_repository_takes_it_back() {
        let r = Rig::new(FakeAdapter::new(vec![]));
        let moved = r.s.0.join("moved");
        std::fs::rename(&r.repo, &moved).unwrap();
        assert_eq!(r.hub.repo_info(&r.project.id).unwrap_err(), format!("folder_not_found: {}", r.repo.display()));
        assert!(r.hub.submit_task(&r.project.id, "x", None).unwrap_err().starts_with("folder_not_found"));

        // a folder that is not a repository is refused, the project stays where it was
        let plain = r.s.0.join("plain");
        std::fs::create_dir(&plain).unwrap();
        let mut p = r.project.clone();
        p.path = plain;
        assert_eq!(r.hub.update_project(p.clone()).unwrap_err(), "not a git repo");
        assert!(r.hub.project(&r.project.id).unwrap().path == r.repo);

        // the same repository at its new place
        p.path = moved.clone();
        r.hub.update_project(p).unwrap();
        assert_eq!(r.hub.project(&r.project.id).unwrap().path, moved.canonicalize().unwrap());
        assert_eq!(r.hub.repo_info(&r.project.id).unwrap()["is_repo"], true);
        r.hub.submit_task(&r.project.id, "x", None).unwrap();
    }

    #[test]
    fn another_repository_cannot_take_the_place_of_a_project_whose_folder_is_still_there() {
        let r = Rig::new(FakeAdapter::new(vec![]));
        let other = r.s.0.join("other");
        std::fs::create_dir(&other).unwrap();
        g(&other, &["init", "-q", "-b", "main"]);
        g(&other, &["config", "user.email", "t@example.com"]);
        g(&other, &["config", "user.name", "T"]);
        std::fs::write(other.join("z.txt"), "z").unwrap();
        g(&other, &["add", "-A"]);
        g(&other, &["commit", "-q", "-m", "first"]);
        let mut p = r.project.clone();
        p.path = other.clone();
        assert!(r.hub.update_project(p).unwrap_err().starts_with("not_same_repo"));
        assert_eq!(r.hub.project(&r.project.id).unwrap().path, r.repo);
    }

    #[test]
    fn a_github_repository_is_cloned_by_gh_into_the_parent_and_becomes_a_project_once() {
        let calls = fake_gh();
        let r = Rig::new(FakeAdapter::new(vec![]));
        let parent = r.s.0.to_string_lossy().to_string();
        let p = r.hub.clone_project(&parent, "https://github.com/o/demo.git").unwrap();
        assert_eq!(p.name, "demo");
        assert!(p.path.join(".git").is_dir() && r.hub.project(&p.id).is_ok());
        assert!(std::fs::read_to_string(&calls).unwrap().contains("repo clone o/demo"));
        assert!(r.hub.clone_project(&parent, "o/demo").unwrap_err().starts_with("folder_exists"));
        assert_eq!(r.hub.clone_project(&parent, "o/--upload-pack=x").unwrap_err(), "bad_repo_name");
        assert!(r.hub.clone_project(&format!("{parent}/nope"), "o/x").unwrap_err().starts_with("path_missing"));
    }

    #[test]
    fn a_new_project_is_a_folder_with_git_and_one_commit_and_bad_names_and_existing_folders_are_refused() {
        let r = Rig::new(FakeAdapter::new(vec![]));
        let parent = r.s.0.to_string_lossy().to_string();
        let p = r.hub.create_project(&parent, " fresh ").unwrap();
        assert_eq!(p.name, "fresh");
        assert!(p.path.join(".git").is_dir());
        assert_eq!(git::repo_info(&p.path).base_branch.as_deref(), Some("main"));
        let log = nightshift_core::proc::cmd("git").args(["log", "--oneline"]).current_dir(&p.path).output().unwrap();
        assert_eq!(String::from_utf8_lossy(&log.stdout).lines().count(), 1);
        assert!(r.hub.project(&p.id).is_ok());
        for bad in ["", "  ", "..", ".", "a/b", "../x", "a\\b"] {
            assert_eq!(r.hub.create_project(&parent, bad).unwrap_err(), "bad_project_name", "{bad:?}");
        }
        assert!(r.hub.create_project(&parent, "fresh").unwrap_err().starts_with("folder_exists"));
        assert!(r.hub.create_project(&format!("{parent}/nope"), "x").unwrap_err().starts_with("path_missing"));
        assert!(!r.s.0.join("x").exists());
        // "~/…" is the home (a scratch one here: the real home is never touched); a missing folder there is made
        let home = r.s.0.join("home");
        std::fs::create_dir_all(&home).unwrap();
        let made = parent_folder("~/Projects/deep", Some(&home)).unwrap();
        assert!(made.ends_with("Projects/deep") && made.is_dir());
        assert_eq!(parent_folder("~", Some(&home)).unwrap(), home.canonicalize().unwrap());
        for bad in ["~user/x", "rel/x", "~/../outside", "~/nope/../../x"] {
            assert!(parent_folder(bad, Some(&home)).unwrap_err().starts_with("path_missing"), "{bad}");
        }
        assert!(parent_folder("~/x", None).is_err(), "no home, nothing made");
        // a link inside the home that leads out of it: nothing is made behind it
        let out = r.s.0.join("outside");
        std::fs::create_dir_all(&out).unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&out, home.join("link")).unwrap();
            assert!(parent_folder("~/link/new", Some(&home)).is_err());
            assert!(!out.join("new").exists());
        }
        assert!(parent_folder(&format!("{}/missing", r.s.0.display()), Some(&home)).is_err(), "a missing absolute folder is not made");
        assert!(!r.s.0.join("missing").exists());
    }

    #[test]
    fn a_folder_is_added_once_even_through_a_link_and_a_project_with_unfinished_tasks_is_not_removed() {
        let r = Rig::new(FakeAdapter::new(vec![]));
        let link = r.s.0.join("link-to-repo");
        std::os::unix::fs::symlink(&r.repo, &link).unwrap();
        assert_eq!(r.hub.add_project(&link.to_string_lossy()).unwrap_err(), "already added");
        assert_eq!(r.hub.add_project(&r.s.0.to_string_lossy()).unwrap_err(), "not a git repo");
        let other = r.s.0.join("other");
        std::fs::create_dir(&other).unwrap();
        g(&other, &["init", "-q", "-b", "main"]);
        g(&other, &["config", "user.email", "t@example.com"]);
        g(&other, &["config", "user.name", "T"]);
        std::fs::write(other.join("z"), "z").unwrap();
        g(&other, &["add", "-A"]);
        g(&other, &["commit", "-q", "-m", "first"]);
        let added = r.hub.add_project(&other.to_string_lossy()).unwrap();
        assert_eq!(r.hub.list_tasks().len(), 0);

        r.hub.freeze(true);
        let id = r.hub.submit_task(&r.project.id, "Add f", None).unwrap();
        assert_eq!(r.hub.remove_project(&r.project.id).unwrap_err(), "tasks_running");
        r.hub.remove_project(&added.id).unwrap();
        r.hub.engine.cancel(&id).unwrap();
        r.hub.remove_project(&r.project.id).unwrap();
        assert!(r.hub.project(&r.project.id).is_err());
    }
}
