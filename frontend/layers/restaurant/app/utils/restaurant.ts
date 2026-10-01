/** Restaurant core types + helpers (API: /restaurant/*, /shops/{id}/restaurant/*). */

export interface RestaurantSettings {
  accepting_orders: boolean
  dine_in: boolean
  takeaway: boolean
  delivery: boolean
  service_charge_bps: number
  prep_minutes: number
}

export interface MenuSection {
  id: string
  shop_id: string
  name: string
  name_lo: string
  position: number
}

export interface MenuItem {
  id: string
  shop_id: string
  section_id: string | null
  name: string
  name_lo: string
  description: string
  price_cents: number
  image_url: string
  spicy: number
  available: boolean
  position: number
}

export interface DiningTable {
  id: string
  shop_id: string
  label: string
  seats: number
  token: string
  active: boolean
}

export type OrderMode = 'dine_in' | 'takeaway' | 'delivery'
export const KITCHEN_FLOW = ['new', 'accepted', 'preparing', 'ready', 'served', 'completed'] as const

export interface FoodOrder {
  id: string
  shop_id: string
  number: number
  day: string
  mode: OrderMode
  table_label: string
  customer_name: string
  customer_phone: string
  address: string
  note: string
  status: string
  subtotal_cents: number
  service_cents: number
  total_cents: number
  currency: string
  paid: boolean
  payment_method: string
  track_token: string
  created_at: string
  items: { id: string; name: string; unit_price_cents: number; qty: number; note: string }[]
  shop?: { name: string; slug: string; phone: string; prep_minutes: number }
}

export interface Place {
  id: string
  slug: string
  name: string
  description: string
  logo_url: string | null
  currency: string
  address: string
  verified: boolean
  open: boolean
  delivery: boolean
  prep_minutes: number
  dishes: number
  from_cents: number | null
  images: string[]
}

/** Dish / section name in the current language. */
export const localName = (x: { name: string; name_lo: string }, locale: string) => (locale === 'lo' && x.name_lo ? x.name_lo : x.name)

/** The next kitchen step for the board's main button. */
export function nextStep(status: string, mode: OrderMode): string | null {
  const flow = mode === 'dine_in' ? KITCHEN_FLOW : KITCHEN_FLOW.filter((s) => s !== 'served')
  const i = flow.indexOf(status as never)
  return i >= 0 && i < flow.length - 1 ? flow[i + 1]! : null
}
