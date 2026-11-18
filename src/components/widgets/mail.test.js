import { describe, expect, it } from 'vitest'
import { EVERY_MS, dueForSort, sortedLine, workHours } from './mail.js'

const at = (y, m, d, h, min = 0) => new Date(y, m - 1, d, h, min)

describe('mail sorting clock', () => {
  it('works Monday to Friday, 08:00 to 19:00 only', () => {
    expect(workHours(at(2026, 11, 18, 8))).toBe(true) // Wednesday 08:00
    expect(workHours(at(2026, 11, 18, 18, 59))).toBe(true)
    expect(workHours(at(2026, 11, 18, 19))).toBe(false)
    expect(workHours(at(2026, 11, 18, 7, 59))).toBe(false)
    expect(workHours(at(2026, 11, 22, 10))).toBe(false) // Sunday
  })

  it('is due 30 minutes after the last attempt, never while one runs', () => {
    const now = at(2026, 11, 18, 10)
    expect(dueForSort(now, { sortedAt: 0, triedAt: 0 })).toBe(true)
    expect(dueForSort(now, { sortedAt: now.getTime() - EVERY_MS + 1000 })).toBe(false)
    expect(dueForSort(now, { sortedAt: now.getTime() - EVERY_MS })).toBe(true)
    expect(dueForSort(now, { sortedAt: 0, triedAt: now.getTime() - 60_000 }), 'a failed try also waits').toBe(false)
    expect(dueForSort(now, { running: true })).toBe(false)
    expect(dueForSort(at(2026, 11, 22, 10), {})).toBe(false)
  })

  it('says when the mail was sorted', () => {
    const now = at(2026, 11, 18, 10).getTime()
    expect(sortedLine(0, now)).toBe('Not sorted yet')
    expect(sortedLine(at(2026, 11, 18, 9, 40).getTime(), now)).toMatch(/^Sorted 09:40$/)
    expect(sortedLine(at(2026, 11, 17, 17, 30).getTime(), now)).toMatch(/^Sorted yesterday 17:30$/)
  })
})
