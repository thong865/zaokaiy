<script setup lang="ts">
import type { CodRiskAnchor } from '../../utils/codRisk'

const props = defineProps<{ height: number }>()
const emit = defineEmits<{ close: [] }>()
const api = useApi()
interface Proof {
  block: { height: number; prev_hash: string; hash: string; event: string; payload: string; ts_micros: number }
  hash_ok: boolean
  anchor: CodRiskAnchor | null
  proof: { sibling: string; side: 'left' | 'right' }[] | null
  root_ok: boolean | null
}
const { data, error } = useLazyAsyncData(`codrisk-proof-${props.height}`, () => api<Proof>(`/admin/cod-risk/proof/${props.height}`), { server: false })
const pretty = computed(() => {
  try {
    return JSON.stringify(JSON.parse(data.value?.block.payload ?? '{}'), null, 2)
  } catch {
    return data.value?.block.payload
  }
})
</script>

<template>
  <UModal :open="true" :title="$t('codrisk.admin.chain.proofTitle', { height })" :ui="{ content: 'sm:max-w-2xl' }" @update:open="(o) => !o && emit('close')">
    <template #body>
      <p v-if="error" class="text-sm text-brand-700">{{ apiError(error) }}</p>
      <div v-else-if="data" class="space-y-4 text-sm">
        <div class="flex flex-wrap gap-2">
          <UBadge :color="data.hash_ok ? 'success' : 'error'" variant="soft" :icon="data.hash_ok ? 'i-lucide-check' : 'i-lucide-x'">{{ data.hash_ok ? $t('codrisk.admin.chain.hashOk') : $t('codrisk.admin.chain.hashBad') }}</UBadge>
          <UBadge v-if="data.root_ok !== null" :color="data.root_ok ? 'success' : 'error'" variant="soft" :icon="data.root_ok ? 'i-lucide-link' : 'i-lucide-unlink'">{{ data.root_ok ? $t('codrisk.admin.chain.rootOk') : $t('codrisk.admin.chain.rootBad') }}</UBadge>
          <UBadge v-else color="neutral" variant="soft" icon="i-lucide-clock">{{ $t('codrisk.admin.customer.notAnchored') }}</UBadge>
        </div>
        <dl class="grid grid-cols-[8rem_1fr] gap-x-3 gap-y-1 font-mono text-xs">
          <dt class="font-sans text-muted">{{ $t('codrisk.admin.chain.cols.event') }}</dt><dd>{{ $t(eventKey(data.block.event)) }} ({{ data.block.event }})</dd>
          <dt class="font-sans text-muted">{{ $t('codrisk.admin.chain.cols.hash') }}</dt><dd class="break-all">{{ data.block.hash }}</dd>
          <dt class="font-sans text-muted">prev_hash</dt><dd class="break-all">{{ data.block.prev_hash }}</dd>
          <dt class="font-sans text-muted">ts_micros</dt><dd>{{ data.block.ts_micros }}</dd>
          <template v-if="data.anchor">
            <dt class="font-sans text-muted">{{ $t('codrisk.admin.chain.cols.root') }}</dt><dd class="break-all">{{ data.anchor.merkle_root }}</dd>
            <dt class="font-sans text-muted">{{ $t('codrisk.admin.chain.cols.tx') }}</dt><dd class="break-all">{{ data.anchor.tx_ref }}</dd>
          </template>
        </dl>
        <pre class="max-h-56 overflow-auto rounded-xl bg-[var(--field)] p-3 text-xs">{{ pretty }}</pre>
        <div v-if="data.proof?.length">
          <div class="label">Merkle path</div>
          <ol class="space-y-0.5 font-mono text-[11px]">
            <li v-for="(s, i) in data.proof" :key="i" class="break-all"><span class="text-muted">{{ i + 1 }}. {{ s.side }}</span> {{ s.sibling }}</li>
          </ol>
        </div>
        <p v-if="data.anchor" class="text-xs text-muted">{{ $t('codrisk.admin.chain.proofHelp', { tx: data.anchor.tx_ref }) }}</p>
      </div>
    </template>
  </UModal>
</template>
