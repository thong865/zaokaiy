/** Promotions & loyalty points (backend module `promo`). */

export type PromoTrigger = 'signup' | 'first_order' | 'code'
export type RewardType = 'amount' | 'percent' | 'free_shipping' | 'points'
export type PromoChannel = 'web' | 'social' | 'pos'
export const PROMO_CHANNELS: PromoChannel[] = ['web', 'social', 'pos']
export const SIGNUP_METHODS = ['facebook', 'whatsapp', 'google', 'email'] as const

export interface Campaign {
  id: string
  shop_id: string | null
  name: string
  description: string
  trigger: PromoTrigger
  providers: string[]
  code: string | null
  reward_type: RewardType
  reward_value: number
  max_discount_cents: number
  min_spend_cents: number
  currency: string
  coupon_days: number
  max_grants: number
  grants: number
  channels: PromoChannel[]
  starts_at: string
  ends_at: string | null
  active: boolean
  created_at: string
  used: number
  discount_cents: number
}

export interface Applied {
  shop_id: string | null
  coupon_id: string | null
  coupon_code: string | null
  coupon_type: RewardType | null
  coupon_cents: number
  platform_cents: number
  points: number
  points_cents: number
  discount_cents: number
  points_balance: number | null
}

export interface CouponView {
  id: string
  code: string
  campaign: string
  description: string
  shop_id: string | null
  shop_name: string | null
  shop_slug: string | null
  reward_type: RewardType
  reward_value: number
  max_discount_cents: number
  min_spend_cents: number
  currency: string
  channels: PromoChannel[]
  status: 'active' | 'used' | 'expired' | 'void'
  expires_at: string
  used_at: string | null
}

export interface LoyaltySettings {
  enabled: boolean
  earn_per_cents: number
  point_value_cents: number
  min_redeem_points: number
  max_redeem_bps: number
  expiry_days: number
  channels: PromoChannel[]
  stats?: { members: number; outstanding_points: number; outstanding_cents: number }
}

export interface LoyaltyMember {
  id: string
  phone: string
  name: string
  balance: number
  lifetime_points: number
  orders: number
  last_activity_at: string
  created_at: string
}

export interface LedgerRow {
  id: number
  delta: number
  balance_after: number
  reason: 'earn' | 'redeem' | 'bonus' | 'adjust' | 'reverse' | 'expire'
  ref_type: string | null
  ref_id: string | null
  note: string
  created_at: string
}

/** "₭20,000 off", "10% off (max ₭50,000)", "Free shipping", "+20 points". */
export function rewardText(t: (k: string, p?: Record<string, unknown>) => string, r: { reward_type: RewardType; reward_value: number; max_discount_cents: number; currency: string }) {
  switch (r.reward_type) {
    case 'amount':
      return t('promo.reward.amountOff', { amount: money(r.reward_value, r.currency) })
    case 'percent': {
      const pct = (r.reward_value / 100).toLocaleString('en', { maximumFractionDigits: 2 })
      return r.max_discount_cents ? t('promo.reward.percentOffMax', { pct, max: money(r.max_discount_cents, r.currency) }) : t('promo.reward.percentOff', { pct })
    }
    case 'free_shipping':
      return t('promo.reward.freeShipping')
    case 'points':
      return t('promo.reward.points', { n: r.reward_value })
  }
}

/**
 * State shared with the core pages (cart, chat-order checkout, POS). The core only reads `body`
 * (sent as `promo` with the order) and `discount` (preview, by shop id; '' for single-shop pages).
 */
export interface PromoHookState {
  body: Record<string, unknown> | null
  discount: Record<string, number>
}
export const usePromoHookState = (key: string) => useState<PromoHookState>(key, () => ({ body: null, discount: {} }))
