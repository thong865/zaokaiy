import type { Product } from '~/utils/types'

/** Products the current shop can promote: its own plus those it resells. */
export function useSellable() {
  const api = useApi()
  const { shopId } = useShop()
  return useAsyncData(
    'sellable',
    async () => {
      const [own, listed] = await Promise.all([
        api<Product[]>(`/shops/${shopId.value}/products`, { query: { status: 'active' } }),
        api<{ product_id: string; name: string; images: string[] }[]>(`/shops/${shopId.value}/listings`),
      ])
      return [
        ...own.map((p) => ({ id: p.id, name: p.name, image: p.images?.[0] ?? null, resold: false })),
        ...listed.map((l) => ({ id: l.product_id, name: l.name, image: l.images?.[0] ?? null, resold: true })),
      ]
    },
    { watch: [shopId], default: () => [] },
  )
}
