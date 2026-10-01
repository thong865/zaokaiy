/**
 * USB / Bluetooth barcode scanners "type" the code very fast and press Enter. Keystrokes that
 * arrive < 40 ms apart and end with Enter are treated as a scan anywhere on the screen, except
 * while the cashier is typing in a form field (a scan into the search box is handled there).
 */
export function useScanner(onScan: (code: string) => void) {
  let buf = ''
  let last = 0
  const onKey = (e: KeyboardEvent) => {
    const el = e.target as HTMLElement | null
    if (el && (el.tagName === 'INPUT' || el.tagName === 'TEXTAREA' || el.isContentEditable)) return
    const now = performance.now()
    if (now - last > 40) buf = ''
    last = now
    if (e.key === 'Enter') {
      if (buf.length >= 4) {
        e.preventDefault()
        onScan(buf)
      }
      buf = ''
      return
    }
    if (e.key.length === 1) buf += e.key
  }
  onMounted(() => window.addEventListener('keydown', onKey, true))
  onBeforeUnmount(() => window.removeEventListener('keydown', onKey, true))
}

let ctx: AudioContext | null = null
export function beep(ok = true) {
  try {
    ctx ??= new AudioContext()
    const o = ctx.createOscillator()
    const g = ctx.createGain()
    o.frequency.value = ok ? 1500 : 320
    g.gain.value = 0.06
    o.connect(g).connect(ctx.destination)
    o.start()
    o.stop(ctx.currentTime + (ok ? 0.07 : 0.25))
  } catch {
    /* no audio device */
  }
}
