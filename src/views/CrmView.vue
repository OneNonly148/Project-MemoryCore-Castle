<script setup>
import { computed, onMounted, ref, watch } from 'vue'
import { chooseFolder, crmContact, crmOpenProfile, crmOverview, crmSetFolder } from '../api/castle.js'
import { ASKS, DAILY, filterContacts, goneQuiet, localDay, starRadius } from './crm/crm.js'

// CRM (this fork): the member's TNEX prospects from the CMS folder, read only. Every change is a
// request typed into the companion's session (emit 'ask'), which the member sends with Enter.
const props = defineProps({
  companion: { type: String, default: 'Your companion' },
  // a contact to open, e.g. from the Mail widget's CRM chip
  focus: { type: String, default: '' },
})
const emit = defineEmits(['ask'])

const data = ref(null)
const problem = ref('')
const loading = ref(true)
const stage = ref('To contact')
const list = ref('all')
const only = ref(null)
const query = ref('')
const shown = ref(60)
const openId = ref('')
const card = ref(null)
const cardProblem = ref('')
const today = localDay()

const contacts = computed(() => data.value?.contacts ?? [])
const rows = computed(() => filterContacts(contacts.value, { stage: stage.value, list: list.value, only: only.value, query: query.value, today }))
const quiet = computed(() => goneQuiet(contacts.value, today))
const open = computed(() => contacts.value.find((c) => c.id === openId.value) ?? null)
const biggest = computed(() => Math.max(0, ...(data.value?.stages ?? []).map(([, n]) => n)))
const channelCount = (name) => (data.value?.channels ?? []).find(([c]) => c === name)?.[1] ?? 0
const sent = computed(() => data.value?.sentToday ?? { email: 0, whatsapp: 0 })
const stageCount = computed(() => {
  if (!stage.value) return contacts.value.length
  return [...(data.value?.stages ?? []), ...(data.value?.parked ?? [])].find(([s]) => s === stage.value)?.[1] ?? 0
})

// the stage path: stars spread along a gentle line
const STAR_X = [70, 260, 470, 720, 930]
const STAR_Y = [70, 52, 58, 70, 52]
const stars = computed(() =>
  (data.value?.stages ?? []).map(([name, count], i) => ({ name, count, x: STAR_X[i], y: STAR_Y[i], r: starRadius(count, biggest.value) })),
)

async function load() {
  loading.value = true
  problem.value = ''
  try {
    data.value = await crmOverview(today)
  } catch (reason) {
    problem.value = String(reason?.message ?? reason)
  }
  loading.value = false
}

async function pickFolder() {
  try {
    const folder = await chooseFolder('Choose your CMS folder (the one with data/prospects.csv)')
    if (!folder) return
    await crmSetFolder(folder)
    await load()
  } catch (reason) {
    problem.value = String(reason?.message ?? reason)
  }
}

function setStage(name) {
  stage.value = stage.value === name ? null : name
  only.value = null
}
function setOnly(name) {
  only.value = only.value === name ? null : name
  if (only.value) stage.value = null
}

async function openContact(id) {
  openId.value = id
  card.value = null
  cardProblem.value = ''
  try {
    card.value = await crmContact(id)
  } catch (reason) {
    cardProblem.value = String(reason?.message ?? reason)
  }
}

async function openProfile() {
  try {
    await crmOpenProfile(card.value.profile)
  } catch (reason) {
    cardProblem.value = String(reason?.message ?? reason)
  }
}

const field = (name) => card.value?.fields?.[name]?.trim() ?? ''
const pillClass = (status) => 's-' + status.toLowerCase().replace(/[^a-z]+/g, '-')

watch([stage, list, only, query], () => (shown.value = 60))
watch(
  () => props.focus,
  async (id) => {
    if (!id) return
    if (!data.value) await load()
    const c = contacts.value.find((x) => x.id === id)
    if (c) {
      stage.value = c.status
      only.value = null
      list.value = 'all'
      query.value = ''
      openContact(id)
    }
  },
)

onMounted(async () => {
  await load()
  if (props.focus) {
    const c = contacts.value.find((x) => x.id === props.focus)
    if (c) {
      stage.value = c.status
      openContact(c.id)
    }
  }
})
</script>

<template>
  <section class="crm-room" aria-label="CRM">
    <header class="top">
      <div>
        <h2>CRM</h2>
        <p class="dim">Your TNEX prospects, read from your CMS folder. Changes go through {{ companion }}.</p>
      </div>
      <div class="actions">
        <button @click="emit('ask', ASKS.report())">Weekly report</button>
        <button class="primary" @click="emit('ask', ASKS.pack())">Build tomorrow's pack</button>
      </div>
    </header>

    <div v-if="problem" class="panel notice">
      <p>{{ problem }}</p>
      <button class="primary" @click="pickFolder">Choose CMS folder</button>
    </div>
    <p v-else-if="loading && !data" class="dim">Reading your CRM…</p>

    <template v-if="data">
      <div class="panel today">
        <div>
          <h3>TODAY</h3>
          <p class="big">{{ sent.email + sent.whatsapp }} of {{ DAILY.email + DAILY.whatsapp }} sends logged</p>
        </div>
        <div class="quota">
          <div class="row"><span>Email</span><span class="dim num">{{ sent.email }} / {{ DAILY.email }}</span></div>
          <div class="meter"><i :style="{ width: `${Math.min(100, (sent.email / DAILY.email) * 100)}%` }"></i></div>
        </div>
        <div class="quota">
          <div class="row"><span>WhatsApp</span><span class="dim num">{{ sent.whatsapp }} / {{ DAILY.whatsapp }}</span></div>
          <div class="meter"><i :style="{ width: `${Math.min(100, (sent.whatsapp / DAILY.whatsapp) * 100)}%` }"></i></div>
        </div>
        <p class="dim small num">{{ data.touchesThisMonth.toLocaleString() }} touchpoints this month</p>
      </div>

      <div class="panel path">
        <p class="dim small">Where every contact stands. Choose a star to list them.</p>
        <svg viewBox="0 0 1000 120" role="group" aria-label="Pipeline stages">
          <path d="M70 70 C 200 30, 300 80, 470 58 S 640 40, 720 70 S 880 78, 930 52" fill="none" class="trail" />
          <g
            v-for="s in stars"
            :key="s.name"
            class="star"
            :class="{ on: stage === s.name && !only }"
            tabindex="0"
            role="button"
            :aria-pressed="stage === s.name && !only"
            :aria-label="`${s.name}: ${s.count}`"
            @click="setStage(s.name)"
            @keydown.enter.prevent="setStage(s.name)"
            @keydown.space.prevent="setStage(s.name)"
          >
            <circle class="halo" :cx="s.x" :cy="s.y" :r="s.r + 16" />
            <circle class="core" :cx="s.x" :cy="s.y" :r="s.r" />
            <text :x="s.x" :y="s.y - s.r - 14" text-anchor="middle" class="count">{{ s.count.toLocaleString() }}</text>
            <text :x="s.x" :y="s.y + s.r + 24" text-anchor="middle">{{ s.name === 'Meeting scheduled' ? 'Meeting' : s.name }}</text>
          </g>
        </svg>
        <div class="parked">
          <span class="dim small">Parked:</span>
          <button v-for="[name, count] in data.parked" :key="name" class="chip" :class="{ on: stage === name && !only }" @click="setStage(name)">
            {{ name }} <span class="num">{{ count.toLocaleString() }}</span>
          </button>
        </div>
      </div>

      <div class="grid">
        <aside class="panel rail" aria-label="Lists">
          <div>
            <h3>LISTS</h3>
            <button class="item" :class="{ on: list === 'all' }" @click="list = 'all'"><span>All contacts</span><span class="num">{{ contacts.length.toLocaleString() }}</span></button>
            <button class="item" :class="{ on: list === 'email' }" @click="list = 'email'"><span>Email (direct)</span><span class="num">{{ channelCount('Direct').toLocaleString() }}</span></button>
            <button class="item" :class="{ on: list === 'whatsapp' }" @click="list = 'whatsapp'"><span>WhatsApp</span><span class="num">{{ channelCount('WhatsApp').toLocaleString() }}</span></button>
            <button class="item" :class="{ on: list === 'linkedin' }" @click="list = 'linkedin'"><span>LinkedIn</span><span class="num">{{ channelCount('LinkedIn').toLocaleString() }}</span></button>
          </div>
          <div>
            <h3>SHOW ONLY</h3>
            <button class="item" :class="{ on: only === 'missing' }" @click="setOnly('missing')"><span>Missing email</span><span class="num">{{ data.missingEmail }}</span></button>
            <button class="item" :class="{ on: only === 'quiet' }" @click="setOnly('quiet')"><span>Gone quiet</span><span class="num">{{ quiet.length }}</span></button>
          </div>
          <div v-if="quiet.length" class="nudge">
            <p>{{ quiet.length }} {{ quiet.length === 1 ? 'deal has' : 'deals have' }} been quiet for a week or more.</p>
            <button class="link" @click="emit('ask', ASKS.quiet(quiet.slice(0, 10)))">Ask {{ companion }} for follow-ups</button>
          </div>
        </aside>

        <section class="panel list" aria-label="Contacts">
          <div class="list-top">
            <strong>{{ only === 'quiet' ? 'Gone quiet' : only === 'missing' ? 'Missing email' : stage ?? 'All stages' }} <span class="dim num">{{ rows.length.toLocaleString() }}</span></strong>
            <input v-model="query" class="search" type="search" placeholder="Search company, person, sector" aria-label="Search contacts" />
          </div>
          <div class="table-wrap">
            <table>
              <thead>
                <tr><th>Company</th><th class="wide">Sector</th><th>Status</th><th class="wide">Last contact</th><th>Via</th></tr>
              </thead>
              <tbody>
                <tr
                  v-for="c in rows.slice(0, shown)"
                  :key="c.id"
                  :class="{ on: c.id === openId }"
                  tabindex="0"
                  @click="openContact(c.id)"
                  @keydown.enter="openContact(c.id)"
                >
                  <td><div class="co">{{ c.company || '(no company)' }}</div><div class="sub">{{ [c.person, c.position].filter(Boolean).join(' · ') }}</div></td>
                  <td class="wide dim">{{ c.sector }}</td>
                  <td><span class="pill" :class="pillClass(c.status)">{{ c.status || '—' }}</span></td>
                  <td class="wide dim num">{{ c.contactDate || '—' }}</td>
                  <td class="dim small">{{ c.channel }}</td>
                </tr>
                <tr v-if="!rows.length"><td colspan="5" class="dim">No contacts here.</td></tr>
              </tbody>
            </table>
          </div>
          <button v-if="rows.length > shown" class="more" @click="shown += 100">Show {{ Math.min(100, rows.length - shown) }} more of {{ (rows.length - shown).toLocaleString() }}</button>
        </section>

        <aside class="panel contact" aria-label="Contact">
          <template v-if="open">
            <div>
              <h3>{{ open.channel.toUpperCase() }} CONTACT</h3>
              <p class="name">{{ open.company }}</p>
            </div>
            <dl>
              <template v-if="open.person"><dt>Person</dt><dd>{{ open.person }}</dd></template>
              <template v-if="open.position"><dt>Position</dt><dd>{{ open.position }}</dd></template>
              <template v-if="open.sector"><dt>Sector</dt><dd>{{ open.sector }}</dd></template>
              <dt>Status</dt><dd><span class="pill" :class="pillClass(open.status)">{{ open.status }}</span></dd>
              <template v-if="open.product"><dt>Product</dt><dd>{{ open.product }}</dd></template>
              <template v-if="open.email"><dt>Email</dt><dd class="sel">{{ open.email }}</dd></template>
              <template v-if="field('phone')"><dt>Phone</dt><dd class="sel">{{ field('phone') }}</dd></template>
            </dl>
            <p v-if="field('talking_points') || field('notes')" class="talk">{{ field('talking_points') || field('notes') }}</p>
            <p v-if="cardProblem" class="warn small">{{ cardProblem }}</p>
            <div>
              <h3>TOUCHPOINTS</h3>
              <p v-if="!card" class="dim small">{{ cardProblem ? '' : 'Reading…' }}</p>
              <ul v-else-if="card.touches.length" class="timeline">
                <li v-for="(t, i) in card.touches.slice(0, 8)" :key="i">
                  <span class="when num">{{ t.date }}</span>
                  <span>{{ t.status }}<span v-if="t.product" class="dim"> · {{ t.product }}</span></span>
                  <span v-if="t.note" class="dim small">{{ t.note }}</span>
                </li>
              </ul>
              <p v-else class="dim small">No touchpoints yet.</p>
            </div>
            <div>
              <h3>ASK {{ companion.toUpperCase() }}</h3>
              <div class="asks">
                <button class="primary" @click="emit('ask', ASKS.reply(open))">Draft a reply<small>To Outlook Drafts</small></button>
                <button @click="emit('ask', ASKS.touch(open))">Log a touchpoint<small>Dated today</small></button>
                <button :disabled="!card?.profile" @click="openProfile">Open profile<small>{{ card?.profile ? 'Accounts folder' : 'No profile yet' }}</small></button>
                <button @click="emit('ask', ASKS.status(open))">Change status<small>Keeps the history</small></button>
              </div>
              <p class="dim small">A request is typed into {{ companion }}'s session on HOME. Press Enter there to send it.</p>
            </div>
          </template>
          <p v-else class="dim">Choose a contact to see its touchpoints and what to do next.</p>
        </aside>
      </div>
    </template>
  </section>
</template>

<style scoped>
.crm-room {
  display: grid;
  gap: 14px;
  align-content: start;
  height: 100%;
  overflow: auto;
  padding: 20px 24px 40px;
  box-sizing: border-box;
}
.top {
  display: flex;
  flex-wrap: wrap;
  gap: 12px 24px;
  align-items: flex-end;
  justify-content: space-between;
}
h2 {
  margin: 0;
  letter-spacing: 0.12em;
}
h3 {
  margin: 0 0 6px;
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.12em;
  color: var(--text-dim);
}
.top p {
  margin: 4px 0 0;
}
.dim {
  color: var(--text-dim);
}
.small {
  font-size: 12px;
}
.warn {
  color: var(--warn);
}
.num {
  font-variant-numeric: tabular-nums;
}
.actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}
button {
  padding: 6px 12px;
  border: 1px solid var(--line);
  border-radius: 8px;
  background: var(--hover);
  color: var(--text);
  font: inherit;
  font-size: 13px;
  cursor: pointer;
}
button:hover {
  border-color: var(--accent);
}
button.primary {
  background: var(--accent);
  border-color: var(--accent);
  color: var(--background, #050b1f);
  font-weight: 600;
}
button:disabled {
  opacity: 0.5;
  cursor: default;
}
button:focus-visible,
[tabindex]:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
}
.panel {
  padding: 14px 16px;
  border: 1px solid color-mix(in srgb, var(--line) 60%, transparent);
  border-radius: 14px;
  background: var(--panel-background);
  -webkit-backdrop-filter: blur(var(--glass-blur));
  backdrop-filter: blur(var(--glass-blur));
}
.notice {
  display: flex;
  flex-wrap: wrap;
  gap: 12px;
  align-items: center;
}
.notice p {
  margin: 0;
}

.today {
  display: grid;
  grid-template-columns: minmax(0, 1.2fr) minmax(0, 1fr) minmax(0, 1fr) auto;
  gap: 16px;
  align-items: center;
}
.today .big {
  margin: 0;
  font-family: var(--serif, Georgia, serif);
  font-size: 18px;
}
.today > p {
  margin: 0;
}
.quota {
  display: grid;
  gap: 6px;
}
.quota .row {
  display: flex;
  justify-content: space-between;
  font-size: 12px;
}
.meter {
  height: 6px;
  border-radius: 3px;
  background: var(--hover);
  overflow: hidden;
}
.meter i {
  display: block;
  height: 100%;
  background: var(--good);
}

.path p {
  margin: 0;
}
.path svg {
  display: block;
  width: 100%;
  height: auto;
}
.trail {
  stroke: var(--line);
  stroke-width: 1.2;
  stroke-dasharray: 3 5;
}
.star {
  cursor: pointer;
  outline: none;
}
.star .halo {
  fill: var(--accent);
  opacity: 0;
  transition: opacity 0.2s;
}
.star:hover .halo,
.star:focus-visible .halo,
.star.on .halo {
  opacity: 0.18;
}
.star .core {
  fill: var(--text);
}
.star.on .core {
  fill: var(--gold, #f4d58d);
}
.star text {
  fill: var(--text-dim);
  font-size: 13px;
}
.star.on text {
  fill: var(--text);
}
.star text.count {
  fill: var(--text);
  font-size: 14px;
  font-variant-numeric: tabular-nums;
}
.parked {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  align-items: center;
}
.chip {
  padding: 2px 10px;
  border-radius: 999px;
  background: none;
  font-size: 12px;
  color: var(--text-dim);
}
.chip.on {
  border-color: var(--accent);
  color: var(--text);
}

.grid {
  display: grid;
  grid-template-columns: 210px minmax(0, 1fr) 340px;
  gap: 14px;
  align-items: start;
}
.rail {
  display: grid;
  gap: 14px;
}
.item {
  display: flex;
  justify-content: space-between;
  gap: 8px;
  width: 100%;
  padding: 6px 8px;
  border: 0;
  border-radius: 8px;
  background: none;
  text-align: left;
}
.item:hover,
.item.on {
  background: var(--hover);
}
.item.on {
  box-shadow: inset 2px 0 0 var(--accent);
  border-radius: 0 8px 8px 0;
}
.item .num {
  color: var(--text-dim);
  font-size: 12px;
}
.nudge {
  padding-top: 12px;
  border-top: 1px solid color-mix(in srgb, var(--line) 60%, transparent);
  font-size: 13px;
}
.nudge p {
  margin: 0 0 6px;
}
.link {
  padding: 0;
  border: 0;
  background: none;
  color: var(--accent);
  text-decoration: underline;
  text-underline-offset: 2px;
}

.list {
  min-width: 0;
  padding: 0;
}
.list-top {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  align-items: center;
  justify-content: space-between;
  padding: 12px 14px;
}
.search {
  width: 240px;
  max-width: 100%;
  min-width: 0;
  padding: 6px 10px;
  border: 1px solid color-mix(in srgb, var(--line) 60%, transparent);
  border-radius: 8px;
  background: rgba(0, 0, 0, 0.25);
  color: var(--text);
  font: inherit;
  font-size: 13px;
}
.table-wrap {
  overflow-x: auto;
}
table {
  width: 100%;
  border-collapse: collapse;
  font-size: 13px;
}
th {
  padding: 8px 14px;
  border-bottom: 1px solid color-mix(in srgb, var(--line) 60%, transparent);
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.08em;
  text-align: left;
  color: var(--text-dim);
  white-space: nowrap;
}
td {
  padding: 9px 14px;
  border-bottom: 1px solid color-mix(in srgb, var(--line) 35%, transparent);
  vertical-align: top;
}
td .co {
  font-weight: 600;
  overflow-wrap: anywhere;
}
td .sub {
  margin-top: 2px;
  font-size: 12px;
  color: var(--text-dim);
}
tbody tr {
  cursor: pointer;
}
tbody tr:hover {
  background: var(--hover);
}
tbody tr.on {
  background: color-mix(in srgb, var(--accent) 12%, transparent);
  box-shadow: inset 2px 0 0 var(--accent);
}
.pill {
  display: inline-block;
  padding: 1px 8px;
  border: 1px solid currentColor;
  border-radius: 999px;
  font-size: 11px;
  white-space: nowrap;
  color: var(--text-dim);
}
.s-profiling {
  color: var(--accent);
}
.s-to-contact {
  color: var(--gold, #f4d58d);
}
.s-in-discussion {
  color: var(--good);
}
.s-meeting-scheduled {
  color: #c9b3ff;
}
.s-on-hold {
  color: var(--warn);
}
.s-not-interested,
.s-bounced,
.s-lost {
  color: var(--bad);
}
.more {
  margin: 10px 14px 14px;
}

.contact {
  display: grid;
  gap: 14px;
  position: sticky;
  top: 0;
  max-height: calc(100vh - 120px);
  overflow: auto;
}
.contact .name {
  margin: 0;
  font-size: 18px;
  font-weight: 600;
  overflow-wrap: anywhere;
}
dl {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr);
  gap: 4px 12px;
  margin: 0;
  font-size: 13px;
}
dt {
  color: var(--text-dim);
}
dd {
  margin: 0;
  overflow-wrap: anywhere;
}
.sel {
  user-select: text;
}
.talk {
  margin: 0;
  padding: 10px 12px;
  border: 1px solid color-mix(in srgb, var(--gold, #f4d58d) 25%, transparent);
  border-radius: 10px;
  background: color-mix(in srgb, var(--gold, #f4d58d) 6%, transparent);
  font-family: var(--serif, Georgia, serif);
  font-size: 14px;
  line-height: 1.5;
  white-space: pre-line;
  overflow-wrap: anywhere;
  max-height: 180px;
  overflow: auto;
}
.timeline {
  display: grid;
  gap: 10px;
  margin: 0;
  padding: 0 0 0 14px;
  border-left: 1px solid var(--line);
  list-style: none;
  font-size: 13px;
}
.timeline li {
  display: grid;
  gap: 2px;
  position: relative;
  overflow-wrap: anywhere;
}
.timeline li::before {
  content: '';
  position: absolute;
  left: -18px;
  top: 5px;
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: var(--accent);
}
.timeline .when {
  font-size: 11px;
  color: var(--text-dim);
}
.asks {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px;
  margin-bottom: 6px;
}
.asks button {
  text-align: left;
}
.asks small {
  display: block;
  margin-top: 2px;
  font-size: 11px;
  font-weight: 400;
  opacity: 0.75;
}
.contact p.small {
  margin: 0;
}

@media (max-width: 1200px) {
  .grid {
    grid-template-columns: 200px minmax(0, 1fr);
  }
  .contact {
    grid-column: 1 / -1;
    position: static;
    max-height: none;
  }
  .today {
    grid-template-columns: 1fr 1fr;
  }
}
@media (max-width: 760px) {
  .grid,
  .today {
    grid-template-columns: 1fr;
  }
  .wide {
    display: none;
  }
}
@media (prefers-reduced-motion: reduce) {
  .star .halo {
    transition: none;
  }
}
</style>
