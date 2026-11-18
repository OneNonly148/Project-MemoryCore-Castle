//! HOME's Mail widget (this fork only): a short background Claude Code run, allowed to READ
//! Outlook through the member's claude.ai Microsoft 365 connector and nothing else, sorts the newest
//! mail into a snapshot the widget shows. Castle itself never signs in to Microsoft. The page decides
//! when to sort (every 30 minutes on workdays, and on Refresh); a run already going is not doubled.

use crate::env::{self, NEVER_INHERITED};
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};

/// The only tools the run may use: search the mailbox and read one message. No send, draft, move,
/// label or delete tool is allowed, and Claude Code's own tools (files, shell, web) are switched off.
const READ_ONLY_TOOLS: &str = "mcp__claude_ai_Microsoft_365__outlook_email_search,mcp__claude_ai_Microsoft_365__read_resource";
const LIMIT: Duration = Duration::from_secs(240);
const RULES: &str = include_str!("../resources/mail-rules.md");

static RUNNING: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Mail {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub from: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub subject: String,
    #[serde(default)]
    pub received: String,
    #[serde(default)]
    pub unread: bool,
    #[serde(default, alias = "needs_reply")]
    pub needs_reply: bool,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub link: String,
    /// The sender's CRM status, when the sender is a CRM contact (filled in by Castle, not the run).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crm: Option<CrmLink>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CrmLink {
    pub id: String,
    pub company: String,
    pub status: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    /// When the mail was last sorted, in milliseconds since 1970; 0 when it never was.
    pub sorted_at: u64,
    pub unread: u32,
    pub items: Vec<Mail>,
    /// Why the last attempt failed, in words; empty when it worked. The old mail stays shown.
    pub problem: String,
    pub tried_at: u64,
    pub running: bool,
}

fn folder() -> Option<PathBuf> {
    env::app_data().map(|a| a.join("MemoryCore Castle").join("mail"))
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn load() -> Snapshot {
    folder()
        .and_then(|f| std::fs::read_to_string(f.join("snapshot.json")).ok())
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn store(snapshot: &Snapshot) {
    if let Some(f) = folder() {
        let _ = std::fs::create_dir_all(&f);
        let tmp = f.join("snapshot.json.tmp");
        if std::fs::write(&tmp, serde_json::to_string_pretty(snapshot).unwrap_or_default()).is_ok() {
            let _ = std::fs::rename(tmp, f.join("snapshot.json"));
        }
    }
}

#[derive(Deserialize)]
struct Answer {
    #[serde(default)]
    unread: u32,
    #[serde(default)]
    items: Vec<Mail>,
}

/// The run's answer as mail: the one JSON object in its text (a code fence or a stray line around
/// it is tolerated). Items without a subject and a sender are dropped; summaries are kept short.
pub fn parse(text: &str) -> Result<(u32, Vec<Mail>), String> {
    let (start, end) = (text.find('{'), text.rfind('}'));
    let json = match (start, end) {
        (Some(s), Some(e)) if e > s => &text[s..=e],
        _ => return Err("The mail sort gave no answer Castle could read.".into()),
    };
    let answer: Answer = serde_json::from_str(json).map_err(|_| "The mail sort gave no answer Castle could read.".to_string())?;
    let items = answer
        .items
        .into_iter()
        .filter(|m| !m.subject.trim().is_empty() || !m.from.trim().is_empty())
        .map(|mut m| {
            if m.summary.chars().count() > 200 {
                m.summary = m.summary.chars().take(199).collect::<String>() + "…";
            }
            if !m.link.starts_with("https://") {
                m.link.clear();
            }
            m
        })
        .take(25)
        .collect();
    Ok((answer.unread, items))
}

/// What a failed run says to the member, from what Claude Code printed.
fn problem_from(out: &str, err: &str) -> String {
    let all = format!("{out}\n{err}").to_lowercase();
    if all.contains("not logged in") || all.contains("/login") {
        "Claude Code is not signed in. Sign in, then press Refresh.".into()
    } else if all.contains("microsoft") && (all.contains("auth") || all.contains("connect")) {
        "Microsoft 365 needs connecting again in your claude.ai connectors.".into()
    } else if all.contains("usage limit") || all.contains("rate limit") {
        "Claude Code's usage limit is reached; mail sorts again when it resets.".into()
    } else {
        "The mail sort did not finish. The last sorted mail is shown.".into()
    }
}

fn run_once() -> Result<(u32, Vec<Mail>), String> {
    let folder = folder().ok_or("Castle has no folder for mail.")?;
    std::fs::create_dir_all(&folder).map_err(|_| "Castle could not make its mail folder.")?;
    crate::oneshot::write_if_changed(&folder.join("rules.md"), RULES);
    let program = env::find_on_path("claude").ok_or("Claude Code is not installed.")?;
    let mut command = Command::new(program);
    command
        .args(["-p", "Sort my inbox now.", "--system-prompt-file", "rules.md", "--tools", "", "--allowedTools", READ_ONLY_TOOLS, "--output-format", "text"])
        .current_dir(&folder)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for name in NEVER_INHERITED {
        command.env_remove(name);
    }
    for name in ["CASTLE_TOOLS", "CASTLE_STYLES", "CASTLE_IMAGE", "CASTLE_CLI", "CASTLE_EXE", "CASTLE_MEMORY"] {
        command.env_remove(name);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    let mut child = command.spawn().map_err(|_| "Claude Code could not be started.")?;
    let (mut stdout, mut stderr) = (child.stdout.take().ok_or("No output.")?, child.stderr.take().ok_or("No output.")?);
    let out = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = stdout.read_to_string(&mut s);
        s
    });
    let err = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = stderr.read_to_string(&mut s);
        s
    });
    let until = Instant::now() + LIMIT;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < until => std::thread::sleep(Duration::from_millis(250)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("The mail sort took too long and was stopped.".into());
            }
        }
    };
    let (out, err) = (out.join().unwrap_or_default(), err.join().unwrap_or_default());
    if !status.success() {
        return Err(problem_from(&out, &err));
    }
    parse(&out).map_err(|e| if out.trim().is_empty() { problem_from(&out, &err) } else { e })
}

/// The last sorted mail, with the sender's CRM status where the sender is a CRM contact.
#[tauri::command]
pub fn mail_snapshot(settings: tauri::State<crate::settings::SettingsStore>) -> Snapshot {
    let mut snapshot = load();
    snapshot.running = RUNNING.load(Ordering::SeqCst);
    crate::crm::link_mail(&settings, &mut snapshot.items);
    snapshot
}

/// Sorts the mail now, in the background; `mail-changed` is sent when it is done. False when a
/// sort is already running.
#[tauri::command]
pub fn mail_refresh(app: AppHandle) -> bool {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return false;
    }
    let _ = app.emit("mail-changed", ());
    std::thread::spawn(move || {
        let mut snapshot = load();
        snapshot.tried_at = now_ms();
        match run_once() {
            Ok((unread, items)) => {
                snapshot.unread = unread;
                snapshot.items = items;
                snapshot.sorted_at = snapshot.tried_at;
                snapshot.problem.clear();
            }
            Err(problem) => snapshot.problem = problem,
        }
        snapshot.running = false;
        store(&snapshot);
        RUNNING.store(false, Ordering::SeqCst);
        let _ = app.emit("mail-changed", ());
    });
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_answer_is_read_from_its_json_even_inside_a_fence() {
        let text = "```json\n{\"unread\": 3, \"items\": [{\"from\": \"Ana\", \"subject\": \"Demo\", \"needs_reply\": true, \"summary\": \"Wants Thursday.\", \"link\": \"https://outlook.office.com/x\"}]}\n```";
        let (unread, items) = parse(text).unwrap();
        assert_eq!(unread, 3);
        assert_eq!(items.len(), 1);
        assert!(items[0].needs_reply);
        assert_eq!(items[0].link, "https://outlook.office.com/x");
    }

    #[test]
    fn empty_items_and_odd_links_are_dropped_and_long_summaries_cut() {
        let long = "x".repeat(400);
        let text = format!("{{\"items\": [{{\"from\": \"\", \"subject\": \"\"}}, {{\"from\": \"B\", \"subject\": \"S\", \"summary\": \"{long}\", \"link\": \"javascript:alert(1)\"}}]}}");
        let (_, items) = parse(&text).unwrap();
        assert_eq!(items.len(), 1);
        assert!(items[0].link.is_empty(), "only https links open");
        assert_eq!(items[0].summary.chars().count(), 200);
    }

    #[test]
    fn no_json_is_a_problem_in_words() {
        assert!(parse("I could not reach Outlook.").unwrap_err().contains("no answer"));
    }

    #[test]
    fn the_run_may_only_read_outlook() {
        for tool in READ_ONLY_TOOLS.split(',') {
            assert!(tool.ends_with("outlook_email_search") || tool.ends_with("read_resource"), "{tool}");
        }
    }
}
