<script setup>
import { computed, onMounted, onUnmounted, ref, watch } from 'vue'
import { chooseFolder, crmOverview, crmSetFolder, crmSetStatus, crmSnooze, mailSnapshot } from '../api/castle.js'
import CrmSheet from './crm/CrmSheet.vue'
import { ASKS, DAILY, STAGES, ago, comingUp, daysBetween, filterContacts, hue, initials, localDay, needNudge, plain, weekFrom, workdays } from './crm/crm.js'

// CRM (this fork): Wan's TNEX prospects as a morning desk - Today, Deals, Outreach, Contacts -
// read from the CMS folder. Moving a stage (a drag or the contact sheet) writes prospects.csv,
// backed up first; replies, logging and packs are typed to the companion (emit 'ask'), which the
// member sends with Enter.
const props = defineProps({
  companion: { type: String, default: 'Your companion' },
  focus: { type: String, default: '' }, // a contact to open (the Mail widget's CRM chip)
})
const emit = defineEmits(['ask'])

const today = localDay()
const data = ref(null)
const mail = ref(null)
const problem = ref('')
const view = ref('today')
const chip = ref('all')
const query = ref('')
const find = ref('')
const findOpen = ref(false)
const findInput = ref(null)
const waitingEl = ref(null)
function closeFind() {
  setTimeout(() => (findOpen.value = false), 150)
}
const shown = ref(60)
const openId = ref('')
const toast = ref(null) // { text, undo }
const dragId = ref('')
const overCol = ref('')
let toastTimer = null

const contacts = computed(() => data.value?.contacts ?? [])
const byId = computed(() => Object.fromEntries(contacts.value.map((c) => [c.id, c])))
const open = computed(() => byId.value[openId.value] ?? null)
const snoozed = computed(() => data.value?.snoozed ?? {})
const sent = computed(() => data.value?.sentToday ?? { email: 0, whatsapp: 0 })
const count = (status) => [...(data.value?.stages ?? []), ...(data.value?.parked ?? [])].find(([s]) => s === status)?.[1] ?? 0

// Today
const waiting = computed(() => (mail.value?.items ?? []).filter((m) => m.needsReply))
const nudges = computed(() => needNudge(contacts.value, today, snoozed.value))
const meetings = computed(() => comingUp(contacts.value, today))
const days = computed(() => workdays(data.value?.perDay, today, 10))
const weekTotal = computed(() => days.value.slice(-5).reduce((a, d) => a + d.n, 0))
const onTarget = computed(() => days.value.slice(-5).filter((d) => !d.today && d.n >= DAILY.email + DAILY.whatsapp).length)
const briefing = computed(() => {
  const parts = []
  const w = waiting.value.length
  if (w) parts.push(`${w} ${w === 1 ? 'person is' : 'people are'} waiting on a reply`)
  const n = nudges.value.length
  if (n) parts.push(`${n} ${n === 1 ? 'deal has' : 'deals have'} gone a week without a word from you`)
  const s = sent.value.email + sent.value.whatsapp
  const target = DAILY.email + DAILY.whatsapp
  parts.push(s ? `${s} of today's ${target} sends are logged` : `nothing from today's pack is logged yet`)
  const line = parts.join(', ')
  return line.charAt(0).toUpperCase() + line.slice(1) + '.'
})
const dayName = new Date().toLocaleDateString('en-GB', { weekday: 'long' }).toUpperCase()

// Deals: the live conversations, and the paused ones touched in the last 90 days
const COLUMNS = [
  { status: 'In discussion', title: 'Talking' },
  { status: 'Meeting scheduled', title: 'Meeting set' },
  { status: 'On hold', title: 'Paused', hint: 'Touched in the last 90 days' },
]
const columns = computed(() =>
  COLUMNS.map((col) => ({
    ...col,
    cards: contacts.value
      .filter((c) => c.status === col.status && (col.status !== 'On hold' || (daysBetween(c.lastTouch, today) ?? 999) <= 90))
      .map((c) => ({ ...c, quiet: daysBetween(c.lastTouch, today) }))
      .sort((a, b) => (b.quiet ?? 9999) - (a.quiet ?? 9999)),
  })),
)

// Outreach
const journey = computed(() => {
  const max = Math.sqrt(Math.max(1, ...STAGES.map((s) => count(s.csv))))
  return STAGES.map((s) => ({ ...s, n: count(s.csv), width: (Math.sqrt(count(s.csv)) / max) * 100 }))
})
const rate = (a, b) => (b ? `${((a / b) * 100).toFixed(1).replace(/\.0$/, '')}%` : '-')
const liveSectors = computed(() => {
  const m = {}
  for (const c of contacts.value.filter((c) => ['In discussion', 'Meeting scheduled'].includes(c.status))) m[c.sector || 'Other'] = (m[c.sector || 'Other'] ?? 0) + 1
  const rows = Object.entries(m).sort((a, b) => b[1] - a[1]).slice(0, 6)
  const top = rows[0]?.[1] ?? 1
  return rows.map(([name, n]) => ({ name, n, width: (n / top) * 100 }))
})

// Contacts
const people = computed(() => filterContacts(contacts.value, { chip: chip.value, query: query.value }))
const CHIPS = computed(() => [
  ['all', 'Everyone', contacts.value.length],
  ['talking', 'Talking', count('In discussion')],
  ['meeting', 'Meeting set', count('Meeting scheduled')],
  ['ready', 'Ready to pitch', count('To contact')],
  ['know', 'Getting to know', count('Profiling')],
  ['missing', 'No email yet', data.value?.missingEmail ?? 0],
  ['email', 'By email', null],
  ['whatsapp', 'By WhatsApp', null],
  ['linkedin', 'On LinkedIn', null],
])
const findHits = computed(() => (find.value.trim() ? filterContacts(contacts.value, { query: find.value }).slice(0, 7) : []))

async function load() {
  problem.value = ''
  try {
    data.value = await crmOverview(today)
  } catch (reason) {
    problem.value = String(reason?.message ?? reason)
  }
  try {
    mail.value = await mailSnapshot()
  } catch {
    mail.value = null
  }
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

function say(text, undo = null) {
  toast.value = { text, undo }
  clearTimeout(toastTimer)
  toastTimer = setTimeout(() => (toast.value = null), 7000)
}

async function move(contact, status, quiet = false) {
  const was = contact.status
  try {
    await crmSetStatus(contact.id, status)
    await load()
    if (!quiet) say(`${contact.company} is now ${plain(status)}.`, () => move({ ...contact, status }, was, true))
  } catch (reason) {
    say(String(reason?.message ?? reason))
  }
}

async function snooze(contact) {
  try {
    await crmSnooze(contact.id, weekFrom(today))
    await load()
    say(`${contact.company} is snoozed for a week.`, async () => {
      await crmSnooze(contact.id, '')
      await load()
    })
  } catch (reason) {
    say(String(reason?.message ?? reason))
  }
}

function openContact(id) {
  openId.value = id
  findOpen.value = false
  find.value = ''
}
const mailsOf = (c) => (mail.value?.items ?? []).filter((m) => m.crm?.id === c.id)

// drag a deal card to another column
function dragStart(e, id) {
  dragId.value = id
  e.dataTransfer.effectAllowed = 'move'
  e.dataTransfer.setData('text/plain', id)
}
function drop(status) {
  const c = byId.value[dragId.value]
  overCol.value = ''
  dragId.value = ''
  if (c && c.status !== status) move(c, status)
}

function onKey(e) {
  if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'k') {
    e.preventDefault()
    findInput.value?.focus()
  }
}

watch([chip, query], () => (shown.value = 60))
watch(
  () => props.focus,
  async (id) => {
    if (!id) return
    if (!data.value) await load()
    if (byId.value[id]) openContact(id)
  },
)
onMounted(async () => {
  await load()
  if (props.focus && byId.value[props.focus]) openContact(props.focus)
  window.addEventListener('keydown', onKey)
})
onUnmounted(() => {
  window.removeEventListener('keydown', onKey)
  clearTimeout(toastTimer)
})
</script>

<template>
  <section class="crm-room" aria-label="CRM">
    <header class="head">
      <h2>CRM</h2>
      <div class="views" role="tablist" aria-label="CRM views">
        <button v-for="[id, label] in [['today', 'Today'], ['deals', 'Deals'], ['outreach', 'Outreach'], ['people', 'Contacts']]" :key="id" role="tab" :aria-selected="view === id" @click="view = id">
          {{ label }}<span v-if="id === 'today' && (waiting.length + nudges.length)" class="badge">{{ waiting.length + nudges.length }}</span>
        </button>
      </div>
      <div class="find">
        <input ref="findInput" v-model="find" type="search" placeholder="Find a company or person" aria-label="Find a company or person" autocomplete="off" @focus="findOpen = true" @blur="closeFind" @keydown.esc="find = ''" />
        <kbd aria-hidden="true">⌘K</kbd>
        <div v-if="findOpen && find.trim()" class="results">
          <button v-for="c in findHits" :key="c.id" @mousedown.prevent="openContact(c.id)">
            <span class="avatar" :style="{ background: hue(c.company) }" aria-hidden="true">{{ initials(c) }}</span>
            <span class="r-main"><b>{{ c.person || c.company }}</b><span class="dim small">{{ c.person ? c.company : c.sector }}</span></span>
            <span class="stage" :class="`st-${c.status.replace(/\W+/g, '-').toLowerCase()}`">{{ plain(c.status) }}</span>
          </button>
          <p v-if="!findHits.length" class="dim small pad">No one matches.</p>
        </div>
      </div>
    </header>

    <div v-if="problem" class="panel notice">
      <p>{{ problem }}</p>
      <button class="btn primary" @click="pickFolder">Choose CMS folder</button>
    </div>
    <p v-else-if="!data" class="dim">Reading your CRM…</p>

    <template v-if="data">
      <!-- TODAY -->
      <div v-show="view === 'today'" class="today">
        <div class="main-col">
          <div class="panel brief">
            <div class="by">{{ companion.toUpperCase() }} · {{ dayName }}</div>
            <p>{{ briefing }}</p>
            <div class="acts">
              <button class="btn primary" @click="emit('ask', ASKS.pack())">Go through today's pack</button>
              <button v-if="waiting.length" class="btn" @click="emit('ask', ASKS.replies(waiting))">Draft the {{ waiting.length }} {{ waiting.length === 1 ? 'reply' : 'replies' }}</button>
              <button v-if="nudges.length" class="btn quiet" @click="view = 'deals'">See the quiet deals</button>
            </div>
          </div>

          <div class="tiles">
            <button class="tile" :class="{ alert: waiting.length }" @click="waitingEl?.scrollIntoView({ behavior: 'smooth', block: 'center' })">
              <span class="n">{{ waiting.length }}</span><span class="l">Waiting on you</span><span class="s">{{ mail?.sortedAt ? 'From your Outlook' : 'Mail not sorted yet' }}</span>
            </button>
            <button class="tile" :class="{ alert: nudges.length }" @click="view = 'deals'">
              <span class="n">{{ nudges.length }}</span><span class="l">Need a nudge</span><span class="s">Quiet {{ 7 }}+ days</span>
            </button>
            <button class="tile" @click="view = 'outreach'">
              <span class="n">{{ sent.email + sent.whatsapp }}<span class="of"> / {{ DAILY.email + DAILY.whatsapp }}</span></span><span class="l">Sent today</span><span class="s">Logged touchpoints</span>
            </button>
            <button class="tile go" @click="chip = 'meeting'; view = 'people'">
              <span class="n">{{ count('Meeting scheduled') }}</span><span class="l">Meetings set</span><span class="s">{{ meetings[0] ? `Next: ${meetings[0].demoDate}` : 'None dated yet' }}</span>
            </button>
          </div>

          <div ref="waitingEl" class="panel">
            <h3>WAITING ON YOU <span class="dim small">from your Outlook</span></h3>
            <div v-if="waiting.length" class="rows">
              <div v-for="m in waiting" :key="m.id" class="row" :tabindex="m.crm ? 0 : -1" @click="m.crm && openContact(m.crm.id)" @keydown.enter="m.crm && openContact(m.crm.id)">
                <span class="avatar" :style="{ background: hue(m.from) }" aria-hidden="true">{{ initials({ person: m.from }) }}</span>
                <div class="txt">
                  <div class="t">{{ m.from }}<span v-if="m.crm" class="dim normal"> · {{ m.crm.company }}</span></div>
                  <div class="d">{{ m.summary || m.subject }}</div>
                </div>
                <span v-if="m.crm" class="stage" :class="`st-${m.crm.status.replace(/\W+/g, '-').toLowerCase()}`">{{ plain(m.crm.status) }}</span>
              </div>
            </div>
            <p v-else class="dim small">{{ mail?.sortedAt ? 'Nobody is waiting on you.' : 'The Mail widget on HOME has not sorted your mail yet.' }}</p>
          </div>

          <div class="panel">
            <h3>NEED A NUDGE <button v-if="nudges.length" class="link small" @click="emit('ask', ASKS.nudges(nudges.slice(0, 10)))">Ask {{ companion }} for follow-ups</button></h3>
            <div v-if="nudges.length" class="rows">
              <div v-for="c in nudges.slice(0, 6)" :key="c.id" class="row" tabindex="0" @click="openContact(c.id)" @keydown.enter="openContact(c.id)">
                <span class="avatar" :style="{ background: hue(c.company) }" aria-hidden="true">{{ initials(c) }}</span>
                <div class="txt">
                  <div class="t">{{ c.company }}</div>
                  <div class="d">{{ [c.person, c.product].filter(Boolean).join(' · ') || c.sector }}</div>
                </div>
                <div class="side">
                  <span class="chip" :class="c.quiet === null || c.quiet >= 14 ? 'hot' : 'warm'">{{ c.quiet === null ? 'no touch logged' : `${c.quiet} days quiet` }}</span>
                  <button class="btn sm quiet" @click.stop="snooze(c)">Snooze</button>
                </div>
              </div>
            </div>
            <p v-else class="dim small">Every live deal has heard from you this week.</p>
          </div>
        </div>

        <aside class="side-col">
          <div class="panel">
            <h3>YOUR LAST TWO WEEKS <span class="dim small">{{ data.touchesThisMonth }} this month</span></h3>
            <div class="bars" role="img" :aria-label="`Touchpoints per workday, last 10: ${days.map((d) => d.n).join(', ')}`">
              <div class="target" :style="{ bottom: `${(40 / Math.max(100, ...days.map((d) => d.n))) * 100}%` }"><span>40 a day</span></div>
              <div v-for="d in days" :key="d.date" class="b" :class="{ zero: !d.n, low: d.n && d.n < 40, now: d.today }" :style="{ height: `${(d.n / Math.max(100, ...days.map((x) => x.n))) * 100}%` }" :title="`${d.date}: ${d.n}`"></div>
            </div>
            <div class="days"><span v-for="d in days" :key="d.date">{{ d.day }}</span></div>
            <p class="split"><span class="dim">This week</span><span>{{ weekTotal }} touchpoints · {{ onTarget }} day{{ onTarget === 1 ? '' : 's' }} on target</span></p>
          </div>
          <div class="panel">
            <h3>COMING UP</h3>
            <div v-for="c in meetings.slice(0, 4)" :key="c.id" class="meet" tabindex="0" @click="openContact(c.id)" @keydown.enter="openContact(c.id)">
              <div class="date"><span>{{ new Date(`${c.demoDate}T00:00:00`).toLocaleDateString('en-GB', { weekday: 'short' }).toUpperCase() }}</span><b>{{ Number(c.demoDate.slice(8)) }}</b></div>
              <div><b>{{ c.company }}</b><div class="dim small">{{ [c.person, c.product].filter(Boolean).join(' · ') }}</div></div>
            </div>
            <p v-if="!meetings.length" class="dim small">No demo dates ahead in the CRM.</p>
          </div>
          <div v-if="data.missingEmail" class="panel">
            <h3>WORTH A LOOK</h3>
            <div class="health">
              <span>{{ data.missingEmail }} ready-to-pitch {{ data.missingEmail === 1 ? 'company has' : 'companies have' }} no email yet</span>
              <button class="btn sm" @click="emit('ask', ASKS.missing())">Find them</button>
            </div>
          </div>
        </aside>
      </div>

      <!-- DEALS -->
      <div v-show="view === 'deals'">
        <p class="dim lead">Live conversations. Drag a card to move it; Castle updates prospects.csv and keeps a backup first. The bar fills as a deal goes quiet.</p>
        <div class="board">
          <div v-for="col in columns" :key="col.status" class="panel col" :class="{ over: overCol === col.status }" @dragover.prevent="overCol = col.status" @dragleave="overCol = ''" @drop.prevent="drop(col.status)">
            <div class="col-h"><b>{{ col.title }}</b><span class="dim small">{{ col.cards.length }}{{ col.hint ? ` · ${col.hint}` : '' }}</span></div>
            <div
              v-for="c in col.cards"
              :key="c.id"
              class="deal"
              :class="{ dragging: dragId === c.id }"
              draggable="true"
              tabindex="0"
              @dragstart="dragStart($event, c.id)"
              @dragend="dragId = ''; overCol = ''"
              @click="openContact(c.id)"
              @keydown.enter="openContact(c.id)"
            >
              <div class="top">
                <span class="avatar" :style="{ background: hue(c.company) }" aria-hidden="true">{{ initials(c) }}</span>
                <div class="txt"><div class="co">{{ c.company }}</div><div class="dim small">{{ [c.person, c.position].filter(Boolean).join(' · ') || c.sector }}</div></div>
              </div>
              <p v-if="c.notes" class="next">{{ c.notes }}</p>
              <div class="age" :title="c.quiet === null ? 'No touch logged' : `${c.quiet} days since the last touch`">
                <i :style="{ width: `${Math.min(100, ((c.quiet ?? 14) / 14) * 100)}%`, background: (c.quiet ?? 99) >= 10 ? 'var(--bad)' : (c.quiet ?? 0) >= 7 ? 'var(--warn)' : 'var(--good)' }"></i>
              </div>
              <div class="foot"><span v-if="c.product" class="chip">{{ c.product }}</span><span class="dim small">{{ ago(c.lastTouch, today) }}</span></div>
            </div>
            <p v-if="!col.cards.length" class="drop">Drop a deal here.</p>
          </div>
        </div>
      </div>

      <!-- OUTREACH -->
      <div v-show="view === 'outreach'" class="outreach">
        <div class="panel">
          <h3>HOW FAR EVERYONE HAS COME</h3>
          <div class="journey">
            <button v-for="s in journey" :key="s.csv" class="step" @click="chip = { know: 'know', ready: 'ready', talking: 'talking', meeting: 'meeting' }[s.key] ?? 'all'; view = 'people'">
              <span class="lab">{{ s.name }}<small>{{ s.csv }}</small></span>
              <span class="track"><i :class="`st-bg-${s.key}`" :style="{ width: `${s.width}%` }"></i></span>
              <span class="v">{{ s.n.toLocaleString() }}</span>
            </button>
          </div>
          <div class="rates">
            <div><b>{{ rate(count('In discussion') + count('Meeting scheduled'), count('Contacted') + count('In discussion') + count('Meeting scheduled')) }}</b><span class="dim">of people reached are talking</span></div>
            <div><b>{{ rate(count('Meeting scheduled'), count('In discussion') + count('Meeting scheduled')) }}</b><span class="dim">of talks have a meeting set</span></div>
          </div>
          <p class="dim small">Off the journey: {{ (data.parked ?? []).map(([s, n]) => `${n.toLocaleString()} ${plain(s).toLowerCase()}`).join(', ') }}.</p>
        </div>
        <div class="side-col">
          <div class="panel">
            <h3>TODAY'S PACK</h3>
            <p class="split"><span>Email</span><span class="dim">{{ sent.email }} of {{ DAILY.email }} logged</span></p>
            <div class="meter"><i :style="{ width: `${Math.min(100, (sent.email / DAILY.email) * 100)}%` }"></i></div>
            <p class="split"><span>WhatsApp</span><span class="dim">{{ sent.whatsapp }} of {{ DAILY.whatsapp }} logged</span></p>
            <div class="meter"><i :style="{ width: `${Math.min(100, (sent.whatsapp / DAILY.whatsapp) * 100)}%` }"></i></div>
            <div class="acts">
              <button class="btn primary" @click="emit('ask', ASKS.pack())">Open the pack</button>
              <button class="btn" @click="emit('ask', ASKS.logSends())">I've sent them</button>
              <button class="btn quiet" @click="emit('ask', ASKS.plan())">Plan tomorrow</button>
              <button class="btn quiet" @click="emit('ask', ASKS.report())">Weekly report</button>
            </div>
          </div>
          <div class="panel">
            <h3>WHERE THE TALKS ARE</h3>
            <div class="journey">
              <div v-for="s in liveSectors" :key="s.name" class="step static">
                <span class="lab">{{ s.name }}</span><span class="track thin"><i class="st-bg-talking" :style="{ width: `${s.width}%` }"></i></span><span class="v">{{ s.n }}</span>
              </div>
            </div>
            <p class="dim small">Talking and Meeting set, by sector.</p>
          </div>
        </div>
      </div>

      <!-- CONTACTS -->
      <div v-show="view === 'people'">
        <div class="filters">
          <button v-for="[id, label, n] in CHIPS" :key="id" class="fchip" :aria-pressed="chip === id" @click="chip = id">{{ label }}<span v-if="n !== null" class="dim"> {{ n.toLocaleString() }}</span></button>
          <input v-model="query" class="search" type="search" placeholder="Filter these" aria-label="Filter contacts" />
        </div>
        <div class="people">
          <button v-for="c in people.slice(0, shown)" :key="c.id" class="person" @click="openContact(c.id)">
            <span class="avatar" :style="{ background: hue(c.company) }" aria-hidden="true">{{ initials(c) }}</span>
            <span class="txt"><span class="t">{{ c.person || c.company }}</span><span class="d">{{ c.person ? [c.position, c.company].filter(Boolean).join(' · ') : c.sector }}</span></span>
            <span class="meta"><span class="stage" :class="`st-${c.status.replace(/\W+/g, '-').toLowerCase()}`">{{ plain(c.status) }}</span><span class="dim">{{ c.channel === 'Direct' ? 'Email' : c.channel }} · {{ c.lastTouch ? ago(c.lastTouch, today) : 'not contacted' }}</span></span>
          </button>
        </div>
        <p v-if="!people.length" class="dim">No one here.</p>
        <button v-if="people.length > shown" class="btn more" @click="shown += 120">Show more ({{ (people.length - shown).toLocaleString() }} left)</button>
      </div>
    </template>

    <CrmSheet v-if="open" :contact="open" :today="today" :companion="companion" :mails="mailsOf(open)" @close="openId = ''" @ask="emit('ask', $event)" @move="move" @snooze="snooze" />

    <div v-if="toast" class="toast" role="status" aria-live="polite">
      <span>{{ toast.text }}</span>
      <button v-if="toast.undo" class="link" @click="toast.undo(); toast = null">Undo</button>
    </div>
  </section>
</template>

<style scoped>
.crm-room {
  display: grid;
  gap: 16px;
  align-content: start;
  height: 100%;
  overflow: auto;
  padding: 18px 24px 48px;
  box-sizing: border-box;
}
.head {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 12px 20px;
}
h2 {
  margin: 0;
  letter-spacing: 0.12em;
}
h3 {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  gap: 8px;
  margin: 0 0 10px;
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.12em;
  color: var(--text-dim);
}
h3 .small,
h3 .link {
  letter-spacing: 0;
  font-weight: 400;
}
.dim {
  color: var(--text-dim);
}
.small {
  font-size: 12px;
}
.normal {
  font-weight: 400;
}
.lead {
  margin: 0 0 12px;
}
.views {
  display: inline-flex;
  gap: 2px;
  padding: 3px;
  border: 1px solid color-mix(in srgb, var(--line) 50%, transparent);
  border-radius: 11px;
  background: rgba(0, 0, 0, 0.18);
}
.views button {
  padding: 6px 14px;
  border: 0;
  border-radius: 8px;
  background: none;
  color: var(--text-dim);
  font: inherit;
  font-size: 13px;
  cursor: pointer;
}
.views button[aria-selected='true'] {
  background: var(--hover);
  color: var(--text);
}
.badge {
  margin-left: 6px;
  font-size: 11px;
  color: var(--gold, #f4d58d);
}
.find {
  position: relative;
  flex: 0 1 320px;
  min-width: 0;
  margin-left: auto;
}
.find input,
.search {
  width: 100%;
  box-sizing: border-box;
  padding: 8px 12px;
  border: 1px solid color-mix(in srgb, var(--line) 50%, transparent);
  border-radius: 10px;
  background: rgba(0, 0, 0, 0.28);
  color: var(--text);
  font: inherit;
  font-size: 13px;
}
.find kbd {
  position: absolute;
  right: 9px;
  top: 8px;
  padding: 0 5px;
  border: 1px solid color-mix(in srgb, var(--line) 50%, transparent);
  border-radius: 5px;
  font-size: 11px;
  color: var(--text-dim);
}
.results {
  position: absolute;
  z-index: 30;
  top: 42px;
  left: 0;
  right: 0;
  padding: 6px;
  border: 1px solid var(--line);
  border-radius: 12px;
  background: color-mix(in srgb, var(--background, #050b1f) 92%, var(--accent) 3%);
  box-shadow: 0 16px 40px rgba(0, 0, 0, 0.45);
}
.results button {
  display: flex;
  gap: 10px;
  align-items: center;
  width: 100%;
  padding: 7px 8px;
  border: 0;
  border-radius: 8px;
  background: none;
  color: var(--text);
  font: inherit;
  text-align: left;
  cursor: pointer;
}
.results button:hover {
  background: var(--hover);
}
.r-main {
  display: grid;
  min-width: 0;
}
.pad {
  padding: 8px;
  margin: 0;
}
.panel {
  padding: 16px 18px;
  border: 1px solid color-mix(in srgb, var(--line) 50%, transparent);
  border-radius: 16px;
  background: var(--panel-background);
  -webkit-backdrop-filter: blur(var(--glass-blur));
  backdrop-filter: blur(var(--glass-blur));
  min-width: 0;
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
.btn {
  padding: 7px 12px;
  border: 1px solid var(--line);
  border-radius: 9px;
  background: var(--hover);
  color: var(--text);
  font: inherit;
  font-size: 13px;
  white-space: nowrap;
  cursor: pointer;
}
.btn:hover {
  border-color: var(--accent);
}
.btn.primary {
  background: var(--accent);
  border-color: var(--accent);
  color: var(--background, #050b1f);
  font-weight: 600;
}
.btn.quiet {
  background: none;
  border-color: transparent;
  color: var(--text-dim);
}
.btn.sm {
  padding: 4px 9px;
  font-size: 12px;
}
.link {
  padding: 0;
  border: 0;
  background: none;
  color: var(--accent);
  font: inherit;
  text-decoration: underline;
  text-underline-offset: 2px;
  cursor: pointer;
}
button:focus-visible,
[tabindex]:focus-visible,
input:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
}
.avatar {
  flex: none;
  display: grid;
  place-items: center;
  width: 34px;
  height: 34px;
  border-radius: 50%;
  font-size: 12px;
  font-weight: 600;
  color: #07112c;
}
.stage {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-size: 12px;
  white-space: nowrap;
  color: var(--text-dim);
}
.stage::before {
  content: '';
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: currentColor;
}
.st-profiling {
  color: var(--accent);
}
.st-to-contact {
  color: var(--gold, #f4d58d);
}
.st-in-discussion {
  color: var(--good);
}
.st-meeting-scheduled {
  color: #c9b3ff;
}
.st-on-hold {
  color: var(--warn);
}
.st-not-interested,
.st-bounced,
.st-lost {
  color: var(--bad);
}
.chip {
  padding: 1px 7px;
  border-radius: 6px;
  font-size: 11.5px;
  white-space: nowrap;
  background: color-mix(in srgb, var(--accent) 12%, transparent);
  color: var(--accent);
}
.chip.warm {
  background: color-mix(in srgb, var(--warn) 14%, transparent);
  color: var(--warn);
}
.chip.hot {
  background: color-mix(in srgb, var(--bad) 14%, transparent);
  color: var(--bad);
}

/* today */
.today {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 360px;
  gap: 16px;
  align-items: start;
}
.main-col,
.side-col {
  display: grid;
  gap: 16px;
  min-width: 0;
}
.brief {
  display: grid;
  gap: 12px;
}
.brief .by {
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.12em;
  color: var(--gold, #f4d58d);
}
.brief p {
  margin: 0;
  max-width: 62ch;
  font-family: var(--serif, Georgia, serif);
  font-size: 19px;
  line-height: 1.45;
}
.acts {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}
.tiles {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 12px;
}
.tile {
  display: grid;
  gap: 4px;
  padding: 14px 16px;
  border: 1px solid color-mix(in srgb, var(--line) 50%, transparent);
  border-radius: 14px;
  background: var(--panel-background);
  color: var(--text);
  font: inherit;
  text-align: left;
  cursor: pointer;
}
.tile:hover {
  border-color: var(--line);
}
.tile .n {
  font-size: 28px;
  font-weight: 300;
  line-height: 1.1;
  font-variant-numeric: tabular-nums;
}
.tile .of {
  font-size: 15px;
  color: var(--text-dim);
}
.tile .l {
  font-size: 13px;
}
.tile .s {
  font-size: 12px;
  color: var(--text-dim);
}
.tile.alert .n {
  color: var(--warn);
}
.tile.go .n {
  color: var(--good);
}
.rows {
  display: grid;
}
.row {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto;
  gap: 12px;
  align-items: center;
  padding: 10px 6px;
  border-top: 1px solid color-mix(in srgb, var(--line) 35%, transparent);
  border-radius: 8px;
  cursor: pointer;
}
.row:first-child {
  border-top: 0;
}
.row:hover {
  background: var(--hover);
}
.txt {
  display: grid;
  min-width: 0;
}
.t {
  font-weight: 600;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}
.d {
  font-size: 13px;
  color: var(--text-dim);
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}
.side {
  display: flex;
  gap: 6px;
  align-items: center;
}
.bars {
  position: relative;
  display: grid;
  grid-template-columns: repeat(10, 1fr);
  gap: 6px;
  align-items: end;
  height: 120px;
}
.bars .b {
  border-radius: 4px 4px 0 0;
  background: var(--accent);
  opacity: 0.85;
}
.bars .b.low {
  background: var(--warn);
}
.bars .b.zero {
  height: 3px !important;
  background: color-mix(in srgb, var(--line) 50%, transparent);
}
.bars .b.now {
  outline: 1px dashed var(--gold, #f4d58d);
  outline-offset: 2px;
}
.target {
  position: absolute;
  left: 0;
  right: 0;
  border-top: 1px dashed var(--gold, #f4d58d);
  opacity: 0.7;
}
.target span {
  position: absolute;
  right: 0;
  top: -16px;
  font-size: 11px;
  color: var(--gold, #f4d58d);
}
.days {
  display: grid;
  grid-template-columns: repeat(10, 1fr);
  gap: 6px;
  margin-top: 6px;
  font-size: 11px;
  text-align: center;
  color: var(--text-dim);
}
.split {
  display: flex;
  justify-content: space-between;
  gap: 8px;
  margin: 10px 0 6px;
  font-size: 13px;
}
.meet {
  display: grid;
  grid-template-columns: 54px minmax(0, 1fr);
  gap: 12px;
  padding: 10px 0;
  border-top: 1px solid color-mix(in srgb, var(--line) 35%, transparent);
  cursor: pointer;
}
.meet:first-of-type {
  border-top: 0;
}
.date {
  padding: 4px 0;
  border: 1px solid var(--line);
  border-radius: 10px;
  text-align: center;
}
.date span {
  font-size: 10px;
  letter-spacing: 0.1em;
  color: var(--text-dim);
}
.date b {
  display: block;
  font-size: 18px;
  font-weight: 500;
}
.health {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 10px;
  font-size: 13px;
}

/* deals */
.board {
  display: grid;
  grid-template-columns: repeat(3, minmax(260px, 1fr));
  gap: 14px;
  overflow-x: auto;
  padding-bottom: 6px;
}
.col {
  display: grid;
  gap: 10px;
  align-content: start;
  min-height: 300px;
  max-height: calc(100vh - 230px);
  overflow-y: auto;
}
.col.over {
  border-color: var(--accent);
  background: color-mix(in srgb, var(--accent) 8%, var(--panel-background));
}
.col-h {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  gap: 8px;
}
.deal {
  display: grid;
  gap: 8px;
  padding: 12px;
  border: 1px solid color-mix(in srgb, var(--line) 40%, transparent);
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.035);
  cursor: grab;
}
.deal:hover {
  border-color: var(--line);
}
.deal.dragging {
  opacity: 0.45;
}
.deal .top {
  display: flex;
  gap: 10px;
  align-items: center;
  min-width: 0;
}
.co {
  font-weight: 600;
  overflow-wrap: anywhere;
}
.next {
  margin: 0;
  font-family: var(--serif, Georgia, serif);
  font-size: 13px;
  line-height: 1.4;
  display: -webkit-box;
  -webkit-line-clamp: 3;
  -webkit-box-orient: vertical;
  overflow: hidden;
}
.age {
  height: 3px;
  border-radius: 2px;
  background: color-mix(in srgb, var(--line) 40%, transparent);
  overflow: hidden;
}
.age i {
  display: block;
  height: 100%;
}
.foot {
  display: flex;
  flex-wrap: wrap;
  justify-content: space-between;
  align-items: center;
  gap: 6px;
}
.drop {
  margin: 0;
  padding: 14px;
  border: 1px dashed var(--line);
  border-radius: 12px;
  text-align: center;
  font-size: 12.5px;
  color: var(--text-dim);
}

/* outreach */
.outreach {
  display: grid;
  grid-template-columns: minmax(0, 1.25fr) minmax(0, 1fr);
  gap: 16px;
  align-items: start;
}
.journey {
  display: grid;
  gap: 8px;
}
.step {
  display: grid;
  grid-template-columns: 150px minmax(0, 1fr) 64px;
  gap: 12px;
  align-items: center;
  padding: 4px;
  border: 0;
  border-radius: 8px;
  background: none;
  color: var(--text);
  font: inherit;
  text-align: left;
  cursor: pointer;
}
.step:hover {
  background: var(--hover);
}
.step.static {
  cursor: default;
}
.step.static:hover {
  background: none;
}
.lab {
  font-size: 13px;
}
.lab small {
  display: block;
  font-size: 11.5px;
  color: var(--text-dim);
  opacity: 0.8;
}
.track {
  height: 22px;
  border-radius: 6px;
  background: rgba(255, 255, 255, 0.04);
  overflow: hidden;
}
.track.thin {
  height: 12px;
}
.track i {
  display: block;
  height: 100%;
  min-width: 4px;
  border-radius: 6px;
  opacity: 0.75;
}
.st-bg-know {
  background: var(--accent);
}
.st-bg-ready {
  background: var(--gold, #f4d58d);
}
.st-bg-reached {
  background: var(--text-dim);
}
.st-bg-talking {
  background: var(--good);
}
.st-bg-meeting {
  background: #c9b3ff;
}
.v {
  text-align: right;
  font-size: 15px;
  font-variant-numeric: tabular-nums;
}
.rates {
  display: flex;
  flex-wrap: wrap;
  gap: 18px;
  margin: 12px 0 8px;
  font-size: 13px;
}
.rates b {
  display: block;
  font-size: 20px;
  font-weight: 400;
}
.meter {
  height: 8px;
  border-radius: 4px;
  background: color-mix(in srgb, var(--line) 40%, transparent);
  overflow: hidden;
}
.meter i {
  display: block;
  height: 100%;
  background: var(--good);
}
.outreach .acts {
  margin-top: 12px;
}

/* contacts */
.filters {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  align-items: center;
  margin-bottom: 12px;
}
.fchip {
  padding: 4px 12px;
  border: 1px solid color-mix(in srgb, var(--line) 50%, transparent);
  border-radius: 999px;
  background: transparent;
  color: var(--text-dim);
  font: inherit;
  font-size: 12.5px;
  cursor: pointer;
}
.fchip[aria-pressed='true'] {
  border-color: var(--accent);
  background: var(--hover);
  color: var(--text);
}
.search {
  width: 200px;
  max-width: 100%;
  margin-left: auto;
  padding: 5px 10px;
}
.people {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
  gap: 12px;
}
.person {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr);
  gap: 10px 12px;
  padding: 14px;
  border: 1px solid color-mix(in srgb, var(--line) 50%, transparent);
  border-radius: 14px;
  background: var(--panel-background);
  color: var(--text);
  font: inherit;
  text-align: left;
  cursor: pointer;
}
.person:hover {
  border-color: var(--line);
}
.meta {
  grid-column: 1 / -1;
  display: flex;
  justify-content: space-between;
  gap: 8px;
  font-size: 12px;
}
.more {
  justify-self: start;
  margin-top: 12px;
}

.toast {
  position: fixed;
  z-index: 70;
  right: 20px;
  bottom: 20px;
  display: flex;
  gap: 14px;
  align-items: center;
  max-width: min(420px, calc(100% - 40px));
  padding: 12px 14px;
  border: 1px solid color-mix(in srgb, var(--gold, #f4d58d) 35%, transparent);
  border-radius: 12px;
  background: color-mix(in srgb, var(--background, #050b1f) 92%, var(--accent) 3%);
  font-size: 14px;
  box-shadow: 0 16px 40px rgba(0, 0, 0, 0.4);
}

@media (max-width: 1150px) {
  .today,
  .outreach {
    grid-template-columns: 1fr;
  }
  .tiles {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
  .find {
    flex-basis: 100%;
    margin-left: 0;
  }
}
@media (max-width: 560px) {
  .step {
    grid-template-columns: 110px minmax(0, 1fr) 48px;
  }
}
</style>
