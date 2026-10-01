/**
 * POS open bills: several sales in progress at once (a table, a customer still shopping, a parked
 * sale). Bills live on the server so every till of the shop sees the same list; each edit is saved
 * a moment later with the version the till last saw, and a save that lost a race (409) reloads the
 * bill instead of overwriting another till's change.
 */
export interface PosBillData<L = unknown> {
  lines: L[]
  billDiscount: number
  note: string
  customer: { name: string; tax_id: string; branch: string; address: string; phone: string }
}
export interface PosBill<L = unknown> {
  id: string
  number: number
  label: string
  version: number
  data: PosBillData<L>
  created_by_name: string | null
  updated_by_name: string | null
  updated_at: string
}

export const emptyBillData = <L>(): PosBillData<L> => ({
  lines: [],
  billDiscount: 0,
  note: '',
  customer: { name: '', tax_id: '', branch: '', address: '', phone: '' },
})

function normalize<L>(d: Partial<PosBillData<L>> | null | undefined): PosBillData<L> {
  const e = emptyBillData<L>()
  return { ...e, ...(d ?? {}), customer: { ...e.customer, ...(d?.customer ?? {}) }, lines: Array.isArray(d?.lines) ? d!.lines : [] }
}

export function usePosBills<L>(shopId: Ref<string>, onNotice: (msg: string, bad?: boolean) => void) {
  const api = useApi()
  const { t } = useI18n()
  const bills = ref<PosBill<L>[]>([]) as Ref<PosBill<L>[]>
  const activeId = ref<string | null>(null)
  const dirty = ref(new Set<string>())
  const saving = ref(new Set<string>())
  const offline = ref(false)
  const ready = ref(false)
  const active = computed(() => bills.value.find((b) => b.id === activeId.value) ?? null)
  const activeKey = computed(() => `zk_pos_bill:${shopId.value}`)
  const timers = new Map<string, ReturnType<typeof setTimeout>>()

  const fix = (b: PosBill<L>): PosBill<L> => ({ ...b, data: normalize<L>(b.data) })
  function setActive(id: string | null) {
    activeId.value = id
    try {
      if (id) localStorage.setItem(activeKey.value, id)
    } catch {}
  }

  async function create(data?: PosBillData<L>, label = ''): Promise<PosBill<L> | null> {
    try {
      const b = fix(await api<PosBill<L>>(`/shops/${shopId.value}/pos/bills`, { method: 'POST', body: { label, data: data ?? emptyBillData<L>() } }))
      bills.value = [...bills.value, b].sort((a, c) => a.number - c.number)
      offline.value = false
      return b
    } catch (e) {
      onNotice(apiError(e), true)
      return null
    }
  }

  async function load() {
    ready.value = false
    bills.value = []
    activeId.value = null
    if (!shopId.value) return
    try {
      bills.value = (await api<PosBill<L>[]>(`/shops/${shopId.value}/pos/bills`)).map(fix)
      offline.value = false
    } catch (e) {
      onNotice(apiError(e), true)
      return
    }
    // One-time move of the cart the old single-bill till kept in this browser.
    const legacyKey = `zk_pos_cart:${shopId.value}`
    try {
      const legacy = JSON.parse(localStorage.getItem(legacyKey) || 'null')
      if (legacy?.lines?.length) {
        const b = await create({ ...emptyBillData<L>(), lines: legacy.lines, billDiscount: legacy.billDiscount ?? 0 })
        if (b) localStorage.removeItem(legacyKey)
      } else if (legacy) {
        localStorage.removeItem(legacyKey)
      }
    } catch {}
    if (!bills.value.length) await create()
    let saved: string | null = null
    try {
      saved = localStorage.getItem(activeKey.value)
    } catch {}
    setActive(bills.value.find((b) => b.id === saved)?.id ?? bills.value[0]?.id ?? null)
    ready.value = true
  }

  async function save(id: string) {
    const b = bills.value.find((x) => x.id === id)
    if (!b || saving.value.has(id)) return
    saving.value = new Set(saving.value).add(id)
    const sent = JSON.stringify(b.data)
    try {
      const r = fix(await api<PosBill<L>>(`/pos/bills/${id}`, { method: 'PUT', body: { label: b.label, data: b.data, version: b.version } }))
      const cur = bills.value.find((x) => x.id === id)
      if (cur) {
        cur.version = r.version
        cur.updated_by_name = r.updated_by_name
        cur.updated_at = r.updated_at
        // Edited again while saving? keep it dirty so the newer data is saved next.
        if (JSON.stringify(cur.data) === sent) {
          const d = new Set(dirty.value)
          d.delete(id)
          dirty.value = d
        }
      }
      offline.value = false
    } catch (e) {
      const status = (e as { status?: number; statusCode?: number })?.status ?? (e as { statusCode?: number })?.statusCode
      if (status === 409) {
        // Another till changed it: take theirs.
        await reload(id)
        onNotice(t('pos.bills.conflict'), true)
      } else if (status === 404) {
        drop(id)
        onNotice(t('pos.bills.closedElsewhere'), true)
      } else {
        offline.value = true
      }
    } finally {
      const s = new Set(saving.value)
      s.delete(id)
      saving.value = s
      if (dirty.value.has(id) && !offline.value) schedule(id)
    }
  }

  /** Mark a bill changed and save it shortly. */
  function schedule(id: string, delay = 700) {
    dirty.value = new Set(dirty.value).add(id)
    clearTimeout(timers.get(id))
    timers.set(id, setTimeout(() => save(id), delay))
  }

  async function reload(id: string) {
    try {
      const fresh = fix(await api<PosBill<L>>(`/pos/bills/${id}`))
      bills.value = bills.value.map((b) => (b.id === id ? fresh : b))
      const d = new Set(dirty.value)
      d.delete(id)
      dirty.value = d
      if (id === activeId.value) onActiveReplaced?.()
    } catch {
      drop(id)
    }
  }

  function drop(id: string) {
    clearTimeout(timers.get(id))
    const i = bills.value.findIndex((b) => b.id === id)
    bills.value = bills.value.filter((b) => b.id !== id)
    const d = new Set(dirty.value)
    d.delete(id)
    dirty.value = d
    if (activeId.value === id) {
      const next = bills.value[Math.min(i, bills.value.length - 1)]
      setActive(next?.id ?? null)
      onActiveReplaced?.()
    }
  }

  /** Pull the shared list: new bills from other tills appear, closed ones disappear, and bills this
   *  till isn't editing take the other tills' latest version. */
  async function sync() {
    if (!ready.value || !shopId.value) return
    let server: PosBill<L>[]
    try {
      server = (await api<PosBill<L>[]>(`/shops/${shopId.value}/pos/bills`)).map(fix)
      offline.value = false
    } catch {
      offline.value = true
      return
    }
    for (const id of [...dirty.value]) if (!saving.value.has(id)) save(id) // retry after being offline
    const ids = new Set(server.map((b) => b.id))
    const activeGone = !!activeId.value && !ids.has(activeId.value) && !dirty.value.has(activeId.value)
    let activeChanged = false
    bills.value = server.map((s) => {
      const mine = bills.value.find((b) => b.id === s.id)
      if (mine && (dirty.value.has(s.id) || saving.value.has(s.id) || mine.version >= s.version)) return mine
      if (s.id === activeId.value) activeChanged = true
      return s
    }).concat(bills.value.filter((b) => !ids.has(b.id) && dirty.value.has(b.id)))
    if (activeGone) {
      setActive(bills.value[0]?.id ?? null)
      onNotice(t('pos.bills.closedElsewhere'), true)
      activeChanged = true
    }
    if (!bills.value.length) {
      const b = await create()
      if (b) setActive(b.id)
      activeChanged = true
    }
    if (activeChanged) onActiveReplaced?.()
  }

  async function newBill() {
    const b = await create()
    if (b) {
      setActive(b.id)
      onActiveReplaced?.()
    }
  }

  async function remove(id: string) {
    try {
      await api(`/pos/bills/${id}`, { method: 'DELETE' })
    } catch (e) {
      if ((e as { status?: number })?.status !== 404) {
        onNotice(apiError(e), true)
        return
      }
    }
    drop(id)
    if (!bills.value.length) await newBill()
  }

  /** The sale closed this bill on the server. */
  async function closedBySale(id: string) {
    drop(id)
    if (!bills.value.length) await newBill()
  }

  function rename(id: string, label: string) {
    const b = bills.value.find((x) => x.id === id)
    if (!b) return
    b.label = label.trim().slice(0, 40)
    schedule(id, 0)
  }

  function activate(id: string) {
    if (id === activeId.value) return
    setActive(id)
    onActiveReplaced?.()
  }

  let onActiveReplaced: (() => void) | undefined
  /** Called whenever the active bill's contents come from elsewhere (switch, reload, sync). */
  const onActiveChange = (fn: () => void) => (onActiveReplaced = fn)

  let poll: ReturnType<typeof setInterval> | undefined
  onMounted(() => {
    poll = setInterval(sync, 10_000)
  })
  onBeforeUnmount(() => {
    clearInterval(poll)
    // Flush pending edits.
    for (const [id, tm] of timers) {
      clearTimeout(tm)
      if (dirty.value.has(id)) save(id)
    }
  })

  /** Resolves once the bills are loaded and one is active (right after opening the POS). */
  async function whenReady() {
    if (!ready.value) await new Promise<void>((res) => { const stop = watch(ready, (v) => { if (v) { stop(); res() } }) })
    if (!activeId.value) await newBill()
  }

  return { whenReady, bills, active, activeId, dirty, saving, offline, ready, load, sync, newBill, remove, rename, activate, schedule, closedBySale, onActiveChange }
}
