<script setup lang="ts">
import type { Product, PublicMedia, ReviewEvent, Shop } from '~/utils/types'

definePageMeta({ layout: 'admin', middleware: 'admin' })
const api = useApi()
const route = useRoute()
interface Detail {
  product: Product
  shop: Shop
  owner: { email: string; name: string }
  shop_stats: { approved: number; rejected: number }
  category_path: string | null
  media: PublicMedia[]
  history: ReviewEvent[]
}
const { data, refresh } = await useAsyncData(`admin-product:${route.params.id}`, () => api<Detail>(`/admin/products/${route.params.id}`))
if (!data.value) throw createError({ statusCode: 404, statusMessage: tr('admin.product.notFound') })

const note = ref('')
const error = ref('')
const busy = ref(false)
const { t } = useI18n()
const templates = computed(() =>
  ['photos', 'description', 'category', 'price', 'notAllowed'].map((k) => ({ short: t(`admin.product.templates.${k}.short`), full: t(`admin.product.templates.${k}.full`) })),
)
async function decide(action: 'approve' | 'reject') {
  error.value = ''
  busy.value = true
  try {
    await api(`/admin/products/${route.params.id}/review`, { method: 'POST', body: { action, note: note.value } })
    note.value = ''
    await refresh()
    // jump to the next pending item, if any
    const next = await api<{ id: string }[]>('/admin/products', { query: { review_status: 'pending', limit: 1 } })
    if (next[0] && next[0].id !== route.params.id) await navigateTo(`/admin/products/${next[0].id}`)
  } catch (e) {
    error.value = apiError(e)
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <div v-if="data">
    <NuxtLink to="/admin" class="text-sm text-muted hover:text-ink">{{ $t('admin.product.back') }}</NuxtLink>
    <div class="mt-2 flex flex-wrap items-center gap-3">
      <h1 class="page-title">{{ data.product.name }}</h1>
      <StatusBadge :status="data.product.review_status" :label="reviewLabel(data.product.review_status)" />
      <StatusBadge :status="data.product.status" />
    </div>

    <div class="mt-6 grid gap-6 lg:grid-cols-[1fr_360px]">
      <div class="space-y-6">
        <div class="card p-5">
          <ProductGallery v-if="data.media.length" :media="data.media" :name="data.product.name" class="mx-auto max-w-lg" />
          <p v-else class="rounded-xl bg-sun-400/15 p-4 text-sm text-amber-800">{{ $t('admin.product.noMedia') }}</p>
        </div>
        <div class="card space-y-4 p-5">
          <dl class="grid grid-cols-2 gap-4 text-sm sm:grid-cols-4">
            <div><dt class="text-xs text-muted">{{ $t('admin.product.price') }}</dt><dd class="font-bold">{{ money(data.product.price_cents, data.shop.currency) }}</dd></div>
            <div><dt class="text-xs text-muted">{{ $t('admin.product.stock') }}</dt><dd class="font-bold">{{ data.product.stock }}</dd></div>
            <div><dt class="text-xs text-muted">{{ $t('admin.product.sku') }}</dt><dd class="font-mono text-xs">{{ data.product.sku }}</dd></div>
            <div><dt class="text-xs text-muted">{{ $t('admin.product.sellStaff') }}</dt><dd class="font-bold">{{ data.product.allow_resell ? pct(data.product.commission_bps) : $t('admin.product.off') }}</dd></div>
          </dl>
          <div>
            <div class="text-xs text-muted">{{ $t('admin.product.marketplaceCategory') }}</div>
            <div class="font-semibold" :class="!data.category_path && 'text-brand-700'">{{ data.category_path ?? $t('admin.product.none') }}</div>
          </div>
          <div>
            <div class="text-xs text-muted">{{ $t('admin.product.description') }}</div>
            <p class="mt-1 whitespace-pre-line text-sm">{{ data.product.description || '—' }}</p>
          </div>
        </div>
      </div>

      <div class="space-y-4">
        <div class="card sticky top-20 space-y-3 p-5">
          <div class="font-bold">{{ $t('admin.product.decision') }}</div>
          <UTextarea v-model="note" :rows="3" :placeholder="$t('admin.product.notePlaceholder')" />
          <div class="flex flex-wrap gap-1.5">
            <button v-for="tpl in templates" :key="tpl.full" type="button" class="chip border border-line bg-surface px-2 py-1 text-left font-medium hover:border-ink" @click="note = tpl.full">{{ tpl.short }}</button>
          </div>
          <p v-if="error" class="text-sm text-brand-700">{{ error }}</p>
          <div class="grid grid-cols-2 gap-2">
            <UButton color="neutral" variant="ghost" type="submit" class="bg-brand-50 text-brand-700 hover:bg-brand-100" :disabled="busy || !note.trim()" @click="decide('reject')">{{ $t('admin.product.requestChanges') }}</UButton>
            <UButton type="submit" :disabled="busy" @click="decide('approve')">{{ $t('admin.product.approve') }}</UButton>
          </div>
          <p class="text-xs text-muted">{{ $t('admin.product.decisionHint') }}</p>
        </div>
        <div class="card space-y-2 p-5 text-sm">
          <div class="font-bold">{{ $t('admin.product.seller') }}</div>
          <NuxtLink :to="`/s/${data.shop.slug}`" target="_blank" class="font-semibold hover:underline">{{ data.shop.name }} ↗</NuxtLink>
          <div class="text-muted">{{ data.owner.name }} · {{ data.owner.email }}</div>
          <div class="text-muted">{{ $t('admin.product.joined', { date: fmtDate(data.shop.created_at), approved: data.shop_stats.approved, rejected: data.shop_stats.rejected }) }}</div>
          <span v-if="data.shop.auto_approve" class="chip bg-mint-500/15 text-mint-500">{{ $t('admin.product.trusted') }}</span>
        </div>
        <div class="card p-5 text-sm">
          <div class="font-bold">{{ $t('admin.product.history') }}</div>
          <ol class="mt-2 space-y-2">
            <li v-for="(h, i) in data.history" :key="i">
              <b>{{ $te(`admin.product.actions.${h.action}`) ? $t(`admin.product.actions.${h.action}`) : h.action }}</b> <span class="text-muted">· {{ h.actor ?? $t('admin.product.system') }} · {{ ago(h.created_at) }}</span>
              <div v-if="h.note" class="text-muted">“{{ h.note }}”</div>
            </li>
            <li v-if="!data.history.length" class="text-muted">{{ $t('admin.product.noActivity') }}</li>
          </ol>
        </div>
      </div>
    </div>
  </div>
</template>
