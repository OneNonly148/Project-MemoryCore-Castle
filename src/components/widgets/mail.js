// The Mail widget's clock (this fork): when the next sort is due, and how the last one reads.

export const EVERY_MS = 30 * 60 * 1000
export const WORK_START = 8 // 08:00
export const WORK_END = 19 // until 19:00

// A workday hour, by the member's own clock: Monday to Friday, 08:00 to 19:00.
export function workHours(date) {
  const day = date.getDay()
  const hour = date.getHours()
  return day >= 1 && day <= 5 && hour >= WORK_START && hour < WORK_END
}

// Due: in work hours, nothing running, and the last attempt (worked or not) 30 minutes ago or more.
export function dueForSort(date, snap = {}) {
  if (snap.running || !workHours(date)) return false
  const last = Math.max(snap.sortedAt || 0, snap.triedAt || 0)
  return date.getTime() - last >= EVERY_MS
}

// "Sorted 09:40", "Sorted yesterday 17:30", "Sorted 14 Nov", or "Not sorted yet".
export function sortedLine(sortedAt, now = Date.now()) {
  if (!sortedAt) return 'Not sorted yet'
  const at = new Date(sortedAt)
  const today = new Date(now)
  const hm = at.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', hourCycle: 'h23' })
  if (at.toDateString() === today.toDateString()) return `Sorted ${hm}`
  const yesterday = new Date(today)
  yesterday.setDate(today.getDate() - 1)
  if (at.toDateString() === yesterday.toDateString()) return `Sorted yesterday ${hm}`
  return `Sorted ${at.toLocaleDateString([], { day: 'numeric', month: 'short' })}`
}
