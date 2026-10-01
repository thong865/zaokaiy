<script setup lang="ts">
import type { PayMethod, PosDoc, PosPayment } from '~/utils/types'

definePageMeta({ layout: 'pos', middleware: 'seller' })
const { t } = useI18n()
useHead({ title: () => `${t('pos.title')} · zaokaiy` })

const api = useApi()
const { shop, shopId, shops, select } = useShop()
const { user } = useAuth()
const { print } = usePrint()
const { data: shopCats } = await useShopCategories()
const cur = computed(() => shop.value?.currency ?? 'THB')

// ---------------------------------------------------------------- products
interface Hit { id: string; name: string; sku: string; barcode: string | null; price_cents: number; stock: number; image: string | null; status: string; shop_category_id: string | null; exact: boolean }
const q = ref('')
const cat = ref<string | null>(null)
const hits = ref<Hit[]>([])
const searchEl = ref<HTMLInputElement | null>(null)
const loading = ref(false)
let seq = 0
async function search() {
  if (!shopId.value) return
  const my = ++seq
  loading.value = true
  try {
    const r = await api<Hit[]>(`/shops/${shopId.value}/pos/products`, { query: { q: q.value || undefined, shop_category_id: cat.value || undefined, limit: 90 } })
    if (my === seq) hits.value = r
  } finally {
    if (my === seq) loading.value = false
  }
}
let timer: ReturnType<typeof setTimeout> | undefined
watch(q, () => { clearTimeout(timer); timer = setTimeout(search, 180) })
watch([cat, shopId], search)
onMounted(search)

function resetSearch() {
  q.value = ''
  // Don't wait for the debounce: restore the full grid right away for the next scan/tap.
  nextTick(() => {
    clearTimeout(timer)
    search()
  })
}

// ---------------------------------------------------------------- barcode scanning
const { beep, enabled: scanSound } = useScanSound()
/** "3*8851234000012", "3x…" or "3×…" = add 3 at once. */
const QTY_RE = /^(\d{1,4})\s*[*xX×]\s*(\S.*)$/
const looksLikeCode = (s: string) => /^[0-9A-Za-z\-_.]{6,}$/.test(s) && /\d/.test(s)
const lastAdded = ref<{ id: string; at: number } | null>(null)
const lastScan = ref<{ code: string; ok: boolean; name?: string } | null>(null)
const camera = ref(false)

function splitQty(raw: string): { code: string; qty: number } {
  const m = QTY_RE.exec(raw.trim())
  return m ? { code: m[2]!.trim(), qty: Math.min(Math.max(Number(m[1]), 1), 9999) } : { code: raw.trim(), qty: 1 }
}

/** Exact lookup (barcode in any GTIN length, or SKU). Returns true when a product was added. */
async function scanCode(code: string, qty: number, billId?: string | null): Promise<boolean | 'error'> {
  try {
    const r = await api<{ code: string; product: Hit | null }>(`/shops/${shopId.value}/pos/scan`, { query: { code } })
    if (!r.product) return false
    add(r.product, qty, billId)
    beep(true)
    const mark = { id: r.product.id, at: Date.now() }
    lastAdded.value = mark
    setTimeout(() => { if (lastAdded.value === mark) lastAdded.value = null }, 1200)
    lastScan.value = { code: r.code, ok: true, name: r.product.name }
    flash(qty > 1 ? t('pos.scan.addedQty', { qty, name: r.product.name }) : t('pos.scan.added', { name: r.product.name }))
    return true
  } catch (e) {
    beep(false)
    flash(apiError(e), true)
    return 'error'
  }
}

/** A code from a hardware scanner (anywhere on the page, whatever has the cursor) or the camera.
 *  The item goes to the bill that is open when the code is scanned. */
async function onScanned(raw: string) {
  const { code, qty } = splitQty(raw)
  if (!code || !shopId.value) return
  // Scanning the next customer's first item right after a sale starts the new sale.
  if (done.value) newSale()
  if (custom.open) custom.open = false
  // Just opened the POS: wait until the bills are loaded, so the item isn't overwritten.
  await posBills.whenReady()
  const r = await scanCode(code, qty, posBills.activeId.value)
  if (r === false) unknownCode(code, qty)
}
// Scanners of this till: keyboard-mode (USB / Bluetooth HID) always on, plus HID-POS / serial devices you add.
// While the payment or another dialog is open, scans are caught (never typed into the dialog) but not added.
const scanners = useScannerDevices(
  onScanned,
  computed(() => !paying.value && !unknown.open && !invForm.open),
  () => { beep(false); flash(paying.value ? t('pos.scan.blockedPaying') : t('pos.scan.blocked'), true) },
)
const scanHelp = ref(false)
const scannerName = computed(() => {
  const d = scanners.primary.value
  return d ? d.name || (d.kind === 'keyboard' ? t('pos.scanners.defaultName') : d.reported || t(`pos.scanners.kind.${d.kind}`)) : ''
})
const scannerOk = computed(() => {
  const d = scanners.primary.value
  return !!d && (d.kind === 'keyboard' ? d.status !== 'error' : d.status === 'connected')
})

/** Enter in the search box: a scanned/typed code, or pick the single search result. */
async function onEnter() {
  const { code, qty } = splitQty(q.value)
  if (!code) return
  clearTimeout(timer)
  const r = await scanCode(code, qty)
  if (r === true) return resetSearch()
  if (r === 'error') return
  q.value = code
  await search()
  if (hits.value.length === 1) {
    add(hits.value[0]!, qty)
    beep(true)
    resetSearch()
  } else if (looksLikeCode(code)) {
    unknownCode(code, qty)
    resetSearch()
  } else {
    flash(t('pos.noMatch', { code }), true)
  }
}

// Unknown barcode: assign it to an existing product right here (then it scans next time).
const unknown = reactive({ open: false, code: '', qty: 1, q: '', results: [] as Hit[], busy: false, error: '' })
function unknownCode(code: string, qty: number) {
  beep(false)
  lastScan.value = { code, ok: false }
  Object.assign(unknown, { open: true, code, qty, q: '', results: [], error: '' })
}
let unknownTimer: ReturnType<typeof setTimeout> | undefined
watch(() => unknown.q, (v) => {
  clearTimeout(unknownTimer)
  unknownTimer = setTimeout(async () => {
    if (!v.trim()) return (unknown.results = [])
    unknown.results = await api<Hit[]>(`/shops/${shopId.value}/pos/products`, { query: { q: v.trim(), limit: 8 } }).catch(() => [])
  }, 200)
})
async function assignBarcode(h: Hit) {
  unknown.busy = true
  unknown.error = ''
  try {
    await api(`/products/${h.id}`, { method: 'PATCH', body: { barcode: unknown.code } })
    add({ ...h, barcode: unknown.code }, unknown.qty)
    beep(true)
    flash(t('pos.scan.assigned', { code: unknown.code, name: h.name }))
    unknown.open = false
  } catch (e) {
    unknown.error = apiError(e)
  } finally {
    unknown.busy = false
  }
}

// ---------------------------------------------------------------- cart
interface Line { key: string; product_id: string | null; name: string; sku: string; unit: number; base: number; qty: number; discount: number; stock: number | null }
const lines = ref<Line[]>([])
const billDiscount = ref(0) // cents
const note = ref('')
const customer = reactive({ name: '', tax_id: '', branch: '', address: '', phone: '' })
const expanded = ref<string | null>(null)

// ---------------------------------------------------------------- open bills
// The cart above is the working copy of the active bill; other bills wait in tabs. Every change is
// saved to the server, so all tills of the shop share the same open bills.
const posBills = usePosBills<Line>(shopId, (m, bad) => flash(m, bad))
let applying = false
function loadActive() {
  const d = posBills.active.value?.data ?? emptyBillData<Line>()
  applying = true
  lines.value = JSON.parse(JSON.stringify(d.lines))
  billDiscount.value = d.billDiscount
  note.value = d.note
  Object.assign(customer, d.customer)
  expanded.value = null
  discountInput.value = ''
  nextTick(() => (applying = false))
}
posBills.onActiveChange(loadActive)
watch([lines, billDiscount, note, customer], () => {
  const b = posBills.active.value
  if (applying || !b) return
  b.data = JSON.parse(JSON.stringify({ lines: lines.value, billDiscount: billDiscount.value, note: note.value, customer }))
  posBills.schedule(b.id)
}, { deep: true })
onMounted(async () => {
  await posBills.load()
  loadActive()
})
watch(shopId, async () => {
  await posBills.load()
  loadActive()
})
const billItems = (b: { data: { lines: { qty: number }[] } }) => b.data.lines.reduce((n, l) => n + (Number(l.qty) || 0), 0)
const billName = (b: { label: string; number: number }) => b.label || t('pos.bills.default', { n: b.number })
const renaming = ref<string | null>(null)
const renameDraft = ref('')
function startRename(b: { id: string; label: string; number: number }) {
  renaming.value = b.id
  renameDraft.value = b.label
  nextTick(() => (document.getElementById('bill-rename') as HTMLInputElement | null)?.select())
}
function finishRename() {
  if (renaming.value) posBills.rename(renaming.value, renameDraft.value)
  renaming.value = null
}
async function cancelBill(id: string) {
  const b = posBills.bills.value.find((x) => x.id === id)
  if (b && b.data.lines.length && !confirm(t('pos.bills.cancelConfirm', { name: billName(b) }))) return
  await posBills.remove(id)
}
function nextBill() {
  const list = posBills.bills.value
  const i = list.findIndex((b) => b.id === posBills.activeId.value)
  const n = list[(i + 1) % list.length]
  if (n) posBills.activate(n.id)
}

function add(h: Hit, qty = 1, billId?: string | null) {
  // The cashier switched bills while the scan was being looked up: put it on the bill it was scanned into.
  if (billId && billId !== posBills.activeId.value) {
    const b = posBills.bills.value.find((x) => x.id === billId)
    if (b) {
      const ls = b.data.lines
      const ex = ls.find((l) => l.product_id === h.id && l.unit === h.price_cents && !l.discount)
      if (ex) ex.qty = Math.min(ex.qty + qty, 10000)
      else ls.push({ key: crypto.randomUUID(), product_id: h.id, name: h.name, sku: h.sku, unit: h.price_cents, base: h.price_cents, qty, discount: 0, stock: h.stock })
      posBills.schedule(b.id)
      flash(t('pos.scan.addedToBill', { name: h.name, bill: billName(b) }))
      return
    }
  }
  const existing = lines.value.find((l) => l.product_id === h.id && l.unit === h.price_cents && !l.discount)
  if (existing) existing.qty = Math.min(existing.qty + qty, 10000)
  else lines.value.push({ key: crypto.randomUUID(), product_id: h.id, name: h.name, sku: h.sku, unit: h.price_cents, base: h.price_cents, qty, discount: 0, stock: h.stock })
  const qtyInCart = lines.value.filter((l) => l.product_id === h.id).reduce((n, l) => n + l.qty, 0)
  if (qtyInCart > h.stock) flash(t('pos.onlyInStock', { n: h.stock, name: h.name }), true)
  nextTick(() => document.getElementById('cart-end')?.scrollIntoView({ block: 'nearest' }))
}
function setQty(l: Line, qty: number) {
  if (qty <= 0) lines.value = lines.value.filter((x) => x.key !== l.key)
  else l.qty = Math.min(qty, 10000)
}
const lineTotal = (l: Line) => Math.max(0, l.unit * l.qty - l.discount)
const subtotal = computed(() => lines.value.reduce((n, l) => n + lineTotal(l), 0))
const bps = computed(() => shop.value?.vat_bps ?? 0)
const inclusive = computed(() => shop.value?.prices_include_vat ?? true)
// Plug-in module `promo` (layers/promo): member points / coupon from the `pos.promo` slot, taken off like a bill discount.
const promoHook = useState<{ body: Record<string, unknown> | null; discount: Record<string, number> }>('zk-promo:pos', () => ({ body: null, discount: {} }))
const promoOff = computed(() => Math.min(promoHook.value.discount[''] ?? 0, Math.max(0, subtotal.value - billDiscount.value)))
const net = computed(() => Math.max(0, subtotal.value - billDiscount.value - promoOff.value))
const divRound = (a: number, b: number) => Math.floor((a * 2 + b) / (b * 2))
const vat = computed(() => (bps.value ? (inclusive.value ? divRound(net.value * bps.value, 10000 + bps.value) : divRound(net.value * bps.value, 10000)) : 0))
const total = computed(() => (inclusive.value ? net.value : net.value + vat.value))
const units = computed(() => lines.value.reduce((n, l) => n + l.qty, 0))

// money inputs are in major units in the UI
const toCents = (v: string | number) => Math.round((Number(v) || 0) * 100)
const discountMode = ref<'amount' | 'percent'>('amount')
const discountInput = ref('')
function applyBillDiscount() {
  const v = Number(discountInput.value) || 0
  billDiscount.value = Math.min(subtotal.value, discountMode.value === 'percent' ? Math.round((subtotal.value * Math.min(v, 100)) / 100) : toCents(v))
}
watch(subtotal, () => { if (billDiscount.value > subtotal.value) billDiscount.value = subtotal.value })

const custom = reactive({ open: false, name: '', price: '' })
function addCustom() {
  if (!custom.name.trim() || custom.price === '') return
  lines.value.push({ key: crypto.randomUUID(), product_id: null, name: custom.name.trim(), sku: '', unit: toCents(custom.price), base: toCents(custom.price), qty: 1, discount: 0, stock: null })
  Object.assign(custom, { open: false, name: '', price: '' })
}
function clearSale() {
  lines.value = []
  billDiscount.value = 0
  discountInput.value = ''
  note.value = ''
  Object.assign(customer, { name: '', tax_id: '', branch: '', address: '', phone: '' })
  expanded.value = null
  promoHook.value = { body: null, discount: {} }
}

// ---------------------------------------------------------------- payment
const paying = ref(false)
const payments = ref<PosPayment[]>([])
const method = ref<PayMethod>('cash')
const amount = ref('')
const reference = ref('')
const issueInvoice = ref(false)
const busy = ref(false)
const error = ref('')
const paid = computed(() => payments.value.reduce((n, p) => n + p.amount_cents, 0))
const remaining = computed(() => Math.max(0, total.value - paid.value))
const change = computed(() => Math.max(0, paid.value - total.value))
const methods: { id: PayMethod; key: string }[] = [
  { id: 'cash', key: '1' },
  { id: 'card', key: '2' },
  { id: 'transfer', key: '3' },
  { id: 'qr', key: '4' },
  { id: 'other', key: '5' },
]
const quickCash = computed(() => {
  const due = remaining.value / 100
  const notes = [20, 50, 100, 500, 1000]
  const out = new Set<number>([due])
  for (const n of notes) out.add(Math.ceil(due / n) * n)
  return [...out].filter((v) => v >= due && v > 0).sort((a, b) => a - b).slice(0, 5)
})
function openPay() {
  if (!lines.value.length) return
  error.value = ''
  payments.value = []
  method.value = 'cash'
  amount.value = ''
  reference.value = ''
  issueInvoice.value = !!customer.name && !!customer.address
  paying.value = true
  nextTick(() => document.getElementById('pay-amount')?.focus())
}
function addPayment(value?: number) {
  const cents = value !== undefined ? Math.round(value * 100) : amount.value === '' ? remaining.value : toCents(amount.value)
  if (cents <= 0) return
  if (method.value !== 'cash' && cents > remaining.value) {
    error.value = t('pos.errExceed')
    return
  }
  error.value = ''
  payments.value.push({ method: method.value, amount_cents: cents, reference: reference.value || undefined })
  amount.value = ''
  reference.value = ''
  if (remaining.value > 0) nextTick(() => document.getElementById('pay-amount')?.focus())
}

const done = ref<PosDoc | null>(null)
async function complete() {
  if (remaining.value > 0 && total.value > 0) {
    addPayment() // empty amount = the exact remaining amount
    if (remaining.value > 0) return
  }
  busy.value = true
  error.value = ''
  try {
    done.value = await api<PosDoc>(`/shops/${shopId.value}/pos/sales`, {
      method: 'POST',
      body: {
        items: lines.value.map((l) => ({
          product_id: l.product_id,
          name: l.product_id ? undefined : l.name,
          qty: l.qty,
          unit_price_cents: l.unit !== l.base || !l.product_id ? l.unit : undefined,
          discount_cents: l.discount || undefined,
        })),
        discount_cents: billDiscount.value || undefined,
        payments: payments.value,
        note: note.value || undefined,
        customer: customer.name ? { ...customer } : undefined,
        issue_invoice: issueInvoice.value,
        bill_id: posBills.activeId.value ?? undefined,
        promo: promoHook.value.body ?? undefined,
      },
    })
    promoHook.value = { body: null, discount: {} }
    paying.value = false
    lastDoc.value = done.value
    billViewTab.value = 'last'
    printed.value = ''
    if (printer.settings.value.autoPrint && done.value) printSale(done.value)
    const closed = posBills.activeId.value
    if (closed) await posBills.closedBySale(closed)
    else clearSale()
    search() // refresh stock
  } catch (e) {
    error.value = apiError(e)
  } finally {
    busy.value = false
  }
}
function newSale() {
  done.value = null
  billViewTab.value = 'current'
  nextTick(() => searchEl.value?.focus())
}

// Tax invoice after the sale
const invForm = reactive({ open: false, name: '', tax_id: '', branch: '', address: '', phone: '', error: '' })
async function issueInvoiceNow() {
  if (!done.value) return
  invForm.error = ''
  try {
    done.value = await api<PosDoc>(`/pos/sales/${done.value.sale.id}/invoice`, {
      method: 'POST',
      body: { customer_name: invForm.name, customer_tax_id: invForm.tax_id, customer_branch: invForm.branch, customer_address: invForm.address, customer_phone: invForm.phone },
    })
    invForm.open = false
    print(`/print/invoice/${done.value.sale.id}`)
  } catch (e) {
    invForm.error = apiError(e)
  }
}
// ---------------------------------------------------------------- printing
// Default receipt printer of this till (browser / direct thermal), auto-print after checkout.
const printer = usePosPrinter()
const printerOpen = ref(false)
const printed = ref<'' | 'ok' | 'failed'>('')
async function printSale(doc: PosDoc) {
  printed.value = (await printer.printReceipt(doc)) ? 'ok' : 'failed'
}

// Bill view: a panel on the left with the receipt of the open bill (as it will print) and the last sale.
const billView = ref(false)
const billViewTab = ref<'current' | 'last'>('current')
const lastDoc = ref<PosDoc | null>(null)
onMounted(() => {
  try { billView.value = localStorage.getItem('zk_pos_billview') === '1' } catch {}
})
watch(billView, (v) => { try { localStorage.setItem('zk_pos_billview', v ? '1' : '0') } catch {} })
/** The open bill as a receipt (not saved yet): what the customer would get now. */
const previewDoc = computed<PosDoc | null>(() => {
  const sh = shop.value
  const b = posBills.active.value
  if (!sh) return null
  const net_ = net.value
  return {
    sale: {
      id: 'preview', shop_id: sh.id, number: b ? billName(b) : '—', status: 'completed', subtotal_cents: subtotal.value, discount_cents: billDiscount.value + promoOff.value,
      vat_bps: bps.value, prices_include_vat: inclusive.value, vat_cents: vat.value, total_cents: total.value, paid_cents: 0, change_cents: 0, payments: [],
      currency: sh.currency, note: note.value, invoice_number: null, invoice_issued_at: null, customer_name: customer.name, customer_tax_id: customer.tax_id,
      customer_branch: customer.branch, customer_address: customer.address, customer_phone: customer.phone, void_reason: '', voided_at: null, created_at: now.value.toISOString(),
    },
    items: lines.value.map((l) => ({ id: l.key, product_id: l.product_id, name: l.name, sku: l.sku, qty: l.qty, unit_price_cents: l.unit, discount_cents: l.discount, line_total_cents: lineTotal(l) })),
    cashier: user.value?.display_name ?? null,
    amount_before_vat_cents: inclusive.value ? net_ - vat.value : net_,
    shop: { id: sh.id, name: sh.name, slug: sh.slug, logo_url: sh.logo_url, legal_name: sh.legal_name, tax_id: sh.tax_id, branch: sh.branch, address: sh.address, phone: sh.phone, currency: sh.currency, receipt_footer: sh.receipt_footer },
  } as PosDoc
})

// ---------------------------------------------------------------- UX helpers
const toast = ref<{ msg: string; bad: boolean } | null>(null)
function flash(msg: string, bad = false) {
  toast.value = { msg, bad }
  setTimeout(() => (toast.value = null), 2200)
}
const now = ref(new Date())
onMounted(() => {
  const i = setInterval(() => (now.value = new Date()), 30_000)
  onBeforeUnmount(() => clearInterval(i))
})
function onKey(e: KeyboardEvent) {
  if (e.key === 'F2') { e.preventDefault(); searchEl.value?.focus() }
  else if (e.key === 'F8' && !paying.value && !done.value) { e.preventDefault(); posBills.newBill() }
  else if (e.key === 'F7' && !paying.value && !done.value) { e.preventDefault(); nextBill() }
  else if (e.key === 'F9') { e.preventDefault(); if (!paying.value && !done.value) openPay(); else if (paying.value) complete() }
  else if (e.key === 'Escape') {
    if (invForm.open) invForm.open = false
    else if (unknown.open) unknown.open = false
    else if (camera.value) camera.value = false
    else if (paying.value) paying.value = false
    else if (done.value) newSale()
  } else if (paying.value && e.altKey && /^[1-5]$/.test(e.key)) {
    method.value = methods[Number(e.key) - 1]!.id
  }
}
onMounted(() => window.addEventListener('keydown', onKey))
onBeforeUnmount(() => window.removeEventListener('keydown', onKey))
const methodLabel = (m: string) => (methods.some((x) => x.id === m) ? t(`common.pay.${m}`) : m)
</script>

<template>
  <div class="flex h-full flex-col">
    <!-- Top bar -->
    <header class="flex h-14 shrink-0 items-center gap-4 border-b border-line bg-surface px-4">
      <UButton color="neutral" variant="soft" size="sm" to="/dashboard">{{ $t('pos.toDashboard') }}</UButton>
      <div class="flex items-center gap-2 font-extrabold">
        <span class="grid size-7 place-items-center rounded-lg bg-brand-500 text-sm text-white">Z</span> POS
      </div>
      <select v-if="shops.length > 1" class="input w-48 py-1.5" :value="shopId" @change="select(($event.target as HTMLSelectElement).value)">
        <option v-for="s in shops" :key="s.id" :value="s.id">{{ s.name }}</option>
      </select>
      <span v-else class="font-semibold">{{ shop?.name }}</span>
      <div class="ml-auto flex items-center gap-4 text-sm text-muted">
        <UButton size="sm" :color="billView ? 'primary' : 'neutral'" :variant="billView ? 'solid' : 'soft'" icon="i-lucide-receipt-text" :aria-pressed="billView" @click="billView = !billView">{{ billView ? $t('pos.billView.hide') : $t('pos.billView.show') }}</UButton>
        <ClientOnly>
          <UButton size="sm" color="neutral" variant="soft" icon="i-lucide-printer" :title="$t('pos.printer.title')" @click="printerOpen = true">
            <span class="max-w-32 truncate">{{ printer.displayName.value }}</span>
            <span class="size-2 rounded-full" :class="printer.ready.value ? 'bg-mint-500' : 'bg-brand-500'" />
          </UButton>
        </ClientOnly>
        <NuxtLink to="/dashboard/pos-sales" class="font-semibold text-ink hover:underline">{{ $t('pos.salesReport') }}</NuxtLink>
        <span class="hidden md:inline">{{ user?.display_name }}</span>
        <ClientOnly><span class="tabular-nums">{{ fmtWeekdayTime(now) }}</span></ClientOnly>
      </div>
    </header>

    <div class="flex min-h-0 flex-1">
      <!-- Bill view: the receipt as it will print (hide / show from the top bar) -->
      <Transition enter-from-class="-ml-[340px] opacity-0" leave-to-class="-ml-[340px] opacity-0" enter-active-class="transition-all duration-200" leave-active-class="transition-all duration-200">
        <aside v-if="billView" class="flex w-[340px] shrink-0 flex-col border-r border-line bg-paper" :aria-label="$t('pos.billView.title')">
          <div class="flex items-center gap-1 border-b border-line bg-surface px-2 py-2">
            <button v-for="tb in (['current', 'last'] as const)" :key="tb" class="rounded-lg px-2.5 py-1 text-xs font-semibold" :class="billViewTab === tb ? 'bg-ink text-paper' : 'text-muted hover:text-ink'" :disabled="tb === 'last' && !lastDoc" @click="billViewTab = tb">{{ $t(`pos.billView.${tb}`) }}</button>
            <UButton size="xs" color="neutral" variant="ghost" icon="i-lucide-panel-left-close" class="ml-auto" :aria-label="$t('pos.billView.hide')" @click="billView = false" />
          </div>
          <div class="min-h-0 flex-1 overflow-y-auto p-3">
            <template v-if="billViewTab === 'current'">
              <div v-if="!lines.length" class="p-6 text-center text-xs text-muted">{{ $t('pos.billView.empty') }}</div>
              <PosReceipt v-else-if="previewDoc" :doc="previewDoc" :width="printer.settings.value.paper === '58' ? 58 : 80" preview class="origin-top scale-[0.92] shadow-card" />
            </template>
            <PosReceipt v-else-if="lastDoc" :doc="lastDoc" :width="printer.settings.value.paper === '58' ? 58 : 80" class="origin-top scale-[0.92] shadow-card" />
          </div>
          <div class="border-t border-line bg-surface p-2">
            <UButton v-if="billViewTab === 'current'" block size="sm" color="neutral" icon="i-lucide-printer" :disabled="!lines.length || !previewDoc" @click="previewDoc && printer.printDoc(previewDoc, { preview: true })">{{ $t('pos.billView.printBill') }}</UButton>
            <UButton v-else block size="sm" color="neutral" icon="i-lucide-printer" :disabled="!lastDoc" @click="lastDoc && printSale(lastDoc)">{{ $t('pos.billView.reprint') }}</UButton>
          </div>
        </aside>
      </Transition>
      <!-- Products -->
      <section class="flex min-w-0 flex-1 flex-col">
        <div class="space-y-3 border-b border-line bg-paper p-4">
          <form class="relative" @submit.prevent="onEnter">
            <span class="pointer-events-none absolute left-4 top-1/2 -translate-y-1/2 text-muted">⌕</span>
            <input
              ref="searchEl"
              v-model="q"
              data-scan-target="pos"
              autofocus
              autocomplete="off"
              class="input rounded-2xl py-3.5 pl-10 text-base"
              :placeholder="$t('pos.searchPlaceholder')"
              :aria-label="$t('pos.searchAria')"
            >
            <button type="button" class="absolute right-2 top-1/2 -translate-y-1/2 rounded-xl bg-ink px-3 py-2 text-xs font-bold text-paper hover:bg-night/85" :title="$t('pos.scan.camera')" @click="camera = true">📷 {{ $t('pos.scan.cameraShort') }}</button>
          </form>
          <div class="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-muted">
            <button type="button" class="inline-flex items-center gap-1.5 hover:text-ink" :aria-haspopup="'dialog'" @click="scanHelp = true">
              <span class="size-2 rounded-full" :class="scannerOk ? 'bg-mint-500' : 'bg-amber-500'" />
              <ClientOnly><span class="font-semibold text-ink">{{ scannerName }}</span></ClientOnly>
              <span>· {{ $t('pos.scan.ready') }}</span>
              <span class="underline">{{ $t('pos.scanners.manage') }}</span>
            </button>
            <span v-if="lastScan" class="font-mono" :class="lastScan.ok ? 'text-ink' : 'text-brand-700'">{{ lastScan.ok ? '✓' : '✕' }} {{ lastScan.code }}<span v-if="lastScan.name" class="font-sans"> · {{ lastScan.name }}</span></span>
            <span class="hidden md:inline">{{ $t('pos.scan.qtyHint') }}</span>
            <label class="ml-auto inline-flex cursor-pointer items-center gap-1"><input v-model="scanSound" type="checkbox" class="size-3.5 accent-brand-500">{{ $t('pos.scan.sound') }}</label>
          </div>
          <div v-if="shopCats.length" class="flex gap-2 overflow-x-auto pb-1">
            <button class="chip shrink-0 border px-3 py-1.5" :class="!cat ? 'border-brand-500 bg-brand-500 text-white shadow-[0_6px_16px_-8px_var(--brand-500)]' : 'border-transparent bg-surface shadow-card hover:text-brand-500'" @click="cat = null">{{ $t('common.all') }}</button>
            <button v-for="c in shopCats.filter((c) => c.depth === 0)" :key="c.id" class="chip shrink-0 border px-3 py-1.5" :class="cat === c.id ? 'border-brand-500 bg-brand-500 text-white shadow-[0_6px_16px_-8px_var(--brand-500)]' : 'border-transparent bg-surface shadow-card hover:text-brand-500'" @click="cat = c.id">{{ c.name }}</button>
          </div>
        </div>
        <div class="min-h-0 flex-1 overflow-y-auto p-4">
          <div v-if="hits.length" class="grid grid-cols-2 gap-3 sm:grid-cols-3 xl:grid-cols-5">
            <button
              v-for="h in hits"
              :key="h.id"
              class="card group flex flex-col overflow-hidden text-left transition hover:border-ink/40 active:scale-[.98]"
              :class="h.stock <= 0 && 'opacity-50'"
              @click="add(h)"
            >
              <ProductThumb :src="h.image" :name="h.name" class="aspect-[4/3] w-full" rounded="rounded-none" />
              <div class="flex flex-1 flex-col p-2.5">
                <div class="line-clamp-2 text-sm font-semibold leading-snug">{{ h.name }}</div>
                <div class="mt-auto flex items-end justify-between pt-1.5">
                  <span class="font-extrabold">{{ money(h.price_cents, cur) }}</span>
                  <span class="text-[11px]" :class="h.stock <= 0 ? 'font-bold text-brand-700' : h.stock <= 5 ? 'text-amber-700' : 'text-muted'">{{ h.stock <= 0 ? $t('pos.outOfStock') : $t('pos.left', { n: h.stock }) }}</span>
                </div>
              </div>
            </button>
          </div>
          <EmptyState v-else-if="!loading" :title="q ? $t('pos.noMatches') : $t('pos.noProducts')" :text="q ? $t('pos.noMatchesText') : $t('pos.noProductsText')" />
        </div>
      </section>

      <!-- Cart -->
      <aside class="flex w-[400px] shrink-0 flex-col border-l border-line bg-surface xl:w-[440px]">
        <!-- Open bills (shared by every till of the shop) -->
        <div class="border-b border-line bg-paper px-2 pt-2">
          <div class="flex items-end gap-1 overflow-x-auto" role="tablist" :aria-label="$t('pos.bills.title')">
            <div
              v-for="b in posBills.bills.value"
              :key="b.id"
              role="tab"
              :aria-selected="b.id === posBills.activeId.value"
              class="group flex shrink-0 cursor-pointer items-center gap-1.5 rounded-t-xl border border-b-0 px-3 py-1.5 text-sm"
              :class="b.id === posBills.activeId.value ? 'border-line bg-surface font-bold' : 'border-transparent text-muted hover:bg-surface/60 hover:text-ink'"
              @click="posBills.activate(b.id)"
              @dblclick="startRename(b)"
            >
              <input
                v-if="renaming === b.id"
                id="bill-rename"
                v-model="renameDraft"
                maxlength="40"
                class="w-28 rounded bg-[var(--field)] px-1 text-sm outline-none"
                :placeholder="$t('pos.bills.default', { n: b.number })"
                @keydown.enter.prevent="finishRename"
                @keydown.esc.stop.prevent="renaming = null"
                @blur="finishRename"
                @click.stop
              >
              <span v-else class="max-w-32 truncate">{{ billName(b) }}</span>
              <span v-if="billItems(b)" class="rounded-md bg-brand-500 px-1.5 text-[10px] font-bold text-white">{{ billItems(b) }}</span>
              <span v-if="posBills.dirty.value.has(b.id)" class="size-1.5 rounded-full bg-amber-500" :title="$t('pos.bills.saving')" />
            </div>
            <UTooltip :text="`${$t('pos.bills.new')} (F8)`">
              <button class="mb-1 ml-1 grid size-7 shrink-0 place-items-center rounded-lg text-muted hover:bg-surface hover:text-ink" :aria-label="$t('pos.bills.new')" @click="posBills.newBill()"><UIcon name="i-lucide-plus" class="size-4" /></button>
            </UTooltip>
          </div>
        </div>
        <div class="flex items-center justify-between border-b border-line px-4 py-3">
          <div class="min-w-0">
            <div class="truncate font-bold">{{ posBills.active.value ? billName(posBills.active.value) : $t('pos.currentSale') }} <span class="font-normal text-muted">· {{ $t('common.items', units) }}</span></div>
            <div class="text-[11px] text-muted">
              <template v-if="posBills.offline.value"><span class="font-semibold text-brand-700">{{ $t('pos.bills.offline') }}</span></template>
              <template v-else-if="posBills.active.value?.updated_by_name">{{ $t('pos.bills.lastBy', { name: posBills.active.value.updated_by_name }) }}</template>
              <template v-else>{{ $t('pos.bills.shared') }}</template>
            </div>
          </div>
          <div class="flex shrink-0 gap-1">
            <UTooltip v-if="posBills.active.value" :text="$t('pos.bills.rename')"><UButton color="neutral" variant="ghost" size="sm" icon="i-lucide-pencil" :aria-label="$t('pos.bills.rename')" @click="startRename(posBills.active.value)" /></UTooltip>
            <UTooltip v-if="posBills.bills.value.length > 1 && posBills.active.value" :text="$t('pos.bills.cancel')"><UButton color="neutral" variant="ghost" size="sm" icon="i-lucide-trash-2" class="text-brand-700" :aria-label="$t('pos.bills.cancel')" @click="cancelBill(posBills.active.value.id)" /></UTooltip>
            <UButton color="neutral" variant="soft" size="sm" type="submit" @click="custom.open = !custom.open">{{ $t('pos.addCustom') }}</UButton>
            <UButton color="neutral" variant="soft" size="sm" type="submit" v-if="lines.length" class="text-brand-700" @click="clearSale">{{ $t('pos.clear') }}</UButton>
          </div>
        </div>
        <form v-if="custom.open" class="flex gap-2 border-b border-line bg-paper p-3" @submit.prevent="addCustom">
          <UInput v-model="custom.name" :placeholder="$t('pos.itemName')" autofocus />
          <UInput v-model.number="custom.price" type="number" step="0.01" min="0" class="w-28" :placeholder="$t('common.price')" />
          <UButton color="neutral" size="sm" type="submit">{{ $t('common.add') }}</UButton>
        </form>

        <div class="min-h-0 flex-1 overflow-y-auto">
          <div v-if="!lines.length" class="grid h-full place-items-center p-8 text-center text-sm text-muted">
            <div><div class="mb-2 text-3xl">🧾</div>{{ $t('pos.emptyCart') }}</div>
          </div>
          <ul v-else class="divide-y divide-line/70">
            <li v-for="l in lines" :key="l.key" class="px-4 py-3 transition-colors duration-700" :class="lastAdded?.id === l.product_id && 'bg-mint-500/15'">
              <div class="flex items-start gap-3">
                <button class="min-w-0 flex-1 text-left" @click="expanded = expanded === l.key ? null : l.key">
                  <div class="truncate font-semibold">{{ l.name }}</div>
                  <div class="text-xs text-muted">
                    {{ money(l.unit, cur) }}<span v-if="l.unit !== l.base" class="text-amber-700"> {{ $t('pos.was', { price: money(l.base, cur) }) }}</span>
                    <span v-if="l.discount" class="text-brand-700"> · −{{ money(l.discount, cur) }}</span>
                    <span v-if="l.stock !== null && l.qty > l.stock" class="font-bold text-brand-700"> · {{ $t('pos.onlyStock', { n: l.stock }) }}</span>
                  </div>
                </button>
                <div class="flex items-center rounded-full border border-line">
                  <button class="px-2.5 py-1 text-lg leading-none" :aria-label="$t('pos.decrease')" @click="setQty(l, l.qty - 1)">−</button>
                  <input :value="l.qty" type="number" min="1" class="w-10 bg-transparent text-center text-sm font-bold outline-none [appearance:textfield]" :aria-label="$t('common.quantity')" @change="setQty(l, Number(($event.target as HTMLInputElement).value))" >
                  <button class="px-2.5 py-1 text-lg leading-none" :aria-label="$t('pos.increase')" @click="setQty(l, l.qty + 1)">+</button>
                </div>
                <div class="w-20 text-right font-bold tabular-nums">{{ money(lineTotal(l), cur) }}</div>
              </div>
              <div v-if="expanded === l.key" class="mt-2 grid grid-cols-3 gap-2 rounded-xl bg-paper p-2 text-xs">
                <label>{{ $t('pos.unitPrice') }}<UInput size="md" :model-value="l.unit / 100" type="number" step="0.01" min="0" class="mt-1" @change="l.unit = toCents(($event.target as HTMLInputElement).value)" /></label>
                <label>{{ $t('common.discount') }}<UInput size="md" :model-value="l.discount / 100" type="number" step="0.01" min="0" class="mt-1" @change="l.discount = Math.min(l.unit * l.qty, toCents(($event.target as HTMLInputElement).value))" /></label>
                <div class="flex items-end"><UButton color="neutral" variant="soft" size="sm" type="submit" class="w-full text-brand-700" @click="setQty(l, 0)">{{ $t('common.remove') }}</UButton></div>
              </div>
            </li>
            <li id="cart-end" />
          </ul>
        </div>

        <!-- Totals -->
        <div class="space-y-3 border-t border-line p-4">
          <ModuleSlot name="pos.promo" :shop-id="shopId" :subtotal="Math.max(0, subtotal - billDiscount)" :currency="cur" />
          <details class="text-sm">
            <summary class="cursor-pointer font-semibold text-muted">{{ $t('pos.moreOptions') }}</summary>
            <div class="mt-3 space-y-2">
              <div class="flex gap-2">
                <select v-model="discountMode" class="input w-24 py-2"><option value="amount">{{ cur }}</option><option value="percent">%</option></select>
                <UInput v-model.number="discountInput" type="number" min="0" step="0.01" :placeholder="$t('pos.billDiscount')" @change="applyBillDiscount" />
                <UButton color="neutral" variant="soft" size="sm" type="submit" @click="applyBillDiscount">{{ $t('pos.apply') }}</UButton>
              </div>
              <UInput v-model="customer.name" :placeholder="$t('pos.customerName')" />
              <div class="flex gap-2">
                <UInput v-model="customer.tax_id" :placeholder="$t('pos.taxId')" />
                <UInput v-model="customer.branch" :placeholder="$t('pos.branch')" />
              </div>
              <UTextarea v-model="customer.address" :rows="2" :placeholder="$t('common.address')" />
              <UInput v-model="note" :placeholder="$t('pos.notePrinted')" />
            </div>
          </details>
          <dl class="space-y-1 text-sm">
            <div class="flex justify-between"><dt class="text-muted">{{ $t('common.subtotal') }}</dt><dd class="tabular-nums">{{ money(subtotal, cur) }}</dd></div>
            <div v-if="billDiscount" class="flex justify-between text-brand-700"><dt>{{ $t('common.discount') }} <button class="ml-1 text-xs underline" @click="billDiscount = 0; discountInput = ''">{{ $t('pos.removeDiscount') }}</button></dt><dd class="tabular-nums">−{{ money(billDiscount, cur) }}</dd></div>
            <div v-if="promoOff" class="flex justify-between text-mint-500"><dt>{{ $t('common.discount') }} ★</dt><dd class="tabular-nums">−{{ money(promoOff, cur) }}</dd></div>
            <div v-if="bps" class="flex justify-between text-muted"><dt>{{ $t('pos.vat', { rate: bps / 100 }) }} {{ inclusive ? $t('pos.vatIncluded') : '' }}</dt><dd class="tabular-nums">{{ money(vat, cur) }}</dd></div>
          </dl>
          <UButton type="submit" class="w-full justify-between rounded-2xl px-5 py-4 text-lg" :disabled="!lines.length" @click="openPay">
            <span>{{ $t('pos.charge') }} <span class="text-xs font-normal opacity-80">(F9)</span></span><span class="tabular-nums">{{ money(total, cur) }}</span>
          </UButton>
        </div>
      </aside>
    </div>

    <!-- Payment -->
    <div v-if="paying" class="fixed inset-0 z-50 grid place-items-center bg-night/50 p-4" @click.self="paying = false">
      <div class="card w-full max-w-2xl overflow-hidden shadow-2xl" role="dialog" :aria-label="$t('pos.payment')">
        <div class="grid md:grid-cols-[1fr_260px]">
          <div class="space-y-4 p-6">
            <div class="flex items-baseline justify-between">
              <div class="text-sm font-semibold text-muted">{{ $t('pos.amountDue') }}</div>
              <div class="text-3xl font-extrabold tabular-nums">{{ money(total, cur) }}</div>
            </div>
            <div class="grid grid-cols-5 gap-1.5">
              <button v-for="m in methods" :key="m.id" class="rounded-xl border-2 px-2 py-2.5 text-xs font-bold" :class="method === m.id ? 'border-brand-500 bg-brand-500 text-white shadow-[0_6px_16px_-8px_var(--brand-500)]' : 'border-transparent bg-surface shadow-card hover:text-brand-500'" :title="`Alt+${m.key}`" @click="method = m.id">{{ $t(`common.pay.${m.id}`) }}</button>
            </div>
            <form class="space-y-2" @submit.prevent="remaining > 0 ? addPayment() : complete()">
              <div class="flex gap-2">
                <UInput id="pay-amount" v-model.number="amount" type="number" min="0" step="0.01" class="py-3 text-lg tabular-nums" :placeholder="$t('pos.exactHint', { amount: (remaining / 100).toFixed(2) })" />
                <UButton color="neutral" type="button" :disabled="remaining <= 0" @click="addPayment()">{{ $t('common.add') }}</UButton>
              </div>
              <UInput v-if="method !== 'cash'" v-model="reference" :placeholder="$t('pos.reference')" />
            </form>
            <div v-if="method === 'cash' && remaining > 0" class="flex flex-wrap gap-2">
              <UButton color="neutral" variant="soft" size="sm" type="submit" v-for="v in quickCash" :key="v" class="tabular-nums" @click="addPayment(v)">{{ money(v * 100, cur) }}</UButton>
            </div>
            <label class="flex items-center gap-2 text-sm">
              <input v-model="issueInvoice" type="checkbox" class="size-4 accent-brand-500" :disabled="!customer.name || !customer.address">
              {{ $t('pos.issueInvoice') }}
              <span v-if="!customer.name || !customer.address" class="text-xs text-muted">{{ $t('pos.issueInvoiceHint') }}</span>
            </label>
            <p v-if="error" class="text-sm text-brand-700">{{ error }}</p>
          </div>
          <div class="flex flex-col bg-paper p-6">
            <div class="text-xs font-bold uppercase tracking-wider text-muted">{{ $t('pos.payments') }}</div>
            <ul class="mt-2 flex-1 space-y-1.5 text-sm">
              <li v-for="(p, i) in payments" :key="i" class="flex items-center justify-between">
                <span>{{ methodLabel(p.method) }}<span v-if="p.reference" class="text-xs text-muted"> · {{ p.reference }}</span></span>
                <span class="flex items-center gap-2 tabular-nums">{{ money(p.amount_cents, cur) }}<button class="text-muted hover:text-brand-700" :aria-label="$t('pos.removePayment')" @click="payments.splice(i, 1)">✕</button></span>
              </li>
              <li v-if="!payments.length" class="text-muted">{{ $t('pos.noneYet') }}</li>
            </ul>
            <dl class="mt-4 space-y-1 border-t border-line pt-3 text-sm">
              <div class="flex justify-between"><dt>{{ $t('pos.paid') }}</dt><dd class="tabular-nums">{{ money(paid, cur) }}</dd></div>
              <div class="flex justify-between font-bold" :class="remaining ? 'text-brand-700' : ''"><dt>{{ $t('pos.remaining') }}</dt><dd class="tabular-nums">{{ money(remaining, cur) }}</dd></div>
              <div class="flex justify-between text-lg font-extrabold text-mint-500"><dt>{{ $t('pos.change') }}</dt><dd class="tabular-nums">{{ money(change, cur) }}</dd></div>
            </dl>
            <UButton type="submit" class="mt-4 w-full py-3" :disabled="busy" @click="complete">
              {{ busy ? $t('common.saving') : remaining > 0 ? $t('pos.payAndFinish', { amount: money(remaining, cur) }) : $t('pos.completeSale') }}
            </UButton>
            <UButton color="neutral" variant="soft" size="sm" type="submit" class="mt-2" @click="paying = false">{{ $t('pos.backEsc') }}</UButton>
          </div>
        </div>
      </div>
    </div>

    <!-- Done -->
    <div v-if="done" class="fixed inset-0 z-50 grid place-items-center bg-night/50 p-4">
      <div class="card w-full max-w-md p-6 text-center shadow-2xl" role="dialog" :aria-label="$t('pos.saleComplete')">
        <div class="mx-auto grid size-14 place-items-center rounded-full bg-mint-500/15 text-2xl text-mint-500">✓</div>
        <div class="mt-3 text-sm text-muted">{{ $t('pos.saleLine', { number: done.sale.number, amount: money(done.sale.total_cents, done.sale.currency) }) }}</div>
        <div v-if="done.sale.change_cents" class="mt-2 text-4xl font-extrabold tabular-nums">{{ $t('pos.changeAmount', { amount: money(done.sale.change_cents, done.sale.currency) }) }}</div>
        <div v-else class="mt-2 text-2xl font-extrabold">{{ $t('pos.paidInFull') }}</div>

        <div class="mt-6 grid gap-2">
          <div v-if="printed" class="flex items-center justify-center gap-1.5 text-xs" :class="printed === 'ok' ? 'text-mint-500' : 'text-brand-700'">
            <UIcon :name="printed === 'ok' ? 'i-lucide-printer-check' : 'i-lucide-printer'" class="size-4" />
            {{ printed === 'ok' ? $t('pos.printer.sentTo', { name: printer.displayName.value }) : $t('pos.printer.failed', { error: printer.error.value }) }}
          </div>
          <UButton color="neutral" type="submit" icon="i-lucide-printer" @click="printSale(done)">{{ printed === 'ok' ? $t('pos.billView.reprint') : $t('pos.printReceipt') }}</UButton>
          <UButton color="neutral" variant="soft" type="submit" v-if="done.sale.invoice_number" @click="print(`/print/invoice/${done.sale.id}`)">{{ $t('pos.printTaxInvoice', { number: done.sale.invoice_number }) }}</UButton>
          <UButton color="neutral" variant="soft" type="submit" v-else @click="Object.assign(invForm, { open: true, error: '' })">{{ $t('pos.issueTaxInvoice') }}</UButton>
          <UButton type="submit" @click="newSale">{{ $t('pos.newSale') }}</UButton>
        </div>
      </div>
    </div>

    <!-- Invoice form -->
    <div v-if="invForm.open" class="fixed inset-0 z-[60] grid place-items-center bg-night/50 p-4" @click.self="invForm.open = false">
      <form class="card w-full max-w-md space-y-3 p-6 shadow-2xl" @submit.prevent="issueInvoiceNow">
        <div class="text-lg font-bold">{{ $t('pos.fullTaxInvoice') }}</div>
        <p class="text-xs text-muted">{{ $t('pos.invoicePermanent') }}</p>
        <UInput v-model="invForm.name" required :placeholder="$t('pos.customerNameReq')" autofocus />
        <div class="flex gap-2">
          <UInput v-model="invForm.tax_id" :placeholder="$t('pos.taxIdHint')" />
          <UInput v-model="invForm.branch" :placeholder="$t('pos.branchHint')" />
        </div>
        <UTextarea v-model="invForm.address" required :rows="3" :placeholder="$t('pos.addressReq')" />
        <UInput v-model="invForm.phone" :placeholder="$t('common.phone')" />
        <p v-if="invForm.error" class="text-sm text-brand-700">{{ invForm.error }}</p>
        <div class="flex justify-end gap-2">
          <UButton color="neutral" variant="soft" type="button" @click="invForm.open = false">{{ $t('common.cancel') }}</UButton>
          <UButton type="submit">{{ $t('pos.issuePrint') }}</UButton>
        </div>
      </form>
    </div>

    <!-- Receipt printer -->
    <PosPrinterPanel v-if="printerOpen" @close="printerOpen = false" />

    <!-- Scanner devices -->
    <PosScannerPanel
      v-if="scanHelp"
      :devices="scanners.devices.value"
      :supported="scanners.supported.value"
      :diagnostic="scanners.diagnostic.value"
      @close="scanHelp = false"
      @add-hid="scanners.addHid()"
      @add-serial="(b) => scanners.addSerial(b)"
      @reconnect="(id) => scanners.reconnect(id)"
      @disconnect="(id) => scanners.disconnect(id)"
      @remove="(id) => scanners.remove(id)"
      @rename="(id, n) => scanners.rename(id, n)"
    />

    <!-- Camera scanner -->
    <BarcodeCameraScanner v-if="camera" @detected="onScanned" @close="camera = false" />

    <!-- Unknown barcode -->
    <div v-if="unknown.open" class="fixed inset-0 z-[60] grid place-items-center bg-night/50 p-4" @click.self="unknown.open = false">
      <div class="card w-full max-w-md space-y-3 p-5" role="dialog" :aria-label="$t('pos.scan.unknownTitle')">
        <div class="flex items-start justify-between gap-3">
          <div>
            <div class="font-extrabold">{{ $t('pos.scan.unknownTitle') }}</div>
            <div class="font-mono text-lg">{{ unknown.code }}<span v-if="unknown.qty > 1" class="font-sans text-sm text-muted"> × {{ unknown.qty }}</span></div>
          </div>
          <button class="text-muted hover:text-ink" :aria-label="$t('common.close')" @click="unknown.open = false">✕</button>
        </div>
        <p class="text-sm text-muted">{{ $t('pos.scan.unknownHint') }}</p>
        <UInput v-model="unknown.q" :placeholder="$t('pos.scan.findProduct')" autofocus />
        <ul v-if="unknown.results.length" class="max-h-64 divide-y divide-line/70 overflow-y-auto rounded-xl border border-line">
          <li v-for="h in unknown.results" :key="h.id" class="flex items-center gap-3 p-2">
            <ProductThumb :src="h.image" :name="h.name" sizes="40px" class="size-10 shrink-0" rounded="rounded-lg" />
            <div class="min-w-0 flex-1">
              <div class="truncate text-sm font-semibold">{{ h.name }}</div>
              <div class="text-xs text-muted">{{ h.sku }}<span v-if="h.barcode"> · {{ $t('pos.scan.hasBarcode', { code: h.barcode }) }}</span></div>
            </div>
            <UButton color="neutral" size="sm" type="submit" :disabled="unknown.busy" @click="assignBarcode(h)">{{ h.barcode ? $t('pos.scan.replace') : $t('pos.scan.assign') }}</UButton>
          </li>
        </ul>
        <p v-if="unknown.error" class="text-sm text-brand-700">{{ unknown.error }}</p>
        <div class="flex justify-between gap-2 pt-1">
          <UButton color="neutral" variant="soft" size="sm" :to="`/dashboard/products/new?barcode=${encodeURIComponent(unknown.code)}`" target="_blank">{{ $t('pos.scan.createProduct') }}</UButton>
          <UButton color="neutral" variant="soft" size="sm" type="submit" @click="unknown.open = false">{{ $t('common.cancel') }}</UButton>
        </div>
      </div>
    </div>

    <div v-if="toast" class="fixed bottom-6 left-1/2 z-[70] -translate-x-1/2 rounded-full px-4 py-2 text-sm font-semibold text-white shadow-lg" :class="toast.bad ? 'bg-brand-700' : 'bg-night'">{{ toast.msg }}</div>
  </div>
</template>
