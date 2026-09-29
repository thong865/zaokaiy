/**
 * USB / Bluetooth barcode scanners act as a keyboard: they "type" the code very fast and press
 * Enter. This listens on the whole page so a scan works even when nothing is focused, and tells a
 * scanner burst apart from human typing by the time between keys.
 *
 * When a text field is focused, the scan is left to that field (unless it is marked
 * `data-scan-target`), so typing in forms — payment amount, customer name — is never hijacked.
 */
export function useBarcodeScanner(onScan: (code: string) => void, opts: { maxGapMs?: number; minLength?: number; enabled?: Ref<boolean> } = {}) {
  const maxGap = opts.maxGapMs ?? 45
  const minLen = opts.minLength ?? 4
  let buf = ''
  let last = 0
  let gaps: number[] = []
  const lastScan = ref<{ code: string; at: number } | null>(null)

  function editable(el: Element | null) {
    if (!el) return false
    const tag = el.tagName
    return (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT' || (el as HTMLElement).isContentEditable) && !(el as HTMLElement).dataset.scanTarget
  }

  function onKey(e: KeyboardEvent) {
    if (opts.enabled && !opts.enabled.value) return
    if (e.ctrlKey || e.metaKey || e.altKey) return
    if (editable(document.activeElement)) {
      buf = ''
      return
    }
    const now = performance.now()
    const gap = now - last
    last = now
    if (e.key === 'Enter' || e.key === 'Tab') {
      const fast = gaps.length >= minLen - 1 && gaps.reduce((a, b) => a + b, 0) / gaps.length <= maxGap
      if (buf.length >= minLen && fast && gap <= maxGap * 4) {
        e.preventDefault()
        e.stopPropagation()
        const code = buf
        lastScan.value = { code, at: Date.now() }
        onScan(code)
      }
      buf = ''
      gaps = []
      return
    }
    if (e.key.length !== 1) return // shift, arrows, F-keys…
    if (gap > maxGap * 4) {
      buf = ''
      gaps = []
    } else if (buf) gaps.push(gap)
    buf += e.key
  }

  onMounted(() => window.addEventListener('keydown', onKey, true))
  onBeforeUnmount(() => window.removeEventListener('keydown', onKey, true))
  return { lastScan }
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
