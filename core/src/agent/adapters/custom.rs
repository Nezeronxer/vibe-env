//! "Own command" from Settings.custom_agents: a shell template with {prompt} {cwd} {session};
//! plain-text output, exit 0 = ok. No value is ever pasted into the shell text: each placeholder
//! becomes `"$1"` / `"$2"` / `"$3"` and the values travel as positional parameters of `sh -c`.
//! The template is scanned with shell quote state, so a placeholder bare, inside "..." or inside '...'
//! always arrives as one literal word (no splitting, no globbing, no expansion).
//! No sandbox and no push ban here -- the settings screen warns about it.
use super::common::*;
use crate::agent::{Adapter, AgentEvent, Run};
use crate::model::CustomAgent;
use regex::Regex;
use std::process::Command;

pub struct Custom(pub CustomAgent);

fn script(template: &str) -> String {
    let re = Regex::new(r"\{(prompt|cwd|session)\}").unwrap();
    let (mut out, mut last) = (String::new(), 0);
    let (mut dq, mut sq, mut esc) = (false, false, false);
    let mut pos = 0;
    // walk the template tracking shell quote state, so a value is always one literal word:
    // bare -> "$N"; inside "..." -> $N (already quoted); inside '...' -> close, "$N", reopen
    for (i, ch) in template.char_indices() {
        if i < pos {
            continue;
        }
        if let Some(m) = re.find(&template[i..]).filter(|m| m.start() == 0) {
            let n = match m.as_str() {
                "{prompt}" => 1,
                "{cwd}" => 2,
                _ => 3,
            };
            out.push_str(&template[last..i]);
            out.push_str(&if sq { format!("'\"${n}\"'") } else if dq { format!("${n}") } else { format!("\"${n}\"") });
            pos = i + m.end();
            last = pos;
            continue;
        }
        if esc {
            esc = false;
        } else if ch == '\\' && !sq {
            esc = true;
        } else if ch == '\'' && !dq {
            sq = !sq;
        } else if ch == '"' && !sq {
            dq = !dq;
        }
    }
    out.push_str(&template[last..]);
    out
}

impl Adapter for Custom {
    fn id(&self) -> &str {
        &self.0.id
    }
    fn name(&self) -> &str {
        &self.0.name
    }
    fn available(&self) -> Result<(), String> {
        Ok(())
    }
    fn command(&self, run: &Run) -> Command {
        let template = match (&run.resume, &self.0.resume) {
            (Some(_), Some(r)) => r,
            _ => &self.0.run,
        };
        let mut c = Command::new("sh");
        c.current_dir(&run.cwd)
            .arg("-c")
            .arg(script(template))
            .arg("sh")
            .arg(with_system(&run.system, &run.prompt))
            .arg(&run.cwd)
            .arg(run.resume.as_deref().unwrap_or(""));
        c
    }
    fn parse(&self, line: &str) -> Vec<AgentEvent> {
        vec![AgentEvent::Text(line.to_string())]
    }
    fn finish(&self, exit_ok: bool, all_text: &str) -> Vec<AgentEvent> {
        if exit_ok {
            return vec![AgentEvent::Done { session: None, text: all_text.trim().to_string(), is_error: false }];
        }
        // a limit message is looked for only in the last lines (where the failure is), not in the whole output
        let lines: Vec<&str> = all_text.lines().filter(|l| !l.trim().is_empty()).collect();
        let last = lines[lines.len().saturating_sub(5)..].join("\n");
        vec![match failure_event(&last) {
            AgentEvent::Done { .. } => AgentEvent::Done {
                session: None,
                text: if last.is_empty() { "command failed".into() } else { last },
                is_error: true,
            },
            limit => limit,
        }]
    }
}
