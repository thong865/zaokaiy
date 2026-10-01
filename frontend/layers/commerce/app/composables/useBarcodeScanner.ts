/**
 * USB / Bluetooth (HID) barcode scanners act as a keyboard: they "type" the code very fast and
 * usually press Enter. This listens on the whole page and tells a scanner burst apart from human
 * typing by the time between keys — wherever the cursor is.
 *
 * - **Any focus.** A scan is caught even when the cursor is in a text box (quantity, custom item,
 *   bill name, payment amount…). The characters the burst typed into that box are taken back out,
 *   and its Enter is swallowed, so a barcode never ends up in a field or submits a form. People type
 *   far slower than a scanner, so normal typing is never touched.
 * - **Bluetooth HID** keys arrive in radio packets: most gaps are a few ms, some 50–150 ms. The
 *   *median* gap decides (human typing is ~100–250 ms per key) and a few slow gaps are allowed.
 * - **Keyboard layout.** The scanner sends key positions; with the Lao or Thai keyboard active,
 *   "8851…" arrives as Lao/Thai letters. The code is rebuilt from `KeyboardEvent.code`.
 * - **Alt + numpad mode** (scanners set to "emulate ALT+keypad" for international keyboards):
 *   each character is Alt held + its decimal code on the numpad; those are decoded too.
 * - **Android / IME** keyboards that report keys as "Unidentified": the typed text is taken from
 *   the input event instead.
 * - **No Enter suffix:** a fast burst that simply stops is taken as a scan too.
 *
 * `enabled = false` (e.g. while a payment dialog is open) still catches and removes scans, but
 * reports them to `onBlocked` instead of `onScan`.
 */

/** Physical key → character (US layout), independent of the active keyboard language. */
const CODE_CHARS: Record<string, [string, string]> = {
  Minus: ['-', '_'], Equal: ['=', '+'], BracketLeft: ['[', '{'], BracketRight: [']', '}'], Backslash: ['\\', '|'],
  Semicolon: [';', ':'], Quote: ["'", '"'], Comma: [',', '<'], Period: ['.', '>'], Slash: ['/', '?'], Backquote: ['`', '~'], Space: [' ', ' '],
  NumpadDecimal: ['.', '.'], NumpadAdd: ['+', '+'], NumpadSubtract: ['-', '-'], NumpadMultiply: ['*', '*'], NumpadDivide: ['/', '/'],
}
const SHIFT_DIGITS = ')!@#$%^&*('

/** The character a key stands for on a US keyboard (what the scanner meant), or null. */
export function scanChar(e: Pick<KeyboardEvent, 'key' | 'code' | 'shiftKey'>): string | null {
  const k = e.key
  if (k.length === 1 && k.charCodeAt(0) < 128) return k
  const c = e.code || ''
  let m = /^Digit(\d)$/.exec(c)
  if (m) return e.shiftKey ? SHIFT_DIGITS[Number(m[1])]! : m[1]!
  m = /^Numpad(\d)$/.exec(c)
  if (m) return m[1]!
  m = /^Key([A-Z])$/.exec(c)
  if (m) return e.shiftKey ? m[1]! : m[1]!.toLowerCase()
  const p = CODE_CHARS[c]
  if (p) return e.shiftKey ? p[1] : p[0]
  return k.length === 1 ? k : null
}

/** What the last burst of keys looked like — shown in the POS scanner test. */
export interface ScanDiagnostic {
  typed: string
  code: string
  keys: number
  medianGapMs: number
  maxGapMs: number
  ended: 'Enter' | 'Tab' | 'pause'
  accepted: boolean
  reason: '' | 'slow' | 'short' | 'field'
  at: number
}

const median = (xs: number[]) => {
  if (!xs.length) return 0
  const s = [...xs].sort((a, b) => a - b)
  return s[Math.floor(s.length / 2)]!
}
type Field = HTMLInputElement | HTMLTextAreaElement
const isField = (el: Element | null): el is Field => !!el && (el.tagName === 'INPUT' || el.tagName === 'TEXTAREA')

export function useBarcodeScanner(
  onScan: (code: string) => void,
  opts: { maxGapMs?: number; minLength?: number; idleMs?: number; enabled?: Ref<boolean>; onBlocked?: (code: string) => void } = {},
) {
  const maxGap = opts.maxGapMs ?? 50
  const maxPause = 300
  const minLen = opts.minLength ?? 4
  const idleMs = opts.idleMs ?? 250
  let typed = ''
  let code = ''
  let last = 0
  let gaps: number[] = []
  let idle: ReturnType<typeof setTimeout> | undefined
  // The text box the burst started in, and its value before the burst.
  let field: Field | null = null
  let fieldBefore = ''
  // Alt + numpad decoding
  let altDigits = ''
  let lastKeyUnidentified = 0
  const lastScan = ref<{ code: string; at: number } | null>(null)
  const diagnostic = ref<ScanDiagnostic | null>(null)

  function reset() {
    typed = ''
    code = ''
    gaps = []
    field = null
    clearTimeout(idle)
  }

  function finish(ended: ScanDiagnostic['ended'], e?: Event) {
    clearTimeout(idle)
    const med = median(gaps)
    const longest = gaps.length ? Math.max(...gaps) : 0
    const fast = gaps.length >= Math.min(minLen, code.length) - 1 && med <= maxGap && longest <= maxPause
    const long = code.length >= (ended === 'pause' ? Math.max(minLen, 6) : minLen)
    const accepted = fast && long
    if (code.length >= 2) {
      diagnostic.value = { typed, code, keys: code.length, medianGapMs: Math.round(med), maxGapMs: Math.round(longest), ended, accepted, reason: accepted ? '' : !long ? 'short' : 'slow', at: Date.now() }
    }
    if (accepted) {
      e?.preventDefault()
      e?.stopPropagation()
      // Take the scanned characters back out of the text box they landed in.
      const f = field
      if (f && f.isConnected && f.value !== fieldBefore) {
        f.value = fieldBefore
        f.dispatchEvent(new Event('input', { bubbles: true }))
      }
      const c = code
      reset()
      lastScan.value = { code: c, at: Date.now() }
      if (!opts.enabled || opts.enabled.value) onScan(c)
      else opts.onBlocked?.(c)
      return
    }
    reset()
  }

  /** One character of a possible burst. */
  function feed(ch: string, keyShown: string) {
    const now = performance.now()
    const gap = now - last
    last = now
    if (gap > maxPause) reset()
    else if (code) gaps.push(gap)
    if (!code) {
      const el = document.activeElement
      field = isField(el) ? el : null
      // The first character is typed into the box *after* keydown; remember the value before it.
      fieldBefore = field ? field.value : ''
    }
    code += ch
    typed += keyShown
    clearTimeout(idle)
    idle = setTimeout(() => finish('pause'), idleMs)
  }

  function onKeyDown(e: KeyboardEvent) {
    if (e.ctrlKey || e.metaKey) return
    // Alt + numpad digits: collect, decode on Alt release.
    if (e.altKey) {
      const m = /^Numpad(\d)$/.exec(e.code || '')
      if (m) altDigits += m[1]
      return
    }
    if (e.key === 'Enter' || e.key === 'Tab') {
      if (code) finish(e.key, e)
      return
    }
    if (e.key === 'Unidentified' || e.key === 'Process' || e.keyCode === 229) {
      lastKeyUnidentified = performance.now()
      return
    }
    const ch = scanChar(e)
    if (ch === null) return // shift, arrows, F-keys…
    feed(ch, e.key.length === 1 ? e.key : ch)
  }

  function onKeyUp(e: KeyboardEvent) {
    if (e.key === 'Alt' && altDigits) {
      const n = Number.parseInt(altDigits, 10)
      altDigits = ''
      if (n === 13) return finish('Enter')
      if (n >= 32 && n < 127) feed(String.fromCharCode(n), String.fromCharCode(n))
    }
  }

  /** Keyboards that only say "Unidentified" on keydown (Android / IME): read the typed text. */
  function onBeforeInput(e: Event) {
    const ie = e as InputEvent
    if (performance.now() - lastKeyUnidentified > 100) return
    if (ie.inputType === 'insertLineBreak' || ie.inputType === 'insertParagraph') {
      if (code) finish('Enter', e)
      return
    }
    if (ie.inputType === 'insertText' && ie.data) for (const ch of ie.data) feed(ch, ch)
  }

  onMounted(() => {
    window.addEventListener('keydown', onKeyDown, true)
    window.addEventListener('keyup', onKeyUp, true)
    window.addEventListener('beforeinput', onBeforeInput, true)
  })
  onBeforeUnmount(() => {
    window.removeEventListener('keydown', onKeyDown, true)
    window.removeEventListener('keyup', onKeyUp, true)
    window.removeEventListener('beforeinput', onBeforeInput, true)
    clearTimeout(idle)
  })
  return { lastScan, diagnostic }
}

/** Short confirmation sounds for scans (WebAudio, no files). */
export function useScanSound() {
  const enabled = ref(true)
  onMounted(() => {
    try {
      enabled.value = localStorage.getItem('zk_scan_sound') !== 'off'
    } catch {}
  })
  watch(enabled, (v) => {
    try {
      localStorage.setItem('zk_scan_sound', v ? 'on' : 'off')
    } catch {}
  })
  let ctx: AudioContext | null = null
  function beep(ok = true) {
    if (!enabled.value || typeof window === 'undefined') return
    try {
      ctx ??= new (window.AudioContext || (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext)()
      const o = ctx.createOscillator()
      const g = ctx.createGain()
      o.type = ok ? 'sine' : 'square'
      o.frequency.value = ok ? 1760 : 220
      g.gain.setValueAtTime(0.08, ctx.currentTime)
      g.gain.exponentialRampToValueAtTime(0.0001, ctx.currentTime + (ok ? 0.09 : 0.3))
      o.connect(g).connect(ctx.destination)
      o.start()
      o.stop(ctx.currentTime + (ok ? 0.1 : 0.32))
    } catch {}
  }
  return { enabled, beep }
}
