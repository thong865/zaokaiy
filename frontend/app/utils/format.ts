type I18nLike = { locale: { value: string }; t: (k: string, p?: Record<string, unknown>) => string }
/** Client-side handle, bound once by plugins/i18n-bind.ts. `tryUseNuxtApp()` is empty during template render in the browser. */
let clientI18n: I18nLike | null = null
export function bindI18n(i18n: I18nLike) {
  clientI18n = i18n
}
const i18n = (): I18nLike | undefined => (import.meta.client && clientI18n) || (tryUseNuxtApp()?.$i18n as I18nLike | undefined)

/** Current UI locale as a BCP-47 tag for Intl ('lo-LA' | 'en-US'). Works in setup, templates and computed (reactive). */
export function intlLocale(): string {
  return i18n()?.locale?.value === 'lo' ? 'lo-LA' : 'en-US'
}

/** Translate outside components (utils, plain functions). Inside components prefer `$t` / `useI18n().t`. */
export function tr(key: string, params?: Record<string, unknown>): string {
  const t = i18n()?.t
  return t ? t(key, params ?? {}) : key
}

/*
 * Lao dates/numbers are formatted here, not with Intl: Chrome/Chromium ship without Lao locale data
 * (Intl silently falls back to English in the browser while Node formats Lao → hydration mismatches).
 * Numbers always use Latin digits with ',' grouping in both languages.
 */
const NUM_LOCALE = 'en-US'
const isLo = (loc: string) => loc === 'lo' || loc.startsWith('lo-')

export function money(cents: number | null | undefined, currency = 'THB') {
  const v = (cents ?? 0) / 100
  try {
    const nf = new Intl.NumberFormat(NUM_LOCALE, { style: 'currency', currency, currencyDisplay: 'narrowSymbol' })
    const max = nf.resolvedOptions().maximumFractionDigits ?? 2
    return new Intl.NumberFormat(NUM_LOCALE, {
      style: 'currency',
      currency,
      currencyDisplay: 'narrowSymbol',
      minimumFractionDigits: 0,
      maximumFractionDigits: v % 1 === 0 ? 0 : max,
    }).format(v)
  } catch {
    return `${v.toFixed(2)} ${currency}`
  }
}

/** Plain number, Latin digits with ',' grouping. */
export const num = (n: number | null | undefined, digits = 0) =>
  new Intl.NumberFormat(NUM_LOCALE, { maximumFractionDigits: digits }).format(n ?? 0)

export const pct = (bps: number | null | undefined) => `${((bps ?? 0) / 100).toFixed(bps && bps % 100 ? 1 : 0)}%`

const LO_MONTHS = ['ມັງກອນ', 'ກຸມພາ', 'ມີນາ', 'ເມສາ', 'ພຶດສະພາ', 'ມິຖຸນາ', 'ກໍລະກົດ', 'ສິງຫາ', 'ກັນຍາ', 'ຕຸລາ', 'ພະຈິກ', 'ທັນວາ']
const LO_MONTHS_SHORT = ['ມ.ກ.', 'ກ.ພ.', 'ມ.ນ.', 'ມ.ສ.', 'ພ.ພ.', 'ມິ.ຖ.', 'ກ.ລ.', 'ສ.ຫ.', 'ກ.ຍ.', 'ຕ.ລ.', 'ພ.ຈ.', 'ທ.ວ.']
const LO_DAYS = ['ອາທິດ', 'ຈັນ', 'ອັງຄານ', 'ພຸດ', 'ພະຫັດ', 'ສຸກ', 'ເສົາ']
const pad = (n: number) => String(n).padStart(2, '0')

/** "5 min ago" / "5 ນາທີກ່ອນ". */
export function ago(iso: string) {
  const s = (Date.now() - new Date(iso).getTime()) / 1000
  if (isLo(intlLocale())) {
    if (s < 60) return 'ດຽວນີ້'
    if (s < 3600) return `${Math.floor(s / 60)} ນາທີກ່ອນ`
    if (s < 86400) return `${Math.floor(s / 3600)} ຊົ່ວໂມງກ່ອນ`
    if (s < 86400 * 2) return 'ມື້ວານ'
    if (s < 86400 * 30) return `${Math.floor(s / 86400)} ມື້ກ່ອນ`
    return fmtDate(iso)
  }
  const rtf = new Intl.RelativeTimeFormat('en-US', { numeric: 'auto', style: 'narrow' })
  if (s < 60) return rtf.format(0, 'second')
  if (s < 3600) return rtf.format(-Math.floor(s / 60), 'minute')
  if (s < 86400) return rtf.format(-Math.floor(s / 3600), 'hour')
  if (s < 86400 * 30) return rtf.format(-Math.floor(s / 86400), 'day')
  return fmtDate(iso)
}

type DateStyle = 'full' | 'long' | 'medium' | 'short'
type TimeStyle = DateStyle | 'none'

function loDate(d: Date, style: DateStyle) {
  const day = d.getDate(), m = d.getMonth(), y = d.getFullYear()
  if (style === 'short') return `${day}/${m + 1}/${y}`
  if (style === 'medium') return `${day} ${LO_MONTHS_SHORT[m]} ${y}`
  if (style === 'long') return `${day} ${LO_MONTHS[m]} ${y}`
  return `ວັນ${LO_DAYS[d.getDay()]} ທີ ${day} ${LO_MONTHS[m]} ${y}`
}
const loTime = (d: Date, seconds: boolean) => `${pad(d.getHours())}:${pad(d.getMinutes())}${seconds ? ':' + pad(d.getSeconds()) : ''}`

/**
 * Date/time formatting shared by screens and printed documents.
 * `loc` defaults to the UI locale; print docs pass their own ('lo' | 'lo-LA' | 'en-GB' …).
 */
export function fmtDate(iso: string | Date, style: DateStyle = 'medium', loc = intlLocale()) {
  const d = new Date(iso)
  return isLo(loc) ? loDate(d, style) : d.toLocaleDateString(loc, { dateStyle: style })
}
export function fmtDateTime(iso: string | Date, date: DateStyle = 'medium', time: TimeStyle = 'short', loc = intlLocale()) {
  const d = new Date(iso)
  if (time === 'none') return fmtDate(d, date, loc)
  if (isLo(loc)) return `${loDate(d, date)} ${loTime(d, time !== 'short')}`
  return d.toLocaleString(loc, { dateStyle: date, timeStyle: time })
}
/** Time only (HH:MM, or HH:MM:SS with seconds). */
export function fmtTime(iso: string | Date, seconds = false, loc = intlLocale()) {
  const d = new Date(iso)
  return isLo(loc) ? loTime(d, seconds) : d.toLocaleTimeString(loc, { hour: '2-digit', minute: '2-digit', ...(seconds ? { second: '2-digit' } : {}) })
}
/** "27 Sep" / "27 ກ.ຍ." (chart axes). */
export function fmtDayMonth(iso: string | Date, loc = intlLocale()) {
  const d = new Date(iso)
  return isLo(loc) ? `${d.getDate()} ${LO_MONTHS_SHORT[d.getMonth()]}` : d.toLocaleDateString(loc, { day: 'numeric', month: 'short' })
}
/** "Sun 14:05" / "ອາທິດ 14:05" (POS clock). */
export function fmtWeekdayTime(iso: string | Date, loc = intlLocale()) {
  const d = new Date(iso)
  return isLo(loc) ? `${LO_DAYS[d.getDay()]} ${loTime(d, false)}` : d.toLocaleString(loc, { weekday: 'short', hour: '2-digit', minute: '2-digit' })
}
/** "27 Sep 2026" with 2-digit day (invoices). */
export function fmtDocDate(iso: string | Date, loc = intlLocale()) {
  const d = new Date(iso)
  return isLo(loc) ? `${pad(d.getDate())} ${LO_MONTHS_SHORT[d.getMonth()]} ${d.getFullYear()}` : d.toLocaleDateString(loc, { day: '2-digit', month: 'short', year: 'numeric' })
}

export const shortId = (id: string) => id.slice(0, 8).toUpperCase()

/** Content kind label, e.g. kindLabel('video_script'). */
export const kindLabel = (k: string) => tr(`common.kind.${k}`)

export function bytes(n: number | null | undefined) {
  const v = n ?? 0
  if (v < 1024) return `${v} B`
  if (v < 1024 ** 2) return `${(v / 1024).toFixed(0)} KB`
  if (v < 1024 ** 3) return `${(v / 1024 ** 2).toFixed(1)} MB`
  return `${(v / 1024 ** 3).toFixed(2)} GB`
}

export const isVideoUrl = (url: string | null | undefined) => !!url && /\.(mp4|webm|mov)(\?|#|$)/i.test(url)

export const MAX_CATEGORY_DEPTH = 3

/** Review status label, e.g. reviewLabel('pending'). */
export const reviewLabel = (k: string) => tr(`common.review.${k}`)

/** All ids in the subtree rooted at `id` (nodes are a flat, pre-ordered tree). */
export function subtreeIds(nodes: { id: string; parent_id: string | null }[], id: string): Set<string> {
  const out = new Set([id])
  let grew = true
  while (grew) {
    grew = false
    for (const n of nodes) if (n.parent_id && out.has(n.parent_id) && !out.has(n.id)) { out.add(n.id); grew = true }
  }
  return out
}
