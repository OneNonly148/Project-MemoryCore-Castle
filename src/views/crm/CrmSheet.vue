<script setup>
import { computed, nextTick, onMounted, ref, watch } from 'vue'
import { crmContact, crmOpenProfile, openUrl } from '../../api/castle.js'
import GlassSelect from '../../components/GlassSelect.vue'
import { ASKS, STAGES, ago, hue, initials, plain, rung } from './crm.js'

// One person, opened from anywhere in the CRM: where they stand, what to do next, the
// conversation so far, and their details. Moving the stage writes prospects.csv (via the room);
// replies, logging and next steps are typed to the companion.
const props = defineProps({
  contact: { type: Object, required: true },
  today: { type: String, required: true },
  companion: { type: String, default: 'Your companion' },
  // this person's mail in the Mail widget's last sort
  mails: { type: Array, default: () => [] },
})
const emit = defineEmits(['close', 'ask', 'move', 'snooze'])

const card = ref(null)
const problem = ref('')
const moving = ref(false)
const moveTo = ref('')
const copied = ref('')
const closeBtn = ref(null)

const step = computed(() => rung(props.contact.status))
const field = (name) => card.value?.fields?.[name]?.trim() ?? ''
const convo = computed(() => {
  const touches = (card.value?.touches ?? []).map((t) => ({ when: t.date, text: [t.status, t.product].filter(Boolean).join(' · ') + (t.note ? ` - ${t.note}` : ''), inbound: false }))
  const mail = props.mails.map((m) => ({ when: (m.received || '').slice(0, 10), text: `${m.subject}${m.summary ? ` - ${m.summary}` : ''}`, inbound: true, link: m.link }))
  return [...mail, ...touches].sort((a, b) => (b.when || '').localeCompare(a.when || '')).slice(0, 10)
})
const stageChoices = [...STAGES.map((s) => s.csv), 'On hold', 'Not interested', 'Lost']

async function load() {
  card.value = null
  problem.value = ''
  moving.value = false
  try {
    card.value = await crmContact(props.contact.id)
  } catch (reason) {
    problem.value = String(reason?.message ?? reason)
  }
}

async function copy(text) {
  try {
    await navigator.clipboard.writeText(text)
    copied.value = text
    setTimeout(() => (copied.value = ''), 1500)
  } catch {
    copied.value = ''
  }
}

async function openProfile() {
  try {
    await crmOpenProfile(card.value.profile)
  } catch (reason) {
    problem.value = String(reason?.message ?? reason)
  }
}

function confirmMove() {
  if (moveTo.value && moveTo.value !== props.contact.status) emit('move', props.contact, moveTo.value)
  moving.value = false
}

watch(() => props.contact.id, load)
onMounted(async () => {
  await load()
  await nextTick()
  closeBtn.value?.focus()
})
</script>

<template>
  <div class="scrim" @click="emit('close')"></div>
  <aside class="sheet" role="dialog" aria-modal="true" :aria-label="contact.company" @keydown.esc="emit('close')">
    <header>
      <span class="avatar lg" :style="{ background: hue(contact.company) }" aria-hidden="true">{{ initials(contact) }}</span>
      <div class="who">
        <h2>{{ contact.person || contact.company }}</h2>
        <p>{{ [contact.position, contact.person ? contact.company : contact.sector].filter(Boolean).join(' · ') }}</p>
      </div>
      <button ref="closeBtn" class="close" aria-label="Close" @click="emit('close')">✕</button>
    </header>

    <div v-if="step >= 0" class="ladder" :aria-label="`Stage: ${plain(contact.status)}`">
      <div v-for="(s, i) in STAGES" :key="s.csv" :class="{ done: i < step, now: i === step }">{{ s.name }}</div>
    </div>
    <p v-else class="off">{{ plain(contact.status) }}</p>

    <section class="next">
      <h3>NEXT STEP</h3>
      <p v-if="contact.notes" class="serif">{{ contact.notes }}</p>
      <p v-else class="dim">No notes yet.</p>
      <button class="primary" @click="emit('ask', ASKS.next(contact))">Ask {{ companion }} for the next step</button>
    </section>

    <div class="quick">
      <button @click="emit('ask', ASKS.reply(contact))"><span class="ico" aria-hidden="true">✉</span>Reply</button>
      <button @click="emit('ask', ASKS.touch(contact))"><span class="ico" aria-hidden="true">✎</span>Log a talk</button>
      <button :aria-expanded="moving" @click="moving = !moving; moveTo = contact.status"><span class="ico" aria-hidden="true">⇄</span>Move stage</button>
      <button @click="emit('snooze', contact)"><span class="ico" aria-hidden="true">◷</span>Snooze a week</button>
    </div>
    <form v-if="moving" class="move" @submit.prevent="confirmMove">
      <span>Move {{ contact.company }} to</span>
      <GlassSelect v-model="moveTo" :options="stageChoices.map((s) => ({ value: s, label: plain(s) }))" :aria-label="`Move ${contact.company} to`" />
      <button class="primary" type="submit" :disabled="moveTo === contact.status">Move</button>
      <button type="button" @click="moving = false">Cancel</button>
      <p class="dim small">Castle changes prospects.csv and keeps a backup copy first.</p>
    </form>

    <section>
      <h3>THE CONVERSATION SO FAR</h3>
      <p v-if="!card && !problem" class="dim small">Reading…</p>
      <ul v-else-if="convo.length" class="convo">
        <li v-for="(c, i) in convo" :key="i">
          <span class="ic" :class="{ in: c.inbound }" aria-hidden="true">{{ c.inbound ? '↙' : '↗' }}</span>
          <div>
            <span class="when">{{ c.when ? ago(c.when, today) : '' }}{{ c.inbound ? ' · mail from them' : '' }}</span>
            <p>{{ c.text }}</p>
            <button v-if="c.link" class="link small" @click="openUrl(c.link)">Open in Outlook</button>
          </div>
        </li>
      </ul>
      <p v-else class="dim small">Nothing logged yet.</p>
    </section>

    <section>
      <h3>DETAILS</h3>
      <dl>
        <dt>Stage</dt><dd>{{ plain(contact.status) }}</dd><dd></dd>
        <template v-if="contact.sector"><dt>Sector</dt><dd>{{ contact.sector }}</dd><dd></dd></template>
        <template v-if="contact.product"><dt>Fit</dt><dd>{{ contact.product }}</dd><dd></dd></template>
        <dt>Reached by</dt><dd>{{ contact.channel === 'Direct' ? 'Email' : contact.channel }}</dd><dd></dd>
        <dt>Last touch</dt><dd>{{ ago(contact.lastTouch, today) }}</dd><dd></dd>
        <template v-if="contact.email"><dt>Email</dt><dd class="sel">{{ contact.email }}</dd><dd><button class="mini" @click="copy(contact.email)">{{ copied === contact.email ? 'Copied' : 'Copy' }}</button></dd></template>
        <template v-if="field('phone')"><dt>Phone</dt><dd class="sel">{{ field('phone') }}</dd><dd><button class="mini" @click="copy(field('phone'))">{{ copied === field('phone') ? 'Copied' : 'Copy' }}</button></dd></template>
      </dl>
      <button v-if="card?.profile" class="small-btn" @click="openProfile">Open account profile</button>
    </section>
    <p v-if="problem" class="warn small">{{ problem }}</p>
  </aside>
</template>

<style scoped>
.scrim {
  position: fixed;
  inset: 0;
  z-index: 50;
  background: rgba(2, 6, 20, 0.55);
}
.sheet {
  position: fixed;
  z-index: 60;
  top: 0;
  right: 0;
  bottom: 0;
  width: min(480px, 100%);
  overflow-y: auto;
  display: grid;
  gap: 18px;
  align-content: start;
  padding: 22px;
  box-sizing: border-box;
  background: color-mix(in srgb, var(--background, #050b1f) 88%, var(--accent) 4%);
  border-left: 1px solid var(--line);
  box-shadow: -20px 0 50px rgba(0, 0, 0, 0.35);
}
header {
  display: flex;
  gap: 14px;
  align-items: center;
}
.who {
  min-width: 0;
}
h2 {
  margin: 0;
  font-size: 20px;
  overflow-wrap: anywhere;
}
header p {
  margin: 2px 0 0;
  font-size: 13px;
  color: var(--text-dim);
}
h3 {
  margin: 0 0 8px;
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.12em;
  color: var(--text-dim);
}
section {
  min-width: 0;
}
.avatar {
  flex: none;
  display: grid;
  place-items: center;
  border-radius: 50%;
  font-weight: 600;
  color: #07112c;
}
.avatar.lg {
  width: 52px;
  height: 52px;
  font-size: 17px;
}
.close {
  margin-left: auto;
  align-self: flex-start;
  width: 32px;
  height: 32px;
  border: 1px solid var(--line);
  border-radius: 8px;
  background: none;
  color: var(--text-dim);
  cursor: pointer;
}
.ladder {
  display: grid;
  grid-template-columns: repeat(5, 1fr);
  gap: 4px;
}
.ladder div {
  display: grid;
  gap: 5px;
  font-size: 11px;
  text-align: center;
  color: var(--text-dim);
  opacity: 0.75;
}
.ladder div::before {
  content: '';
  height: 4px;
  border-radius: 2px;
  background: var(--hover);
}
.ladder .done::before {
  background: color-mix(in srgb, var(--accent) 60%, transparent);
}
.ladder .now {
  color: var(--text);
  opacity: 1;
}
.ladder .now::before {
  background: var(--good);
}
.off {
  margin: 0;
  color: var(--warn);
  font-size: 13px;
}
.next {
  display: grid;
  gap: 8px;
  padding: 12px 14px;
  border: 1px solid color-mix(in srgb, var(--gold, #f4d58d) 28%, transparent);
  border-radius: 12px;
  background: color-mix(in srgb, var(--gold, #f4d58d) 6%, transparent);
}
.next h3 {
  margin: 0;
  color: var(--gold, #f4d58d);
}
.next p {
  margin: 0;
  max-height: 160px;
  overflow: auto;
  white-space: pre-line;
  overflow-wrap: anywhere;
}
.serif {
  font-family: var(--serif, Georgia, serif);
  font-size: 15px;
  line-height: 1.45;
}
button {
  font: inherit;
  color: var(--text);
  cursor: pointer;
}
button.primary,
.move button,
.small-btn {
  justify-self: start;
  padding: 6px 12px;
  border: 1px solid var(--line);
  border-radius: 8px;
  background: var(--hover);
  font-size: 13px;
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
select:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
}
.quick {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 8px;
}
.quick button {
  display: grid;
  gap: 4px;
  justify-items: center;
  padding: 10px 4px;
  border: 1px solid color-mix(in srgb, var(--line) 60%, transparent);
  border-radius: 12px;
  background: rgba(255, 255, 255, 0.03);
  font-size: 12px;
}
.quick button:hover {
  border-color: var(--accent);
}
.ico {
  font-size: 18px;
  color: var(--accent);
}
.move {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  align-items: center;
  font-size: 13px;
}
.move select {
  padding: 6px 8px;
  border: 1px solid var(--line);
  border-radius: 8px;
  background: rgba(0, 0, 0, 0.3);
  color: var(--text);
  font: inherit;
}
.move p {
  width: 100%;
  margin: 0;
}
.convo {
  display: grid;
  gap: 12px;
  margin: 0;
  padding: 0;
  list-style: none;
}
.convo li {
  display: grid;
  grid-template-columns: 22px minmax(0, 1fr);
  gap: 10px;
}
.ic {
  display: grid;
  place-items: center;
  width: 22px;
  height: 22px;
  border-radius: 50%;
  font-size: 11px;
  background: var(--hover);
  color: var(--accent);
}
.ic.in {
  background: color-mix(in srgb, var(--good) 15%, transparent);
  color: var(--good);
}
.when {
  font-size: 11.5px;
  color: var(--text-dim);
}
.convo p {
  margin: 2px 0 0;
  font-size: 13.5px;
  overflow-wrap: anywhere;
}
dl {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto;
  gap: 8px 12px;
  align-items: center;
  margin: 0 0 10px;
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
.mini {
  padding: 2px 8px;
  border: 1px solid color-mix(in srgb, var(--line) 60%, transparent);
  border-radius: 6px;
  background: none;
  font-size: 12px;
  color: var(--text-dim);
}
.link {
  padding: 0;
  border: 0;
  background: none;
  color: var(--accent);
  text-decoration: underline;
  text-underline-offset: 2px;
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
@media (max-width: 520px) {
  .quick {
    grid-template-columns: repeat(2, 1fr);
  }
}
</style>
