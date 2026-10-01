/**
 * Direct thermal printing (ESC/POS) for POS receipts.
 *
 * Thermal printers' built-in fonts have no Lao (and often no Thai), so the receipt is drawn on a
 * canvas with the site's fonts and sent as a bitmap (`GS v 0` raster). That works for any language,
 * with any ESC/POS printer (HPRT, Xprinter, Epson TM…), over USB-COM or Bluetooth (SPP).
 */
import type { PosDoc } from '~/utils/types'
import { dlLocal, docLang, docLocale, type DocKey } from './docLabels'
import { fmtDateTime, money } from '~/utils/format'

type Align = 'left' | 'center' | 'right'
export type RLine =
  | { k: 'text'; text: string; align?: Align; bold?: boolean; scale?: number }
  | { k: 'pair'; left: string; right: string; bold?: boolean; scale?: number; indent?: boolean }
  | { k: 'rule' }
  | { k: 'gap'; px: number }
  | { k: 'image'; url: string }

const METHODS: Record<string, string> = { cash: 'Cash', card: 'Card', transfer: 'Transfer', qr: 'QR / PromptPay', other: 'Other' }

/** The receipt as lines — same content as components/pos/Receipt.vue. */
export function receiptLines(doc: PosDoc, opts: { preview?: boolean } = {}): RLine[] {
  const s = doc.sale
  const lo = docLang(s.currency) === 'lo'
  const tx = (key: DocKey | (string & {}), fallback: string) => (lo ? dlLocal(key, 'lo') : fallback)
  const m = (c: number) => money(c, s.currency)
  const hasTax = !!doc.shop.tax_id
  const out: RLine[] = []
  if (opts.preview) out.push({ k: 'text', text: `[ ${tx('preview', 'PREVIEW — NOT PAID YET')} ]`, align: 'center', bold: true })
  if (doc.shop.logo_url) out.push({ k: 'image', url: doc.shop.logo_url })
  out.push({ k: 'text', text: doc.shop.name, align: 'center', bold: true, scale: 1.35 })
  if (doc.shop.legal_name) out.push({ k: 'text', text: doc.shop.legal_name, align: 'center' })
  for (const l of (doc.shop.address || '').split('\n').filter(Boolean)) out.push({ k: 'text', text: l, align: 'center' })
  if (doc.shop.phone) out.push({ k: 'text', text: `${tx('tel', 'Tel.')} ${doc.shop.phone}`, align: 'center' })
  if (hasTax) out.push({ k: 'text', text: `${tx('taxId', 'TAX ID')} ${doc.shop.tax_id}${doc.shop.branch ? ` · ${doc.shop.branch}` : ''}`, align: 'center' })
  out.push({ k: 'rule' })
  out.push({ k: 'text', text: lo ? dlLocal(hasTax ? 'receiptAbb' : 'receipt', 'lo') : hasTax ? 'RECEIPT / TAX INVOICE (ABB)' : 'RECEIPT', align: 'center', bold: true })
  out.push({ k: 'pair', left: `${tx('no', 'No.')} ${s.number}`, right: fmtDateTime(s.created_at, 'short', 'short', docLocale(lo ? 'lo' : null)) })
  if (doc.cashier) out.push({ k: 'text', text: `${tx('cashier', 'Cashier')}: ${doc.cashier}` })
  out.push({ k: 'rule' })
  for (const it of doc.items) {
    out.push({ k: 'text', text: it.name })
    out.push({ k: 'pair', left: `${it.qty} × ${m(it.unit_price_cents)}`, right: m(it.qty * it.unit_price_cents), indent: true })
    if (it.discount_cents) out.push({ k: 'pair', left: tx('discount', 'Discount'), right: `−${m(it.discount_cents)}`, indent: true })
  }
  out.push({ k: 'rule' })
  out.push({ k: 'pair', left: tx('items', 'Items'), right: String(doc.items.reduce((n, i) => n + i.qty, 0)) })
  out.push({ k: 'pair', left: tx('subtotal', 'Subtotal'), right: m(s.subtotal_cents) })
  if (s.discount_cents) out.push({ k: 'pair', left: tx('discount', 'Discount'), right: `−${m(s.discount_cents)}` })
  out.push({ k: 'pair', left: tx('total', 'TOTAL'), right: m(s.total_cents), bold: true, scale: 1.3 })
  if (s.vat_bps) {
    out.push({ k: 'pair', left: tx('beforeVatShort', 'Before VAT'), right: m(doc.amount_before_vat_cents) })
    out.push({ k: 'pair', left: `${tx('vatShort', 'VAT')} ${s.vat_bps / 100}%${s.prices_include_vat ? ` ${tx('vatIncl', '(incl.)')}` : ''}`, right: m(s.vat_cents) })
  }
  if (s.payments.length) {
    out.push({ k: 'rule' })
    for (const p of s.payments) out.push({ k: 'pair', left: `${lo ? dlLocal(`pay_${p.method}`, 'lo') : METHODS[p.method] ?? p.method}${p.reference ? ` (${p.reference})` : ''}`, right: m(p.amount_cents) })
    if (s.change_cents) out.push({ k: 'pair', left: tx('change', 'Change'), right: m(s.change_cents), bold: true })
  }
  if (s.invoice_number || s.note) {
    out.push({ k: 'rule' })
    if (s.invoice_number) out.push({ k: 'text', text: `${tx('fullTaxInvoice', 'Full tax invoice')}: ${s.invoice_number}` })
    for (const l of (s.note || '').split('\n').filter(Boolean)) out.push({ k: 'text', text: l })
  }
  out.push({ k: 'rule' })
  for (const l of (doc.shop.receipt_footer || '').split('\n').filter(Boolean)) out.push({ k: 'text', text: l, align: 'center' })
  out.push({ k: 'text', text: `${s.number} · zaokaiy POS`, align: 'center', scale: 0.85 })
  return out
}

const FONT = '"Noto Sans Lao", "Noto Sans Thai", "Plus Jakarta Sans", "Leelawadee UI", "Phetsarath OT", sans-serif'

function loadImage(url: string): Promise<HTMLImageElement | null> {
  return new Promise((res) => {
    const img = new Image()
    img.crossOrigin = 'anonymous'
    img.onload = () => res(img)
    img.onerror = () => res(null)
    img.src = url
    setTimeout(() => res(null), 3000)
  })
}

/** Draw lines on a canvas `widthDots` wide (58 mm paper = 384 dots, 80 mm = 576 dots at 203 dpi). */
export async function renderReceipt(lines: RLine[], widthDots: number): Promise<HTMLCanvasElement> {
  const base = widthDots >= 512 ? 24 : 20
  const pad = 8
  const inner = widthDots - pad * 2
  try {
    await Promise.all([document.fonts?.load(`${base}px "Noto Sans Lao"`), document.fonts?.load(`bold ${base}px "Noto Sans Lao"`)])
  } catch {}
  const images = new Map<string, HTMLImageElement | null>()
  for (const l of lines) if (l.k === 'image' && !images.has(l.url)) images.set(l.url, await loadImage(l.url))

  const measure = document.createElement('canvas').getContext('2d')!
  const font = (bold?: boolean, scale = 1) => `${bold ? 'bold ' : ''}${Math.round(base * scale)}px ${FONT}`
  function wrap(text: string, f: string, max: number): string[] {
    measure.font = f
    const words = text.split(/(\s+)/)
    const rows: string[] = []
    let cur = ''
    for (const w of words) {
      if (measure.measureText(cur + w).width <= max) cur += w
      else if (!cur.trim()) {
        // One long word (Lao has no spaces): split by characters.
        for (const ch of w) {
          if (measure.measureText(cur + ch).width > max && cur) {
            rows.push(cur)
            cur = ''
          }
          cur += ch
        }
      } else {
        rows.push(cur.trimEnd())
        cur = w.trimStart()
      }
    }
    if (cur.trim()) rows.push(cur.trimEnd())
    return rows.length ? rows : ['']
  }

  // Layout pass: list of draw operations with y positions.
  type Op = (ctx: CanvasRenderingContext2D, y: number) => void
  const ops: { h: number; draw: Op }[] = []
  for (const l of lines) {
    if (l.k === 'rule') {
      ops.push({ h: 14, draw: (c, y) => { c.setLineDash([6, 4]); c.lineWidth = 2; c.beginPath(); c.moveTo(pad, y + 7); c.lineTo(widthDots - pad, y + 7); c.stroke(); c.setLineDash([]) } })
    } else if (l.k === 'gap') {
      ops.push({ h: l.px, draw: () => {} })
    } else if (l.k === 'image') {
      const img = images.get(l.url)
      if (!img) continue
      const h = Math.min(96, img.height)
      const w = Math.min(inner, (img.width * h) / img.height)
      ops.push({ h: h + 6, draw: (c, y) => { c.filter = 'grayscale(1)'; c.drawImage(img, (widthDots - w) / 2, y, w, h); c.filter = 'none' } })
    } else if (l.k === 'text') {
      const f = font(l.bold, l.scale)
      const lh = Math.round(base * (l.scale ?? 1) * 1.35)
      for (const row of wrap(l.text, f, inner)) {
        ops.push({ h: lh, draw: (c, y) => {
          c.font = f
          c.textAlign = l.align ?? 'left'
          const x = l.align === 'center' ? widthDots / 2 : l.align === 'right' ? widthDots - pad : pad
          c.fillText(row, x, y + lh * 0.78)
        } })
      }
    } else {
      const f = font(l.bold, l.scale)
      const lh = Math.round(base * (l.scale ?? 1) * 1.35)
      measure.font = f
      const rightW = measure.measureText(l.right).width
      const x0 = pad + (l.indent ? base : 0)
      const leftRows = wrap(l.left, f, Math.max(40, inner - (l.indent ? base : 0) - rightW - base / 2))
      leftRows.forEach((row, i) => {
        ops.push({ h: lh, draw: (c, y) => {
          c.font = f
          c.textAlign = 'left'
          c.fillText(row, x0, y + lh * 0.78)
          if (i === 0) {
            c.textAlign = 'right'
            c.fillText(l.right, widthDots - pad, y + lh * 0.78)
          }
        } })
      })
    }
  }
  const height = ops.reduce((n, o) => n + o.h, 0) + pad * 2
  const canvas = document.createElement('canvas')
  canvas.width = widthDots
  canvas.height = height
  const ctx = canvas.getContext('2d')!
  ctx.fillStyle = '#fff'
  ctx.fillRect(0, 0, widthDots, height)
  ctx.fillStyle = '#000'
  ctx.strokeStyle = '#000'
  let y = pad
  for (const o of ops) {
    o.draw(ctx, y)
    y += o.h
  }
  return canvas
}

/** ESC/POS bytes for a canvas: init, raster image in bands, feed, cut, optional cash-drawer kick. */
export function canvasToEscPos(canvas: HTMLCanvasElement, opts: { cut?: boolean; drawer?: boolean; feedLines?: number } = {}): Uint8Array {
  const w = canvas.width
  const h = canvas.height
  const bytesPerRow = Math.ceil(w / 8)
  let px: Uint8ClampedArray
  try {
    px = canvas.getContext('2d')!.getImageData(0, 0, w, h).data
  } catch {
    // A logo from another site "tainted" the canvas: nothing to read. Caller retries without it.
    throw new Error('tainted')
  }
  const out: number[] = [0x1b, 0x40] // ESC @ — reset
  const BAND = 128
  for (let top = 0; top < h; top += BAND) {
    const rows = Math.min(BAND, h - top)
    out.push(0x1d, 0x76, 0x30, 0x00, bytesPerRow & 0xff, bytesPerRow >> 8, rows & 0xff, rows >> 8)
    for (let y = top; y < top + rows; y++) {
      for (let bx = 0; bx < bytesPerRow; bx++) {
        let byte = 0
        for (let bit = 0; bit < 8; bit++) {
          const x = bx * 8 + bit
          if (x >= w) continue
          const i = (y * w + x) * 4
          const lum = 0.299 * px[i]! + 0.587 * px[i + 1]! + 0.114 * px[i + 2]!
          if (lum < 160) byte |= 0x80 >> bit
        }
        out.push(byte)
      }
    }
  }
  out.push(0x1b, 0x64, opts.feedLines ?? 4) // ESC d n — feed
  if (opts.cut !== false) out.push(0x1d, 0x56, 0x42, 0x00) // GS V B 0 — partial cut after feed
  if (opts.drawer) out.push(0x1b, 0x70, 0x00, 0x19, 0xfa) // ESC p 0 — open cash drawer
  return new Uint8Array(out)
}
