// The CRM room's pure parts (this fork): filtering the contact list, "gone quiet" deals, the
// stage path's star sizes, and the requests typed to the companion.

export const DAILY = { email: 20, whatsapp: 20 }
export const QUIET_DAYS = 7
const CHANNEL = { email: 'Direct', whatsapp: 'WhatsApp', linkedin: 'LinkedIn' }

// The member's own date as YYYY-MM-DD.
export function localDay(date = new Date()) {
  const p = (n) => String(n).padStart(2, '0')
  return `${date.getFullYear()}-${p(date.getMonth() + 1)}-${p(date.getDate())}`
}

// In discussion, and the last contact QUIET_DAYS or more before `today` (no date counts as quiet).
export function goneQuiet(contacts, today) {
  const limit = new Date(`${today}T00:00:00`)
  limit.setDate(limit.getDate() - QUIET_DAYS)
  const cut = localDay(limit)
  return contacts.filter((c) => c.status === 'In discussion' && (!c.contactDate || c.contactDate <= cut))
}

// The contacts the list shows: one stage (or all), one channel list (or all), an optional "only"
// view, and the search words (company, person, sector, position, email).
export function filterContacts(contacts, { stage = null, list = 'all', only = null, query = '', today = localDay() } = {}) {
  let found = contacts
  if (only === 'quiet') found = goneQuiet(found, today)
  else if (only === 'missing') found = found.filter((c) => c.status === 'To contact' && c.channel === 'Direct' && !c.email.includes('@'))
  if (stage) found = found.filter((c) => c.status === stage)
  if (list !== 'all') found = found.filter((c) => c.channel === CHANNEL[list])
  const words = query.trim().toLowerCase().split(/\s+/).filter(Boolean)
  if (words.length) {
    found = found.filter((c) => {
      const hay = `${c.company} ${c.person} ${c.sector} ${c.position} ${c.email}`.toLowerCase()
      return words.every((w) => hay.includes(w))
    })
  }
  // newest contact first, never-contacted after, then by company
  return [...found].sort((a, b) => (b.contactDate || '').localeCompare(a.contactDate || '') || a.company.localeCompare(b.company))
}

// A star's radius on the stage path: grows with the count's square root, between 4 and 12.
export function starRadius(count, biggest) {
  if (!biggest) return 4
  return Math.round((4 + 8 * Math.sqrt(count / biggest)) * 10) / 10
}

const who = (c) => `${c.company}${c.person ? ` (${c.person})` : ''}, CRM #${c.id}`

// What the companion is asked: typed into its session, sent only when the member presses Enter.
export const ASKS = {
  reply: (c) => `For ${who(c)}: read our latest mail thread in Outlook and draft a reply in my voice as an Outlook draft. Do not send it.`,
  touch: (c) => `Log a touchpoint for ${who(c)} dated today with cms_log_touchpoint. Ask me what it was before you log it.`,
  status: (c) => `For ${who(c)}: its status is "${c.status}". Ask me which status it should move to, then update it with cms_update_prospect.`,
  pack: () => `Build tomorrow's action pack (emails and WhatsApp) from the CRM, following the action-pack skill. Show me the pack before anything goes to Mail.`,
  report: () => `Run this week's touchpoint report (report_weekly.py) and show me the summary.`,
  quiet: (list) => `These In discussion contacts have gone quiet for a week or more: ${list.map(who).join('; ')}. Suggest a follow-up for each; do not send anything.`,
}
