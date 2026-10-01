// Types + helpers of the COD risk module (auto-imported inside the app).
export type RiskLevel = 'none' | 'low' | 'medium' | 'high' | 'blocked'
export type ReportStatus = 'pending' | 'confirmed' | 'dismissed' | 'withdrawn'
export const RISK_LEVELS: RiskLevel[] = ['none', 'low', 'medium', 'high', 'blocked']
export const REPORT_REASONS = ['refused', 'unreachable', 'fake_address', 'no_show', 'changed_mind', 'other'] as const

export interface RiskSignal { level: RiskLevel; confirmed_reports: number; shops: number }
export interface CodRiskOrder {
  kind: 'order' | 'social'
  id: string
  number: string
  customer: string
  phone: string
  carrier_code: string | null
  tracking_no: string
  cod_amount_cents: number
  currency: string
  cod_status: string
  order_status: string
  created_at: string
  updated_at: string
  risk: RiskSignal | null
  report?: { id: string; status: ReportStatus } | null
  reportable?: boolean
}
export interface CodRiskReport {
  id: string
  order_kind: 'order' | 'social'
  order_id: string
  order_number: string
  shop_id: string
  customer_key: string
  customer_name: string
  phone: string
  reason: string
  note: string
  amount_cents: number
  currency: string
  carrier_code: string | null
  tracking_no: string
  status: ReportStatus
  decision_note: string
  decided_at: string | null
  block_height: number | null
  created_at: string
  shop_name?: string
  level?: RiskLevel
  customer_confirmed?: number
  customer_reports?: number
}
export interface CodRiskCustomer {
  customer_key: string
  phone_hint: string
  phone: string | null
  name: string | null
  level: RiskLevel
  effective_level: RiskLevel
  level_note: string
  level_set_at: string | null
  level_expires_at: string | null
  confirmed: number
  pending: number
  shops: number
  updated_at: string
}
export interface CodRiskStats { confirmed: number; pending: number; dismissed: number; shops: number; confirmed_90d: number; amount_cents: number }
export interface LedgerEvent { height: number; hash: string; event: string; payload: Record<string, unknown>; at: string; anchor_tx: string | null }
export interface CodRiskCustomerDoc { customer: CodRiskCustomer; stats: CodRiskStats; suggested_level: RiskLevel; reports: CodRiskReport[]; ledger: LedgerEvent[] }
export interface CodRiskAnchor { id: number; from_height: number; to_height: number; merkle_root: string; driver: string; tx_ref: string; status: 'anchored' | 'failed'; error: string; created_at: string }

type Color = 'success' | 'info' | 'warning' | 'error' | 'neutral' | 'secondary'
export const riskColor = (l: string | null | undefined): Color =>
  (({ none: 'success', low: 'info', medium: 'warning', high: 'error', blocked: 'error' }) as Record<string, Color>)[l ?? 'none'] ?? 'neutral'
export const reportColor = (s: string): Color =>
  (({ pending: 'warning', confirmed: 'error', dismissed: 'neutral', withdrawn: 'neutral' }) as Record<string, Color>)[s] ?? 'neutral'
/** Ledger event name → i18n key suffix ("report.filed" → "report_filed"). */
export const eventKey = (e: string) => `codrisk.event.${e.replace('.', '_')}`
export const shortHash = (h: string | null | undefined, n = 10) => (h ? `${h.slice(0, n)}…` : '—')
