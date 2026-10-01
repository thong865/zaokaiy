/** Finance core types (API: /finance/policy, /shops/{id}/finance, /admin/finance/*). */

export interface TaxSettings {
  vat_bps: number
  ecommerce_tax_bps: number
  due_day: number
  tax_office: string
  agent_name: string
  agent_tax_id: string
  policy_version: string
  policy_en: string
  policy_lo: string
  updated_at: string
}

export interface Taxes {
  gross_cents: number
  vat_cents: number
  net_cents: number
  ecommerce_tax_cents: number
  total_due_cents: number
}

export interface SourceLine {
  key: string
  label: string
  gross_cents: number
  count: number
}

export interface Mandate {
  status: 'active' | 'revoked'
  policy_version: string
  signer_name: string
  signer_title: string
  tax_id: string
  vat_registered: boolean
  accepted_at: string
  revoked_at: string | null
  revoke_reason: string
}

export interface Filing extends Taxes {
  id: string
  period: string
  shop_id: string
  currency: string
  sources: SourceLine[]
  vat_bps: number
  ecommerce_tax_bps: number
  vat_registered: boolean
  due_on: string
  status: 'draft' | 'submitted' | 'paid' | 'void'
  reference_no: string
  note: string
  submitted_at: string | null
  paid_at: string | null
  shop_name?: string
  shop_slug?: string
  legal_name?: string
  tax_id?: string
}

export interface ShopFinance {
  shop: { id: string; name: string; legal_name: string; tax_id: string; currency: string; entity_type: string; kyb_verified: boolean }
  settings: TaxSettings
  mandate: Mandate | null
  policy_outdated: boolean
  preview: { period: string; vat_registered: boolean; lines: { currency: string; sources: SourceLine[]; taxes: Taxes }[] }
  filings: Filing[]
}

/** Tax-source keys → i18n label (falls back to the API label). */
export function sourceLabel(s: SourceLine) {
  const key = `finance.source.${s.key.replace('.', '_')}`
  const v = tr(key)
  return v === key ? s.label : v
}

const MONTHS_LO = ['ມັງກອນ', 'ກຸມພາ', 'ມີນາ', 'ເມສາ', 'ພຶດສະພາ', 'ມິຖຸນາ', 'ກໍລະກົດ', 'ສິງຫາ', 'ກັນຍາ', 'ຕຸລາ', 'ພະຈິກ', 'ທັນວາ']
const MONTHS_EN = ['January', 'February', 'March', 'April', 'May', 'June', 'July', 'August', 'September', 'October', 'November', 'December']
/** "2026-09-01" → "September 2026" / "ກັນຍາ 2026" (no Intl: browsers lack Lao data). */
export function monthLabel(iso: string) {
  const [y, m] = iso.slice(0, 7).split('-').map(Number)
  const names = intlLocale().startsWith('lo') ? MONTHS_LO : MONTHS_EN
  return `${names[(m ?? 1) - 1]} ${y}`
}
export const FILING_COLOR = { draft: 'neutral', submitted: 'info', paid: 'success', void: 'error' } as const
