/**
 * Barcode scanners at the till, in three connection modes:
 *
 * - **Keyboard (HID keyboard)** — the default for USB and Bluetooth scanners (e.g. HPRT-FB63HID):
 *   they type the code like a keyboard. Always on, nothing to pair. Browsers don't reveal which
 *   keyboard typed a key, so the till shows the name you give it and "last scan" as its status.
 * - **HID-POS** (WebHID, Chrome/Edge on a computer) — scanners switched to "HID POS" mode. The
 *   browser gives the real product name and live connected / disconnected status.
 * - **Serial** (Web Serial, Chrome/Edge on a computer) — USB-COM or Bluetooth SPP mode (the
 *   scanner shows up as a COM port). Live status; name from the port or given by you.
 *
 * Devices you allowed once reconnect by themselves the next time the POS opens. The list and the
 * names are kept in this browser only (each till has its own scanners).
 */
import { useBarcodeScanner, type ScanDiagnostic } from './useBarcodeScanner'

export type ScannerKind = 'keyboard' | 'hid' | 'serial'
export type ScannerStatus = 'ready' | 'connected' | 'disconnected' | 'error'
export interface ScannerDevice {
  id: string
  kind: ScannerKind
  name: string
  /** Name reported by the device / browser (HID product name, serial USB ids…). */
  reported: string
  status: ScannerStatus
  error: string
  lastScanAt: number | null
  lastCode: string
  scans: number
  baudRate?: number
  vendorId?: number
  productId?: number
}

interface Saved { id: string; kind: ScannerKind; name: string; vendorId?: number; productId?: number; baudRate?: number }
const STORE = 'zk_scanners'
/* Minimal typings for WebHID / Web Serial (not in TypeScript's DOM lib yet). */
interface HidDevice extends EventTarget { productName: string; vendorId: number; productId: number; opened: boolean; open(): Promise<void>; close(): Promise<void> }
interface HidReport extends Event { device: HidDevice; reportId: number; data: DataView }
interface SerialPortLike extends EventTarget {
  readable: ReadableStream<Uint8Array> | null
  open(o: { baudRate: number }): Promise<void>
  close(): Promise<void>
  getInfo(): { usbVendorId?: number; usbProductId?: number; bluetoothServiceClassId?: number | string }
}
const nav = () => (typeof navigator === 'undefined' ? {} : navigator) as unknown as {
  hid?: EventTarget & { requestDevice(o: { filters: object[] }): Promise<HidDevice[]>; getDevices(): Promise<HidDevice[]> }
  serial?: EventTarget & { requestPort(o?: object): Promise<SerialPortLike>; getPorts(): Promise<SerialPortLike[]> }
}

const hex = (n?: number) => (n === undefined ? '' : n.toString(16).padStart(4, '0'))
/** "]E0885…" (AIM symbology id), stray control characters → the plain code. */
export function cleanCode(raw: string): string {
  return raw.replace(/[\x00-\x1f\x7f]/g, '').replace(/^\][A-Za-z][0-9A-Za-z]/, '').trim()
}

export function useScannerDevices(onScan: (code: string) => void, enabled: Ref<boolean>, onBlocked?: (code: string) => void) {
  const { t } = useI18n()
  const devices = ref<ScannerDevice[]>([])
  const supported = ref({ hid: false, serial: false })
  const hidHandles = new Map<string, HidDevice>()
  const serialHandles = new Map<string, { port: SerialPortLike; reader?: ReadableStreamDefaultReader<Uint8Array> }>()
  let saved: Saved[] = []

  const persist = () => {
    try {
      localStorage.setItem(STORE, JSON.stringify(devices.value.map(({ id, kind, name, vendorId, productId, baudRate }) => ({ id, kind, name, vendorId, productId, baudRate }))))
    } catch {}
  }
  const find = (id: string) => devices.value.find((d) => d.id === id)
  function upsert(d: Partial<ScannerDevice> & { id: string; kind: ScannerKind }) {
    const cur = find(d.id)
    if (cur) Object.assign(cur, d)
    else devices.value.push({ name: '', reported: '', status: 'ready', error: '', lastScanAt: null, lastCode: '', scans: 0, ...d })
    persist()
  }
  function scanned(id: string, raw: string) {
    const code = cleanCode(raw)
    if (code.length < 3) return
    const d = find(id)
    if (d) Object.assign(d, { lastScanAt: Date.now(), lastCode: code, scans: d.scans + 1, status: d.kind === 'keyboard' ? 'ready' : 'connected', error: '' })
    if (enabled.value) onScan(code)
    else onBlocked?.(code)
  }

  // ---- keyboard-mode scanners (always on)
  const { lastScan, diagnostic } = useBarcodeScanner((code) => scanned('keyboard', code))

  // ---- HID-POS
  const hidId = (d: HidDevice) => `hid:${hex(d.vendorId)}:${hex(d.productId)}`
  async function openHid(dev: HidDevice) {
    const id = hidId(dev)
    const known = saved.find((s) => s.id === id)
    upsert({ id, kind: 'hid', reported: dev.productName || `HID ${hex(dev.vendorId)}:${hex(dev.productId)}`, name: find(id)?.name || known?.name || dev.productName || '', vendorId: dev.vendorId, productId: dev.productId })
    try {
      if (!dev.opened) await dev.open()
      hidHandles.set(id, dev)
      let buf = ''
      let idle: ReturnType<typeof setTimeout> | undefined
      const flush = () => {
        if (buf) scanned(id, buf)
        buf = ''
      }
      dev.addEventListener('inputreport', (ev) => {
        const { data } = ev as HidReport
        for (let i = 0; i < data.byteLength; i++) {
          const b = data.getUint8(i)
          if (b === 0x0d || b === 0x0a) flush()
          else if (b >= 0x20 && b < 0x7f) buf += String.fromCharCode(b)
        }
        clearTimeout(idle)
        idle = setTimeout(flush, 80)
      })
      upsert({ id, kind: 'hid', status: 'connected', error: '' })
    } catch (e) {
      upsert({ id, kind: 'hid', status: 'error', error: (e as Error)?.name === 'NotAllowedError' || (e as Error)?.name === 'SecurityError' ? t('pos.scanners.errKeyboardMode') : String((e as Error)?.message || e) })
    }
  }
  async function addHid() {
    const hid = nav().hid
    if (!hid) return
    try {
      // No filter: HID POS scanners (usage page 0x8C) don't always declare it, so list every HID device.
      const picked = await hid.requestDevice({ filters: [] })
      for (const d of picked) await openHid(d)
    } catch {}
  }

  // ---- Serial (USB-COM / Bluetooth SPP)
  const serialId = (p: SerialPortLike, i: number) => {
    const info = p.getInfo()
    return info.usbVendorId !== undefined ? `serial:${hex(info.usbVendorId)}:${hex(info.usbProductId)}` : `serial:bt:${i}`
  }
  async function openSerial(port: SerialPortLike, i: number, baudRate?: number) {
    const id = serialId(port, i)
    const info = port.getInfo()
    const known = saved.find((s) => s.id === id)
    const baud = baudRate ?? known?.baudRate ?? find(id)?.baudRate ?? 9600
    const reported = info.usbVendorId !== undefined ? `USB ${hex(info.usbVendorId)}:${hex(info.usbProductId)}` : info.bluetoothServiceClassId !== undefined ? t('pos.scanners.bluetoothSerial') : t('pos.scanners.serialPort')
    upsert({ id, kind: 'serial', reported, name: find(id)?.name || known?.name || '', baudRate: baud, vendorId: info.usbVendorId, productId: info.usbProductId })
    try {
      await port.open({ baudRate: baud })
    } catch (e) {
      // Already open is fine (e.g. hot reload); anything else is an error.
      if ((e as Error)?.name !== 'InvalidStateError') {
        upsert({ id, kind: 'serial', status: 'error', error: String((e as Error)?.message || e) })
        return
      }
    }
    const handle: { port: SerialPortLike; reader?: ReadableStreamDefaultReader<Uint8Array> } = { port }
    serialHandles.set(id, handle)
    upsert({ id, kind: 'serial', status: 'connected', error: '' })
    ;(async () => {
      const decoder = new TextDecoder()
      let buf = ''
      let idle: ReturnType<typeof setTimeout> | undefined
      const flush = () => {
        if (buf.trim()) scanned(id, buf)
        buf = ''
      }
      while (port.readable) {
        const reader = port.readable.getReader()
        handle.reader = reader
        try {
          for (;;) {
            const { value, done } = await reader.read()
            if (done) break
            const text = decoder.decode(value, { stream: true })
            for (const ch of text) {
              if (ch === '\r' || ch === '\n') flush()
              else buf += ch
            }
            clearTimeout(idle)
            idle = setTimeout(flush, 100)
          }
        } catch {
          break
        } finally {
          reader.releaseLock()
        }
      }
      if (find(id)?.status === 'connected') upsert({ id, kind: 'serial', status: 'disconnected' })
    })()
  }
  async function addSerial(baudRate = 9600) {
    const serial = nav().serial
    if (!serial) return
    try {
      const port = await serial.requestPort()
      const ports = await serial.getPorts()
      await openSerial(port, Math.max(0, ports.indexOf(port)), baudRate)
    } catch {}
  }

  async function disconnect(id: string) {
    const h = hidHandles.get(id)
    if (h) {
      await h.close().catch(() => {})
      hidHandles.delete(id)
    }
    const s = serialHandles.get(id)
    if (s) {
      await s.reader?.cancel().catch(() => {})
      await s.port.close().catch(() => {})
      serialHandles.delete(id)
    }
    const d = find(id)
    if (d) d.status = 'disconnected'
  }
  async function remove(id: string) {
    await disconnect(id)
    devices.value = devices.value.filter((d) => d.id !== id)
    persist()
  }
  async function reconnect(id: string) {
    const d = find(id)
    if (!d) return
    if (d.kind === 'hid') {
      const dev = (await nav().hid?.getDevices())?.find((x) => hidId(x) === id)
      if (dev) await openHid(dev)
      else upsert({ id, kind: 'hid', status: 'disconnected', error: t('pos.scanners.errNotFound') })
    } else if (d.kind === 'serial') {
      const ports = (await nav().serial?.getPorts()) ?? []
      const i = ports.findIndex((p, n) => serialId(p, n) === id)
      if (i >= 0) await openSerial(ports[i]!, i)
      else upsert({ id, kind: 'serial', status: 'disconnected', error: t('pos.scanners.errNotFound') })
    }
  }
  function rename(id: string, name: string) {
    const d = find(id)
    if (d) {
      d.name = name.trim().slice(0, 60)
      persist()
    }
  }

  onMounted(async () => {
    const n = nav()
    supported.value = { hid: !!n.hid, serial: !!n.serial }
    try {
      saved = JSON.parse(localStorage.getItem(STORE) || '[]')
    } catch {
      saved = []
    }
    const kb = saved.find((s) => s.kind === 'keyboard')
    devices.value = [{ id: 'keyboard', kind: 'keyboard', name: kb?.name ?? '', reported: t('pos.scanners.keyboardReported'), status: 'ready', error: '', lastScanAt: null, lastCode: '', scans: 0 }]
    // Previously added devices: show them, and reconnect the ones the browser still allows.
    for (const s of saved.filter((x) => x.kind !== 'keyboard')) {
      devices.value.push({ id: s.id, kind: s.kind, name: s.name, reported: '', status: 'disconnected', error: '', lastScanAt: null, lastCode: '', scans: 0, vendorId: s.vendorId, productId: s.productId, baudRate: s.baudRate })
    }
    if (n.hid) {
      for (const dev of await n.hid.getDevices().catch(() => [] as HidDevice[])) if (saved.some((s) => s.id === hidId(dev))) await openHid(dev)
      n.hid.addEventListener('connect', (e) => {
        const dev = (e as unknown as { device: HidDevice }).device
        if (find(hidId(dev))) openHid(dev)
      })
      n.hid.addEventListener('disconnect', (e) => {
        const dev = (e as unknown as { device: HidDevice }).device
        const d = find(hidId(dev))
        if (d) d.status = 'disconnected'
        hidHandles.delete(hidId(dev))
      })
    }
    if (n.serial) {
      const ports = await n.serial.getPorts().catch(() => [] as SerialPortLike[])
      for (const [i, p] of ports.entries()) if (saved.some((s) => s.id === serialId(p, i))) await openSerial(p, i)
      n.serial.addEventListener('connect', async () => {
        const all = (await n.serial!.getPorts()) ?? []
        for (const [i, p] of all.entries()) {
          const d = find(serialId(p, i))
          if (d && d.status !== 'connected') openSerial(p, i)
        }
      })
      n.serial.addEventListener('disconnect', (e) => {
        const port = (e as unknown as { target: SerialPortLike }).target
        for (const [id, h] of serialHandles) {
          if (h.port === port) {
            const d = find(id)
            if (d) d.status = 'disconnected'
            serialHandles.delete(id)
          }
        }
      })
    }
  })
  onBeforeUnmount(() => {
    for (const id of [...hidHandles.keys(), ...serialHandles.keys()]) disconnect(id)
  })

  /** The device to show in the POS header: the one that scanned last, else a connected one. */
  const primary = computed(() => {
    const ds = devices.value
    const byScan = [...ds].filter((d) => d.lastScanAt).sort((a, b) => b.lastScanAt! - a.lastScanAt!)[0]
    return byScan ?? ds.find((d) => d.status === 'connected') ?? ds[0] ?? null
  })

  return { devices, supported, primary, diagnostic, lastScan, addHid, addSerial, disconnect, reconnect, remove, rename }
}
export type { ScanDiagnostic }
