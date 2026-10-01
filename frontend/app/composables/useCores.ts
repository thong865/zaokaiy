import type { Component } from 'vue'
import { addPagePermissions, type PagePermission } from '~/utils/access'

/**
 * Super-app core registry (front end). Each layer in `layers/<core>` registers what it adds to
 * the shell from a plugin (`registerCore`); the shell reads it to build the home screen,
 * dashboard / admin navigation, shop onboarding and per-shop-type pages.
 *
 * A core only shows when the API has it enabled (`GET /api/cores`), so turning a core off on
 * the server hides it here too.
 */
export interface CoreNavItem {
  to: string
  /** i18n key */
  label: string
  /** Lucide icon name (without the `i-lucide-` prefix) */
  icon: string
  /** Shop types that see the item (omit = all). */
  verticals?: string[]
  /** Only for business shops. */
  business?: boolean
  order?: number
}

export interface CoreNavGroup {
  id: string
  /** i18n key (the first core to register a group names it) */
  label: string
  order: number
  items: CoreNavItem[]
}

export interface CoreOnboarding {
  id: string
  /** i18n keys */
  label: string
  hint: string
  vertical: string
  kind?: 'seller' | 'creator'
  /** Pre-tick "registered business" (goes on to corporate KYC). */
  business?: boolean
  order: number
}

export interface CoreDef {
  name: string
  order: number
  /** Super-app home tile. */
  tile?: { to: string; label: string; hint: string; icon: string; color: string }
  onboarding?: CoreOnboarding[]
  nav?: CoreNavGroup[]
  adminNav?: { to: string; label: string; icon: string; badge?: string; order: number }[]
  /** shop vertical → storefront body for /s/[slug] */
  storefronts?: Record<string, Component>
  /** shop vertical → dashboard overview */
  overviews?: Record<string, Component>
  /** Sections on the home screen, below the tiles. */
  homeSections?: { order: number; component: Component }[]
  /** Listing extension: when the product payload carries `key`, this component renders the product page. */
  productDetail?: { key: string; component: Component }
  /** Staff permission each dashboard page needs (path prefix → permission; 'owner' = owner only). */
  pages?: Record<string, PagePermission>
}

export interface ApiCore {
  name: string
  title: string
  title_lo: string
  icon: string
  verticals: string[]
  served_by: string
}

const defs = new Map<string, CoreDef>()

export function registerCore(def: CoreDef) {
  defs.set(def.name, def)
  if (def.pages) addPagePermissions(def.pages)
}

export function useCores() {
  const enabled = useState<ApiCore[] | null>('zk-cores', () => null)
  const all = computed(() =>
    [...defs.values()]
      .filter((d) => !enabled.value || enabled.value.some((c) => c.name === d.name))
      .sort((a, b) => a.order - b.order),
  )
  const has = (name: string) => all.value.some((d) => d.name === name)

  /** Dashboard navigation for a shop, merged across cores. */
  function nav(vertical: string | undefined, business: boolean) {
    const groups = new Map<string, CoreNavGroup>()
    for (const d of all.value) {
      for (const g of d.nav ?? []) {
        const cur = groups.get(g.id) ?? { ...g, items: [] }
        cur.items.push(...g.items.filter((i) => (!i.verticals || (vertical && i.verticals.includes(vertical))) && (!i.business || business)))
        groups.set(g.id, cur)
      }
    }
    return [...groups.values()]
      .map((g) => ({ ...g, items: [...g.items].sort((a, b) => (a.order ?? 50) - (b.order ?? 50)) }))
      .filter((g) => g.items.length)
      .sort((a, b) => a.order - b.order)
  }

  const adminNav = computed(() => all.value.flatMap((d) => d.adminNav ?? []).sort((a, b) => a.order - b.order))
  const tiles = computed(() => all.value.filter((d) => d.tile).map((d) => ({ name: d.name, ...d.tile! })))
  const onboarding = computed(() => all.value.flatMap((d) => d.onboarding ?? []).sort((a, b) => a.order - b.order))
  const homeSections = computed(() => all.value.flatMap((d) => d.homeSections ?? []).sort((a, b) => a.order - b.order))
  const storefront = (vertical?: string) => (vertical ? all.value.find((d) => d.storefronts?.[vertical])?.storefronts?.[vertical] : undefined)
  const overview = (vertical?: string) => (vertical ? all.value.find((d) => d.overviews?.[vertical])?.overviews?.[vertical] : undefined)
  /** Product page renderer from a listing extension present in the payload. */
  const productDetail = (payload: Record<string, unknown> | null | undefined) =>
    all.value.find((d) => d.productDetail && payload?.[d.productDetail.key])?.productDetail?.component

  return { enabled, all, has, nav, adminNav, tiles, onboarding, homeSections, storefront, overview, productDetail }
}
