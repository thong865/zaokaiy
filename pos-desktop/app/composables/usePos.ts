import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { AppInfo, Settings, SyncStatus } from '~/utils/types'
import { DEFAULT_SETTINGS } from '~/utils/types'
import { translate, type Lang } from '~/utils/i18n'

/** Call a Rust command; errors come back as plain strings. */
export async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args)
  } catch (e) {
    throw new Error(typeof e === 'string' ? e : (e as Error)?.message ?? String(e))
  }
}

let listening = false

/** App-wide state: pairing, shop, terminal, sync status, settings and language. */
export function usePos() {
  const info = useState<AppInfo | null>('pos:info', () => null)
  const status = useState<SyncStatus | null>('pos:status', () => null)
  const lang = useState<Lang>('pos:lang', () => 'en')
  /** Bumped after each sync that changed data, so product lists re-query stock. */
  const dataVersion = useState('pos:dataVersion', () => 0)

  const settings = computed<Settings>(() => {
    const s = (info.value?.settings ?? {}) as Partial<Settings>
    return { ...DEFAULT_SETTINGS, ...s, printer: { ...DEFAULT_SETTINGS.printer, ...(s.printer ?? {}) } }
  })
  const shop = computed(() => info.value?.shop ?? null)
  const device = computed(() => info.value?.device ?? null)
  const currency = computed(() => shop.value?.currency ?? 'LAK')
  const paired = computed(() => !!status.value?.paired)

  async function refresh() {
    info.value = await call<AppInfo>('app_info')
    status.value = info.value.status
    lang.value = info.value.lang === 'lo' ? 'lo' : 'en'
    if (import.meta.client) document.documentElement.lang = lang.value
    return info.value
  }

  async function startListening() {
    if (listening) return
    listening = true
    await listen<SyncStatus>('sync-status', async (e) => {
      const before = status.value
      status.value = e.payload
      if (!before || before.last_sync_at !== e.payload.last_sync_at || before.pending !== e.payload.pending) {
        dataVersion.value++
        // shop settings / terminal / cashier rights may have changed on the server
        await refresh().catch(() => {})
      }
    })
  }

  async function setLang(l: Lang) {
    lang.value = l
    document.documentElement.lang = l
    await call('set_lang', { lang: l })
  }

  async function saveSettings(next: Settings) {
    await call('save_settings', { settings: next })
    if (info.value) info.value = { ...info.value, settings: next }
  }

  async function syncNow() {
    status.value = { ...(status.value as SyncStatus), syncing: true }
    status.value = await call<SyncStatus>('sync_now')
    dataVersion.value++
    await refresh()
  }

  const t = (key: string, params?: Record<string, string | number>) => translate(lang.value, key, params)

  return { info, status, lang, settings, shop, device, currency, paired, dataVersion, refresh, startListening, setLang, saveSettings, syncNow, t }
}
