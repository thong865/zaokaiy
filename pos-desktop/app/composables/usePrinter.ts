import type { SaleRecord } from '~/utils/types'
import { receiptCanvas, toRaster } from '~/utils/receipt'

/** Send receipts to the configured printer (network / installed printer / system print dialog). */
export function usePrinter() {
  const { settings, lang } = usePos()
  const p = computed(() => settings.value.printer)
  const enabled = computed(() => p.value.kind !== 'none')

  const target = () => ({ kind: p.value.kind, host: p.value.host, port: Number(p.value.port) || 9100, name: p.value.name })

  async function printCanvas(canvas: HTMLCanvasElement, opts: { drawer?: boolean } = {}) {
    if (p.value.kind === 'window') return printViaDialog(canvas)
    if (p.value.kind === 'none') return
    const r = toRaster(canvas)
    await call('print_raster', { target: target(), widthBytes: r.widthBytes, height: r.height, data: r.data, cut: p.value.cut, drawer: !!opts.drawer })
  }

  async function printSale(rec: SaleRecord, opts: { reprint?: boolean } = {}) {
    const canvas = receiptCanvas(rec, lang.value, p.value.paper)
    const cash = rec.sale.payments.some((x) => x.method === 'cash')
    await printCanvas(canvas, { drawer: !opts.reprint && p.value.drawer && cash })
  }

  async function openDrawer() {
    if (p.value.kind === 'network' || p.value.kind === 'system') await call('open_drawer', { target: target() })
  }

  return { enabled, printCanvas, printSale, openDrawer }
}

/** Fallback: the operating system's print dialog with the receipt image at paper width. */
function printViaDialog(canvas: HTMLCanvasElement) {
  const mm = canvas.width === 384 ? 58 : 80
  const frame = document.createElement('iframe')
  frame.style.cssText = 'position:fixed;right:0;bottom:0;width:0;height:0;border:0'
  document.body.appendChild(frame)
  const doc = frame.contentDocument!
  doc.open()
  doc.write(`<!doctype html><html><head><style>@page{size:${mm}mm auto;margin:0}body{margin:0}img{width:${mm}mm;display:block}</style></head><body><img src="${canvas.toDataURL('image/png')}"></body></html>`)
  doc.close()
  const img = doc.querySelector('img')!
  const go = () => {
    frame.contentWindow!.focus()
    frame.contentWindow!.print()
    setTimeout(() => frame.remove(), 60_000)
  }
  if (img.complete) go()
  else img.onload = go
}
