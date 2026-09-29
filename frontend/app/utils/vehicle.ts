/** Vehicle listings: option lists (API values), labels, loan maths and contact links. */
import { num, tr } from './format'

export const VEHICLE_TYPES = ['car', 'motorbike', 'truck', 'van', 'other'] as const
export const CONDITIONS = ['used', 'new', 'reconditioned'] as const
export const FUELS = ['petrol', 'diesel', 'hybrid', 'electric', 'lpg', 'other'] as const
export const TRANSMISSIONS = ['automatic', 'manual', 'cvt', 'semi_auto'] as const
export const DRIVES = ['fwd', 'rwd', '4wd', 'awd'] as const
export const SALE_STATUSES = ['available', 'reserved', 'sold'] as const
export const LEAD_KINDS = ['enquiry', 'test_drive', 'offer', 'reserve', 'finance'] as const
export const LEAD_STATUSES = ['new', 'contacted', 'scheduled', 'won', 'lost'] as const
export const CAR_BODIES = ['sedan', 'hatchback', 'suv', 'crossover', 'pickup', 'mpv', 'van', 'coupe', 'convertible', 'wagon', 'truck', 'bus', 'other']
export const BIKE_BODIES = ['scooter', 'underbone', 'sport', 'naked', 'cruiser', 'touring', 'offroad', 'other']
export const bodiesFor = (type: string) => (type === 'motorbike' ? BIKE_BODIES : CAR_BODIES)

export const CAR_MAKES = [
  'Toyota', 'Honda', 'Hyundai', 'Kia', 'Ford', 'Isuzu', 'Mitsubishi', 'Nissan', 'Mazda', 'Suzuki', 'Chevrolet', 'Lexus',
  'Mercedes-Benz', 'BMW', 'Audi', 'Volkswagen', 'Volvo', 'Subaru', 'Peugeot', 'BYD', 'MG', 'Changan', 'Chery', 'GWM',
  'Haval', 'Geely', 'Wuling', 'Tesla', 'Land Rover', 'Porsche', 'Kolao', 'Hino', 'Fuso', 'Dongfeng', 'Foton', 'JAC',
]
export const BIKE_MAKES = ['Honda', 'Yamaha', 'Suzuki', 'Kawasaki', 'Vespa', 'SYM', 'Kymco', 'Ducati', 'Harley-Davidson', 'Royal Enfield', 'KTM', 'Triumph', 'BMW', 'Kolao', 'Deco']
export const makesFor = (type: string) => (type === 'motorbike' ? BIKE_MAKES : CAR_MAKES)

/** Lao provinces (plate province / location suggestions). Proper names, not translated. */
export const LAO_PROVINCES = [
  'Vientiane Capital', 'Vientiane', 'Luang Prabang', 'Savannakhet', 'Champasak', 'Khammouane', 'Bolikhamxay', 'Xayaboury',
  'Oudomxay', 'Luang Namtha', 'Bokeo', 'Phongsaly', 'Houaphanh', 'Xiengkhouang', 'Xaysomboun', 'Salavan', 'Sekong', 'Attapeu',
]

export interface VehicleLike {
  make: string
  model: string
  variant?: string
  year: number
}
/** "2020 Toyota Hilux Revo 2.4 E" */
export const vehicleTitle = (v: VehicleLike, withVariant = true) =>
  [v.year, v.make, v.model, withVariant ? v.variant : ''].filter(Boolean).join(' ')

export const km = (n: number | null | undefined) => tr('vehicle.unit.km', { n: num(n ?? 0) })

/** Translated option label, falling back to the raw value. */
export function optLabel(group: 'type' | 'types' | 'condition' | 'fuel' | 'transmission' | 'drive' | 'body' | 'sale', v: string | null | undefined) {
  if (!v) return ''
  const k = `vehicle.${group}.${v}`
  const s = tr(k)
  return s === k ? v : s
}

/**
 * Flat-rate car loan (common with Lao banks / leasing): interest = principal × rate × years,
 * repaid in equal monthly instalments. All amounts in minor units.
 */
export function flatLoan(price: number, downPct: number, months: number, ratePct: number) {
  const down = Math.round((price * Math.min(100, Math.max(0, downPct))) / 100)
  const principal = Math.max(0, price - down)
  const interest = Math.round((principal * (ratePct / 100) * months) / 12)
  const monthly = months > 0 ? Math.ceil((principal + interest) / months) : 0
  return { down, principal, interest, monthly, total: principal + interest }
}

/** Digits-only phone for wa.me / tel: links (Lao local 020… → 85620…). */
export function intlDigits(phone: string | null | undefined): string {
  const t = (phone ?? '').trim()
  let d = t.replace(/\D/g, '')
  if (t.startsWith('00')) d = d.slice(2)
  else if (!t.startsWith('+') && d.startsWith('0')) d = '856' + d.slice(1)
  return d.length >= 8 ? d : ''
}
export const waLink = (phone: string | null | undefined, text: string) => {
  const d = intlDigits(phone)
  return d ? `https://wa.me/${d}?text=${encodeURIComponent(text)}` : ''
}
export const telLink = (phone: string | null | undefined) => {
  const d = intlDigits(phone)
  return d ? `tel:+${d}` : ''
}
