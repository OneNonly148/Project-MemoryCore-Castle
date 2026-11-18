<script setup>
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { mailRefresh, mailSnapshot, onMailChanged, openUrl } from '../../api/castle.js'
import { dueForSort, sortedLine } from './mail.js'

// HOME's Mail widget (this fork): the newest Outlook mail as the companion last sorted it. A
// read-only Claude Code run sorts it every 30 minutes on workdays (8:00-19:00) and on Refresh;
// a click shows the companion's one-line summary, then Outlook opens that mail.
defineProps({ companion: { type: String, default: 'Your companion' } })
const emit = defineEmits(['crm'])

const snap = ref(null)
const filter = ref('all')
const openId = ref('')
const now = ref(Date.now())
let stop = null
let timer = null

const items = computed(() =>
  (snap.value?.items ?? []).filter((m) => filter.value === 'all' || (filter.value === 'reply' ? m.needsReply : Boolean(m.crm))).slice(0, 6),
)
const counts = computed(() => ({
  reply: (snap.value?.items ?? []).filter((m) => m.needsReply).length,
  crm: (snap.value?.items ?? []).filter((m) => m.crm).length,
}))

async function load() {
  try {
    snap.value = await mailSnapshot()
  } catch {
    snap.value = { items: [], problem: 'Mail works inside Castle only.', sortedAt: 0, triedAt: 0, unread: 0, running: false }
  }
}

async function refresh() {
  try {
    await mailRefresh()
  } catch {
    // outside Castle's window
  }
  await load()
}

function time(iso) {
  const d = new Date(iso)
  if (Number.isNaN(d.getTime())) return ''
  const today = new Date()
  if (d.toDateString() === today.toDateString()) return d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', hourCycle: 'h23' })
  return d.toLocaleDateString([], { day: 'numeric', month: 'short' })
}

function tick() {
  now.value = Date.now()
  if (snap.value && dueForSort(new Date(), snap.value)) refresh()
}

onMounted(async () => {
  await load()
  stop = await onMailChanged(load).catch(() => null)
  tick()
  timer = setInterval(tick, 60_000)
})
onUnmounted(() => {
  stop?.()
  clearInterval(timer)
})
</script>

<template>
  <section class="widget card mail" aria-label="Mail">
    <h3>
      <span class="icon">✉</span>MAIL
      <span v-if="snap?.unread" class="unread">{{ snap.unread }} unread</span>
    </h3>

    <div class="filters" role="group" aria-label="Show">
      <button :class="{ on: filter === 'all' }" @click="filter = 'all'">All</button>
      <button :class="{ on: filter === 'reply' }" @click="filter = 'reply'">Needs reply<span v-if="counts.reply"> {{ counts.reply }}</span></button>
      <button :class="{ on: filter === 'crm' }" @click="filter = 'crm'">CRM<span v-if="counts.crm"> {{ counts.crm }}</span></button>
    </div>

    <ul v-if="items.length" class="list">
      <li v-for="m in items" :key="m.id || m.subject" :class="{ unread: m.unread, open: openId === m.id }">
        <button class="row" :aria-expanded="openId === m.id" @click="openId = openId === m.id ? '' : m.id">
          <span class="dot" aria-hidden="true"></span>
          <span class="from">{{ m.from || m.email }}</span>
          <span class="when">{{ time(m.received) }}</span>
          <span class="subject">{{ m.subject }}</span>
        </button>
        <div v-if="m.needsReply || m.crm" class="tags">
          <span v-if="m.needsReply" class="tag reply">Needs reply</span>
          <button v-if="m.crm" class="tag crm" :title="`Open ${m.crm.company} in CRM`" @click="emit('crm', m.crm.id)">CRM · {{ m.crm.status }}</button>
        </div>
        <div v-if="openId === m.id" class="peek">
          <p>{{ m.summary || 'No summary for this one.' }}</p>
          <button v-if="m.link" class="link" @click="openUrl(m.link)">Open in Outlook</button>
        </div>
      </li>
    </ul>
    <p v-else class="empty">
      {{ snap?.running ? 'Sorting your mail…' : filter === 'all' ? (snap?.sortedAt ? 'Your inbox is empty.' : `${companion} sorts your Outlook mail here. Press Refresh for the first sort.`) : 'Nothing here right now.' }}
    </p>

    <p v-if="snap?.problem" class="problem">{{ snap.problem }}</p>
    <footer>
      <span class="dim">{{ snap?.running ? 'Sorting…' : sortedLine(snap?.sortedAt, now) }}</span>
      <button :disabled="snap?.running" @click="refresh">Refresh</button>
    </footer>
  </section>
</template>

<style scoped>
.card {
  width: 100%;
  min-width: 280px;
  max-width: 100%;
  padding: 14px 18px;
  box-sizing: border-box;
}
h3 {
  display: flex;
  align-items: baseline;
  margin: 0 0 10px;
  padding-bottom: 8px;
  border-bottom: 1px solid var(--line);
  font-size: 13px;
  font-weight: 500;
  letter-spacing: 0.14em;
}
.icon {
  margin-right: 10px;
  color: var(--accent);
}
.unread {
  margin-left: auto;
  font-size: 12px;
  letter-spacing: 0;
  color: var(--accent);
  font-variant-numeric: tabular-nums;
}
.filters {
  display: inline-flex;
  margin-bottom: 6px;
  border: 1px solid var(--line);
  border-radius: 8px;
  overflow: hidden;
}
.filters button {
  padding: 3px 10px;
  border: 0;
  background: none;
  color: var(--text-dim);
  font: inherit;
  font-size: 12px;
  cursor: pointer;
}
.filters button.on {
  background: var(--hover);
  color: var(--text);
}
.list {
  display: grid;
  margin: 0;
  padding: 0;
  list-style: none;
}
.list li {
  border-top: 1px solid color-mix(in srgb, var(--line) 60%, transparent);
}
.row {
  display: grid;
  grid-template-columns: 8px minmax(0, 1fr) auto;
  gap: 2px 10px;
  width: 100%;
  padding: 8px 2px;
  border: 0;
  border-radius: 6px;
  background: none;
  color: var(--text);
  font: inherit;
  text-align: left;
  cursor: pointer;
}
.row:hover {
  background: var(--hover);
}
.dot {
  width: 7px;
  height: 7px;
  margin-top: 6px;
  border-radius: 50%;
}
.unread .dot {
  background: var(--accent);
}
.from,
.subject {
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}
.from {
  font-size: 13px;
}
.unread .from {
  font-weight: 600;
}
.subject {
  grid-column: 2 / 4;
  font-size: 12.5px;
  color: var(--text-dim);
}
.when {
  font-size: 11px;
  color: var(--text-dim);
  font-variant-numeric: tabular-nums;
}
.tags {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  padding: 0 2px 8px 20px;
}
.tag {
  padding: 1px 7px;
  border: 0;
  border-radius: 6px;
  font: inherit;
  font-size: 11px;
}
.tag.reply {
  background: color-mix(in srgb, var(--warn) 16%, transparent);
  color: var(--warn);
}
.tag.crm {
  background: color-mix(in srgb, var(--good) 16%, transparent);
  color: var(--good);
  cursor: pointer;
}
.peek {
  padding: 0 2px 10px 20px;
  font-size: 13px;
  line-height: 1.45;
}
.peek p {
  margin: 0 0 6px;
  overflow-wrap: anywhere;
}
.link {
  padding: 0;
  border: 0;
  background: none;
  color: var(--accent);
  font: inherit;
  font-size: 12px;
  text-decoration: underline;
  text-underline-offset: 2px;
  cursor: pointer;
}
.empty,
.problem {
  margin: 4px 0 6px;
  font-size: 13px;
  color: var(--text-dim);
}
.problem {
  color: var(--warn);
}
footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-top: 6px;
  font-size: 12px;
}
.dim {
  color: var(--text-dim);
}
footer button {
  padding: 4px 10px;
  border: 1px solid var(--line);
  border-radius: 8px;
  background: var(--hover);
  color: var(--text);
  font: inherit;
  font-size: 12px;
  cursor: pointer;
}
footer button:disabled {
  opacity: 0.5;
  cursor: default;
}
button:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
}
</style>
