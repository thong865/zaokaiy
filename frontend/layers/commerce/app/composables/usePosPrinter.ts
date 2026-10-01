/**
 * Receipt printing for one till: the default printer, paper width, copies, and whether receipts
 * print by themselves after checkout. Settings are kept in this browser (each till has its own).
 *
 * Two ways to print:
 * - **browser** — the computer's default printer through the print dialog. Start Chrome/Edge with
 *   `--kiosk-printing` and it prints straight to the Windows default printer, no dialog.
 * - **direct** — a thermal (ESC/POS) printer connected by USB or Bluetooth that shows up as a COM
 *   port (Web Serial, Chrome/Edge on a computer). Prints instantly without any dialog; Lao works
 *   because the receipt is sent as an image. Can also open the cash drawer.
 */
import type { PosDoc } from '~/utils/types'

export type PrintMode = 'browser' | 'direct'
export interface PrinterSettings {
  mode: PrintMode
  /** Name shown in the POS (e.g. "HPRT TP806"); browsers can't read the printer's name. */
  name: string
  paper: '58' | '80'
  copies: number
  autoPrint: boolean
  /** Also print the A4 tax invoice when a sale issues one. */
  autoInvoice: boolean
  /** Direct printer: open the cash drawer after cash sales. */
  drawer: boolean
  baudRate: number
  /** USB ids of the direct printer (to reconnect by itself). */
  usbVendorId?: number
  usbProductId?: number
}
export type PrinterStatus = 'ready' | 'connected' | 'disconnected' | 'printing' | 'error'

const KEY = 'zk_printer'
const DEFAULTS: PrinterSettings = { mode: 'browser', name: '', paper: '80', copies: 1, autoPrint: true, autoInvoice: false, drawer: false, baudRate: 115200 }

interface Port extends EventTarget {
  writable: WritableStream<Uint8Array> | null
  open(o: { baudRate: number }): Promise<void>
  close(): Promise<void>
  getInfo(): { usbVendorId?: number; usbProductId?: number; bluetoothServiceClassId?: number | string }
}
// One connection per page, shared by every component that prints (POS, printer panel).
let port: Port | null = null
let initialized = false

const serialApi = () => (typeof navigator === 'undefined' ? undefined : (navigator as unknown as { serial?: EventTarget & { requestPort(): Promise<Port>; getPorts(): Promise<Port[]> } }).serial)

export function usePosPrinter() {
  const { t } = useI18n()
  const { print: browserPrint } = usePrint()
  const settings = useState<PrinterSettings>('pos-printer', () => ({ ...DEFAULTS }))
  const status = useState<PrinterStatus>('pos-printer-status', () => 'ready')
  const error = useState('pos-printer-error', () => '')
  const lastPrinted = useState<{ number: string; at: number; ok: boolean } | null>('pos-printer-last', () => null)
  const directSupported = ref(false)

  function save() {
    try {
      localStorage.setItem(KEY, JSON.stringify(settings.value))
    } catch {}
  }
  watch(settings, save, { deep: true })

  const displayName = computed(() => settings.value.name || (settings.value.mode === 'direct' ? t('pos.printer.directDefault') : t('pos.printer.browserDefault')))

  async function openPort(p: Port) {
    try {
      await p.open({ baudRate: settings.value.baudRate })
    } catch (e) {
      if ((e as Error)?.name !== 'InvalidStateError') {
        status.value = 'error'
        error.value = String((e as Error)?.message || e)
        return false
      }
    }
    port = p
    const info = p.getInfo()
    settings.value.usbVendorId = info.usbVendorId
    settings.value.usbProductId = info.usbProductId
    status.value = 'connected'
    error.value = ''
    return true
  }

  /** Pick the thermal printer (the browser lists the COM ports). */
  async function connectDirect() {
    const api = serialApi()
    if (!api) return
    try {
      const p = await api.requestPort()
      settings.value.mode = 'direct'
      await openPort(p)
    } catch {}
  }
  async function disconnectDirect() {
    try {
      await port?.close()
    } catch {}
    port = null
    status.value = 'disconnected'
  }

  async function write(bytes: Uint8Array) {
    if (!port?.writable) throw new Error(t('pos.printer.notConnected'))
    const w = port.writable.getWriter()
    try {
      // Small chunks: some Bluetooth printers drop data when flooded.
      for (let i = 0; i < bytes.length; i += 2048) await w.write(bytes.slice(i, i + 2048))
    } finally {
      w.releaseLock()
    }
  }

  async function printDirect(doc: PosDoc, opts: { preview?: boolean; drawer?: boolean } = {}) {
    const width = settings.value.paper === '58' ? 384 : 576
    let lines = receiptLines(doc, { preview: opts.preview })
    let canvas = await renderReceipt(lines, width)
    let bytes: Uint8Array
    try {
      bytes = canvasToEscPos(canvas, { drawer: opts.drawer })
    } catch {
      // The shop logo couldn't be read (another site): print without it.
      lines = lines.filter((l) => l.k !== 'image')
      canvas = await renderReceipt(lines, width)
      bytes = canvasToEscPos(canvas, { drawer: opts.drawer })
    }
    for (let i = 0; i < settings.value.copies; i++) await write(bytes)
  }

  /** Print a finished sale's receipt on this till's default printer. */
  async function printReceipt(doc: PosDoc): Promise<boolean> {
    const s = settings.value
    status.value = s.mode === 'direct' ? 'printing' : status.value
    error.value = ''
    try {
      if (s.mode === 'direct') {
        const cash = doc.sale.payments.some((p) => p.method === 'cash')
        await printDirect(doc, { drawer: s.drawer && cash })
        status.value = 'connected'
      } else {
        browserPrint(`/print/receipt/${doc.sale.id}?w=${s.paper}&copies=${s.copies}`)
      }
      if (s.autoInvoice && doc.sale.invoice_number) setTimeout(() => browserPrint(`/print/invoice/${doc.sale.id}`), s.mode === 'browser' ? 2500 : 0)
      lastPrinted.value = { number: doc.sale.number, at: Date.now(), ok: true }
      return true
    } catch (e) {
      status.value = port ? 'connected' : 'error'
      error.value = (e as Error)?.message || String(e)
      lastPrinted.value = { number: doc.sale.number, at: Date.now(), ok: false }
      return false
    }
  }

  /** Print a document that isn't a saved sale (bill preview for the customer, test page). */
  async function printDoc(doc: PosDoc, opts: { preview?: boolean } = {}): Promise<boolean> {
    error.value = ''
    try {
      if (settings.value.mode === 'direct') {
        status.value = 'printing'
        await printDirect(doc, { preview: opts.preview })
        status.value = 'connected'
      } else {
        sessionStorage.setItem('zk_print_doc', JSON.stringify({ doc, preview: !!opts.preview }))
        browserPrint(`/print/doc?w=${settings.value.paper}`)
      }
      return true
    } catch (e) {
      status.value = port ? 'connected' : 'error'
      error.value = (e as Error)?.message || String(e)
      return false
    }
  }

  /** A sample receipt to check the printer and paper width. */
  async function testPrint(shop: { name: string; currency: string }) {
    const now = new Date().toISOString()
    const doc = {
      sale: { id: 'test', shop_id: '', number: 'TEST', status: 'completed', subtotal_cents: 1500000, discount_cents: 0, vat_bps: 0, prices_include_vat: true, vat_cents: 0, total_cents: 1500000, paid_cents: 1500000, change_cents: 0, payments: [{ method: 'cash', amount_cents: 1500000 }], currency: shop.currency, note: t('pos.printer.testNote'), invoice_number: null, invoice_issued_at: null, customer_name: '', customer_tax_id: '', customer_branch: '', customer_address: '', customer_phone: '', void_reason: '', voided_at: null, created_at: now },
      items: [{ id: '1', product_id: null, name: 'ເບຍລາວ Beerlao 640ml', sku: '', qty: 1, unit_price_cents: 1500000, discount_cents: 0, line_total_cents: 1500000 }],
      cashier: null,
      amount_before_vat_cents: 1500000,
      shop: { id: '', name: shop.name, slug: '', logo_url: null, legal_name: '', tax_id: '', branch: '', address: '', phone: '', currency: shop.currency, receipt_footer: t('pos.printer.testFooter') },
    } as unknown as PosDoc
    await printDoc(doc)
  }

  onMounted(async () => {
    const api = serialApi()
    directSupported.value = !!api
    if (initialized) return
    initialized = true
    try {
      settings.value = { ...DEFAULTS, ...JSON.parse(localStorage.getItem(KEY) || '{}') }
    } catch {}
    if (!api) return
    // Reconnect the thermal printer allowed earlier.
    if (settings.value.mode === 'direct') {
      const ports = await api.getPorts().catch(() => [] as Port[])
      const s = settings.value
      const p = ports.find((x) => x.getInfo().usbVendorId === s.usbVendorId && x.getInfo().usbProductId === s.usbProductId) ?? (ports.length === 1 ? ports[0] : undefined)
      if (p) await openPort(p)
      else status.value = 'disconnected'
    }
    api.addEventListener('disconnect', () => {
      if (settings.value.mode === 'direct') {
        port = null
        status.value = 'disconnected'
      }
    })
    api.addEventListener('connect', async () => {
      if (settings.value.mode === 'direct' && !port) {
        const ports = await api.getPorts()
        const s = settings.value
        const p = ports.find((x) => x.getInfo().usbVendorId === s.usbVendorId && x.getInfo().usbProductId === s.usbProductId)
        if (p) await openPort(p)
      }
    })
  })

  const ready = computed(() => settings.value.mode === 'browser' || status.value === 'connected' || status.value === 'printing')
  return { settings, status, error, lastPrinted, directSupported, displayName, ready, connectDirect, disconnectDirect, printReceipt, printDoc, testPrint }
}
