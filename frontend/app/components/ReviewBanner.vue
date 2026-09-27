<script setup lang="ts">
import type { Product, ReviewEvent } from '~/utils/types'

const props = defineProps<{ product: Product }>()
const emit = defineEmits<{ submitted: [p: Product] }>()
const api = useApi()
const showHistory = ref(false)
const history = ref<ReviewEvent[] | null>(null)
const busy = ref(false)
const error = ref('')

async function loadHistory() {
  showHistory.value = !showHistory.value
  if (showHistory.value && !history.value) history.value = await api<ReviewEvent[]>(`/products/${props.product.id}/reviews`)
}
async function submit() {
  busy.value = true
  error.value = ''
  try {
    const p = await api<Product>(`/products/${props.product.id}/submit`, { method: 'POST' })
    history.value = null
    emit('submitted', p)
  } catch (e) {
    error.value = apiError(e)
  } finally {
    busy.value = false
  }
}

const tone = computed(() => ({
  not_submitted: 'border-line bg-white',
  pending: 'border-sun-400/60 bg-sun-400/10',
  approved: 'border-mint-500/40 bg-mint-500/5',
  rejected: 'border-brand-500/40 bg-brand-50',
})[props.product.review_status])
const { t, te } = useI18n()
const actionLabel = (a: string) => (te(`product.review.actions.${a}`) ? t(`product.review.actions.${a}`) : a)
</script>

<template>
  <div class="rounded-2xl border px-4 py-3 text-sm" :class="tone">
    <div class="flex flex-wrap items-center gap-3">
      <StatusBadge :status="product.review_status" :label="reviewLabel(product.review_status)" />
      <div class="min-w-0 flex-1">
        <template v-if="product.review_status === 'pending'">
          <b>{{ $t('product.review.pendingTitle') }}</b> {{ $t('product.review.pendingText') }}<template v-if="product.submitted_at"> · {{ $t('product.review.submittedAgo', { ago: ago(product.submitted_at) }) }}</template>.
        </template>
        <template v-else-if="product.review_status === 'approved' && product.status === 'active'">
          <b>{{ $t('product.review.liveTitle') }}</b> {{ $t('product.review.liveText') }}
        </template>
        <template v-else-if="product.review_status === 'approved'">{{ $t('product.review.approvedInactive') }}</template>
        <template v-else-if="product.review_status === 'rejected'">
          <b>{{ $t('product.review.rejectedTitle') }}</b> {{ product.review_note || $t('product.review.seeHistory') }}
        </template>
        <template v-else>{{ $t('product.review.notSubmitted') }}</template>
      </div>
      <button v-if="product.review_status === 'not_submitted' || product.review_status === 'rejected'" type="button" class="btn-primary btn-sm" :disabled="busy" @click="submit">
        {{ product.review_status === 'rejected' ? $t('product.review.resubmit') : $t('product.review.submit') }}
      </button>
      <button type="button" class="text-xs font-semibold text-muted underline hover:text-ink" @click="loadHistory">{{ showHistory ? $t('product.review.hide') : $t('product.review.history') }}</button>
    </div>
    <p v-if="error" class="mt-2 text-brand-700">{{ error }}</p>
    <ol v-if="showHistory" class="mt-3 space-y-1.5 border-t border-line/70 pt-3 text-xs">
      <li v-if="history && !history.length" class="text-muted">{{ $t('product.review.noActivity') }}</li>
      <li v-for="(h, i) in history ?? []" :key="i" class="flex gap-2">
        <span class="w-20 shrink-0 text-muted">{{ ago(h.created_at) }}</span>
        <span><b>{{ actionLabel(h.action) }}</b><template v-if="h.actor"> {{ $t('product.review.by', { actor: h.actor }) }}</template><template v-if="h.note"> — “{{ h.note }}”</template></span>
      </li>
    </ol>
  </div>
</template>
