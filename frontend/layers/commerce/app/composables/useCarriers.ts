import type { Carrier } from '~/utils/types'

/** Active couriers (Anousith, HAL, Mixay, shop delivery …) with locale-aware names and tracking links. */
export function useCarriers() {
  const api = useApi()
  const { locale } = useI18n()
  const { data: carriers, refresh } = useAsyncData('carriers', () => api<Carrier[]>('/carriers'), { default: () => [] as Carrier[] })
  const byCode = (code: string | null | undefined) => carriers.value.find((c) => c.code === code)
  /** Lao name when the UI is Lao (falls back to the English name). */
  const carrierName = (code: string | null | undefined, c: { name: string; name_lo: string } | undefined = byCode(code)) =>
    c ? (locale.value === 'lo' && c.name_lo ? c.name_lo : c.name) : code || ''
  /** Public tracking page for a parcel, or null when the courier has none. */
  function trackingLink(code: string | null | undefined, tracking: string | null | undefined) {
    const c = byCode(code)
    const no = (tracking || '').trim().replace(/[^A-Za-z0-9_-]/g, '')
    if (!c?.tracking_url || !no) return null
    return c.tracking_url.includes('{tracking}') ? c.tracking_url.replaceAll('{tracking}', no) : c.tracking_url
  }
  return { carriers, refresh, byCode, carrierName, trackingLink }
}
