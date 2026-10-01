import type { Shop } from '~/utils/types'
import type { PagePermission } from '~/utils/access'

/** The seller's shops and the one currently selected in the dashboard. */
export function useShop() {
  const api = useApi()
  const shops = useState<Shop[]>('shops', () => [])
  const loaded = useState('shops:loaded', () => false)
  const selectedId = useCookie<string | null>('zk_shop', { maxAge: 60 * 60 * 24 * 365, sameSite: 'lax' })

  const shop = computed<Shop | null>(
    () => shops.value.find((s) => s.id === selectedId.value) ?? shops.value[0] ?? null,
  )
  const shopId = computed(() => shop.value?.id ?? '')

  async function loadShops(force = false) {
    if (loaded.value && !force) return shops.value
    shops.value = await api<Shop[]>('/me/shops')
    loaded.value = true
    if (!shops.value.find((s) => s.id === selectedId.value)) selectedId.value = shops.value[0]?.id ?? null
    return shops.value
  }

  function select(id: string) {
    selectedId.value = id
  }

  async function createShop(body: Partial<Shop>) {
    const s = await api<Shop>('/shops', { method: 'POST', body })
    shops.value.push({ ...s, access: OWNER_ACCESS })
    selectedId.value = s.id
    return s
  }

  /** Staff permissions of the selected shop (owners and admins can do everything). */
  const access = computed(() => shop.value?.access ?? OWNER_ACCESS)
  const isOwner = computed(() => access.value.owner)
  const can = (perm: PagePermission) => allowed(shop.value?.access, perm)
  /** Put a shop returned by the API back into the list, keeping the caller's access info. */
  function replaceShop(updated: Shop) {
    const i = shops.value.findIndex((s) => s.id === updated.id)
    if (i >= 0) shops.value[i] = { ...updated, access: shops.value[i]!.access }
  }

  return { shops, shop, shopId, loadShops, select, createShop, access, isOwner, can, replaceShop }
}
