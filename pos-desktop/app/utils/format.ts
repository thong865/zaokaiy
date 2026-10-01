/** Money is integer minor units (×100), like the server. Latin digits in both languages. */
export function money(cents: number | null | undefined, currency = 'LAK') {
  const v = (cents ?? 0) / 100
  try {
    return new Intl.NumberFormat('en-US', {
      style: 'currency',
      currency,
      currencyDisplay: 'narrowSymbol',
      minimumFractionDigits: 0,
      maximumFractionDigits: v % 1 === 0 ? 0 : 2,
    }).format(v)
  } catch {
    return `${v.toFixed(2)} ${currency}`
  }
}

/** Plain number without currency symbol ("12,500"). */
export const num = (cents: number, digits = 2) => new Intl.NumberFormat('en-US', { maximumFractionDigits: digits }).format(cents / 100)

/** Parse what the cashier typed ("12,500" / "12.5") into minor units. */
export function toCents(input: string | number | null | undefined): number {
  if (input === null || input === undefined || input === '') return 0
  const n = typeof input === 'number' ? input : Number(String(input).replace(/[,\s]/g, ''))
  return Number.isFinite(n) ? Math.round(n * 100) : 0
}

export const pct = (bps: number) => `${(bps / 100).toFixed(bps % 100 ? 1 : 0)}%`

const LO_MONTHS_SHORT = ['ມ.ກ.', 'ກ.ພ.', 'ມ.ນ.', 'ມ.ສ.', 'ພ.ພ.', 'ມິ.ຖ.', 'ກ.ລ.', 'ສ.ຫ.', 'ກ.ຍ.', 'ຕ.ລ.', 'ພ.ຈ.', 'ທ.ວ.']
const EN_MONTHS_SHORT = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec']
const pad = (n: number) => String(n).padStart(2, '0')

/** "30 Sep 2026 14:05" / "30 ກ.ຍ. 2026 14:05" (Lao formatted by hand: Chromium has no Lao Intl data). */
export function dateTime(iso: string, lang: 'en' | 'lo' = 'en') {
  const d = new Date(iso)
  const m = (lang === 'lo' ? LO_MONTHS_SHORT : EN_MONTHS_SHORT)[d.getMonth()]
  return `${d.getDate()} ${m} ${d.getFullYear()} ${pad(d.getHours())}:${pad(d.getMinutes())}`
}

export const time = (iso: string) => {
  const d = new Date(iso)
  return `${pad(d.getHours())}:${pad(d.getMinutes())}`
}

export function ago(iso: string | null | undefined, lang: 'en' | 'lo' = 'en') {
  if (!iso) return ''
  const s = Math.max(0, (Date.now() - new Date(iso).getTime()) / 1000)
  if (lang === 'lo') {
    if (s < 60) return 'ດຽວນີ້'
    if (s < 3600) return `${Math.floor(s / 60)} ນາທີກ່ອນ`
    if (s < 86400) return `${Math.floor(s / 3600)} ຊົ່ວໂມງກ່ອນ`
    return `${Math.floor(s / 86400)} ມື້ກ່ອນ`
  }
  if (s < 60) return 'just now'
  if (s < 3600) return `${Math.floor(s / 60)} min ago`
  if (s < 86400) return `${Math.floor(s / 3600)} h ago`
  return `${Math.floor(s / 86400)} d ago`
}

/** Local calendar day [start, end) as ISO instants, for filtering sales stored in UTC. */
export function dayRange(ymd: string): [string, string] {
  const [y, m, d] = ymd.split('-').map(Number)
  const start = new Date(y!, m! - 1, d!)
  const end = new Date(y!, m! - 1, d! + 1)
  return [start.toISOString(), end.toISOString()]
}

export function todayYmd() {
  const d = new Date()
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`
}

/** Quick cash buttons: exact amount plus the next notes up (per currency). */
export function quickCash(dueCents: number, currency: string): number[] {
  const notes: Record<string, number[]> = {
    LAK: [1000, 2000, 5000, 10000, 20000, 50000, 100000],
    THB: [20, 50, 100, 500, 1000],
    USD: [1, 5, 10, 20, 50, 100],
  }
  const list = (notes[currency] ?? notes.USD!).map((n) => n * 100)
  const out = new Set<number>([dueCents])
  for (const n of list) {
    const up = Math.ceil(dueCents / n) * n
    if (up > dueCents) out.add(up)
    if (out.size >= 5) break
  }
  return [...out].sort((a, b) => a - b).slice(0, 5)
}
