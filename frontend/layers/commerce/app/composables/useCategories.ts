import type { CategoryNode } from '~/utils/types'

/** Marketplace category tree (public, cached per request/session). */
export function useMarketplaceCategories() {
  const api = useApi()
  return useAsyncData('marketplace-categories', () => api<CategoryNode[]>('/categories'), {
    default: () => [],
    getCachedData: (key, nuxt) => nuxt.payload.data[key] ?? nuxt.static.data[key],
  })
}

/** The current shop's own category tree (owner view). */
export function useShopCategories() {
  const api = useApi()
  const { shopId } = useShop()
  return useAsyncData('shop-categories', () => (shopId.value ? api<CategoryNode[]>(`/shops/${shopId.value}/categories`) : Promise.resolve([])), {
    default: () => [],
    watch: [shopId],
  })
}
