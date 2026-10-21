//! The council (Совет, G11.3): N members, only reading, propose an approach each, then one round
//! where everyone sees the proposals and votes; the app counts the votes, a tie goes to the Reviewer.
//! One executor writes by the winning approach. On the review the same council votes on the work:
//! the majority decides, a tie goes to the ordinary Reviewer. Proposals and votes are kept for the
//! "Обсуждение" (`swarm_view().council`) and are in the feed.
use super::flow::{Flow, Res};
use super::lanes::{Lane, LaneEnd};
use super::state::{Asked, Member, Proposal, VoteRec};
use super::steps::Structured;
use super::*;
use crate::models;
use crate::prompt::{self, Vendor};
use crate::swarm::{self, parse, prompts};

/// What stops the task in the middle of a round of the council.
enum Interrupt {
    Paused(String),
    Asked(String, String, Vec<String>, Option<String>),
}

/// What a member's turn came to.
enum Turn<T> {
    Got(T),
    /// The member gave nothing usable (twice a wrong answer, or its run failed): it sits this turn out.
    Out(String),
    /// A limit, a login or a stop: the task pauses and the turn is made again.
    Paused(String),
    /// The member asked the person: (question, options, its session).
    Asked(String, Vec<String>, Option<String>),
}

const DIFF_FOR_COUNCIL: usize = 40_000;

impl Engine {
    fn council_log(&self, id: &str, member: Option<&str>, text: &str) {
        let mut ev = LogEvent::new(self.now(), LogKind::Review, text);
        ev.role = Some(RoleKind::Council);
        ev.variant = member.map(str::to_string);
        self.log_ev(id, ev);
    }

    /// One turn of a member: a seat, a read-only run, the answer parsed (one repeat when it is not usable).
    /// `session` is carried between the member's turns; `fresh` starts a session of its own instead.
    #[allow(clippy::too_many_arguments)]
    fn member_turn<T>(
        &self,
        id: &str,
        p: &Project,
        m: &Member,
        at_task: bool,
        fresh: bool,
        cancel: &AtomicBool,
        answer: Option<String>,
        build: &dyn Fn(Vendor) -> String,
        schema: String,
        parse_it: &dyn Fn(&str) -> Result<T, String>,
    ) -> Turn<T> {
        let Some(t) = self.task(id) else { return Turn::Out("task_not_found".into()) };
        self.lane_waiting(id, RoleKind::Council, &m.id, &m.model);
        let Some(_seat) = self.take_seat(&t.agent, cancel) else { return Turn::Paused("cancelled".into()) };
        let (mut session, mut pin) = if fresh { (None, None) } else { (m.session.clone(), m.pin.clone()) };
        let mut err = String::new();
        let mut answer = answer;
        for round in 0..2 {
            let a_text = answer.take();
            let ptext = |v: Vendor, _resumed: bool| match (&a_text, round) {
                (Some(a), _) => a.clone(), // the person's answer goes to the session that asked
                (None, 0) => build(v),
                (None, _) => prompt::format_retry(&build(v), &err),
            };
            let lane = Lane {
                task_id: id,
                project: p,
                role: RoleKind::Council,
                label: &m.id,
                cwd: None,
                model: m.model.clone(),
                session: session.clone(),
                pin: pin.clone(),
                prompt: &ptext,
                schema: Some(schema.clone()),
                at_task,
            };
            let end = self.run_lane(lane, cancel);
            if let Some(why) = end.interrupt() {
                return Turn::Paused(why);
            }
            match end {
                LaneEnd::Failed(e) => return Turn::Out(e),
                LaneEnd::Restart => {
                    // silent again after the nudge: a new session, and it uses up one of the member's two tries
                    session = None;
                    pin = None;
                    err = "no output from the agent".into();
                }
                LaneEnd::Done { text, session: s, pin: pn, .. } => {
                    session = s.or(session);
                    pin = Some(pn);
                    if let Some((q, opts)) = prompt::parse_signals(&text).needs_human {
                        // remembered even in a fresh turn: the answer resumes this session
                        let (s2, p2) = (session.clone(), pin.clone());
                        let _ = self.update_state(id, |st| {
                            if let Some(x) = st.swarm.council.members.iter_mut().find(|x| x.id == m.id) {
                                x.session = s2;
                                x.pin = p2;
                            }
                        });
                        return Turn::Asked(q, opts, session);
                    }
                    if !fresh {
                        let (s2, p2) = (session.clone(), pin.clone());
                        let _ = self.update_state(id, |st| {
                            if let Some(x) = st.swarm.council.members.iter_mut().find(|x| x.id == m.id) {
                                x.session = s2;
                                x.pin = p2;
                            }
                        });
                    }
                    match parse_it(&text) {
                        Ok(v) => return Turn::Got(v),
                        Err(e) => err = e,
                    }
                }
                _ => return Turn::Out("unexpected end of the run".into()),
            }
        }
        Turn::Out(format!("wrong format: {err}"))
    }

    /// Runs `turn` for every member side by side (each takes a seat). Results in the order of the members.
    fn all_members<T: Send>(&self, id: &str, members: &[Member], turn: impl Fn(&Member) -> Turn<T> + Sync) -> Vec<(String, Turn<T>)> {
        let _mode = self.lanes_mode(id);
        std::thread::scope(|sc| {
            let hs: Vec<_> = members.iter().map(|m| (m.id.clone(), sc.spawn(|| turn(m)))).collect();
            hs.into_iter().map(|(l, h)| (l, h.join().unwrap_or_else(|_| Turn::Out("internal_error".into())))).collect()
        })
    }

    /// Takes in what the members gave (each `Got` through `got`, the ones with nothing noted as out); what
    /// stops the task (a pause, a question of a member) is returned, the pause first.
    fn settle<T>(&self, id: &str, phase: &str, outs: Vec<(String, Turn<T>)>, mut got: impl FnMut(&str, T) -> Result<(), String>) -> Result<Option<Interrupt>, String> {
        let (mut pause, mut asked) = (None, None);
        for (letter, out) in outs {
            match out {
                Turn::Got(v) => got(&letter, v)?,
                Turn::Out(why) => {
                    self.council_log(id, Some(&letter), &format!("council_member_out: {letter}: {why}"));
                    let key = format!("{phase}:{letter}");
                    self.update_state(id, |s| s.swarm.council.out.push(key))?;
                }
                Turn::Paused(why) => pause = pause.or(Some(Interrupt::Paused(why))),
                Turn::Asked(q, o, sess) => asked = asked.or(Some(Interrupt::Asked(letter, q, o, sess))),
            }
        }
        Ok(pause.or(asked))
    }

    fn interrupted(&self, t: &mut Task, p: &Project, why: Interrupt) -> Res {
        match why {
            Interrupt::Paused(w) => self.pause_for(t, p, &w),
            Interrupt::Asked(label, question, options, session) => {
                self.lane_asks(&t.id, Asked { label, kind: "Member".into(), question, options, session });
                self.ask_next_lane(t, p, Status::Working)?;
                Ok(Flow::Stop)
            }
        }
    }

    /// Proposals, the vote, the decision. Ok(None): the executor goes on (the decision is in its prompt).
    pub(super) fn council_propose(&self, t: &mut Task, p: &Project, n: u8, cancel: &AtomicBool) -> Result<Option<Flow>, String> {
        let id = t.id.clone();
        if self.state(&id).swarm.council.decided {
            return Ok(None);
        }
        if self.state(&id).swarm.council.members.is_empty() {
            let a = self.adapter(&t.agent).ok_or_else(|| format!("agent_not_found: {}", t.agent))?;
            let cat = models::catalog(&t.agent, &*a, &lk(&self.store));
            let worker = Self::choice_for(t, RoleKind::Worker).unwrap_or(ModelChoice { family: models::DEFAULT_FAMILY.into(), version: None });
            let members: Vec<Member> = swarm::letters(n as usize)
                .into_iter()
                .zip(swarm::assign_models(&cat, &worker, n))
                .map(|(id, model)| Member { id, model, session: None, pin: None })
                .collect();
            self.update_state(&id, |s| s.swarm.council.members = members)?;
            self.stage(t, &format!("council: {n} members"));
        }
        let brief = self.brief_of(t);
        let task_text = t.prompt.clone();
        // round one: each proposes, independently
        let members = self.state(&id).swarm.council.members;
        let answered = self.take_lane_answer(&id);
        let ans = |m: &Member| answered.clone().filter(|(l, _)| *l == m.id).map(|x| x.1);
        let state = self.state(&id).swarm.council;
        let todo: Vec<Member> =
            members.iter().filter(|m| !state.proposals.iter().any(|x| x.id == m.id) && !state.out.contains(&format!("propose:{}", m.id))).cloned().collect();
        let a = self.adapter(&t.agent).ok_or("agent_not_found")?;
        let cat = models::catalog(&t.agent, &*a, &lk(&self.store));
        let outs = self.all_members(&id, &todo, |m| {
            self.member_turn(&id, p, m, false, false, cancel, ans(m), &|_| prompts::proposal_request(&task_text, &brief), prompts::proposal_schema(), &parse::parse_proposal)
        });
        *t = self.task(&id).ok_or("task_not_found")?;
        if cancel.load(Ordering::SeqCst) {
            return Ok(Some(Flow::Stop));
        }
        let stop = self.settle(&id, "propose", outs, |letter, text| {
            let m = members.iter().find(|m| m.id == letter).map(|m| m.model.clone());
            let label = m.map(|m| models::resolve(&cat, &m).label).unwrap_or_default();
            let label = t.runs.iter().rev().find(|r| r.role == RoleKind::Council && r.variant.as_deref() == Some(letter)).map(|r| r.model.clone()).unwrap_or(label);
            self.council_log(&id, Some(letter), &format!("council_proposal: {letter}: {text}"));
            self.update_state(&id, |s| s.swarm.council.proposals.push(Proposal { id: letter.to_string(), model: label, text }))
        })?;
        if let Some(stop) = stop {
            return self.interrupted(t, p, stop).map(Some);
        }
        let mut proposals = self.state(&id).swarm.council.proposals;
        proposals.sort_by(|a, b| a.id.cmp(&b.id));
        if proposals.is_empty() {
            self.stage(t, "council_no_proposals: the executor goes alone");
            self.update_state(&id, |s| s.swarm.collapsed = true)?;
            return Ok(Some(Flow::Next));
        }
        // round two: everybody sees all proposals and votes (a single proposal needs no vote)
        let options: Vec<String> = proposals.iter().map(|x| x.id.clone()).collect();
        let voted = self.state(&id).swarm.council;
        let voters: Vec<Member> = members.iter().filter(|m| !voted.votes.iter().any(|v| v.voter == m.id) && !voted.out.contains(&format!("vote:{}", m.id))).cloned().collect();
        if proposals.len() > 1 && !voters.is_empty() && !voted.decided {
            let shown: Vec<(String, String)> = proposals.iter().map(|x| (x.id.clone(), x.text.clone())).collect();
            let outs = self.all_members(&id, &voters, |m| {
                self.member_turn(&id, p, m, false, false, cancel, ans(m), &|_| prompts::vote_request(&task_text, &shown), prompts::vote_schema(), &|s: &str| parse::parse_vote(s, &options))
            });
            *t = self.task(&id).ok_or("task_not_found")?;
            if cancel.load(Ordering::SeqCst) {
                return Ok(Some(Flow::Stop));
            }
            let stop = self.settle(&id, "vote", outs, |letter, (choice, why)| {
                self.council_log(&id, Some(letter), &format!("council_vote: {letter} -> {choice}"));
                self.update_state(&id, |s| s.swarm.council.votes.push(VoteRec { voter: letter.to_string(), choice, why }))
            })?;
            if let Some(stop) = stop {
                return self.interrupted(t, p, stop).map(Some);
            }
        }
        let votes = self.state(&id).swarm.council.votes;
        let tally = swarm::tally(&votes.iter().map(|v| swarm::Vote { voter: v.voter.clone(), choice: v.choice.clone() }).collect::<Vec<_>>());
        let (winner, by) = match (proposals.len(), tally.winner.clone()) {
            (1, _) => (proposals[0].id.clone(), "votes"),
            (_, Some(w)) => (w, "votes"),
            _ => {
                // a tie (or nobody voted): the Reviewer decides between the leaders (all of them when nobody voted)
                let among: Vec<String> = if tally.tied.is_empty() { options.clone() } else { tally.tied.clone() };
                match self.reviewer_picks(t, p, "Which proposal should be built?", &among, &proposals_text(&proposals), cancel)? {
                    Some(c) => (c, "reviewer"),
                    None => return Ok(Some(Flow::Stop)),
                }
            }
        };
        let text = proposals.iter().find(|x| x.id == winner).map(|x| x.text.clone()).unwrap_or_default();
        self.council_log(&id, None, &format!("council_decided: {winner} ({by})"));
        self.update_state(&id, |s| {
            s.swarm.council.winner = Some(winner.clone());
            s.swarm.council.decided_by = Some(by.to_string());
            s.swarm.council.decided = true;
            s.worker_hint = Some(format!("The council chose this approach; build it that way:\n{}", prompt::fenced("APPROACH", &text)));
        })?;
        Ok(None)
    }

    /// The Reviewer decides a tie: one of `options`. None = the task waits (question, limit, login).
    /// A reply that is not usable falls back to the first option.
    fn reviewer_picks(&self, t: &mut Task, p: &Project, question: &str, options: &[String], context: &str, cancel: &AtomicBool) -> Result<Option<String>, String> {
        let request = prompts::tie_request(question, options, context);
        let base = |_: Vendor, _: bool| request.clone();
        let stage = t.status.clone();
        match self.structured(t, p, RoleKind::Reviewer, stage, prompts::choice_schema(), &base, &|s: &str| parse::parse_choice(s, options), cancel)? {
            Structured::Stopped => Ok(None),
            Structured::Unusable(_) => Ok(Some(options[0].clone())),
            Structured::Parsed(c) => Ok(Some(c)),
        }
    }

    /// The council votes on the finished work. The majority decides; a tie is the ordinary Reviewer's.
    pub(super) fn council_review(&self, t: &mut Task, p: &Project, _n: u8, cancel: &AtomicBool) -> Res {
        let id = t.id.clone();
        let wt = t.worktree.clone().ok_or("no_worktree")?;
        self.stage(t, &format!("reviewing (council, {})", t.agent));
        self.set_step(t, Step::Review, StepState::Running)?;
        let checks = if t.outcome.gates.is_empty() {
            "No tests in this project.".to_string()
        } else {
            t.outcome.gates.iter().map(|g| format!("{}: {}\n{}", g.name, if g.passed { "passed" } else { "failed" }, g.tail)).collect::<Vec<_>>().join("\n\n")
        };
        let diff = git::diff(&wt, &p.base_branch, DIFF_FOR_COUNCIL);
        let task_text = t.prompt.clone();
        let members = self.state(&id).swarm.council.members;
        // a person's answer to a member goes to that member's own session; the others look at the work afresh
        let answered = self.take_lane_answer(&id);
        let ans = |m: &Member| answered.clone().filter(|(l, _)| *l == m.id).map(|x| x.1);
        self.update_state(&id, |s| s.swarm.council.out.retain(|o| !o.starts_with("review:")))?;
        let outs = self.all_members(&id, &members, |m| {
            self.member_turn(
                &id,
                p,
                m,
                true,
                ans(m).is_none(), // each looks at the work with a mind of its own
                cancel,
                ans(m),
                &|_| prompts::council_review_request(&task_text, &diff, &checks),
                prompt::verdict_schema(),
                &|s: &str| prompt::parse_verdict(s).ok_or_else(|| "not a verdict".to_string()),
            )
        });
        *t = self.task(&id).ok_or("task_not_found")?;
        if cancel.load(Ordering::SeqCst) {
            return Ok(Flow::Stop);
        }
        let (votes_cell, issues_cell) = (std::cell::RefCell::new(Vec::<VoteRec>::new()), std::cell::RefCell::new(Vec::<String>::new()));
        let stop = self.settle(&id, "review", outs, |letter, (approved, iss)| {
            let choice = if approved { "approve" } else { "changes" };
            self.council_log(&id, Some(letter), &format!("council_review_vote: {letter} -> {choice}"));
            if !approved {
                issues_cell.borrow_mut().extend(iss.clone());
            }
            votes_cell.borrow_mut().push(VoteRec { voter: letter.to_string(), choice: choice.into(), why: iss.first().cloned() });
            Ok(())
        })?;
        if let Some(stop) = stop {
            return self.interrupted(t, p, stop);
        }
        let (votes, mut issues) = (votes_cell.into_inner(), issues_cell.into_inner());
        self.update_state(&id, |s| s.swarm.council.review_votes = votes.clone())?;
        let tally = swarm::tally(&votes.iter().map(|v| swarm::Vote { voter: v.voter.clone(), choice: v.choice.clone() }).collect::<Vec<_>>());
        match tally.winner.as_deref() {
            Some("approve") => self.apply_verdict(t, true, vec![]),
            Some(_) => {
                if issues.is_empty() {
                    issues.push("the council asked for changes".into());
                }
                self.apply_verdict(t, false, issues)
            }
            None => {
                // a tie (or no vote): the Reviewer decides, as the one reviewer always did
                self.council_log(&id, None, "council_review_tie: the Reviewer decides");
                self.review_plain(t, p, cancel)
            }
        }
    }
}

fn proposals_text(ps: &[Proposal]) -> String {
    ps.iter().map(|x| format!("Proposal {}:\n{}", x.id, x.text)).collect::<Vec<_>>().join("\n\n")
}
