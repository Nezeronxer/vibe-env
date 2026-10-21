use crate::model::{LogEvent, Project, ProjectId, Settings, Task};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("not found: {0}")]
    NotFound(String),
}
pub type Result<T> = std::result::Result<T, StoreError>;

#[derive(Serialize, Deserialize, Default, Clone)]
struct State {
    #[serde(default)]
    projects: Vec<Project>,
    #[serde(default)]
    tasks: Vec<Task>,
    #[serde(default)]
    settings: Settings,
}

pub struct Store {
    dir: PathBuf,
    state: State,
}

impl Store {
    pub fn open(dir: &Path) -> Result<Store> {
        fs::create_dir_all(dir.join("logs"))?;
        let path = dir.join("state.json");
        let state = if path.exists() {
            serde_json::from_slice(&fs::read(&path)?)?
        } else {
            State::default()
        };
        // A leftover tmp file is an interrupted write; the real file is untouched.
        let _ = fs::remove_file(dir.join("state.json.tmp"));
        Ok(Store { dir: dir.to_path_buf(), state })
    }

    /// Apply `f` to a copy, write it to disk, and only then adopt it: after an Err the
    /// in-memory state still equals what is on disk.
    fn commit(&mut self, f: impl FnOnce(&mut State)) -> Result<()> {
        let mut next = self.state.clone();
        f(&mut next);
        let tmp = self.dir.join("state.json.tmp");
        let mut file = fs::File::create(&tmp)?;
        file.write_all(&serde_json::to_vec_pretty(&next)?)?;
        file.sync_all()?;
        fs::rename(&tmp, self.dir.join("state.json"))?;
        self.state = next;
        Ok(())
    }

    pub fn projects(&self) -> Vec<Project> {
        self.state.projects.clone()
    }

    pub fn upsert_project(&mut self, p: Project) -> Result<()> {
        self.commit(|s| match s.projects.iter_mut().find(|x| x.id == p.id) {
            Some(x) => *x = p,
            None => s.projects.push(p),
        })
    }

    pub fn remove_project(&mut self, id: &str) -> Result<()> {
        self.commit(|s| s.projects.retain(|p| p.id != id))
    }

    pub fn tasks(&self) -> Vec<Task> {
        self.state.tasks.clone()
    }

    pub fn task(&self, id: &str) -> Option<Task> {
        self.state.tasks.iter().find(|t| t.id == id).cloned()
    }

    pub fn upsert_task(&mut self, t: &Task) -> Result<()> {
        self.commit(|s| match s.tasks.iter_mut().find(|x| x.id == t.id) {
            Some(x) => *x = t.clone(),
            None => s.tasks.push(t.clone()),
        })
    }

    pub fn remove_task(&mut self, id: &str) -> Result<()> {
        self.commit(|s| s.tasks.retain(|t| t.id != id))?;
        let _ = fs::remove_file(self.log_path(id));
        Ok(())
    }

    fn log_path(&self, id: &str) -> PathBuf {
        // ids are uuids; strip anything path-like defensively
        let safe: String = id.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
        self.dir.join("logs").join(format!("{safe}.jsonl"))
    }

    pub fn append_log(&self, id: &str, ev: &LogEvent) -> Result<()> {
        let mut f = fs::OpenOptions::new().create(true).read(true).append(true).open(self.log_path(id))?;
        let mut line = Vec::new();
        // a crash can leave a torn last line; start on a fresh line so it cannot swallow this event
        if f.metadata()?.len() > 0 {
            let mut last = [0u8; 1];
            f.seek(SeekFrom::End(-1))?;
            f.read_exact(&mut last)?;
            if last[0] != b'\n' {
                line.push(b'\n');
            }
        }
        line.extend(serde_json::to_vec(ev)?);
        line.push(b'\n');
        f.write_all(&line)?;
        Ok(())
    }

    /// Last `limit` events, oldest first. A torn last line (crash mid-append) is skipped.
    pub fn read_log(&self, id: &str, limit: usize) -> Vec<LogEvent> {
        let Ok(text) = fs::read_to_string(self.log_path(id)) else { return vec![] };
        let all: Vec<LogEvent> = text.lines().filter_map(|l| serde_json::from_str(l).ok()).collect();
        let skip = all.len().saturating_sub(limit);
        all.into_iter().skip(skip).collect()
    }

    pub fn settings(&self) -> Settings {
        self.state.settings.clone()
    }

    pub fn set_settings(&mut self, s: Settings) -> Result<()> {
        self.commit(|st| st.settings = s)
    }

    pub fn project(&self, id: &ProjectId) -> Option<Project> {
        self.state.projects.iter().find(|p| &p.id == id).cloned()
    }
}
