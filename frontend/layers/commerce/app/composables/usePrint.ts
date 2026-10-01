/**
 * Print a document page (e.g. /print/receipt/<id>) without leaving the current screen:
 * the page is loaded in a hidden iframe, which calls window.print() itself once rendered.
 */
export function usePrint() {
  const printing = ref(false)

  function print(path: string) {
    if (!import.meta.client) return
    printing.value = true
    document.getElementById('zk-print-frame')?.remove()
    const frame = document.createElement('iframe')
    frame.id = 'zk-print-frame'
    frame.setAttribute('aria-hidden', 'true')
    Object.assign(frame.style, { position: 'fixed', right: '0', bottom: '0', width: '0', height: '0', border: '0' })
    const sep = path.includes('?') ? '&' : '?'
    frame.src = `${path}${sep}auto=1`
    const done = () => (printing.value = false)
    window.addEventListener('message', function onMsg(e) {
      if (e.origin === location.origin && e.data === 'zk:printed') {
        done()
        window.removeEventListener('message', onMsg)
      }
    })
    setTimeout(done, 15000)
    document.body.appendChild(frame)
  }

  /** Open in a new tab (preview / save as PDF). */
  function open(path: string) {
    window.open(path, '_blank', 'noopener')
  }

  return { print, open, printing }
}

/** Used by print pages: auto-print when opened with ?auto=1, then tell the parent. */
export function useAutoPrint(ready: Ref<boolean> | ComputedRef<boolean>) {
  const route = useRoute()
  onMounted(() => {
    if (!route.query.auto) return
    const go = async () => {
      await document.fonts?.ready
      await Promise.all(
        Array.from(document.images).filter((i) => !i.complete).map((i) => new Promise((r) => { i.onload = i.onerror = r })),
      )
      window.addEventListener('afterprint', () => window.parent?.postMessage('zk:printed', location.origin), { once: true })
      window.print()
    }
    if (ready.value) nextTick(go)
    else watch(ready, (v) => v && nextTick(go), { once: true })
  })
}
