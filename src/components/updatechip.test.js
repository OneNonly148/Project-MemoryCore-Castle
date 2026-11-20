// The update chip lives under MEMORYCORE in the top bar (TopBar's brand slot), never fixed over the
// buttons at the right: a fixed chip covered the music toggle and the window buttons (CASTLE-3).
import { describe, expect, it } from 'vitest'
import { readFileSync } from 'node:fs'

const read = (name) => readFileSync(new URL(name, import.meta.url), 'utf8')
const styleOf = (text) => [...text.matchAll(/<style[^>]*>([\s\S]*?)<\/style>/g)].map((m) => m[1]).join('\n')
const chip = read('./UpdateChip.vue')
const bar = read('./TopBar.vue')
const app = read('../App.vue')

describe('update chip under MEMORYCORE', () => {
  it('is not fixed to the window, so it cannot sit on the right-hand buttons', () => {
    expect(styleOf(chip)).not.toMatch(/position:\s*fixed/)
    expect(styleOf(chip)).not.toMatch(/right:\s*\d+px/)
  })

  it('fits in the 48px bar under the name: one line, at most 18px tall', () => {
    const height = Number(styleOf(chip).match(/\.update\s*\{[^}]*height:\s*(\d+)px/)?.[1])
    expect(height).toBeGreaterThan(0)
    expect(height).toBeLessThanOrEqual(18)
    expect(styleOf(chip)).toMatch(/text-overflow:\s*ellipsis/)
  })

  it('TopBar keeps a brand slot under the title, and App puts the chip there', () => {
    expect(bar).toMatch(/class="brand"[\s\S]*class="title"[\s\S]*<slot name="brand" \/>/)
    expect(app).toMatch(/<template #brand>\s*<UpdateChip /)
    expect(app.match(/<UpdateChip /g)).toHaveLength(1)
  })

  it('both kinds are still there: "is out" opens About, "Update now" installs', () => {
    expect(chip).toMatch(/is out/)
    expect(chip).toMatch(/emit\('about'\)/)
    expect(chip).toMatch(/@click="updateNow">Update now/)
  })
})
