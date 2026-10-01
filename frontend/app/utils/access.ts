/**
 * Staff permissions in the dashboard. The API enforces them on every route (backend/crates/kernel/src/access.rs);
 * this map only decides which menu entries and pages a staff member sees.
 */
export const PERMISSIONS = ['stats', 'products', 'inventory', 'media', 'orders', 'delivery', 'pos', 'pos_void', 'social', 'leads', 'marketing', 'network', 'settings'] as const
export type Permission = (typeof PERMISSIONS)[number]
/** 'owner' = only the shop owner (staff management, business verification). */
export type PagePermission = Permission | 'owner'

export interface ShopAccess {
  owner: boolean
  role_id: string | null
  role: string
  permissions: string[]
}
export const OWNER_ACCESS: ShopAccess = { owner: true, role_id: null, role: 'owner', permissions: [...PERMISSIONS] }

/** Dashboard page → permission it needs. Longest matching prefix wins. */
const PAGE_PERM: Record<string, PagePermission> = {
  '/dashboard': 'stats',
  '/dashboard/products': 'products',
  '/dashboard/categories': 'products',
  '/dashboard/barcodes': 'products',
  '/dashboard/inventory': 'inventory',
  '/dashboard/media': 'media',
  '/dashboard/leads': 'leads',
  '/dashboard/orders': 'orders',
  '/dashboard/shipping': 'delivery',
  '/dashboard/cod': 'delivery',
  '/dashboard/cod-risk': 'delivery',
  '/pos': 'pos',
  '/dashboard/pos-sales': 'pos',
  '/dashboard/social': 'social',
  '/dashboard/partners': 'network',
  '/dashboard/marketplace': 'network',
  '/dashboard/commissions': 'network',
  '/dashboard/ads': 'marketing',
  '/dashboard/content': 'marketing',
  '/dashboard/assistant': 'marketing',
  '/dashboard/settings': 'settings',
  '/dashboard/verification': 'owner',
  '/dashboard/staff': 'owner',
}

/** Cores add their dashboard pages at startup (see `CoreDef.pages` in composables/useCores.ts). */
export function addPagePermissions(pages: Record<string, PagePermission>) {
  Object.assign(PAGE_PERM, pages)
}

export function pagePermission(path: string): PagePermission {
  const p = path.replace(/\/+$/, '') || '/'
  let best = ''
  for (const k of Object.keys(PAGE_PERM)) {
    if ((p === k || p.startsWith(`${k}/`)) && k.length > best.length) best = k
  }
  return best ? PAGE_PERM[best]! : 'owner'
}

export function allowed(access: ShopAccess | null | undefined, perm: PagePermission): boolean {
  if (!access || access.owner) return true
  return perm !== 'owner' && access.permissions.includes(perm)
}

/** Where to send a staff member who opens a page they can't use: their first allowed page. */
export const LANDING_ORDER = ['/dashboard', '/pos', '/dashboard/orders', '/dashboard/products', '/dashboard/inventory', '/dashboard/social', '/dashboard/cod',
  '/dashboard/leads', '/dashboard/media', '/dashboard/content', '/dashboard/partners', '/dashboard/settings']
