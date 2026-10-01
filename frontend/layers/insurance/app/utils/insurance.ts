/** Insurance core types (API: /insurance/*, /shops/{id}/insurance/*). */

export const PLAN_KINDS = ['motor', 'health', 'travel', 'life', 'property', 'accident', 'other'] as const
export const KIND_ICON: Record<string, string> = {
  motor: 'car', health: 'heart-pulse', travel: 'plane', life: 'hand-heart', property: 'house', accident: 'bandage', other: 'shield',
}

export interface Plan {
  id: string
  shop_id: string
  kind: string
  insurer: string
  name: string
  name_lo: string
  description: string
  coverage: string[]
  premium_mode: 'fixed' | 'rate'
  premium_cents: number
  rate_bps: number
  min_premium_cents: number
  sum_insured_min_cents: number
  sum_insured_max_cents: number
  term_months: number
  active: boolean
  // public listing
  shop_name?: string
  shop_slug?: string
  currency?: string
  shop_phone?: string
  shop_verified?: boolean
}

export interface Application {
  id: string
  number: string
  plan_id: string
  applicant_name: string
  phone: string
  email: string
  details: Record<string, string | number | boolean>
  sum_insured_cents: number
  premium_cents: number
  currency: string
  start_date: string
  end_date: string
  status: string
  policy_no: string
  paid: boolean
  decision_note: string
  created_at: string
  plan_name?: string
  insurer?: string
  kind?: string
  shop_name?: string
  shop_phone?: string
}

export const planName = (p: { name: string; name_lo: string }, locale: string) => (locale === 'lo' && p.name_lo ? p.name_lo : p.name)

/** Same rule as the API: fixed premium, or rate × sum insured with a minimum. */
export function estimate(p: Plan, sumInsuredCents: number): number | null {
  if (p.premium_mode === 'rate') {
    if (!sumInsuredCents) return null
    return Math.max(p.min_premium_cents, Math.round((sumInsuredCents * p.rate_bps) / 10000))
  }
  return Math.max(p.premium_cents, p.min_premium_cents)
}
