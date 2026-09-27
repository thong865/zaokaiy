<script setup lang="ts">
const props = defineProps<{ provider: string; size?: 'sm' | 'md' }>()
const meta: Record<string, { label: string; short: string; cls: string }> = {
  facebook: { label: 'Facebook', short: 'FB', cls: 'bg-[#1877f2] text-white' },
  tiktok: { label: 'TikTok', short: 'TT', cls: 'bg-black text-white' },
  whatsapp: { label: 'WhatsApp', short: 'WA', cls: 'bg-[#25d366] text-white' },
  webhook: { label: 'Webhook', short: 'API', cls: 'bg-line text-ink' },
}
const m = computed(() => meta[props.provider] ?? meta.webhook!)
</script>

<template>
  <span class="inline-grid shrink-0 place-items-center rounded-md font-extrabold" :class="[m.cls, size === 'md' ? 'h-7 min-w-7 px-1.5 text-[11px]' : 'h-5 min-w-5 px-1 text-[9px]']" :title="m.label">{{ m.short }}</span>
</template>
