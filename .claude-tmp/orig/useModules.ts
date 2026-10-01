/**
 * Optional plug-in modules (frontend `layers/<module>` + backend `src/modules/<module>`).
 * A layer declares its nav entries in its app.config (`zkModules.dashboardNav` / `adminNav`);
 * they show only when `GET /api/modules` says the backend module is enabled.
 */
export interface ModuleNavItem {
  module: string
  to: string
  labelKey: string
  icon: string
  /** Dashboard sidebar group (shop, social, network, grow). */
  group?: string
  /** Insert after the item with this path (default: end of the group). */
  after?: string
}
type ModuleStatus = Record<string, { enabled: boolean } & Record<string, unknown>>

export function useModules() {
  const api = useApi()
  const { t } = useI18n()
  const { data: status } = useLazyAsyncData<ModuleStatus>('zk-modules', () => api<ModuleStatus>('/modules').catch(() => ({})), {
    default: () => ({}),
  })
  const enabled = (m: string) => !!status.value?.[m]?.enabled
  const cfg = useAppConfig() as { zkModules?: { dashboardNav?: ModuleNavItem[]; adminNav?: ModuleNavItem[] } }
  const navItems = (where: 'dashboardNav' | 'adminNav') => (cfg.zkModules?.[where] ?? []).filter((i) => enabled(i.module))

  /** Core nav list + the enabled modules' entries for it, each placed after its `after` path. */
  function withModules<T extends { to: string }>(where: 'dashboardNav' | 'adminNav', items: T[], group?: string, make: (i: ModuleNavItem, label: string) => T = (i, label) => ({ to: i.to, label, icon: i.icon }) as unknown as T): T[] {
    const out = [...items]
    for (const m of navItems(where).filter((i) => !group || (i.group ?? 'shop') === group)) {
      const at = m.after ? out.findIndex((x) => x.to === m.after) : -1
      out.splice(at >= 0 ? at + 1 : out.length, 0, make(m, t(m.labelKey)))
    }
    return out
  }
  return { status, enabled, withModules }
}
