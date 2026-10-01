import type { DiningTable, MenuItem, MenuSection, RestaurantSettings } from '#restaurant/utils/restaurant'

export interface RestaurantOverview {
  shop: { id: string; slug: string; name: string; currency: string; vertical: string }
  settings: RestaurantSettings
  sections: MenuSection[]
  items: MenuItem[]
  tables: DiningTable[]
  today: { orders: number; open: number; revenue_cents: number }
}

/** The current shop's restaurant setup (settings, menu, tables, today's numbers). */
export function useRestaurant() {
  const api = useApi()
  const { shopId } = useShop()
  const res = useAsyncData(`restaurant:${shopId.value}`, () => api<RestaurantOverview>(`/shops/${shopId.value}/restaurant`), { watch: [shopId] })
  return { ...res, api, shopId }
}
