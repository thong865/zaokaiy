<script setup lang="ts">
import type { Content } from '~/utils/types'

definePageMeta({ layout: 'dashboard', middleware: 'seller' })
const api = useApi()
const route = useRoute()
const { shopId } = useShop()
const { t, locale } = useI18n()
const { data: contents, refresh } = await useAsyncData('contents', () => api<Content[]>(`/shops/${shopId.value}/contents`), { watch: [shopId], default: () => [] })
const { data: sellable } = await useSellable()
const { data: aiStatus } = await useAsyncData('ai-status', () => api<{ enabled: boolean; model: string }>('/ai/status'))

const kinds = computed(() => (['post', 'caption', 'video_script', 'description', 'ad_copy'] as const).map((k) => [k, t(`common.kind.${k}`)] as const))
const languages = computed(() => [
  ['English', t('grow.content.lang.en')],
  ['Thai', t('grow.content.lang.th')],
  ['Lao', t('grow.content.lang.lo')],
  ['Vietnamese', t('grow.content.lang.vi')],
  ['Chinese', t('grow.content.lang.zh')],
  ['Japanese', t('grow.content.lang.ja')],
] as const)
const g = reactive({
  product_id: (route.query.product as string) || '',
  kind: 'video_script',
  tone: 'fun, authentic, Gen-Z',
  language: locale.value === 'lo' ? 'Lao' : 'English',
  extra: '',
})
const draft = reactive({ title: '', body: '', media_url: '', ai: false })
const busy = ref(false)
const error = ref('')

async function generate() {
  if (!g.product_id) return (error.value = t('grow.content.chooseProduct'))
  busy.value = true
  error.value = ''
  try {
    const r = await api<{ title: string; body: string; ai: boolean }>(`/shops/${shopId.value}/ai/generate`, { method: 'POST', body: { ...g } })
    Object.assign(draft, { title: r.title, body: r.body, ai: r.ai })
  } catch (e) {
    error.value = apiError(e)
  } finally {
    busy.value = false
  }
}
async function save(status: 'draft' | 'published') {
  error.value = ''
  try {
    await api(`/shops/${shopId.value}/contents`, {
      method: 'POST',
      body: { product_id: g.product_id || null, kind: g.kind, title: draft.title, body: draft.body, media_url: draft.media_url || null, ai_generated: draft.ai, status },
    })
    Object.assign(draft, { title: '', body: '', media_url: '', ai: false })
    await refresh()
  } catch (e) {
    error.value = apiError(e)
  }
}
async function toggle(c: Content) {
  await api(`/contents/${c.id}`, { method: 'PATCH', body: { status: c.status === 'published' ? 'draft' : 'published' } })
  await refresh()
}
async function remove(c: Content) {
  await api(`/contents/${c.id}`, { method: 'DELETE' })
  await refresh()
}
const expanded = ref<string | null>(null)
</script>

<template>
  <div>
    <h1 class="page-title">{{ $t('grow.content.title') }}</h1>
    <p class="text-sm text-muted">{{ $t('grow.content.intro') }}</p>
    <div v-if="aiStatus && !aiStatus.enabled" class="mt-4 rounded-2xl bg-sun-400/20 px-4 py-3 text-sm">
      <i18n-t keypath="grow.content.offline" tag="span"><template #key><code class="rounded bg-surface px-1">ANTHROPIC_API_KEY</code></template><template #env><code class="rounded bg-surface px-1">.env</code></template></i18n-t>
    </div>

    <div class="mt-6 grid gap-6 lg:grid-cols-[340px_1fr]">
      <div class="card h-fit space-y-4 p-5">
        <div class="font-bold">✦ {{ $t('grow.content.generator') }}</div>
        <div>
          <label class="label">{{ $t('common.product') }}</label>
          <select v-model="g.product_id" class="input">
            <option value="" disabled>{{ $t('grow.choose') }}</option>
            <option v-for="s in sellable" :key="s.id" :value="s.id">{{ s.name }}{{ s.resold ? ` (${$t('grow.resold')})` : '' }}</option>
          </select>
        </div>
        <div>
          <label class="label">{{ $t('grow.content.format') }}</label>
          <div class="flex flex-wrap gap-1.5">
            <button v-for="k in kinds" :key="k[0]" type="button" class="chip border px-3 py-1" :class="g.kind === k[0] ? 'border-brand-500 bg-brand-500 text-white shadow-[0_6px_16px_-8px_var(--brand-500)]' : 'border-transparent bg-surface shadow-card hover:text-brand-500'" @click="g.kind = k[0]">{{ k[1] }}</button>
          </div>
        </div>
        <div><label class="label">{{ $t('grow.content.tone') }}</label><UInput v-model="g.tone" /></div>
        <div>
          <label class="label">{{ $t('common.language') }}</label>
          <select v-model="g.language" class="input"><option v-for="l in languages" :key="l[0]" :value="l[0]">{{ l[1] }}</option></select>
        </div>
        <div><label class="label">{{ $t('grow.content.extra') }}</label><UTextarea v-model="g.extra" :rows="2" :placeholder="$t('grow.content.extraPlaceholder')" /></div>
        <UButton type="submit" class="w-full" :disabled="busy" @click="generate">{{ busy ? $t('grow.content.generating') : $t('grow.content.generate') }}</UButton>
      </div>

      <div class="space-y-6">
        <div class="card space-y-3 p-5">
          <div class="flex items-center justify-between">
            <div class="font-bold">{{ $t('grow.content.editor') }}</div>
            <span v-if="draft.ai" class="chip bg-brand-50 text-brand-700">{{ $t('grow.content.aiGenerated') }}</span>
          </div>
          <UInput v-model="draft.title" :placeholder="$t('grow.content.titlePlaceholder')" class="font-semibold" />
          <UTextarea v-model="draft.body" :rows="10" :placeholder="$t('grow.content.bodyPlaceholder')" />
          <MediaField v-model="draft.media_url" :shop-id="shopId" :label="$t('grow.content.cover')" />
          <p v-if="error" class="text-sm text-brand-700">{{ error }}</p>
          <div class="flex justify-end gap-2">
            <UButton color="neutral" variant="soft" type="submit" :disabled="!draft.title || !draft.body" @click="save('draft')">{{ $t('grow.content.saveDraft') }}</UButton>
            <UButton type="submit" :disabled="!draft.title || !draft.body" @click="save('published')">{{ $t('grow.content.publish') }}</UButton>
          </div>
        </div>

        <div>
          <div class="font-bold">{{ $t('grow.content.library', { n: contents.length }) }}</div>
          <div v-if="contents.length" class="mt-3 space-y-3">
            <div v-for="c in contents" :key="c.id" class="card p-4">
              <div class="flex flex-wrap items-center gap-2">
                <span class="text-[11px] font-bold uppercase tracking-wider text-muted">{{ kindLabel(c.kind) }}</span>
                <StatusBadge :status="c.status" />
                <span v-if="c.ai_generated" class="chip bg-brand-50 text-brand-700">AI</span>
                <span class="text-xs text-muted">{{ ago(c.created_at) }}</span>
                <div class="ml-auto flex gap-2">
                  <UButton color="neutral" variant="soft" size="sm" type="submit" @click="toggle(c)">{{ c.status === 'published' ? $t('grow.content.unpublish') : $t('grow.content.publish') }}</UButton>
                  <UButton color="neutral" variant="soft" size="sm" type="submit" @click="remove(c)">{{ $t('common.delete') }}</UButton>
                </div>
              </div>
              <div class="mt-2 cursor-pointer font-semibold" @click="expanded = expanded === c.id ? null : c.id">{{ c.title }}</div>
              <div class="mt-1 flex gap-3"><MediaTile v-if="c.media_url" :asset="{ kind: isVideoUrl(c.media_url) ? 'video' : 'image', url: c.media_url, thumb_url: null, alt: c.title }" class="size-16 shrink-0" /><p class="whitespace-pre-line text-sm text-ink/80" :class="expanded !== c.id && 'line-clamp-3'">{{ c.body }}</p></div>
            </div>
          </div>
          <p v-else class="mt-3 text-sm text-muted">{{ $t('grow.content.empty') }}</p>
        </div>
      </div>
    </div>
  </div>
</template>
