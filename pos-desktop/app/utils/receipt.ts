/**
 * Receipts are drawn on a canvas and printed as a bitmap: thermal printers have no Lao (and often
 * no Thai) character set, but a picture prints every script the same. The same canvas is shown on
 * screen as the preview.
 */
import type { SaleRecord } from './types'
import { translate, type Lang } from './i18n'
import { dateTime, money, pct } from './format'

type Row =
  | { k: 'text'; text: string; align?: 'left' | 'center' | 'right'; bold?: boolean; size?: number }
  | { k: 'pair'; left: string; right: string; bold?: boolean; size?: number }
  | { k: 'rule'; dashed?: boolean }
  | { k: 'gap'; h?: number }

const FONT = '"Noto Sans Lao", "Plus Jakarta Sans", "Noto Sans Thai", sans-serif'

/** Paper width in printer dots (203 dpi): 80 mm → 576, 58 mm → 384. */
export const paperDots = (paper: 58 | 80) => (paper === 58 ? 384 : 576)

function paint(rows: Row[], width: number): HTMLCanvasElement {
  const scale = width / 576
  const base = 22 * scale
  const pad = 8 * scale
  const inner = width - pad * 2
  const measure = document.createElement('canvas').getContext('2d')!
  const font = (size: number, bold?: boolean) => `${bold ? 700 : 400} ${size}px ${FONT}`

  // Word-wrap by measuring (Lao has no spaces between words: fall back to per-character breaks).
  const wrap = (text: string, size: number, bold: boolean | undefined, max: number): string[] => {
    measure.font = font(size, bold)
    const out: string[] = []
    for (const para of text.split('\n')) {
      let line = ''
      for (const token of para.split(/(\s+)/)) {
        const tryLine = line + token
        if (measure.measureText(tryLine).width <= max) {
          line = tryLine
          continue
        }
        if (line.trim()) out.push(line.trimEnd())
        line = token.trimStart()
        while (measure.measureText(line).width > max) {
          let i = line.length
          while (i > 1 && measure.measureText(line.slice(0, i)).width > max) i--
          out.push(line.slice(0, i))
          line = line.slice(i)
        }
      }
      out.push(line.trimEnd())
    }
    return out
  }

  type Laid = { h: number; draw: (ctx: CanvasRenderingContext2D, y: number) => void }
  const laid: Laid[] = rows.map((r) => {
    if (r.k === 'gap') return { h: (r.h ?? 8) * scale, draw: () => {} }
    if (r.k === 'rule') {
      return {
        h: 14 * scale,
        draw: (ctx, y) => {
          ctx.setLineDash(r.dashed === false ? [] : [6 * scale, 4 * scale])
          ctx.lineWidth = Math.max(1, 2 * scale)
          ctx.beginPath()
          ctx.moveTo(pad, y + 7 * scale)
          ctx.lineTo(width - pad, y + 7 * scale)
          ctx.stroke()
          ctx.setLineDash([])
        },
      }
    }
    const size = (r.size ?? 1) * base
    const lh = size * 1.5
    if (r.k === 'text') {
      const lines = wrap(r.text, size, r.bold, inner)
      return {
        h: lines.length * lh,
        draw: (ctx, y) => {
          ctx.font = font(size, r.bold)
          ctx.textAlign = r.align ?? 'left'
          const x = r.align === 'center' ? width / 2 : r.align === 'right' ? width - pad : pad
          lines.forEach((l, i) => ctx.fillText(l, x, y + i * lh + size * 1.1))
        },
      }
    }
    // pair: left text wraps in the space the right-hand amount leaves free
    measure.font = font(size, r.bold)
    const rightW = measure.measureText(r.right).width
    const lines = wrap(r.left, size, r.bold, Math.max(inner - rightW - 12 * scale, inner * 0.4))
    return {
      h: lines.length * lh,
      draw: (ctx, y) => {
        ctx.font = font(size, r.bold)
        ctx.textAlign = 'left'
        lines.forEach((l, i) => ctx.fillText(l, pad, y + i * lh + size * 1.1))
        ctx.textAlign = 'right'
        ctx.fillText(r.right, width - pad, y + (lines.length - 1) * lh + size * 1.1)
      },
    }
  })

  const height = Math.ceil(laid.reduce((s, l) => s + l.h, 0) + pad * 2)
  const canvas = document.createElement('canvas')
  canvas.width = width
  canvas.height = height
  const ctx = canvas.getContext('2d')!
  ctx.fillStyle = '#fff'
  ctx.fillRect(0, 0, width, height)
  ctx.fillStyle = '#000'
  ctx.strokeStyle = '#000'
  let y = pad
  for (const l of laid) {
    l.draw(ctx, y)
    y += l.h
  }
  return canvas
}

export function receiptCanvas(rec: SaleRecord, lang: Lang, paper: 58 | 80): HTMLCanvasElement {
  const t = (k: string, p?: Record<string, string | number>) => translate(lang, k, p)
  const s = rec.sale
  const shop = s.print?.shop
  const cur = s.currency
  const m = (c: number) => money(c, cur)
  const rows: Row[] = []
  if (shop) {
    rows.push({ k: 'text', text: shop.name, align: 'center', bold: true, size: 1.35 })
    if (shop.legal_name && shop.legal_name !== shop.name) rows.push({ k: 'text', text: shop.legal_name, align: 'center' })
    if (shop.branch) rows.push({ k: 'text', text: shop.branch, align: 'center', size: 0.9 })
    if (shop.address) rows.push({ k: 'text', text: shop.address, align: 'center', size: 0.9 })
    if (shop.phone) rows.push({ k: 'text', text: shop.phone, align: 'center', size: 0.9 })
    if (shop.tax_id) rows.push({ k: 'text', text: `${t('receipt.taxId')} ${shop.tax_id}`, align: 'center', size: 0.9 })
  }
  rows.push({ k: 'gap' }, { k: 'text', text: shop?.tax_id ? t('receipt.taxReceipt') : t('receipt.receipt'), align: 'center', bold: true })
  if (rec.status === 'voided') rows.push({ k: 'text', text: `*** ${t('receipt.voided')} ***`, align: 'center', bold: true, size: 1.3 })
  rows.push(
    { k: 'pair', left: t('receipt.no'), right: s.number },
    { k: 'pair', left: t('receipt.date'), right: dateTime(s.created_at, lang) },
  )
  if (s.print?.cashier) rows.push({ k: 'pair', left: t('receipt.cashier'), right: s.print.cashier })
  if (s.print?.device_code) rows.push({ k: 'pair', left: t('receipt.terminal'), right: s.print.device_code })
  rows.push({ k: 'rule' })
  for (const it of s.items) {
    rows.push({ k: 'text', text: it.name })
    rows.push({ k: 'pair', left: `  ${it.qty} × ${m(it.unit_price_cents)}`, right: m(it.unit_price_cents * it.qty), size: 0.95 })
    if (it.discount_cents) rows.push({ k: 'pair', left: `  ${t('receipt.discount')}`, right: `-${m(it.discount_cents)}`, size: 0.95 })
  }
  rows.push({ k: 'rule' })
  if (s.discount_cents) {
    rows.push({ k: 'pair', left: translate(lang, 'till.subtotal'), right: m(s.subtotal_cents) })
    rows.push({ k: 'pair', left: t('receipt.discount'), right: `-${m(s.discount_cents)}` })
  }
  rows.push({ k: 'pair', left: translate(lang, 'till.total'), right: m(s.total_cents), bold: true, size: 1.35 })
  if (s.vat_bps) {
    const net = s.subtotal_cents - s.discount_cents
    const before = s.prices_include_vat ? net - s.vat_cents : net
    rows.push({ k: 'pair', left: t('receipt.beforeVat'), right: m(before), size: 0.9 })
    rows.push({ k: 'pair', left: translate(lang, s.prices_include_vat ? 'till.vatIncl' : 'till.vat', { rate: pct(s.vat_bps) }), right: m(s.vat_cents), size: 0.9 })
  }
  rows.push({ k: 'rule' })
  for (const p of s.payments) {
    rows.push({ k: 'pair', left: translate(lang, `pay.method.${p.method}`) + (p.reference ? ` (${p.reference})` : ''), right: m(p.amount_cents) })
  }
  if (s.change_cents) rows.push({ k: 'pair', left: t('receipt.change'), right: m(s.change_cents), bold: true })
  if (s.customer?.name) rows.push({ k: 'gap' }, { k: 'text', text: `${translate(lang, 'till.customer')}: ${s.customer.name}${s.customer.phone ? ` · ${s.customer.phone}` : ''}`, size: 0.9 })
  if (s.note) rows.push({ k: 'text', text: s.note, size: 0.9 })
  if (shop?.receipt_footer) rows.push({ k: 'gap', h: 12 }, { k: 'text', text: shop.receipt_footer, align: 'center' })
  rows.push({ k: 'gap', h: 16 })
  return paint(rows, paperDots(paper))
}

export interface Summary {
  count: number
  total_cents: number
  vat_cents: number
  voided: number
  voided_cents: number
  methods: Record<string, number>
  top: { name: string; qty: number; amount_cents: number }[]
}

export function summaryCanvas(sum: Summary, day: string, shopName: string, terminal: string, currency: string, lang: Lang, paper: 58 | 80) {
  const t = (k: string, p?: Record<string, string | number>) => translate(lang, k, p)
  const m = (c: number) => money(c, currency)
  const rows: Row[] = [
    { k: 'text', text: shopName, align: 'center', bold: true, size: 1.3 },
    { k: 'text', text: `${t('sales.zreport')} · ${terminal}`, align: 'center' },
    { k: 'text', text: day, align: 'center' },
    { k: 'rule' },
    { k: 'pair', left: t('sales.count'), right: String(sum.count) },
    { k: 'pair', left: t('sales.takings'), right: m(sum.total_cents), bold: true },
    { k: 'pair', left: translate(lang, 'till.vat', { rate: '' }).trim(), right: m(sum.vat_cents) },
    { k: 'pair', left: t('sales.voids'), right: `${sum.voided} · ${m(sum.voided_cents)}` },
    { k: 'rule' },
    { k: 'text', text: t('sales.byMethod'), bold: true },
    ...Object.entries(sum.methods).map(([k, v]) => ({ k: 'pair' as const, left: translate(lang, `pay.method.${k}`), right: m(v) })),
    { k: 'rule' },
    { k: 'text', text: t('sales.top'), bold: true },
    ...sum.top.map((x) => ({ k: 'pair' as const, left: `${x.qty} × ${x.name}`, right: m(x.amount_cents), size: 0.95 })),
    { k: 'gap', h: 16 },
  ]
  return paint(rows, paperDots(paper))
}

export function textCanvas(text: string, paper: 58 | 80) {
  return paint([{ k: 'text', text, align: 'center' }, { k: 'rule' }, { k: 'gap', h: 16 }], paperDots(paper))
}

/** 1-bit raster for ESC/POS `GS v 0`: rows of packed bits, MSB first, 1 = black. */
export function toRaster(canvas: HTMLCanvasElement) {
  const { width, height } = canvas
  const px = canvas.getContext('2d')!.getImageData(0, 0, width, height).data
  const wb = Math.ceil(width / 8)
  const out = new Uint8Array(wb * height)
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const i = (y * width + x) * 4
      const lum = 0.299 * px[i]! + 0.587 * px[i + 1]! + 0.114 * px[i + 2]!
      if (lum < 150) out[y * wb + (x >> 3)]! |= 0x80 >> (x & 7)
    }
  }
  let bin = ''
  for (let i = 0; i < out.length; i += 0x8000) bin += String.fromCharCode(...out.subarray(i, i + 0x8000))
  return { widthBytes: wb, height, data: btoa(bin) }
}
