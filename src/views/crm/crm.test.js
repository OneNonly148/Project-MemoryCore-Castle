import { describe, expect, it } from 'vitest'
import { ASKS, filterContacts, goneQuiet, localDay, starRadius } from './crm.js'

const c = (id, company, status, extra = {}) => ({ id, company, person: '', position: '', sector: '', status, channel: 'Direct', contactDate: '', email: '', product: '', event: '', ...extra })

describe('CRM room', () => {
  const contacts = [
    c('1', 'Perkasa Steel', 'In discussion', { contactDate: '2026-11-17', email: 'a@perkasa.my' }),
    c('2', 'Damai Health', 'In discussion', { contactDate: '2026-11-02' }),
    c('3', 'Sinar Logistik', 'To contact', { sector: 'Transport' }),
    c('4', 'Harum Foods', 'To contact', { channel: 'WhatsApp', email: 'x@harum.my' }),
    c('5', 'Bina Jaya', 'Profiling', { channel: 'WhatsApp', contactDate: '2026-11-10' }),
  ]

  it('finds deals gone quiet for a week', () => {
    expect(goneQuiet(contacts, '2026-11-18').map((x) => x.id)).toEqual(['2'])
    expect(goneQuiet(contacts, '2026-11-24').map((x) => x.id)).toEqual(['1', '2'])
  })

  it('filters by stage, list, missing email and words, newest contact first', () => {
    expect(filterContacts(contacts, { stage: 'In discussion' }).map((x) => x.id)).toEqual(['1', '2'])
    expect(filterContacts(contacts, { list: 'whatsapp' }).map((x) => x.id)).toEqual(['5', '4'])
    expect(filterContacts(contacts, { only: 'missing' }).map((x) => x.id)).toEqual(['3'])
    expect(filterContacts(contacts, { query: 'sinar transport' }).map((x) => x.id)).toEqual(['3'])
    expect(filterContacts(contacts, { only: 'quiet', today: '2026-11-18' }).map((x) => x.id)).toEqual(['2'])
  })

  it('sizes stars by the square root of their count', () => {
    expect(starRadius(0, 100)).toBe(4)
    expect(starRadius(100, 100)).toBe(12)
    expect(starRadius(25, 100)).toBe(8)
    expect(starRadius(5, 0)).toBe(4)
  })

  it('never asks the companion to send', () => {
    expect(ASKS.reply(contacts[0])).toContain('Do not send')
    expect(ASKS.reply(contacts[0])).toContain('CRM #1')
    expect(ASKS.quiet([contacts[1]])).toContain('do not send')
  })

  it('writes the local date', () => {
    expect(localDay(new Date(2026, 0, 5))).toBe('2026-01-05')
  })
})
