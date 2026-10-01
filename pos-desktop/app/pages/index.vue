<script setup lang="ts">
import type { Bill, BillData, Category, Line, Payment, Product, SaleRecord } from '~/utils/types'

const { t, currency, shop, dataVersion, settings, status } = usePos()
const { printSale, enabled: printerOn } = usePrinter()
const toast = useToast()

// ------------------------------------------------------------------ catalogue
const q = ref('')
const categoryId = ref<string | null>(null)
const products = ref<Product[]>([])
const categories = ref<Category[]>([])
const searchBox = ref<{ inputRef?: HTMLInputElement } | null>(null)
const topCategories = computed(() => categories.value.filter((c) => !c.parent_id))

async function loadProducts() {
  products.value = await call<Product[]>('search_products', { q: q.value.replace(/^\d+\s*[*xX]\s*/, ''), categoryId: categoryId.value, limit: 120 })
}
async function loadCategories() {
  categories.value = await call<Category[]>('list_categories')
}
let searchTimer: ReturnType<typeof setTimeout> | undefined
watch(q, () => {
  clearTimeout(searchTimer)
  searchTimer = setTimeout(loadProducts, 120)
})
watch(categoryId, loadProducts)
watch(dataVersion, () => {
  loadProducts()
  loadCategories()
})

// ------------------------------------------------------------------ bills (parked carts, saved locally)
const emptyData = (): BillData => ({ lines: [], discount: { mode: 'amount', value: 0 }, note: '', customer: { name: '', phone: '' } })
const bills = ref<Bill[]>([])
const activeId = ref('')
const bill = computed(() => bills.value.find((b) => b.id === activeId.value) ?? bills.value[0]!)
const cart = computed(() => bill.value.data)

function newBill(select = true) {
  const b: Bill = { id: crypto.randomUUID(), label: '', data: emptyData(), created_at: new Date().toISOString() }
  bills.value.push(b)
  if (select) activeId.value = b.id
  return b
}
async function loadBills() {
  const rows = await call<Bill[]>('list_bills')
  bills.value = rows.map((b) => ({ ...b, data: { ...emptyData(), ...(b.data ?? {}) } }))
  if (!bills.value.length) newBill()
  if (!bills.value.find((b) => b.id === activeId.value)) activeId.value = bills.value[0]!.id
}
function nextBill() {
  const i = bills.value.findIndex((b) => b.id === activeId.value)
  activeId.value = bills.value[(i + 1) % bills.value.length]!.id
}
async function closeBill(id: string) {
  await call('delete_bill', { id })
  bills.value = bills.value.filter((b) => b.id !== id)
  if (!bills.value.length) newBill()
  if (activeId.value === id) activeId.value = bills.value[0]!.id
}
const renaming = ref<Bill | null>(null)
const renameText = ref('')
function startRename(b: Bill) {
  renaming.value = b
  renameText.value = b.label
}
function saveRename() {
  if (renaming.value) renaming.value.label = renameText.value.trim().slice(0, 60)
  renaming.value = null
}
const billName = (b: Bill, i: number) => b.label || t('till.bill', { n: i + 1 })
const itemCount = (b: Bill) => b.data.lines.reduce((s, l) => s + l.qty, 0)

// Autosave the cart so a crash or power cut never loses a sale in progress.
let saveTimer: ReturnType<typeof setTimeout> | undefined
watch(
  bills,
  () => {
    clearTimeout(saveTimer)
    saveTimer = setTimeout(async () => {
      for (const b of bills.value) {
        if (b.data.lines.length || b.label) await call('save_bill', { id: b.id, label: b.label, data: b.data }).catch(() => {})
      }
    }, 400)
  },
  { deep: true },
)

// ------------------------------------------------------------------ cart
const lastKey = ref('')
function addProduct(p: Product, qty = 1) {
  const existing = cart.value.lines.find((l) => l.product_id === p.id && l.discount_cents === 0 && l.unit_price_cents === p.price_cents)
  if (existing) {
    existing.qty += qty
    lastKey.value = existing.key
  } else {
    const line: Line = { key: crypto.randomUUID(), product_id: p.id, name: p.name, sku: p.sku, qty, unit_price_cents: p.price_cents, discount_cents: 0 }
    cart.value.lines.push(line)
    lastKey.value = line.key
  }
  if (p.stock - qty < 0) toast.add({ title: t('till.outOfStock'), description: p.name, color: 'warning', icon: 'i-lucide-package-x' })
}
function setQty(l: Line, qty: number) {
  if (qty <= 0) cart.value.lines = cart.value.lines.filter((x) => x.key !== l.key)
  else l.qty = Math.min(qty, 10000)
}
function clearCart() {
  bill.value.data = emptyData()
}

const lineTotal = (l: Line) => l.unit_price_cents * l.qty - l.discount_cents
const subtotal = computed(() => cart.value.lines.reduce((s, l) => s + lineTotal(l), 0))
const billDiscount = computed(() => {
  const d = cart.value.discount
  const v = d.mode === 'percent' ? Math.round((subtotal.value * Math.min(100, Math.max(0, d.value))) / 100) : toCents(d.value)
  return Math.min(Math.max(0, v), subtotal.value)
})
/** Same arithmetic as the Rust side / server (round half up). */
const vatFor = (net: number, bps: number, incl: boolean): [number, number] => {
  if (!bps) return [0, net]
  const div = (a: number, b: number) => Math.floor((a * 2 + b) / (b * 2))
  if (incl) return [div(net * bps, 10000 + bps), net]
  const v = div(net * bps, 10000)
  return [v, net + v]
}
const totals = computed(() => {
  const [vat, total] = vatFor(subtotal.value - billDiscount.value, shop.value?.vat_bps ?? 0, shop.value?.prices_include_vat ?? true)
  return { vat, total }
})

// ------------------------------------------------------------------ scanning & search
async function scan(raw: string) {
  const m = raw.trim().match(/^(\d{1,4})\s*[*xX]\s*(.+)$/)
  const qty = m ? Number(m[1]) : 1
  const code = m ? m[2]! : raw.trim()
  const p = await call<Product | null>('scan_code', { code })
  if (p) {
    addProduct(p, qty)
    if (settings.value.beep) beep(true)
    return true
  }
  if (settings.value.beep) beep(false)
  toast.add({ title: t('till.unknownCode', { code }), color: 'warning', icon: 'i-lucide-scan-barcode' })
  return false
}
useScanner(scan)

async function onSearchEnter() {
  const value = q.value.trim()
  if (!value) return
  const m = value.match(/^(\d{1,4})\s*[*xX]\s*(.+)$/)
  const exact = await call<Product | null>('scan_code', { code: m ? m[2] : value })
  if (exact) {
    addProduct(exact, m ? Number(m[1]) : 1)
    if (settings.value.beep) beep(true)
  } else if (products.value.length === 1) {
    addProduct(products.value[0]!, m ? Number(m[1]) : 1)
  } else return
  q.value = ''
}

// ------------------------------------------------------------------ custom item & line editor
const customOpen = ref(false)
const custom = reactive({ name: '', price: '' })
function addCustom() {
  const cents = toCents(custom.price)
  if (!custom.name.trim() || cents < 0) return
  cart.value.lines.push({ key: crypto.randomUUID(), product_id: null, name: custom.name.trim(), sku: '', qty: 1, unit_price_cents: cents, discount_cents: 0 })
  custom.name = ''
  custom.price = ''
  customOpen.value = false
}

const editing = ref<Line | null>(null)
const edit = reactive({ qty: 1, price: '', discount: '' })
function openLine(l: Line) {
  editing.value = l
  edit.qty = l.qty
  edit.price = String(l.unit_price_cents / 100)
  edit.discount = l.discount_cents ? String(l.discount_cents / 100) : ''
}
function saveLine() {
  const l = editing.value
  if (!l) return
  l.unit_price_cents = Math.max(0, toCents(edit.price))
  setQty(l, Math.round(Number(edit.qty) || 0))
  l.discount_cents = Math.min(Math.max(0, toCents(edit.discount)), l.unit_price_cents * l.qty)
  editing.value = null
}

// ------------------------------------------------------------------ charge
const payOpen = ref(false)
const paying = ref(false)
const payError = ref('')
const receiptOpen = ref(false)
const lastSale = ref<SaleRecord | null>(null)

function charge() {
  if (!cart.value.lines.length) return
  payError.value = ''
  payOpen.value = true
}

async function complete(payments: Payment[]) {
  paying.value = true
  payError.value = ''
  try {
    const c = cart.value
    const customer = c.customer.name.trim() || c.customer.phone.trim() ? { name: c.customer.name.trim(), phone: c.customer.phone.trim() } : null
    const sale = await call<SaleRecord>('create_sale', {
      input: {
        lines: c.lines.map(({ key: _k, ...l }) => l),
        discount_cents: billDiscount.value,
        payments,
        note: c.note,
        customer,
        bill_id: bill.value.id,
      },
    })
    lastSale.value = sale
    payOpen.value = false
    // The bill is closed in the same local transaction as the sale.
    const id = bill.value.id
    bills.value = bills.value.filter((b) => b.id !== id)
    if (!bills.value.length) newBill()
    else activeId.value = bills.value[0]!.id
    receiptOpen.value = true
    loadProducts()
    if (printerOn.value && settings.value.printer.autoPrint) {
      printSale(sale).catch((e) => toast.add({ title: t('common.error'), description: (e as Error).message, color: 'error' }))
    }
  } catch (e) {
    payError.value = (e as Error).message
  } finally {
    paying.value = false
  }
}

function focusSearch() {
  nextTick(() => searchBox.value?.inputRef?.focus())
}

// ------------------------------------------------------------------ keyboard
function onKey(e: KeyboardEvent) {
  if (payOpen.value || receiptOpen.value || editing.value || customOpen.value || renaming.value) return
  if (e.key === 'F2') {
    e.preventDefault()
    focusSearch()
  } else if (e.key === 'F9') {
    e.preventDefault()
    charge()
  } else if (e.key === 'F8') {
    e.preventDefault()
    newBill()
  } else if (e.key === 'F7') {
    e.preventDefault()
    nextBill()
  } else if (e.key === 'Escape') {
    q.value = ''
    ;(document.activeElement as HTMLElement | null)?.blur()
  }
}
onMounted(async () => {
  window.addEventListener('keydown', onKey)
  await Promise.all([loadBills(), loadProducts(), loadCategories()])
})
onBeforeUnmount(() => window.removeEventListener('keydown', onKey))

// First letter for the placeholder tile (skip Lao/Thai leading vowels: ເ ແ ໂ ໃ ໄ / เ แ โ ใ ไ).
const letter = (name: string) => name.trim().replace(/^[\u0EC0-\u0EC4\u0E40-\u0E44]/, '').charAt(0).toUpperCase()
const hue = (id: string) => ([...id].reduce((h, c) => (h * 131 + c.charCodeAt(0) * 7919) % 100003, 17) * 47) % 360
</script>

<template>
  <div v-if="bills.length" class="grid h-full grid-cols-[1fr_26rem] overflow-hidden">
    <!-- products -->
    <section class="flex min-h-0 flex-col gap-3 p-4">
      <div class="flex gap-2">
        <UInput
          ref="searchBox"
          v-model="q"
          size="xl"
          icon="i-lucide-scan-barcode"
          :placeholder="t('till.search')"
          class="flex-1"
          autofocus
          @keydown.enter.prevent="onSearchEnter"
        />
        <UButton size="xl" color="neutral" variant="soft" icon="i-lucide-plus" @click="customOpen = true">{{ t('till.custom') }}</UButton>
      </div>
      <div v-if="topCategories.length" class="flex gap-2 overflow-x-auto pb-1">
        <UButton :variant="!categoryId ? 'solid' : 'soft'" :color="!categoryId ? 'primary' : 'neutral'" size="sm" @click="categoryId = null">{{ t('till.all') }}</UButton>
        <UButton
          v-for="c in topCategories"
          :key="c.id"
          :variant="categoryId === c.id ? 'solid' : 'soft'"
          :color="categoryId === c.id ? 'primary' : 'neutral'"
          size="sm"
          class="shrink-0"
          @click="categoryId = c.id"
        >{{ c.name }}</UButton>
      </div>
      <div class="min-h-0 flex-1 overflow-y-auto">
        <div v-if="!products.length" class="grid h-full place-items-center text-center text-muted">
          <div>
            <UIcon name="i-lucide-package-search" class="mx-auto size-10" />
            <p class="mt-2 text-sm">{{ status?.products ? t('till.noProducts') : t('till.catalogEmpty') }}</p>
          </div>
        </div>
        <div v-else class="grid grid-cols-[repeat(auto-fill,minmax(9.5rem,1fr))] gap-3">
          <button
            v-for="p in products"
            :key="p.id"
            type="button"
            class="card card-hover flex flex-col overflow-hidden text-left active:scale-[.98]"
            @click="addProduct(p)"
          >
            <div class="relative aspect-[4/3] w-full overflow-hidden">
              <div class="thumb-ph absolute inset-0 grid place-items-center text-3xl font-extrabold" :style="{ '--ph': `hsl(${hue(p.id)} 70% 88%)` }">{{ letter(p.name) }}</div>
              <img v-if="p.image" :src="p.image" alt="" class="absolute inset-0 size-full object-cover" loading="lazy" @error="($event.target as HTMLImageElement).style.display = 'none'" />
              <span
                :class="['chip absolute right-2 top-2', p.stock <= 0 ? 'bg-error text-white' : p.stock <= 5 ? 'bg-amber-400 text-ink' : 'bg-surface/90 text-ink']"
              >{{ p.stock <= 0 ? t('till.outOfStock') : t('till.stock', { n: p.stock }) }}</span>
            </div>
            <div class="flex flex-1 flex-col p-2.5">
              <span class="line-clamp-2 text-sm font-semibold leading-snug">{{ p.name }}</span>
              <span class="mt-auto pt-1 font-extrabold text-brand-600 tabular-nums">{{ money(p.price_cents, currency) }}</span>
            </div>
          </button>
        </div>
      </div>
      <p class="text-[11px] text-muted">{{ t('till.shortcuts') }}</p>
    </section>

    <!-- cart -->
    <aside class="flex min-h-0 flex-col border-l border-line bg-surface">
      <div class="flex items-center gap-1 overflow-x-auto border-b border-line px-2 py-2">
        <button
          v-for="(b, i) in bills"
          :key="b.id"
          type="button"
          :class="['group flex shrink-0 items-center gap-1.5 rounded-lg px-2.5 py-1.5 text-xs font-semibold', b.id === activeId ? 'bg-brand-500 text-white' : 'bg-field hover:bg-[var(--field-hover)]']"
          @click="activeId = b.id"
          @dblclick="startRename(b)"
        >
          {{ billName(b, i) }}
          <span v-if="itemCount(b)" :class="['rounded px-1', b.id === activeId ? 'bg-white/25' : 'bg-ink/10']">{{ itemCount(b) }}</span>
          <UIcon v-if="bills.length > 1" name="i-lucide-x" class="size-3 opacity-60 hover:opacity-100" @click.stop="closeBill(b.id)" />
        </button>
        <UButton size="xs" variant="ghost" color="neutral" icon="i-lucide-plus" :title="t('till.newBill')" @click="newBill()" />
      </div>

      <div class="min-h-0 flex-1 overflow-y-auto">
        <div v-if="!cart.lines.length" class="grid h-full place-items-center p-6 text-center text-sm text-muted">
          <div>
            <UIcon name="i-lucide-scan-line" class="mx-auto size-10" />
            <p class="mt-2">{{ t('till.empty') }}</p>
          </div>
        </div>
        <ul v-else class="divide-y divide-line">
          <li v-for="l in cart.lines" :key="l.key" :class="['flex items-center gap-2 px-3 py-2.5 transition-colors', l.key === lastKey && 'bg-brand-50']">
            <button type="button" class="min-w-0 flex-1 text-left" @click="openLine(l)">
              <div class="truncate text-sm font-semibold">{{ l.name }}</div>
              <div class="text-xs text-muted tabular-nums">
                {{ money(l.unit_price_cents, currency) }}<template v-if="l.discount_cents"> · −{{ money(l.discount_cents, currency) }}</template>
              </div>
            </button>
            <div class="flex items-center gap-1">
              <UButton size="xs" color="neutral" variant="soft" icon="i-lucide-minus" @click="setQty(l, l.qty - 1)" />
              <span class="w-8 text-center text-sm font-bold tabular-nums">{{ l.qty }}</span>
              <UButton size="xs" color="neutral" variant="soft" icon="i-lucide-plus" @click="setQty(l, l.qty + 1)" />
            </div>
            <span class="w-24 text-right text-sm font-bold tabular-nums">{{ money(lineTotal(l), currency) }}</span>
          </li>
        </ul>
      </div>

      <div class="space-y-2 border-t border-line p-4">
        <div class="grid grid-cols-2 gap-2">
          <UInput v-model="cart.customer.name" size="sm" icon="i-lucide-user" :placeholder="t('till.customerName')" />
          <UInput v-model="cart.note" size="sm" icon="i-lucide-sticky-note" :placeholder="t('till.note')" />
        </div>
        <div class="flex items-center gap-2 text-sm">
          <span class="text-muted">{{ t('till.billDiscount') }}</span>
          <UInput v-model.number="cart.discount.value" size="xs" type="number" min="0" class="ml-auto w-24" />
          <UButtonGroup size="xs">
            <UButton :variant="cart.discount.mode === 'amount' ? 'solid' : 'soft'" color="neutral" @click="cart.discount.mode = 'amount'">{{ currency }}</UButton>
            <UButton :variant="cart.discount.mode === 'percent' ? 'solid' : 'soft'" color="neutral" @click="cart.discount.mode = 'percent'">%</UButton>
          </UButtonGroup>
        </div>
        <div class="space-y-0.5 text-sm">
          <div class="flex justify-between text-muted"><span>{{ t('till.subtotal') }}</span><span class="tabular-nums">{{ money(subtotal, currency) }}</span></div>
          <div v-if="billDiscount" class="flex justify-between text-muted"><span>{{ t('till.discount') }}</span><span class="tabular-nums">−{{ money(billDiscount, currency) }}</span></div>
          <div v-if="shop?.vat_bps" class="flex justify-between text-muted">
            <span>{{ shop.prices_include_vat ? t('till.vatIncl', { rate: pct(shop.vat_bps) }) : t('till.vat', { rate: pct(shop.vat_bps) }) }}</span>
            <span class="tabular-nums">{{ money(totals.vat, currency) }}</span>
          </div>
        </div>
        <div class="flex items-center gap-2">
          <UButton color="neutral" variant="ghost" icon="i-lucide-trash-2" :disabled="!cart.lines.length" :title="t('till.clear')" @click="clearCart" />
          <UButton size="xl" class="flex-1 justify-between" :disabled="!cart.lines.length" @click="charge">
            <span>{{ t('till.charge') }} <span class="opacity-70">F9</span></span>
            <span class="text-xl tabular-nums">{{ money(totals.total, currency) }}</span>
          </UButton>
        </div>
      </div>
    </aside>

    <PaymentModal v-model:open="payOpen" :total="totals.total" :currency="currency" :busy="paying" :error="payError" @complete="complete" />
    <ReceiptModal v-model:open="receiptOpen" :sale="lastSale" @next="focusSearch" />

    <UModal v-model:open="customOpen" :title="t('till.custom')">
      <template #body>
        <form class="space-y-3" @submit.prevent="addCustom">
          <UFormField :label="t('till.customName')"><UInput v-model="custom.name" class="w-full" autofocus /></UFormField>
          <UFormField :label="t('till.price')"><UInput v-model="custom.price" inputmode="decimal" class="w-full" /></UFormField>
          <UButton type="submit" block>{{ t('till.add') }}</UButton>
        </form>
      </template>
    </UModal>

    <UModal :open="!!renaming" :title="t('till.rename')" @update:open="(v: boolean) => !v && (renaming = null)">
      <template #body>
        <form class="flex gap-2" @submit.prevent="saveRename">
          <UInput v-model="renameText" class="flex-1" maxlength="60" autofocus />
          <UButton type="submit">{{ t('common.save') }}</UButton>
        </form>
      </template>
    </UModal>

    <UModal :open="!!editing" :title="editing?.name" @update:open="(v: boolean) => !v && (editing = null)">
      <template #body>
        <form class="space-y-3" @submit.prevent="saveLine">
          <UFormField :label="t('till.qty')"><UInputNumber v-model="edit.qty" :min="0" :max="10000" class="w-full" /></UFormField>
          <UFormField :label="t('till.unitPrice')"><UInput v-model="edit.price" inputmode="decimal" class="w-full" /></UFormField>
          <UFormField :label="t('till.lineDiscount')"><UInput v-model="edit.discount" inputmode="decimal" class="w-full" /></UFormField>
          <div class="flex gap-2">
            <UButton color="error" variant="soft" icon="i-lucide-trash-2" @click="editing && setQty(editing, 0); editing = null">{{ t('till.remove') }}</UButton>
            <UButton type="submit" class="flex-1" block>{{ t('common.save') }}</UButton>
          </div>
        </form>
      </template>
    </UModal>
  </div>
</template>
