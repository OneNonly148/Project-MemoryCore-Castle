<script setup>
// On the top bar when a newer Castle is out. Official builds: "Update ready" - the notes can be
// read first, and Update now saves the diary first: the companion is asked to save it, and Castle
// waits until the diary changes (three minutes at most, or Skip). Then the update downloads, its
// signature is checked, it installs and Castle restarts. A member's own build: a quiet "X is out"
// that opens Settings > About (it updates with git pull, never by itself).
// It sits in the top bar's brand block, under MEMORYCORE and as wide as it (TopBar's brand slot),
// so it never covers the buttons at the right. Long words end in "…"; the full line is its tooltip.
import { onBeforeUnmount, onMounted, ref } from 'vue'
import { diaryStamp, terminalState, terminalWrite, updateCheck, updateInstall } from '../api/castle.js'
import MarkdownText from './MarkdownText.vue'
import { typeLine } from './typeline.js'

const emit = defineEmits(['status', 'about'])
const status = ref(null)
const step = ref('')
const notesOpen = ref(false)
let skip = false
let timer

const sleep = (ms) => new Promise((r) => setTimeout(r, ms))

async function check() {
  try {
    status.value = await updateCheck()
    emit('status', status.value)
  } catch {
    // no update line
  }
}

async function saveDiaryFirst() {
  const state = await terminalState().catch(() => null)
  if (!state?.running) return
  const before = await diaryStamp()
  step.value = 'Saving the diary first…'
  await typeLine(terminalWrite, 'save diary')
  for (let waited = 0; waited < 180 && !skip; waited += 3) {
    await sleep(3000)
    if ((await diaryStamp()) > before) {
      // give the companion a moment to finish writing
      await sleep(5000)
      return
    }
  }
}

async function updateNow() {
  skip = false
  try {
    await saveDiaryFirst()
    step.value = 'Downloading the update…'
    await updateInstall()
  } catch (reason) {
    step.value = String(reason)
  }
}

onMounted(() => {
  check()
  timer = setInterval(check, 6 * 60 * 60 * 1000)
})
onBeforeUnmount(() => clearInterval(timer))
</script>

<template>
  <div v-if="status?.available && status.canInstall" class="update ready">
    <template v-if="!step">
      <button
        v-if="status.notes"
        class="label quiet"
        :title="`Update ready: ${status.available} · What's new`"
        :aria-expanded="notesOpen"
        @click="notesOpen = !notesOpen"
      >{{ status.available }} is ready</button>
      <span v-else class="label" :title="`Update ready: ${status.available}`">{{ status.available }} is ready</span>
      <button class="go" @click="updateNow">Update now</button>
    </template>
    <template v-else>
      <span class="label" :title="step">{{ step }}</span>
      <button v-if="step.startsWith('Saving')" class="go" @click="skip = true">Skip</button>
    </template>
    <div v-if="notesOpen && !step" class="notes">
      <MarkdownText :text="status.notes" />
      <p class="hint">The diary is saved first, then Castle restarts on the new version. Your memory, settings and keys stay as they are.</p>
    </div>
  </div>
  <button v-else-if="status?.available" class="update own" :title="status.message || `Castle ${status.available} is out`" @click="emit('about')">
    <span class="label">Castle {{ status.available }} is out</span>
  </button>
</template>

<style scoped>
/* Under MEMORYCORE in the 48px bar: one 18px line, the brand's width (TopBar sets it). */
.update {
  position: relative;
  z-index: 40;
  box-sizing: border-box;
  display: flex;
  gap: 6px;
  align-items: center;
  height: 18px;
  padding: 0 3px 0 8px;
  border: 1px solid var(--good);
  border-radius: 999px;
  background: var(--panel-background);
  color: var(--text);
  font: inherit;
  font-size: 10.5px;
  line-height: 16px;
  letter-spacing: 0.02em;
  white-space: nowrap;
}
.update.own {
  justify-content: center;
  padding: 0 8px;
  border-color: var(--line);
  color: var(--text-dim);
  cursor: pointer;
}
.update.own:hover {
  color: var(--text);
}
.label {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  text-align: left;
}
.update.own .label {
  flex: 0 1 auto;
}
.notes {
  position: absolute;
  top: calc(100% + 8px);
  left: 0;
  width: min(420px, 80vw);
  max-height: 60vh;
  overflow: auto;
  padding: 12px 16px;
  border: 1px solid var(--line);
  border-radius: 12px;
  background: var(--panel-background);
  -webkit-backdrop-filter: blur(var(--glass-blur)); backdrop-filter: blur(var(--glass-blur));
  font-size: 13px;
  line-height: 1.5;
  letter-spacing: normal;
  white-space: normal;
}
.notes .hint {
  margin: 10px 0 0;
  color: var(--text-dim);
}
button {
  font: inherit;
  cursor: pointer;
}
button.quiet {
  padding: 0;
  border: 0;
  background: transparent;
  color: var(--text);
}
button.quiet:hover {
  text-decoration: underline;
}
button.go {
  flex: none;
  padding: 0 8px;
  border: 0;
  border-radius: 999px;
  background: var(--good);
  color: #07142e;
  line-height: 14px;
}
button:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
}
</style>
