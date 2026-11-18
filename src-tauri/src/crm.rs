//! The CRM room (this fork only): reads the member's CMS folder (data/prospects.csv and
//! data/touchpoints.csv) and never writes it. Every change goes through the companion, whose own
//! cms_* tools keep the files safe. The folder is kept in Castle's crm.json.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

/// Where Wan's CMS folder usually is; used until a folder is chosen.
const USUAL_FOLDER: &str = "Library/Mobile Documents/com~apple~CloudDocs/Work/TNEX/Sales Engineering/CMS";
/// The pipeline in its order; any other status is "parked".
pub const STAGES: [&str; 5] = ["Profiling", "To contact", "Contacted", "In discussion", "Meeting scheduled"];
/// Mail domains that say nothing about a company.
const PERSONAL_DOMAINS: [&str; 9] = ["gmail.com", "yahoo.com", "hotmail.com", "outlook.com", "live.com", "icloud.com", "me.com", "ymail.com", "msn.com"];

#[derive(Default, Serialize, Deserialize)]
struct Config {
    #[serde(default)]
    folder: String,
}

fn config_file() -> Option<PathBuf> {
    crate::env::app_data().map(|a| a.join("MemoryCore Castle").join("crm.json"))
}

/// The CMS folder: the chosen one, else the usual one when it is there.
pub fn folder() -> Option<PathBuf> {
    let chosen = config_file()
        .and_then(|f| std::fs::read_to_string(f).ok())
        .and_then(|t| serde_json::from_str::<Config>(&t).ok())
        .map(|c| c.folder)
        .filter(|f| !f.trim().is_empty())
        .map(PathBuf::from);
    chosen.or_else(|| crate::env::home_dir().map(|h| h.join(USUAL_FOLDER))).filter(|f| f.join("data").join("prospects.csv").is_file())
}

/// Reads CSV text (quoted fields, doubled quotes, commas and line breaks inside quotes) as rows.
pub fn parse_csv(text: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let (mut row, mut field, mut quoted) = (Vec::new(), String::new(), false);
    let mut chars = text.trim_start_matches('\u{feff}').chars().peekable();
    while let Some(c) = chars.next() {
        match (quoted, c) {
            (true, '"') if chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            (true, '"') => quoted = false,
            (true, c) => field.push(c),
            (false, '"') if field.is_empty() => quoted = true,
            (false, ',') => row.push(std::mem::take(&mut field)),
            (false, '\r') => {}
            (false, '\n') => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
            }
            (false, c) => field.push(c),
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows
}

/// Rows as name → value maps, by the header row.
fn records(text: &str) -> Vec<HashMap<String, String>> {
    let mut rows = parse_csv(text).into_iter();
    let Some(header) = rows.next() else { return Vec::new() };
    rows.filter(|r| r.iter().any(|f| !f.trim().is_empty()))
        .map(|r| header.iter().cloned().zip(r.into_iter().chain(std::iter::repeat(String::new()))).collect())
        .collect()
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Contact {
    pub id: String,
    pub company: String,
    pub person: String,
    pub position: String,
    pub sector: String,
    pub status: String,
    pub channel: String,
    pub contact_date: String,
    pub email: String,
    pub product: String,
    pub event: String,
    /// The newest of: its touchpoints, contact_date and flow_last_contact (YYYY-MM-DD), or "".
    pub last_touch: String,
    pub demo_date: String,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Touch {
    pub date: String,
    pub source: String,
    pub name: String,
    pub product: String,
    pub status: String,
    pub note: String,
}

struct Data {
    contacts: Vec<Contact>,
    full: Vec<HashMap<String, String>>,
    touches: Vec<Touch>,
}

/// The files as last read, kept until either changes on disk.
static CACHE: Mutex<Option<(PathBuf, Option<SystemTime>, Option<SystemTime>, std::sync::Arc<Data>)>> = Mutex::new(None);

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn get(r: &HashMap<String, String>, k: &str) -> String {
    r.get(k).map(|v| v.trim().to_string()).unwrap_or_default()
}

fn read(folder: &Path) -> Result<std::sync::Arc<Data>, String> {
    let (prospects, touchpoints) = (folder.join("data").join("prospects.csv"), folder.join("data").join("touchpoints.csv"));
    let stamp = (modified(&prospects), modified(&touchpoints));
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((f, a, b, data)) = cache.as_ref() {
        if f == folder && (*a, *b) == stamp {
            return Ok(data.clone());
        }
    }
    let text = std::fs::read_to_string(&prospects).map_err(|_| "prospects.csv could not be read.".to_string())?;
    let full = records(&text);
    let contacts = full
        .iter()
        .map(|r| Contact {
            id: get(r, "id"),
            company: get(r, "company"),
            person: get(r, "contact_person"),
            position: get(r, "position"),
            sector: get(r, "sector"),
            status: get(r, "status"),
            channel: get(r, "channel"),
            contact_date: get(r, "contact_date"),
            email: get(r, "email"),
            product: get(r, "product"),
            event: get(r, "event"),
            last_touch: [get(r, "contact_date"), get(r, "flow_last_contact")].into_iter().max().unwrap_or_default(),
            demo_date: get(r, "demo_date"),
            notes: { let t = get(r, "talking_points"); if t.is_empty() { get(r, "notes") } else { t } },
        })
        .collect::<Vec<Contact>>();
    let touches: Vec<Touch> = std::fs::read_to_string(&touchpoints)
        .map(|t| records(&t))
        .unwrap_or_default()
        .iter()
        .map(|r| Touch { date: get(r, "date"), source: get(r, "source"), name: get(r, "name"), product: get(r, "product"), status: get(r, "status"), note: get(r, "note") })
        .collect();
    let mut contacts = contacts;
    stamp_last_touch(&mut contacts, &touches);
    let data = std::sync::Arc::new(Data { contacts, full, touches });
    *cache = Some((folder.to_path_buf(), stamp.0, stamp.1, data.clone()));
    Ok(data)
}

/// Each contact's last touch, raised to its newest touchpoint (matched by company or person name).
pub fn stamp_last_touch(contacts: &mut [Contact], touches: &[Touch]) {
    let mut newest: HashMap<String, &str> = HashMap::new();
    for t in touches {
        let key = t.name.trim().to_lowercase();
        if key.len() > 2 && newest.get(&key).is_none_or(|d| *d < t.date.as_str()) {
            newest.insert(key, &t.date);
        }
    }
    for c in contacts.iter_mut() {
        for name in [&c.company, &c.person] {
            if let Some(d) = newest.get(&name.trim().to_lowercase()) {
                if *d > c.last_touch.as_str() {
                    c.last_touch = d.to_string();
                }
            }
        }
    }
}

/// A touchpoint's channel: WhatsApp when its source says so, else email.
pub fn touch_channel(source: &str) -> &'static str {
    let s = source.to_lowercase();
    if s.contains("whatsapp") || s.ends_with("ws") || s.contains("– ws") || s.contains("- ws") {
        "whatsapp"
    } else {
        "email"
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Overview {
    pub folder: String,
    pub contacts: Vec<Contact>,
    pub stages: Vec<(String, usize)>,
    pub parked: Vec<(String, usize)>,
    pub channels: Vec<(String, usize)>,
    pub sent_today: HashMap<&'static str, usize>,
    pub touches_this_month: usize,
    pub missing_email: usize,
    /// Touchpoints per date (YYYY-MM-DD), for the activity chart.
    pub per_day: HashMap<String, usize>,
    /// Contacts snoozed until a date (YYYY-MM-DD), by id.
    pub snoozed: HashMap<String, String>,
}

/// Counts in the pipeline's order, then the parked statuses by size.
pub fn overview_of(contacts: &[Contact], touches: &[Touch], today: &str) -> (Vec<(String, usize)>, Vec<(String, usize)>, Vec<(String, usize)>, HashMap<&'static str, usize>, usize, usize) {
    let mut by_status: HashMap<String, usize> = HashMap::new();
    let mut by_channel: HashMap<String, usize> = HashMap::new();
    for c in contacts {
        *by_status.entry(c.status.clone()).or_default() += 1;
        *by_channel.entry(c.channel.clone()).or_default() += 1;
    }
    let stages = STAGES.iter().map(|s| (s.to_string(), by_status.get(*s).copied().unwrap_or(0))).collect();
    let mut parked: Vec<(String, usize)> = by_status.into_iter().filter(|(s, _)| !STAGES.contains(&s.as_str()) && !s.is_empty()).collect();
    parked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let mut channels: Vec<(String, usize)> = by_channel.into_iter().filter(|(c, _)| !c.is_empty()).collect();
    channels.sort_by(|a, b| b.1.cmp(&a.1));
    let mut sent = HashMap::from([("email", 0), ("whatsapp", 0)]);
    for t in touches.iter().filter(|t| t.date == today) {
        *sent.entry(touch_channel(&t.source)).or_default() += 1;
    }
    let month = touches.iter().filter(|t| today.len() >= 7 && t.date.starts_with(&today[..7])).count();
    let missing = contacts.iter().filter(|c| c.status == "To contact" && c.channel == "Direct" && !c.email.contains('@')).count();
    (stages, parked, channels, sent, month, missing)
}

/// The CRM as the room shows it. `today` is the member's own date (YYYY-MM-DD), from the page.
#[tauri::command]
pub async fn crm_overview(today: String) -> Result<Overview, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let folder = folder().ok_or("Choose your CMS folder (the one with data/prospects.csv).")?;
        let data = read(&folder)?;
        let (stages, parked, channels, sent_today, touches_this_month, missing_email) = overview_of(&data.contacts, &data.touches, &today);
        let mut per_day: HashMap<String, usize> = HashMap::new();
        for t in data.touches.iter().filter(|t| t.date.len() == 10) {
            *per_day.entry(t.date.clone()).or_default() += 1;
        }
        let snoozed = snoozes().into_iter().filter(|(_, until)| until.as_str() > today.as_str()).collect();
        Ok(Overview { folder: folder.display().to_string(), contacts: data.contacts.clone(), stages, parked, channels, sent_today, touches_this_month, missing_email, per_day, snoozed })
    })
    .await
    .map_err(|_| "The CRM could not be read.".to_string())?
}

/// Touchpoints that belong to a contact: the same company or person name, newest first.
pub fn touches_for<'a>(contact: &Contact, touches: &'a [Touch]) -> Vec<&'a Touch> {
    let names: Vec<String> = [&contact.company, &contact.person].iter().map(|n| n.trim().to_lowercase()).filter(|n| n.len() > 2).collect();
    let mut found: Vec<&Touch> = touches.iter().filter(|t| names.contains(&t.name.trim().to_lowercase())).collect();
    found.sort_by(|a, b| b.date.cmp(&a.date));
    found
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactCard {
    pub fields: HashMap<String, String>,
    pub touches: Vec<Touch>,
    pub profile: Option<String>,
}

/// One contact in full, with its touchpoints and the path of its account profile when there is one.
#[tauri::command]
pub async fn crm_contact(id: String) -> Result<ContactCard, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let folder = folder().ok_or("No CMS folder.")?;
        let data = read(&folder)?;
        let at = data.contacts.iter().position(|c| c.id == id).ok_or("That contact is no longer in the CRM.")?;
        let contact = &data.contacts[at];
        let touches = touches_for(contact, &data.touches).into_iter().take(30).cloned().collect();
        Ok(ContactCard { fields: data.full[at].clone(), touches, profile: profile_of(&folder, &contact.company) })
    })
    .await
    .map_err(|_| "The contact could not be read.".to_string())?
}

/// The account profile: `../Accounts/<sector>/<Company>.md`, matched by name.
fn profile_of(folder: &Path, company: &str) -> Option<String> {
    let accounts = folder.parent()?.join("Accounts");
    let want = company.trim().to_lowercase();
    if want.is_empty() {
        return None;
    }
    for sector in std::fs::read_dir(&accounts).ok()?.flatten().filter(|e| e.path().is_dir()) {
        for file in std::fs::read_dir(sector.path()).ok()?.flatten() {
            let path = file.path();
            let stem = path.file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
            if path.extension().is_some_and(|e| e == "md") && stem == want {
                return Some(path.display().to_string());
            }
        }
    }
    None
}

/// Chooses the CMS folder (it must hold data/prospects.csv).
#[tauri::command]
pub fn crm_set_folder(folder: String) -> Result<(), String> {
    if !Path::new(&folder).join("data").join("prospects.csv").is_file() {
        return Err("That folder has no data/prospects.csv.".into());
    }
    let file = config_file().ok_or("Castle has no settings folder.")?;
    let _ = std::fs::create_dir_all(file.parent().unwrap_or(Path::new(".")));
    std::fs::write(file, serde_json::to_string(&Config { folder }).unwrap_or_default()).map_err(|_| "The folder could not be saved.".to_string())
}

/// Opens a contact's account profile (only a file inside the CMS folder's Accounts).
#[tauri::command]
pub fn crm_open_profile(path: String) -> Result<(), String> {
    let folder = folder().ok_or("No CMS folder.")?;
    let accounts = folder.parent().ok_or("No Accounts folder.")?.join("Accounts");
    let real = Path::new(&path).canonicalize().map_err(|_| "That profile is gone.")?;
    if !real.starts_with(accounts.canonicalize().unwrap_or(accounts)) {
        return Err("Only account profiles open from here.".into());
    }
    #[cfg(not(windows))]
    return crate::os::open_path(&real, false);
    #[cfg(windows)]
    {
        let _ = real;
        Err("Opening a profile from the CRM room is macOS only for now.".into())
    }
}

// ---------- changing a contact's status (Castle writes prospects.csv itself) ----------

/// The statuses Castle may set: the pipeline and the parked ones the CRM already uses.
pub const SETTABLE: [&str; 9] = ["Profiling", "To contact", "Contacted", "In discussion", "Meeting scheduled", "On hold", "Not interested", "Bounced", "Lost"];

/// Every field of CSV text with its byte span (start, end, quoted), row by row.
fn spans(text: &str) -> Vec<Vec<(usize, usize, bool)>> {
    let bytes = text.as_bytes();
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut i = 0;
    let n = bytes.len();
    if text.starts_with('\u{feff}') {
        i = 3;
    }
    while i < n {
        let start = i;
        let quoted = bytes[i] == b'"';
        if quoted {
            i += 1;
            while i < n {
                if bytes[i] == b'"' {
                    if i + 1 < n && bytes[i + 1] == b'"' {
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                i += 1;
            }
        }
        while i < n && bytes[i] != b',' && bytes[i] != b'\n' && bytes[i] != b'\r' {
            i += 1;
        }
        row.push((start, i, quoted));
        if i >= n {
            rows.push(std::mem::take(&mut row));
            break;
        }
        match bytes[i] {
            b',' => {
                i += 1;
                if i >= n {
                    row.push((i, i, false));
                    rows.push(std::mem::take(&mut row));
                }
            }
            _ => {
                if bytes[i] == b'\r' && i + 1 < n && bytes[i + 1] == b'\n' {
                    i += 1;
                }
                i += 1;
                rows.push(std::mem::take(&mut row));
            }
        }
    }
    rows
}

fn quoted_if_needed(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) { format!("\"{}\"", value.replace('"', "\"\"")) } else { value.to_string() }
}

/// The CSV text with one field of one row (found by its `id` column) set to `value`. Every other
/// byte of the file stays exactly as it was.
pub fn with_field(text: &str, id: &str, column: &str, value: &str) -> Result<String, String> {
    let rows = spans(text);
    let header = rows.first().ok_or("prospects.csv is empty.")?;
    let name = |(a, b, _): &(usize, usize, bool)| text[*a..*b].trim_matches('"').trim().to_string();
    let id_col = header.iter().position(|f| name(f) == "id").ok_or("prospects.csv has no id column.")?;
    let col = header.iter().position(|f| name(f) == column).ok_or(format!("prospects.csv has no {column} column."))?;
    let mut found = rows[1..].iter().filter(|r| r.get(id_col).is_some_and(|f| name(f) == id));
    let row = found.next().ok_or("That contact is not in prospects.csv any more.")?;
    if found.next().is_some() {
        return Err("Two rows share that id; nothing was changed.".into());
    }
    let (a, b, _) = *row.get(col).ok_or("That row is short; nothing was changed.")?;
    Ok(format!("{}{}{}", &text[..a], quoted_if_needed(value), &text[b..]))
}

/// Sets a contact's status in prospects.csv: a copy first (prospects.backup-castle-<time>.csv),
/// then the new text through a temporary file, checked (same rows, same fields, only that one
/// changed) before it replaces the file.
#[tauri::command]
pub fn crm_set_status(id: String, status: String) -> Result<String, String> {
    if !SETTABLE.contains(&status.as_str()) {
        return Err("That is not a CRM status.".into());
    }
    let folder = folder().ok_or("No CMS folder.")?;
    let data_dir = folder.join("data");
    let file = data_dir.join("prospects.csv");
    let before = std::fs::read_to_string(&file).map_err(|_| "prospects.csv could not be read.".to_string())?;
    let after = with_field(&before, &id, "status", &status)?;
    let (old, new) = (records(&before), records(&after));
    let changed: Vec<usize> = (0..old.len()).filter(|i| old.get(*i) != new.get(*i)).collect();
    let ok = old.len() == new.len()
        && changed.len() <= 1
        && changed.iter().all(|i| new[*i].get("id").map(|v| v.trim()) == Some(id.as_str()) && new[*i].get("status").map(|v| v.trim()) == Some(status.as_str()));
    if !ok {
        return Err("The change did not check out, so prospects.csv was left as it was.".into());
    }
    let secs = SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    std::fs::copy(&file, data_dir.join(format!("prospects.backup-castle-{secs}.csv"))).map_err(|_| "The backup copy could not be made, so nothing was changed.".to_string())?;
    let tmp = data_dir.join(".prospects.castle-tmp.csv");
    std::fs::write(&tmp, &after).map_err(|_| "prospects.csv could not be written.".to_string())?;
    std::fs::rename(&tmp, &file).map_err(|_| "prospects.csv could not be replaced.".to_string())?;
    let company = new.iter().find(|r| r.get("id").map(|v| v.trim()) == Some(id.as_str())).map(|r| get(r, "company")).unwrap_or_default();
    Ok(format!("{company} is now {status}."))
}

// ---------- snooze (kept by Castle, not in the CRM) ----------

fn snooze_file() -> Option<PathBuf> {
    crate::env::app_data().map(|a| a.join("MemoryCore Castle").join("crm-snooze.json"))
}

fn snoozes() -> HashMap<String, String> {
    snooze_file().and_then(|f| std::fs::read_to_string(f).ok()).and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

/// Hides a contact from Today until `until` (YYYY-MM-DD); an empty `until` wakes it.
#[tauri::command]
pub fn crm_snooze(id: String, until: String) -> Result<(), String> {
    let mut all = snoozes();
    if until.is_empty() {
        all.remove(&id);
    } else {
        all.insert(id, until);
    }
    let file = snooze_file().ok_or("Castle has no settings folder.")?;
    let _ = std::fs::create_dir_all(file.parent().unwrap_or(Path::new(".")));
    std::fs::write(file, serde_json::to_string(&all).unwrap_or_default()).map_err(|_| "The snooze could not be saved.".to_string())
}

/// Marks each mail whose sender is a CRM contact: by address, else by company domain.
pub fn link(contacts: &[Contact], mail: &mut [crate::mail::Mail]) {
    let mut by_email: HashMap<String, &Contact> = HashMap::new();
    let mut by_domain: HashMap<String, &Contact> = HashMap::new();
    for c in contacts {
        for address in c.email.split([',', ';', ' ']).map(|e| e.trim().to_lowercase()).filter(|e| e.contains('@')) {
            if let Some(domain) = address.rsplit('@').next() {
                if !PERSONAL_DOMAINS.contains(&domain) {
                    by_domain.entry(domain.to_string()).or_insert(c);
                }
            }
            by_email.entry(address).or_insert(c);
        }
    }
    for m in mail.iter_mut() {
        let address = m.email.trim().to_lowercase();
        let domain = address.rsplit('@').next().unwrap_or("").to_string();
        let found = by_email.get(&address).or_else(|| by_domain.get(&domain));
        m.crm = found.map(|c| crate::mail::CrmLink { id: c.id.clone(), company: c.company.clone(), status: c.status.clone() });
    }
}

/// `link` with the CRM as it is on disk; nothing changes when there is no CMS folder.
pub fn link_mail(_store: &crate::settings::SettingsStore, mail: &mut [crate::mail::Mail]) {
    if let Some(data) = folder().and_then(|f| read(&f).ok()) {
        link(&data.contacts, mail);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contact(id: &str, company: &str, status: &str, email: &str) -> Contact {
        Contact { id: id.into(), company: company.into(), person: String::new(), position: String::new(), sector: String::new(), status: status.into(), channel: "Direct".into(), contact_date: String::new(), email: email.into(), product: String::new(), event: String::new(), last_touch: String::new(), demo_date: String::new(), notes: String::new() }
    }

    #[test]
    fn one_field_changes_and_every_other_byte_stays() {
        let text = "id,company,status,notes\n1,\"A, Sdn Bhd\",To contact,\"line one\nline \"\"two\"\"\"\n2,B,Contacted,\r\n3,C,To contact,x";
        let out = with_field(text, "1", "status", "In discussion").unwrap();
        assert_eq!(out, text.replacen("To contact", "In discussion", 1));
        let out = with_field(text, "3", "status", "On hold").unwrap();
        assert!(out.ends_with("3,C,On hold,x") && out.starts_with("id,company,status,notes\n1,\"A, Sdn Bhd\",To contact"));
        assert!(with_field(text, "9", "status", "Lost").unwrap_err().contains("not in prospects.csv"));
        assert!(with_field("id,status\n1,a\n1,b\n", "1", "status", "x").unwrap_err().contains("Two rows"));
        assert_eq!(with_field("id,status\n1,\"To contact\"\n", "1", "status", "Lost").unwrap(), "id,status\n1,Lost\n");
    }

    /// On a copy of a real prospects.csv (CASTLE_TEST_CSV): every row's status can be set alone.
    #[test]
    #[ignore]
    fn a_real_csv_changes_only_the_one_field() {
        let text = std::fs::read_to_string(std::env::var("CASTLE_TEST_CSV").expect("set CASTLE_TEST_CSV")).unwrap();
        let before = records(&text);
        for (i, row) in before.iter().enumerate().step_by(97) {
            let id = row["id"].trim().to_string();
            let after = records(&with_field(&text, &id, "status", "Lost").unwrap());
            assert_eq!(after.len(), before.len());
            let changed: Vec<usize> = (0..before.len()).filter(|j| before[*j] != after[*j]).collect();
            assert!(changed.is_empty() || changed == vec![i], "row {i} changed {changed:?}");
            assert_eq!(after[i]["status"], "Lost");
        }
    }

    #[test]
    fn last_touch_is_the_newest_touchpoint_or_date() {
        let mut cs = vec![contact("1", "Perkasa", "In discussion", "")];
        cs[0].last_touch = "2026-10-01".into();
        let t = |d: &str, n: &str| Touch { date: d.into(), source: "".into(), name: n.into(), product: "".into(), status: "".into(), note: "".into() };
        stamp_last_touch(&mut cs, &[t("2026-11-02", "perkasa"), t("2026-09-01", "Perkasa")]);
        assert_eq!(cs[0].last_touch, "2026-11-02");
    }

    #[test]
    fn csv_keeps_commas_quotes_and_line_breaks_inside_quotes() {
        let rows = parse_csv("id,note\n1,\"a, b\"\n2,\"say \"\"hi\"\"\nnext line\"\n");
        assert_eq!(rows, vec![vec!["id", "note"], vec!["1", "a, b"], vec!["2", "say \"hi\"\nnext line"]]);
    }

    #[test]
    fn short_rows_are_filled_and_blank_rows_skipped() {
        let r = records("a,b,c\n1,2\n\n");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0]["c"], "");
    }

    #[test]
    fn the_overview_counts_stages_in_order_and_today_by_channel() {
        let contacts = vec![contact("1", "A", "Contacted", ""), contact("2", "B", "To contact", ""), contact("3", "C", "On hold", "x@c.com")];
        let touches = vec![
            Touch { date: "2026-11-18".into(), source: "Email".into(), name: "A".into(), product: "".into(), status: "".into(), note: "".into() },
            Touch { date: "2026-11-18".into(), source: "Akashah – WS".into(), name: "B".into(), product: "".into(), status: "".into(), note: "".into() },
            Touch { date: "2026-10-02".into(), source: "Email".into(), name: "A".into(), product: "".into(), status: "".into(), note: "".into() },
        ];
        let (stages, parked, _, sent, month, missing) = overview_of(&contacts, &touches, "2026-11-18");
        assert_eq!(stages[1], ("To contact".to_string(), 1));
        assert_eq!(parked, vec![("On hold".to_string(), 1)]);
        assert_eq!((sent["email"], sent["whatsapp"], month, missing), (1, 1, 2, 1));
    }

    #[test]
    fn mail_links_by_address_then_company_domain_never_by_gmail() {
        let contacts = vec![contact("7", "Perkasa", "In discussion", "ops@perkasa.my"), contact("8", "Solo", "Contacted", "solo@gmail.com")];
        let mut mail = vec![
            crate::mail::Mail { email: "gm@perkasa.my".into(), ..Default::default() },
            crate::mail::Mail { email: "someone@gmail.com".into(), ..Default::default() },
            crate::mail::Mail { email: "Solo@gmail.com".into(), ..Default::default() },
        ];
        link(&contacts, &mut mail);
        assert_eq!(mail[0].crm.as_ref().unwrap().status, "In discussion");
        assert!(mail[1].crm.is_none(), "a shared mail domain says nothing");
        assert_eq!(mail[2].crm.as_ref().unwrap().id, "8");
    }

    #[test]
    fn touchpoints_match_the_company_or_person_newest_first() {
        let mut c = contact("1", "Perkasa Steel", "Contacted", "");
        c.person = "Aminah".into();
        let t = |d: &str, n: &str| Touch { date: d.into(), source: "Email".into(), name: n.into(), product: "".into(), status: "".into(), note: "".into() };
        let touches = vec![t("2026-11-01", "perkasa steel"), t("2026-11-09", "Aminah"), t("2026-11-10", "Other")];
        let found = touches_for(&c, &touches);
        assert_eq!(found.iter().map(|t| t.date.as_str()).collect::<Vec<_>>(), vec!["2026-11-09", "2026-11-01"]);
    }
}
