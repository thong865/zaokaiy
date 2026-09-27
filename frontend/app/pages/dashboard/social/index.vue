<script setup lang="ts">
import type { SocialFeedItem } from '~/utils/types'

definePageMeta({ layout: 'dashboard', middleware: 'seller' })
const api = useApi()
const { t, te } = useI18n()
const { shopId } = useShop()

interface Board {
  session: { id: string; title: string; started_at: string } | null
  currency: string
  totals: { orders: number; revenue_cents: number; comments: number; buyers: number }
  products: { id: string; code: string; name: string; price_cents: number; stock: number; image: string | null; claimed: number; buyers: number }[]
}
const board = ref<Board | null>(null)
const feed = ref<SocialFeedItem[]>([])
const onlyOrders = ref(false)
const error = ref('')

async function loadBoard() {
  if (!shopId.value) return
  board.value = await api<Board>(`/shops/${shopId.value}/social/board`)
}
async function loadFeed(initial = false) {
  if (!shopId.value) return
  const since = !initial && feed.value[0] ? feed.value[0].created_at : undefined
  const rows = await api<SocialFeedItem[]>(`/shops/${shopId.value}/social/feed`, {
    query: { since, session_id: board.value?.session?.id, only_orders: onlyOrders.value || undefined, limit: 150 },
  })
  if (initial) feed.value = rows
  else if (rows.length) {
    const seen = new Set(feed.value.map((f) => f.id))
    feed.value = [...rows.filter((r) => !seen.has(r.id)), ...feed.value].slice(0, 300)
    loadBoard()
  }
}
async function reload() {
  error.value = ''
  try {
    await loadBoard()
    await loadFeed(true)
  } catch (e) {
    error.value = apiError(e)
  }
}
watch([shopId, onlyOrders], reload)
onMounted(() => {
  reload()
  // Near-real-time: poll for new comments every 2s while the page is visible.
  const i = setInterval(() => { if (document.visibilityState === 'visible') loadFeed().catch(() => {}) }, 2000)
  onBeforeUnmount(() => clearInterval(i))
})

// Session controls
const title = ref('')
async function startLive() {
  await api(`/shops/${shopId.value}/social/sessions`, { method: 'POST', body: { title: title.value || t('social.board.defaultTitle', { date: fmtDate(new Date()) }) } })
  title.value = ''
  await reload()
}
async function endLive() {
  if (!board.value?.session) return
  await api(`/social/sessions/${board.value.session.id}/end`, { method: 'POST' })
  await reload()
}
const assigning = ref(false)
async function assignCodes() {
  assigning.value = true
  try {
    await api(`/shops/${shopId.value}/social/assign-codes`, { method: 'POST' })
    await loadBoard()
  } finally {
    assigning.value = false
  }
}

// Simulator
const buyers = ['Nok', 'Ploy', 'Mali', 'Somchai', 'Fah', 'Jib', 'Toon', 'Bee']
const sim = reactive({ provider: 'facebook', user_name: 'Nok', message: '' })
const simBusy = ref(false)
async function simulate(message?: string) {
  const msg = message ?? sim.message
  if (!msg.trim()) return
  simBusy.value = true
  error.value = ''
  try {
    await api(`/shops/${shopId.value}/social/simulate`, { method: 'POST', body: { ...sim, message: msg } })
    sim.message = ''
    await loadFeed()
    await loadBoard()
  } catch (e) {
    error.value = apiError(e)
  } finally {
    simBusy.value = false
  }
}
function randomBuyer() {
  sim.user_name = buyers[Math.floor(Math.random() * buyers.length)]!
}
const cur = computed(() => board.value?.currency ?? 'THB')
const resultLabel = (r: string) => (te(`social.board.result.${r}`) ? t(`social.board.result.${r}`) : undefined)
const replyLabel = (s: string) => (s && s !== 'none' && te(`social.board.reply.${s}`) ? t(`social.board.reply.${s}`) : '')
</script>

<template>
  <div>
    <div class="flex flex-wrap items-end justify-between gap-3">
      <div>
        <h1 class="page-title">{{ $t('social.title') }}</h1>
        <i18n-t keypath="social.board.intro" tag="p" class="text-sm text-muted"><template #code><b class="font-mono">CF A01 x2</b></template></i18n-t>
      </div>
    </div>
    <SocialNav class="mt-5" />
    <p v-if="error" class="mb-3 text-sm text-brand-700">{{ error }}</p>

    <!-- Session bar -->
    <div class="card mb-5 flex flex-wrap items-center gap-3 p-4" :class="board?.session && 'border-brand-500/40 bg-brand-50'">
      <template v-if="board?.session">
        <span class="relative flex size-3"><span class="absolute inline-flex size-full animate-ping rounded-full bg-brand-500 opacity-75" /><span class="relative inline-flex size-3 rounded-full bg-brand-500" /></span>
        <div>
          <div class="font-bold">{{ $t('social.board.live', { title: board.session.title }) }}</div>
          <div class="text-xs text-muted">{{ $t('social.board.since', { time: fmtTime(board.session.started_at) }) }}</div>
        </div>
        <button class="btn-ghost btn-sm ml-auto" @click="endLive">{{ $t('social.board.endLive') }}</button>
      </template>
      <template v-else>
        <div class="text-sm"><b>{{ $t('social.board.noSession') }}</b> {{ $t('social.board.noSessionHint') }}</div>
        <form class="ml-auto flex gap-2" @submit.prevent="startLive">
          <input v-model="title" class="input w-56 py-2" :placeholder="$t('social.board.titlePlaceholder')" >
          <button class="btn-primary btn-sm">{{ $t('social.board.goLive') }}</button>
        </form>
      </template>
    </div>

    <div v-if="board" class="mb-5 grid grid-cols-2 gap-4 lg:grid-cols-4">
      <StatCard :label="$t('social.board.stats.orders')" :value="board.totals.orders" accent />
      <StatCard :label="$t('social.board.stats.revenue')" :value="money(board.totals.revenue_cents, cur)" />
      <StatCard :label="$t('social.board.stats.buyers')" :value="board.totals.buyers" />
      <StatCard :label="$t('social.board.stats.comments')" :value="board.totals.comments" />
    </div>

    <div class="grid gap-5 xl:grid-cols-[1fr_440px]">
      <!-- Product codes -->
      <section>
        <div class="mb-3 flex items-center justify-between">
          <h2 class="font-bold">{{ $t('social.board.productCodes') }}</h2>
          <button class="btn-ghost btn-sm" :disabled="assigning" @click="assignCodes">{{ $t('social.board.autoAssign') }}</button>
        </div>
        <div v-if="board?.products.length" class="grid grid-cols-2 gap-3 sm:grid-cols-3">
          <div v-for="p in board.products" :key="p.id" class="card overflow-hidden" :class="p.stock === 0 && 'opacity-60'">
            <div class="relative">
              <ProductThumb :src="p.image" :name="p.name" class="aspect-[4/3]" rounded="rounded-none" />
              <span class="absolute left-2 top-2 rounded-lg bg-ink px-2 py-1 font-mono text-lg font-extrabold text-white">{{ p.code }}</span>
              <span v-if="p.stock === 0" class="absolute right-2 top-2 rounded-lg bg-brand-700 px-2 py-0.5 text-xs font-bold text-white">{{ $t('social.board.soldOut') }}</span>
            </div>
            <div class="p-3">
              <div class="truncate text-sm font-semibold">{{ p.name }}</div>
              <div class="mt-1 flex items-end justify-between">
                <span class="font-extrabold">{{ money(p.price_cents, cur) }}</span>
                <span class="text-xs" :class="p.stock <= 2 ? 'font-bold text-brand-700' : 'text-muted'">{{ $t('social.board.left', { n: num(p.stock) }) }}</span>
              </div>
              <div class="mt-2 h-1.5 overflow-hidden rounded-full bg-line">
                <div class="h-full rounded-full bg-mint-500" :style="{ width: `${p.claimed + p.stock ? (p.claimed / (p.claimed + p.stock)) * 100 : 0}%` }" />
              </div>
              <div class="mt-1 text-[11px] text-muted">{{ $t('social.board.claimedBuyers', { claimed: num(p.claimed), n: num(p.buyers) }, p.buyers) }}</div>
            </div>
          </div>
        </div>
        <EmptyState v-else :title="$t('social.board.emptyTitle')" :text="$t('social.board.emptyText')">
          <button class="btn-primary" @click="assignCodes">{{ $t('social.board.autoAssign') }}</button>
        </EmptyState>
      </section>

      <!-- Stream -->
      <section class="card flex h-[720px] flex-col overflow-hidden">
        <div class="border-b border-line p-3">
          <div class="flex items-center justify-between">
            <div class="font-bold">{{ $t('social.board.stream') }}</div>
            <label class="flex items-center gap-2 text-xs"><input v-model="onlyOrders" type="checkbox" class="accent-brand-500"> {{ $t('social.board.ordersOnly') }}</label>
          </div>
          <form class="mt-3 space-y-2 rounded-xl bg-paper p-2.5" @submit.prevent="simulate()">
            <div class="flex items-center gap-2 text-[11px] font-bold uppercase tracking-wider text-muted">{{ $t('social.board.testComment') }} <span class="font-normal normal-case tracking-normal">{{ $t('social.board.testCommentHint') }}</span></div>
            <div class="flex gap-2">
              <select v-model="sim.provider" class="input w-28 py-1.5 text-xs"><option value="facebook">Facebook</option><option value="tiktok">TikTok</option><option value="whatsapp">WhatsApp</option></select>
              <input v-model="sim.user_name" class="input py-1.5 text-xs" :placeholder="$t('social.board.customerName')" >
              <button type="button" class="btn-ghost btn-sm" :title="$t('social.board.randomBuyer')" @click="randomBuyer">🎲</button>
            </div>
            <div class="flex gap-2">
              <input v-model="sim.message" class="input py-1.5" :placeholder="$t('social.board.messagePlaceholder')" >
              <button class="btn-dark btn-sm" :disabled="simBusy">{{ $t('social.board.send') }}</button>
            </div>
            <div v-if="board?.products.length" class="flex flex-wrap gap-1">
              <button v-for="p in board.products.slice(0, 6)" :key="p.id" type="button" class="chip border border-line bg-white px-2 py-0.5 font-mono hover:border-ink" @click="randomBuyer(); simulate(`CF ${p.code}`)">CF {{ p.code }}</button>
            </div>
          </form>
        </div>
        <ul class="min-h-0 flex-1 divide-y divide-line/70 overflow-y-auto">
          <li v-for="f in feed" :key="f.id" class="px-3 py-2.5 text-sm" :class="f.result === 'claimed' && 'bg-mint-500/5'">
            <div class="flex items-start gap-2">
              <SocialProviderBadge :provider="f.provider" />
              <div class="min-w-0 flex-1">
                <div class="flex flex-wrap items-center gap-x-2">
                  <span class="font-bold">{{ f.user_name || f.user_id }}</span>
                  <span class="text-[11px] text-muted">{{ fmtTime(f.created_at, true) }}</span>
                  <StatusBadge v-if="f.result !== 'ignored'" :status="f.result" :label="resultLabel(f.result)" />
                  <NuxtLink v-if="f.order_number" :to="{ path: '/dashboard/social/orders', query: { q: f.order_number } }" class="font-mono text-xs font-bold text-brand-600 hover:underline">{{ f.order_number }}</NuxtLink>
                </div>
                <div class="break-words" :class="f.result === 'ignored' && 'text-muted'">{{ f.message }}</div>
                <div v-if="f.reply_text" class="mt-1 rounded-lg bg-paper px-2 py-1 text-xs text-ink/70">
                  <span class="font-semibold" :class="f.reply_status === 'failed' ? 'text-brand-700' : ''">↳ {{ replyLabel(f.reply_status) }}<template v-if="f.reply_error">: {{ f.reply_error }}</template></span>
                  <div class="line-clamp-2 whitespace-pre-line">{{ f.reply_text }}</div>
                </div>
              </div>
            </div>
          </li>
          <li v-if="!feed.length" class="p-8 text-center text-sm text-muted">{{ $t('social.board.waiting') }}</li>
        </ul>
      </section>
    </div>
  </div>
</template>
