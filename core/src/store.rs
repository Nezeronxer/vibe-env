use crate::model::{Catalog, Chat, ChatMsg, EnvId, EnvLimits, LogEvent, Project, ProjectId, Settings, Task};
use chrono::{DateTime, Local, NaiveDate};
use std::collections::BTreeMap;
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
    /// state.json is not readable: nothing was replaced, a copy of the file is at `copy`.
    #[error("state file unreadable, copy saved at {copy}")]
    Unreadable { copy: String },
}
pub type Result<T> = std::result::Result<T, StoreError>;

/// What an env spent on one day and what the person added to its daily cap ("Add $5").
/// Kept apart from the tasks: a retried or removed task does not take its spending with it.
#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
pub struct DaySpend {
    pub day: Option<NaiveDate>,
    pub spent_usd: f64,
    pub extra_usd: f64,
}

#[derive(Serialize, Deserialize, Default, Clone)]
struct State {
    #[serde(default)]
    projects: Vec<Project>,
    #[serde(default)]
    tasks: Vec<Task>,
    #[serde(default)]
    chats: Vec<Chat>,
    #[serde(default)]
    settings: Settings,
}

pub struct Store {
    dir: PathBuf,
    state: State,
    /// Copy of an unreadable settings file made by `open` (G16.4).
    recovered: Option<PathBuf>,
}

/// tmp file + rename: a reader sees the old or the new file, never half of one.
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let tmp = path.with_extension("json.tmp");
    let mut file = fs::File::create(&tmp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::rename(&tmp, path)?;
    Ok(())
}

/// Parses `state.json`. Projects, tasks and chats must parse (an error is not hidden); only a
/// settings block that does not parse is copied next to the file and replaced by defaults.
fn load_state(path: &Path) -> Result<(State, Option<PathBuf>)> {
    let bytes = fs::read(path)?;
    let backup = |path: &Path| -> Result<PathBuf> {
        let b = path.with_extension(format!("json.corrupt-{}", Local::now().format("%Y%m%d%H%M%S")));
        fs::copy(path, &b)?;
        Ok(b)
    };
    // A file that is not JSON at all holds projects and tasks we cannot read: never start
    // from an empty state over it (the next write would erase them). The caller gets an error.
    let Ok(mut v) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
        let copy = backup(path)?;
        return Err(StoreError::Unreadable { copy: copy.display().to_string() });
    };
    let settings = v.as_object_mut().and_then(|o| o.remove("settings"));
    let (settings, recovered) = match settings.map(serde_json::from_value::<Settings>) {
        None => (Settings::default(), None),
        Some(Ok(s)) => (s, None),
        Some(Err(_)) => (Settings::default(), Some(backup(path)?)),
    };
    let mut state: State = serde_json::from_value(v)?;
    state.settings = settings;
    Ok((state, recovered))
}

impl Store {
    pub fn open(dir: &Path) -> Result<Store> {
        fs::create_dir_all(dir.join("logs"))?;
        let path = dir.join("state.json");
        let (state, recovered) = if path.exists() { load_state(&path)? } else { (State::default(), None) };
        // A leftover tmp file is an interrupted write; the real file is untouched.
        let _ = fs::remove_file(dir.join("state.json.tmp"));
        Ok(Store { dir: dir.to_path_buf(), state, recovered })
    }

    /// Where `open` saved a copy of an unreadable settings file (defaults are in use); None = nothing was wrong.
    pub fn settings_recovered(&self) -> Option<&Path> {
        self.recovered.as_deref()
    }

    /// Apply `f` to a copy, write it to disk, and only then adopt it: after an Err the
    /// in-memory state still equals what is on disk.
    fn commit(&mut self, f: impl FnOnce(&mut State)) -> Result<()> {
        let mut next = self.state.clone();
        f(&mut next);
        write_atomic(&self.dir.join("state.json"), &serde_json::to_vec_pretty(&next)?)?;
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

    // ---- chats ----

    pub fn chats(&self, env: &str) -> Vec<Chat> {
        self.state.chats.iter().filter(|c| c.env == env).cloned().collect()
    }

    pub fn chat(&self, id: &str) -> Option<Chat> {
        self.state.chats.iter().find(|c| c.id == id).cloned()
    }

    pub fn upsert_chat(&mut self, c: &Chat) -> Result<()> {
        self.commit(|s| match s.chats.iter_mut().find(|x| x.id == c.id) {
            Some(x) => *x = c.clone(),
            None => s.chats.push(c.clone()),
        })
    }

    pub fn remove_chat(&mut self, id: &str) -> Result<()> {
        self.commit(|s| s.chats.retain(|c| c.id != id))?;
        let _ = fs::remove_file(self.chat_path(id));
        Ok(())
    }

    fn chat_path(&self, id: &str) -> PathBuf {
        let safe: String = id.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
        self.dir.join("chats").join(format!("{safe}.jsonl"))
    }

    fn chat_all(&self, id: &str) -> Vec<ChatMsg> {
        let Ok(text) = fs::read_to_string(self.chat_path(id)) else { return vec![] };
        text.lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
    }

    /// Ok(false) = a message with this id is already in the chat (nothing written): the window may
    /// send the same message twice, e.g. after a retry (G20.8).
    pub fn append_chat_msg(&self, m: &ChatMsg) -> Result<bool> {
        if self.chat_all(&m.chat_id).iter().any(|x| x.id == m.id) {
            return Ok(false);
        }
        fs::create_dir_all(self.dir.join("chats"))?;
        let mut f = fs::OpenOptions::new().create(true).read(true).append(true).open(self.chat_path(&m.chat_id))?;
        let mut line = Vec::new();
        // a torn last line (crash mid-append) must not swallow this message
        if f.metadata()?.len() > 0 {
            let mut last = [0u8; 1];
            f.seek(SeekFrom::End(-1))?;
            f.read_exact(&mut last)?;
            if last[0] != b'\n' {
                line.push(b'\n');
            }
        }
        line.extend(serde_json::to_vec(m)?);
        line.push(b'\n');
        f.write_all(&line)?;
        Ok(true)
    }

    /// A page of the chat, oldest first: the last `limit` messages older than `before`
    /// (None = from the end). The next page = call again with the first message's `at`.
    pub fn read_chat(&self, id: &str, before: Option<DateTime<Local>>, limit: usize) -> Vec<ChatMsg> {
        let all: Vec<ChatMsg> = self.chat_all(id).into_iter().filter(|m| before.is_none_or(|b| m.at < b)).collect();
        let skip = all.len().saturating_sub(limit);
        all.into_iter().skip(skip).collect()
    }

    // ---- snapshots of the model catalog and the limit windows (one file each, keyed by env) ----

    fn read_map<T: serde::de::DeserializeOwned>(&self, file: &str) -> BTreeMap<EnvId, T> {
        fs::read(self.dir.join(file)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }

    fn put_map<T: Serialize + serde::de::DeserializeOwned>(&self, file: &str, env: &str, v: T) -> Result<()> {
        let mut all: BTreeMap<EnvId, T> = self.read_map(file);
        all.insert(env.to_string(), v);
        write_atomic(&self.dir.join(file), &serde_json::to_vec_pretty(&all)?)
    }

    pub fn catalog(&self, env: &str) -> Option<Catalog> {
        self.read_map::<Catalog>("catalog.json").remove(env)
    }

    pub fn set_catalog(&self, c: Catalog) -> Result<()> {
        let env = c.env.clone();
        self.put_map("catalog.json", &env, c)
    }

    pub fn limits(&self, env: &str) -> Option<EnvLimits> {
        self.read_map::<EnvLimits>("limits.json").remove(env)
    }

    pub fn set_limits(&self, env: &str, l: EnvLimits) -> Result<()> {
        self.put_map("limits.json", env, l)
    }

    /// The env's spending on `day`; another day's record counts as nothing.
    pub fn day_spend(&self, env: &str, day: NaiveDate) -> DaySpend {
        self.read_map::<DaySpend>("day_spend.json")
            .remove(env)
            .filter(|d| d.day == Some(day))
            .unwrap_or(DaySpend { day: Some(day), ..DaySpend::default() })
    }

    pub fn add_day_spend(&self, env: &str, day: NaiveDate, spent_usd: f64, extra_usd: f64) -> Result<()> {
        let mut d = self.day_spend(env, day);
        d.spent_usd += spent_usd;
        d.extra_usd += extra_usd;
        self.put_map("day_spend.json", env, d)
    }
}
