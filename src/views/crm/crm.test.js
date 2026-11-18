import { describe, expect, it } from 'vitest'
import { ASKS, ago, comingUp, filterContacts, initials, localDay, needNudge, plain, rung, weekFrom, workdays } from './crm.js'

const c = (id, company, status, extra = {}) => ({ id, company, person: '', position: '', sector: '', status, channel: 'Direct', contactDate: '', email: '', product: '', event: '', lastTouch: '', demoDate: '', notes: '', ...extra })

describe('CRM room', () => {
  const contacts = [
    c('1', 'Perkasa Steel', 'In discussion', { lastTouch: '2026-11-17', email: 'a@perkasa.my', person: 'Aminah Rahman' }),
    c('2', 'Damai Health', 'In discussion', { lastTouch: '2026-11-02' }),
    c('3', 'Sinar Logistik', 'To contact', { sector: 'Transport' }),
    c('4', 'Harum Foods', 'To contact', { channel: 'WhatsApp', email: 'x@harum.my' }),
    c('5', 'Bina Jaya', 'Profiling', { channel: 'WhatsApp', lastTouch: '2026-11-10' }),
    c('6', 'Quiet Co', 'In discussion'),
    c('7', 'Mega Pelita', 'Meeting scheduled', { demoDate: '2026-11-25' }),
  ]

  it('says stages plainly and keeps unknown words', () => {
    expect(plain('To contact')).toBe('Ready to pitch')
    expect(plain('In discussion')).toBe('Talking')
    expect(plain('On hold')).toBe('Paused')
    expect(plain('Something new')).toBe('Something new')
    expect(rung('Contacted')).toBe(2)
    expect(rung('On hold')).toBe(-1)
  })

  it('nudges talks quiet for a week, quietest first, never snoozed ones', () => {
    expect(needNudge(contacts, '2026-11-18').map((x) => x.id)).toEqual(['6', '2'])
    expect(needNudge(contacts, '2026-11-18', { 2: '2026-11-25' }).map((x) => x.id)).toEqual(['6'])
  })

  it('lists meetings ahead, soonest first', () => {
    expect(comingUp(contacts, '2026-11-18').map((x) => x.id)).toEqual(['7'])
    expect(comingUp(contacts, '2026-11-26')).toEqual([])
  })

  it('counts the last ten workdays', () => {
    const days = workdays({ '2026-11-17': 39, '2026-11-16': 97 }, '2026-11-18')
    expect(days).toHaveLength(10)
    expect(days.at(-1)).toMatchObject({ date: '2026-11-18', today: true, n: 0 })
    expect(days.at(-2).n).toBe(39)
    expect(days.some((d) => d.date === '2026-11-15')).toBe(false) // a Sunday
  })

  it('filters by chip and words, newest touch first', () => {
    expect(filterContacts(contacts, { chip: 'talking' }).map((x) => x.id)).toEqual(['1', '2', '6'])
    expect(filterContacts(contacts, { chip: 'missing' }).map((x) => x.id)).toEqual(['3'])
    expect(filterContacts(contacts, { query: 'aminah' }).map((x) => x.id)).toEqual(['1'])
  })

  it('reads time and names plainly', () => {
    expect(ago('', '2026-11-18')).toBe('never')
    expect(ago('2026-11-17', '2026-11-18')).toBe('yesterday')
    expect(ago('2026-11-02', '2026-11-18')).toBe('2 weeks ago')
    expect(initials(contacts[0])).toBe('AR')
    expect(initials(c('9', 'harum foods', 'x'))).toBe('HF')
    expect(weekFrom('2026-11-28')).toBe('2026-12-05')
    expect(localDay(new Date(2026, 0, 5))).toBe('2026-01-05')
  })

  it('never asks the companion to send', () => {
    for (const ask of [ASKS.reply(contacts[0]), ASKS.next(contacts[0]), ASKS.nudges([contacts[1]]), ASKS.replies([{ subject: 'Hi', from: 'A' }])]) {
      expect(ask.toLowerCase()).toMatch(/do not send/)
    }
  })
})
