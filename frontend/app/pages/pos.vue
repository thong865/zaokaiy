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

/** Enter in the search box: scanner or typed code. */
async function onEnter() {
  const code = q.value.trim()
  if (!code) return
  clearTimeout(timer)
  await search()
  const exact = hits.value.find((h) => h.exact)
  const pick = exact ?? (hits.value.length === 1 ? hits.value[0] : undefined)
  if (pick) {
    add(pick)
    resetSearch()
  } else {
    flash(t('pos.noMatch', { code }), true)
  }
}

// ---------------------------------------------------------------- cart
interface Line { key: string; product_id: string | null; name: string; sku: string; unit: number; base: number; qty: number; discount: number; stock: number | null }
const CART_KEY = computed(() => `zk_pos_cart:${shopId.value}`)
const lines = ref<Line[]>([])
const billDiscount = ref(0) // cents
const note = ref('')
const customer = reactive({ name: '', tax_id: '', branch: '', address: '', phone: '' })
const expanded = ref<string | null>(null)

onMounted(() => {
  try {
    const saved = JSON.parse(localStorage.getItem(CART_KEY.value) || 'null')
    if (saved) {
      lines.value = saved.lines ?? []
      billDiscount.value = saved.billDiscount ?? 0
    }
  } catch {}
})
watch([lines, billDiscount], () => {
  try {
    localStorage.setItem(CART_KEY.value, JSON.stringify({ lines: lines.value, billDiscount: billDiscount.value }))
  } catch {}
}, { deep: true })

function add(h: Hit) {
  const existing = lines.value.find((l) => l.product_id === h.id && l.unit === h.price_cents && !l.discount)
  if (existing) existing.qty++
  else lines.value.push({ key: crypto.randomUUID(), product_id: h.id, name: h.name, sku: h.sku, unit: h.price_cents, base: h.price_cents, qty: 1, discount: 0, stock: h.stock })
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
const net = computed(() => Math.max(0, subtotal.value - billDiscount.value))
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
    if (amount.value !== '' || method.value !== 'cash') addPayment()
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
      },
    })
    paying.value = false
    clearSale()
    search() // refresh stock
  } catch (e) {
    error.value = apiError(e)
  } finally {
    busy.value = false
  }
}
function newSale() {
  done.value = null
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
const paperWidth = ref<'80' | '58'>('80')
onMounted(() => {
  try { paperWidth.value = (localStorage.getItem('zk_paper') as '80' | '58') || '80' } catch {}
})
watch(paperWidth, (w) => { try { localStorage.setItem('zk_paper', w) } catch {} })

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
  else if (e.key === 'F9') { e.preventDefault(); if (!paying.value && !done.value) openPay(); else if (paying.value) complete() }
  else if (e.key === 'Escape') {
    if (invForm.open) invForm.open = false
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
    <header class="flex h-14 shrink-0 items-center gap-4 border-b border-line bg-white px-4">
      <NuxtLink to="/dashboard" class="btn-ghost btn-sm">{{ $t('pos.toDashboard') }}</NuxtLink>
      <div class="flex items-center gap-2 font-extrabold">
        <span class="grid size-7 place-items-center rounded-lg bg-brand-500 text-sm text-white">Z</span> POS
      </div>
      <select v-if="shops.length > 1" class="input w-48 py-1.5" :value="shopId" @change="select(($event.target as HTMLSelectElement).value)">
        <option v-for="s in shops" :key="s.id" :value="s.id">{{ s.name }}</option>
      </select>
      <span v-else class="font-semibold">{{ shop?.name }}</span>
      <div class="ml-auto flex items-center gap-4 text-sm text-muted">
        <NuxtLink to="/dashboard/pos-sales" class="font-semibold text-ink hover:underline">{{ $t('pos.salesReport') }}</NuxtLink>
        <span class="hidden md:inline">{{ user?.display_name }}</span>
        <ClientOnly><span class="tabular-nums">{{ fmtWeekdayTime(now) }}</span></ClientOnly>
      </div>
    </header>

    <div class="flex min-h-0 flex-1">
      <!-- Products -->
      <section class="flex min-w-0 flex-1 flex-col">
        <div class="space-y-3 border-b border-line bg-paper p-4">
          <form class="relative" @submit.prevent="onEnter">
            <span class="pointer-events-none absolute left-4 top-1/2 -translate-y-1/2 text-muted">⌕</span>
            <input
              ref="searchEl"
              v-model="q"
              autofocus
              autocomplete="off"
              class="input rounded-2xl py-3.5 pl-10 text-base"
              :placeholder="$t('pos.searchPlaceholder')"
              :aria-label="$t('pos.searchAria')"
            >
          </form>
          <div v-if="shopCats.length" class="flex gap-2 overflow-x-auto pb-1">
            <button class="chip shrink-0 border px-3 py-1.5" :class="!cat ? 'border-ink bg-ink text-white' : 'border-line bg-white'" @click="cat = null">{{ $t('common.all') }}</button>
            <button v-for="c in shopCats.filter((c) => c.depth === 0)" :key="c.id" class="chip shrink-0 border px-3 py-1.5" :class="cat === c.id ? 'border-ink bg-ink text-white' : 'border-line bg-white'" @click="cat = c.id">{{ c.name }}</button>
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
      <aside class="flex w-[400px] shrink-0 flex-col border-l border-line bg-white xl:w-[440px]">
        <div class="flex items-center justify-between border-b border-line px-4 py-3">
          <div class="font-bold">{{ $t('pos.currentSale') }} <span class="font-normal text-muted">· {{ $t('common.items', units) }}</span></div>
          <div class="flex gap-1">
            <button class="btn-ghost btn-sm" @click="custom.open = !custom.open">{{ $t('pos.addCustom') }}</button>
            <button v-if="lines.length" class="btn-ghost btn-sm text-brand-700" @click="clearSale">{{ $t('pos.clear') }}</button>
          </div>
        </div>
        <form v-if="custom.open" class="flex gap-2 border-b border-line bg-paper p-3" @submit.prevent="addCustom">
          <input v-model="custom.name" class="input py-2" :placeholder="$t('pos.itemName')" autofocus >
          <input v-model="custom.price" type="number" step="0.01" min="0" class="input w-28 py-2" :placeholder="$t('common.price')" >
          <button class="btn-dark btn-sm">{{ $t('common.add') }}</button>
        </form>

        <div class="min-h-0 flex-1 overflow-y-auto">
          <div v-if="!lines.length" class="grid h-full place-items-center p-8 text-center text-sm text-muted">
            <div><div class="mb-2 text-3xl">🧾</div>{{ $t('pos.emptyCart') }}</div>
          </div>
          <ul v-else class="divide-y divide-line/70">
            <li v-for="l in lines" :key="l.key" class="px-4 py-3">
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
                <label>{{ $t('pos.unitPrice') }}<input :value="l.unit / 100" type="number" step="0.01" min="0" class="input mt-1 py-1.5" @change="l.unit = toCents(($event.target as HTMLInputElement).value)"></label>
                <label>{{ $t('common.discount') }}<input :value="l.discount / 100" type="number" step="0.01" min="0" class="input mt-1 py-1.5" @change="l.discount = Math.min(l.unit * l.qty, toCents(($event.target as HTMLInputElement).value))"></label>
                <div class="flex items-end"><button class="btn-ghost btn-sm w-full text-brand-700" @click="setQty(l, 0)">{{ $t('common.remove') }}</button></div>
              </div>
            </li>
            <li id="cart-end" />
          </ul>
        </div>

        <!-- Totals -->
        <div class="space-y-3 border-t border-line p-4">
          <details class="text-sm">
            <summary class="cursor-pointer font-semibold text-muted">{{ $t('pos.moreOptions') }}</summary>
            <div class="mt-3 space-y-2">
              <div class="flex gap-2">
                <select v-model="discountMode" class="input w-24 py-2"><option value="amount">{{ cur }}</option><option value="percent">%</option></select>
                <input v-model="discountInput" type="number" min="0" step="0.01" class="input py-2" :placeholder="$t('pos.billDiscount')" @change="applyBillDiscount" >
                <button class="btn-ghost btn-sm" @click="applyBillDiscount">{{ $t('pos.apply') }}</button>
              </div>
              <input v-model="customer.name" class="input py-2" :placeholder="$t('pos.customerName')" >
              <div class="flex gap-2">
                <input v-model="customer.tax_id" class="input py-2" :placeholder="$t('pos.taxId')" >
                <input v-model="customer.branch" class="input py-2" :placeholder="$t('pos.branch')" >
              </div>
              <textarea v-model="customer.address" rows="2" class="input py-2" :placeholder="$t('common.address')" />
              <input v-model="note" class="input py-2" :placeholder="$t('pos.notePrinted')" >
            </div>
          </details>
          <dl class="space-y-1 text-sm">
            <div class="flex justify-between"><dt class="text-muted">{{ $t('common.subtotal') }}</dt><dd class="tabular-nums">{{ money(subtotal, cur) }}</dd></div>
            <div v-if="billDiscount" class="flex justify-between text-brand-700"><dt>{{ $t('common.discount') }} <button class="ml-1 text-xs underline" @click="billDiscount = 0; discountInput = ''">{{ $t('pos.removeDiscount') }}</button></dt><dd class="tabular-nums">−{{ money(billDiscount, cur) }}</dd></div>
            <div v-if="bps" class="flex justify-between text-muted"><dt>{{ $t('pos.vat', { rate: bps / 100 }) }} {{ inclusive ? $t('pos.vatIncluded') : '' }}</dt><dd class="tabular-nums">{{ money(vat, cur) }}</dd></div>
          </dl>
          <button class="btn-primary w-full justify-between rounded-2xl px-5 py-4 text-lg" :disabled="!lines.length" @click="openPay">
            <span>{{ $t('pos.charge') }} <span class="text-xs font-normal opacity-80">(F9)</span></span><span class="tabular-nums">{{ money(total, cur) }}</span>
          </button>
        </div>
      </aside>
    </div>

    <!-- Payment -->
    <div v-if="paying" class="fixed inset-0 z-50 grid place-items-center bg-ink/50 p-4" @click.self="paying = false">
      <div class="card w-full max-w-2xl overflow-hidden shadow-2xl" role="dialog" :aria-label="$t('pos.payment')">
        <div class="grid md:grid-cols-[1fr_260px]">
          <div class="space-y-4 p-6">
            <div class="flex items-baseline justify-between">
              <div class="text-sm font-semibold text-muted">{{ $t('pos.amountDue') }}</div>
              <div class="text-3xl font-extrabold tabular-nums">{{ money(total, cur) }}</div>
            </div>
            <div class="grid grid-cols-5 gap-1.5">
              <button v-for="m in methods" :key="m.id" class="rounded-xl border-2 px-2 py-2.5 text-xs font-bold" :class="method === m.id ? 'border-ink bg-ink text-white' : 'border-line'" :title="`Alt+${m.key}`" @click="method = m.id">{{ $t(`common.pay.${m.id}`) }}</button>
            </div>
            <form class="space-y-2" @submit.prevent="remaining > 0 ? addPayment() : complete()">
              <div class="flex gap-2">
                <input id="pay-amount" v-model="amount" type="number" min="0" step="0.01" class="input py-3 text-lg tabular-nums" :placeholder="$t('pos.exactHint', { amount: (remaining / 100).toFixed(2) })" >
                <button type="button" class="btn-dark" :disabled="remaining <= 0" @click="addPayment()">{{ $t('common.add') }}</button>
              </div>
              <input v-if="method !== 'cash'" v-model="reference" class="input py-2" :placeholder="$t('pos.reference')" >
            </form>
            <div v-if="method === 'cash' && remaining > 0" class="flex flex-wrap gap-2">
              <button v-for="v in quickCash" :key="v" class="btn-ghost btn-sm tabular-nums" @click="addPayment(v)">{{ money(v * 100, cur) }}</button>
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
            <button class="btn-primary mt-4 w-full py-3" :disabled="busy" @click="complete">
              {{ busy ? $t('common.saving') : remaining > 0 ? $t('pos.payAndFinish', { amount: money(remaining, cur) }) : $t('pos.completeSale') }}
            </button>
            <button class="btn-ghost btn-sm mt-2" @click="paying = false">{{ $t('pos.backEsc') }}</button>
          </div>
        </div>
      </div>
    </div>

    <!-- Done -->
    <div v-if="done" class="fixed inset-0 z-50 grid place-items-center bg-ink/50 p-4">
      <div class="card w-full max-w-md p-6 text-center shadow-2xl" role="dialog" :aria-label="$t('pos.saleComplete')">
        <div class="mx-auto grid size-14 place-items-center rounded-full bg-mint-500/15 text-2xl text-mint-500">✓</div>
        <div class="mt-3 text-sm text-muted">{{ $t('pos.saleLine', { number: done.sale.number, amount: money(done.sale.total_cents, done.sale.currency) }) }}</div>
        <div v-if="done.sale.change_cents" class="mt-2 text-4xl font-extrabold tabular-nums">{{ $t('pos.changeAmount', { amount: money(done.sale.change_cents, done.sale.currency) }) }}</div>
        <div v-else class="mt-2 text-2xl font-extrabold">{{ $t('pos.paidInFull') }}</div>

        <div class="mt-6 grid gap-2">
          <div class="flex gap-2">
            <button class="btn-dark flex-1" @click="print(`/print/receipt/${done.sale.id}?w=${paperWidth}`)">{{ $t('pos.printReceipt') }}</button>
            <select v-model="paperWidth" class="input w-24 py-2" :aria-label="$t('pos.paperWidth')"><option value="80">80 mm</option><option value="58">58 mm</option></select>
          </div>
          <button v-if="done.sale.invoice_number" class="btn-ghost" @click="print(`/print/invoice/${done.sale.id}`)">{{ $t('pos.printTaxInvoice', { number: done.sale.invoice_number }) }}</button>
          <button v-else class="btn-ghost" @click="Object.assign(invForm, { open: true, error: '' })">{{ $t('pos.issueTaxInvoice') }}</button>
          <button class="btn-primary" @click="newSale">{{ $t('pos.newSale') }}</button>
        </div>
      </div>
    </div>

    <!-- Invoice form -->
    <div v-if="invForm.open" class="fixed inset-0 z-[60] grid place-items-center bg-ink/50 p-4" @click.self="invForm.open = false">
      <form class="card w-full max-w-md space-y-3 p-6 shadow-2xl" @submit.prevent="issueInvoiceNow">
        <div class="text-lg font-bold">{{ $t('pos.fullTaxInvoice') }}</div>
        <p class="text-xs text-muted">{{ $t('pos.invoicePermanent') }}</p>
        <input v-model="invForm.name" required class="input" :placeholder="$t('pos.customerNameReq')" autofocus >
        <div class="flex gap-2">
          <input v-model="invForm.tax_id" class="input" :placeholder="$t('pos.taxIdHint')" >
          <input v-model="invForm.branch" class="input" :placeholder="$t('pos.branchHint')" >
        </div>
        <textarea v-model="invForm.address" required rows="3" class="input" :placeholder="$t('pos.addressReq')" />
        <input v-model="invForm.phone" class="input" :placeholder="$t('common.phone')" >
        <p v-if="invForm.error" class="text-sm text-brand-700">{{ invForm.error }}</p>
        <div class="flex justify-end gap-2">
          <button type="button" class="btn-ghost" @click="invForm.open = false">{{ $t('common.cancel') }}</button>
          <button class="btn-primary">{{ $t('pos.issuePrint') }}</button>
        </div>
      </form>
    </div>

    <div v-if="toast" class="fixed bottom-6 left-1/2 z-[70] -translate-x-1/2 rounded-full px-4 py-2 text-sm font-semibold text-white shadow-lg" :class="toast.bad ? 'bg-brand-700' : 'bg-ink'">{{ toast.msg }}</div>
  </div>
</template>
