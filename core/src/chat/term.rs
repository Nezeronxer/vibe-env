//! v6 (G83, G85): the chat in the real CLI and back. "В терминале" gives the words that continue the chat's session
//! in its copy of the project (`Adapter::resume_argv`, checked id, a list of words - no shell); the chat is marked
//! open in the terminal until the terminal ends. Back in the chat, a Claude Code session is read from its jsonl
//! (`~/.claude/projects/<key of the copy>/<session>.jsonl`, only what was said after the terminal opened) into the
//! feed, and the chat goes on with the same session; any other env gets the line "the talk went on in the terminal".
use super::*;
use crate::sessions;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

/// Feed messages one return from the terminal adds at most.
const READ_BACK_MAX: usize = 500;
/// A line of a session file longer than this is skipped (a tool's output, a picture).
const LINE_MAX: u64 = 1024 * 1024;

/// What the app opens for "В терминале".
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TermLaunch {
    pub env: EnvId,
    /// The chat's copy of the project.
    pub cwd: PathBuf,
    /// The program and its words; empty = the person's shell (an env with no CLI of its own, an own command).
    pub argv: Vec<String>,
    /// The words continue the chat's session (false = a new conversation).
    pub resumed: bool,
    /// The form of the words was checked against the live CLI (false = "по документации").
    pub checked: bool,
    pub session: Option<String>,
}

impl Chats {
    /// Where Claude Code keeps its sessions (tests give a temporary folder).
    pub fn set_claude_projects(&self, dir: &Path) {
        *lk(&self.claude_projects) = Some(dir.to_path_buf());
    }

    fn claude_projects(&self) -> Option<PathBuf> {
        lk(&self.claude_projects).clone().or_else(|| sessions::claude_dir().map(|d| d.join("projects")))
    }

    /// The words for "В терминале". `fresh` = a new conversation of the env's CLI, chosen by the person. Err (keys):
    /// `chat_not_found`, `project_not_found`, `agent_not_found: <env>`, `env_unavailable: <hint>` (no CLI),
    /// `copy_failed: <why>`, `no_session` (nothing to continue yet), `cannot_resume` (the CLI has no way to),
    /// `session_not_found` (Claude Code: no file of the session for the copy). After an Err the chat is as it was.
    pub fn terminal_open(&self, chat: &str, fresh: bool) -> Result<TermLaunch, String> {
        let c = self.chat(chat).ok_or("chat_not_found")?;
        let p = lk(self.engine.store()).project(&c.project_id).ok_or(reason::PROJECT_NOT_FOUND)?;
        let a = self.engine.adapter(&c.env).ok_or_else(|| format!("{}: {}", reason::AGENT_NOT_FOUND, c.env))?;
        a.available().map_err(|why| format!("{}: {why}", reason::ENV_UNAVAILABLE))?;
        let copy = self.ensure_copy(&c, &p).map_err(|e| format!("{}: {e}", reason::COPY_FAILED))?;
        let (argv, session) = if fresh {
            (a.cli_path().map(|b| vec![b.to_string_lossy().into_owned()]).unwrap_or_default(), None)
        } else {
            let s = c.session.clone().ok_or("no_session")?;
            let argv = a.resume_argv(&s, &copy).ok_or("cannot_resume")?;
            if c.env == "claude" && self.claude_file(&copy, &s).is_none() {
                return Err("session_not_found".into());
            }
            (argv, Some(s))
        };
        if c.terminal.is_none() {
            let mark = ChatTerminal { since: self.now(), session: session.clone() };
            self.update_chat(chat, |x| x.terminal = Some(mark));
            self.line(chat, keys::OPENED_IN_TERMINAL, "", None, None, None);
            self.emit(ChatEvent::Changed(chat.to_string()));
        }
        Ok(TermLaunch { env: c.env, cwd: copy, argv, resumed: session.is_some(), checked: a.resume_checked(), session })
    }

    /// The terminal of the chat ended: the mark goes; Claude Code's talk there comes into the feed (how many messages
    /// is returned) and the chat takes the session the terminal ended with; another env gets one line. Nothing to do
    /// when the chat was not open in a terminal.
    pub fn terminal_closed(&self, chat: &str) -> Result<usize, String> {
        let c = self.chat(chat).ok_or("chat_not_found")?;
        let Some(mark) = c.terminal.clone() else { return Ok(0) };
        self.update_chat(chat, |x| x.terminal = None);
        let n = if c.env == "claude" {
            let n = self.read_back(&c, &mark);
            self.line(chat, keys::BACK_FROM_TERMINAL, &n.to_string(), None, None, None);
            n
        } else {
            self.line(chat, keys::CONTINUED_IN_TERMINAL, "", None, None, None);
            0
        };
        self.emit(ChatEvent::Changed(chat.to_string()));
        Ok(n)
    }

    /// The session file of Claude Code for a run in `copy`, if it is there (a regular file, not a link).
    fn claude_file(&self, copy: &Path, session: &str) -> Option<PathBuf> {
        if !sessions::valid_id(session) {
            return None;
        }
        let f = self.claude_projects()?.join(sessions::key_for(copy)).join(format!("{session}.jsonl"));
        std::fs::symlink_metadata(&f).ok().filter(|m| m.is_file()).map(|_| f)
    }

    /// The session the terminal wrote to: the chat's own when it changed since `since`, else the newest one changed
    /// since (a new conversation, or a CLI that forked it).
    fn terminal_session(&self, copy: &Path, mark: &ChatTerminal) -> Option<(String, PathBuf)> {
        let since = std::time::SystemTime::from(mark.since) - Duration::from_secs(5);
        let dir = self.claude_projects()?.join(sessions::key_for(copy));
        let mut found: Vec<(std::time::SystemTime, String, PathBuf)> = std::fs::read_dir(dir)
            .ok()?
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                let path = e.path();
                let meta = std::fs::symlink_metadata(&path).ok().filter(|m| m.is_file())?;
                let id = path.file_name()?.to_str()?.strip_suffix(".jsonl")?.to_string();
                let t = meta.modified().ok().filter(|t| *t >= since)?;
                sessions::valid_id(&id).then_some((t, id, path))
            })
            .collect();
        found.sort_by(|a, b| b.0.cmp(&a.0));
        let own = mark.session.as_deref().and_then(|s| found.iter().find(|f| f.1 == s));
        own.or(found.first()).map(|f| (f.1.clone(), f.2.clone()))
    }

    fn read_back(&self, c: &Chat, mark: &ChatTerminal) -> usize {
        let copy = self.copy_dir(&c.id);
        let Some((id, file)) = self.terminal_session(&copy, mark) else { return 0 };
        let name = self.engine.adapter(&c.env).map(|a| a.name().to_string()).unwrap_or_else(|| c.env.clone());
        let mut n = 0;
        for (uuid, who, text) in session_talk(&file, mark.since) {
            if n >= READ_BACK_MAX {
                break;
            }
            let model = (who == Who::Model).then(|| format!("{name} · terminal"));
            let m = ChatMsg {
                id: format!("term-{uuid}"),
                chat_id: c.id.clone(),
                at: self.now(),
                who,
                text,
                task_id: None,
                delivered_at: None,
                kind: None,
                tool: None,
                model,
                reply_to: None,
                action: None,
                attachments: vec![],
            };
            if self.push(m).is_some() {
                n += 1;
            }
        }
        if c.session.as_deref() != Some(id.as_str()) {
            self.update_chat(&c.id, |x| x.session = Some(id));
        }
        n
    }
}

/// What the person and the model said in a Claude Code session file at `since` or later: (entry id, who, text). The
/// main conversation only (no subagent's lines, no meta lines, no tool results, no echo of a slash command); a broken
/// or too long line is skipped. The format is Claude Code's own (seen on 2.1.309, not documented).
fn session_talk(file: &Path, since: DateTime<Local>) -> Vec<(String, Who, String)> {
    let Ok(f) = std::fs::File::open(file) else { return vec![] };
    let mut r = BufReader::new(f);
    let mut out = vec![];
    let mut buf = Vec::new();
    for i in 0.. {
        buf.clear();
        match (&mut r).take(LINE_MAX + 1).read_until(b'\n', &mut buf) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        if buf.len() as u64 > LINE_MAX && !buf.ends_with(b"\n") {
            // the rest of a line that is too long
            let mut skip = Vec::new();
            let _ = r.read_until(b'\n', &mut skip);
            continue;
        }
        let Ok(j) = serde_json::from_slice::<serde_json::Value>(&buf) else { continue };
        let who = match j["type"].as_str() {
            Some("user") => Who::Human,
            Some("assistant") => Who::Model,
            _ => continue,
        };
        if j["isSidechain"] == true || j["isMeta"] == true {
            continue;
        }
        let at = j["timestamp"].as_str().and_then(|t| DateTime::parse_from_rfc3339(t).ok()).map(|t| t.with_timezone(&Local));
        if at.is_none_or(|t| t < since) {
            continue;
        }
        let content = &j["message"]["content"];
        let text = match content.as_str() {
            Some(s) => s.to_string(),
            None => content.as_array().into_iter().flatten().filter(|b| b["type"] == "text").filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join("\n"),
        };
        let text = text.trim();
        if text.is_empty() || (who == Who::Human && (text.contains("<command-name>") || text.contains("<local-command"))) {
            continue;
        }
        let uuid = j["uuid"].as_str().filter(|u| u.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')).map(String::from).unwrap_or_else(|| format!("line-{i}"));
        out.push((uuid, who, text.to_string()));
    }
    out
}
