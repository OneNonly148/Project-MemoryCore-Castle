// The CRM room's pure parts (this fork): plain stage names over the CSV's words, who needs a nudge,
// the activity chart's days, filters, and the requests typed to the companion.

export const DAILY = { email: 20, whatsapp: 20 }
export const QUIET_DAYS = 7

// The CSV's status words, said plainly on screen. prospects.csv keeps its own words.
export const STAGES = [
  { csv: 'Profiling', name: 'Getting to know', key: 'know' },
  { csv: 'To contact', name: 'Ready to pitch', key: 'ready' },
  { csv: 'Contacted', name: 'Reached', key: 'reached' },
  { csv: 'In discussion', name: 'Talking', key: 'talking' },
  { csv: 'Meeting scheduled', name: 'Meeting set', key: 'meeting' },
]
const PARKED = { 'On hold': 'Paused', 'Not interested': 'Not now', Bounced: 'Bad address', Lost: 'Lost', Unreachable: 'Unreachable' }

export function plain(status) {
  return STAGES.find((s) => s.csv === status)?.name ?? PARKED[status] ?? status ?? ''
}
export function stageKey(status) {
  return STAGES.find((s) => s.csv === status)?.key ?? (status === 'On hold' ? 'paused' : 'parked')
}
// The ladder step a status stands on (0-4), or -1 off the ladder.
export function rung(status) {
  return STAGES.findIndex((s) => s.csv === status)
}

const CHANNEL = { email: 'Direct', whatsapp: 'WhatsApp', linkedin: 'LinkedIn' }

// The member's own date as YYYY-MM-DD.
export function localDay(date = new Date()) {
  const p = (n) => String(n).padStart(2, '0')
  return `${date.getFullYear()}-${p(date.getMonth() + 1)}-${p(date.getDate())}`
}

// Whole days from `from` (YYYY-MM-DD) to `to`; null without a date.
export function daysBetween(from, to) {
  if (!from) return null
  const a = new Date(`${from}T00:00:00`)
  const b = new Date(`${to}T00:00:00`)
  if (Number.isNaN(a.getTime()) || Number.isNaN(b.getTime())) return null
  return Math.round((b - a) / 86_400_000)
}

// "today", "yesterday", "5 days ago", "3 weeks ago", "never".
export function ago(date, today) {
  const d = daysBetween(date, today)
  if (d === null) return 'never'
  if (d <= 0) return 'today'
  if (d === 1) return 'yesterday'
  if (d < 14) return `${d} days ago`
  if (d < 60) return `${Math.round(d / 7)} weeks ago`
  return `${Math.round(d / 30)} months ago`
}

// Talking, last touched QUIET_DAYS or more ago (or never), not snoozed: the quietest first.
export function needNudge(contacts, today, snoozed = {}) {
  return contacts
    .filter((c) => c.status === 'In discussion' && !snoozed[c.id])
    .map((c) => ({ ...c, quiet: daysBetween(c.lastTouch, today) }))
    .filter((c) => c.quiet === null || c.quiet >= QUIET_DAYS)
    .sort((a, b) => (b.quiet ?? 9999) - (a.quiet ?? 9999))
}

// Meetings from today on, soonest first (Meeting set contacts and any demo date ahead).
export function comingUp(contacts, today) {
  return contacts
    .filter((c) => c.demoDate && c.demoDate >= today)
    .sort((a, b) => a.demoDate.localeCompare(b.demoDate))
}

// The last `count` workdays up to today, oldest first, with their touchpoints.
export function workdays(perDay, today, count = 10) {
  const out = []
  const d = new Date(`${today}T00:00:00`)
  while (out.length < count) {
    const day = d.getDay()
    if (day >= 1 && day <= 5) {
      const key = localDay(d)
      out.unshift({ date: key, day: d.getDate(), n: perDay?.[key] ?? 0, today: key === today })
    }
    d.setDate(d.getDate() - 1)
  }
  return out
}

// The contacts a filter chip shows, newest touch first; `query` matches company, person, sector,
// position and email by every word.
export function filterContacts(contacts, { chip = 'all', query = '' } = {}) {
  const by = {
    all: () => true,
    talking: (c) => c.status === 'In discussion',
    ready: (c) => c.status === 'To contact',
    know: (c) => c.status === 'Profiling',
    meeting: (c) => c.status === 'Meeting scheduled',
    email: (c) => c.channel === CHANNEL.email,
    whatsapp: (c) => c.channel === CHANNEL.whatsapp,
    linkedin: (c) => c.channel === CHANNEL.linkedin,
    missing: (c) => c.status === 'To contact' && c.channel === 'Direct' && !c.email.includes('@'),
  }[chip] ?? (() => true)
  const words = query.trim().toLowerCase().split(/\s+/).filter(Boolean)
  return contacts
    .filter(by)
    .filter((c) => !words.length || words.every((w) => `${c.company} ${c.person} ${c.sector} ${c.position} ${c.email}`.toLowerCase().includes(w)))
    .sort((a, b) => (b.lastTouch || '').localeCompare(a.lastTouch || '') || a.company.localeCompare(b.company))
}

// Initials for an avatar: the person's name, else the company's.
export function initials(c) {
  return (c.person || c.company || '?')
    .split(/\s+/)
    .filter((w) => /[A-Za-z0-9]/.test(w[0] ?? ''))
    .slice(0, 2)
    .map((w) => w[0].toUpperCase())
    .join('') || '?'
}

const HUES = ['#8fb8ff', '#8fe3b8', '#f4d58d', '#c9b3ff', '#ffb8c9', '#9fe1e8']
export function hue(text = '') {
  return HUES[[...text].reduce((a, ch) => a + ch.charCodeAt(0), 0) % HUES.length]
}

// A week from `today`, for a snooze.
export function weekFrom(today) {
  const d = new Date(`${today}T00:00:00`)
  d.setDate(d.getDate() + 7)
  return localDay(d)
}

const who = (c) => `${c.company}${c.person ? ` (${c.person})` : ''}, CRM #${c.id}`

// What the companion is asked: typed into its session, sent only when the member presses Enter.
export const ASKS = {
  reply: (c) => `For ${who(c)}: read our latest mail thread in Outlook and draft a reply in my voice as an Outlook draft. Do not send it.`,
  touch: (c) => `Log a touchpoint for ${who(c)} dated today with cms_log_touchpoint. Ask me what it was before you log it.`,
  next: (c) => `For ${who(c)}: read its account profile and our touchpoints, and suggest the one next step that would move it forward. Do not send anything.`,
  pack: () => `Go through today's action pack with me (emails and WhatsApp), following the action-pack skill. Nothing goes to Mail until I say so.`,
  plan: () => `Build tomorrow's action pack from the CRM, following the action-pack skill. Show me the pack before anything is drafted.`,
  logSends: () => `I've sent today's pack. Log each send as a touchpoint dated today with cms_log_touchpoint, then show me the summary table.`,
  replies: (mails) => `Draft replies in my voice as Outlook drafts for these mails that are waiting on me: ${mails.map((m) => `"${m.subject}" from ${m.from}`).join('; ')}. Do not send them.`,
  nudges: (list) => `These deals have gone quiet for a week or more: ${list.map(who).join('; ')}. Suggest a short follow-up for each; do not send anything.`,
  missing: () => `Find verified inboxes for the Ready to pitch companies that have no email, from their own Contact Us or IR pages only; never guess an address. Update the CRM with cms_update_prospect.`,
  report: () => `Run this week's touchpoint report (report_weekly.py) and show me the summary.`,
}
