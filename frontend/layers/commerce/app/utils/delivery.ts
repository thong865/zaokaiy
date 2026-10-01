import type { DeliveryType, FeePayer, ShippingOption } from '~/utils/types'

/** A buyer's delivery choice for one shop (v-model of <DeliveryPicker>). */
export interface DeliveryChoice {
  carrier_code: string
  delivery_type: DeliveryType
  cod: boolean
}

/** Price of one delivery choice — mirrors backend `logistics::quote`. */
export interface DeliveryQuote {
  /** Who pays the courier fee after the free-shipping rule. */
  fee_payer: FeePayer
  /** Courier fee for this parcel (branch or home). */
  fee_cents: number
  /** Part of the fee added to what the buyer pays (at checkout, or to the courier with COD). */
  charged_fee_cents: number
  cod: boolean
  cod_fee_cents: number
}

export function quote(option: ShippingOption, deliveryType: DeliveryType, cod: boolean, subtotalCents: number): DeliveryQuote {
  const fee = deliveryType === 'home' && option.home_fee_cents != null ? option.home_fee_cents : option.fee_cents
  const free = option.free_over_cents != null && subtotalCents >= option.free_over_cents
  const payer: FeePayer = free ? 'seller' : option.fee_payer
  const useCod = cod && option.cod_enabled && option.supports_cod
  return {
    fee_payer: payer,
    fee_cents: fee,
    charged_fee_cents: payer === 'buyer' ? fee : 0,
    cod: useCod,
    cod_fee_cents: useCod ? option.cod_fee_cents : 0,
  }
}

/** First option, branch pickup, paid now (not COD). */
export function defaultChoice(options: ShippingOption[] | null | undefined): DeliveryChoice | null {
  const o = options?.[0]
  return o ? { carrier_code: o.carrier_code, delivery_type: 'branch', cod: false } : null
}

/** Quote for a choice among options; null when the choice doesn't match an option. */
export function quoteChoice(options: ShippingOption[] | null | undefined, choice: DeliveryChoice | null | undefined, subtotalCents: number) {
  const o = choice && options?.find((x) => x.carrier_code === choice.carrier_code)
  return o ? quote(o, choice.delivery_type, choice.cod, subtotalCents) : null
}

/** Option name in the UI language (Lao name when locale is 'lo'). */
export const optionName = (o: { name: string; name_lo: string }, locale: string) => (locale === 'lo' && o.name_lo ? o.name_lo : o.name)

export const codAvailable = (o: ShippingOption) => o.cod_enabled && o.supports_cod
