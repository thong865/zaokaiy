<script setup lang="ts">
definePageMeta({ layout: 'dashboard', middleware: 'seller' })
const api = useApi()
const { shopId, shop } = useShop()
const { t } = useI18n()
interface Action { tool: string; input: Record<string, unknown>; result: unknown }
interface Msg { role: 'user' | 'assistant'; content: string; actions?: Action[] }
const messages = ref<Msg[]>([])
const input = ref('')
const busy = ref(false)
const error = ref('')
const scroller = ref<HTMLElement | null>(null)
watch(shopId, () => (messages.value = []))

const suggestions = computed(() => [1, 2, 3, 4, 5].map((i) => t(`grow.assistant.suggest.s${i}`)))

async function send(text?: string) {
  const content = (text ?? input.value).trim()
  if (!content || busy.value) return
  messages.value.push({ role: 'user', content })
  input.value = ''
  busy.value = true
  error.value = ''
  await nextTick(() => scroller.value?.scrollTo({ top: 1e9, behavior: 'smooth' }))
  try {
    const r = await api<{ reply: string; actions: Action[] }>(`/shops/${shopId.value}/ai/agent`, {
      method: 'POST',
      body: { messages: messages.value.map(({ role, content }) => ({ role, content })) },
    })
    messages.value.push({ role: 'assistant', content: r.reply, actions: r.actions })
  } catch (e) {
    error.value = apiError(e)
    messages.value.pop()
    input.value = content
  } finally {
    busy.value = false
    await nextTick(() => scroller.value?.scrollTo({ top: 1e9, behavior: 'smooth' }))
  }
}

/** Minimal, safe markdown: escape HTML, then bold/code/bullets. */
function md(s: string) {
  const esc = s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')
  return esc
    .replace(/\*\*(.+?)\*\*/g, '<b>$1</b>')
    .replace(/`([^`]+)`/g, '<code class="rounded bg-paper px-1">$1</code>')
    .replace(/^#{1,3} (.+)$/gm, '<div class="mt-2 font-bold">$1</div>')
    .replace(/^[-•] (.+)$/gm, '<div class="pl-4 -indent-3">• $1</div>')
    .replace(/<\/div>\n/g, '</div>')
    .replace(/\n/g, '<br>')
}
const TOOLS = ['get_shop_stats', 'list_products', 'get_low_stock', 'list_recent_orders', 'list_reseller_requests', 'draft_content', 'draft_ad_campaign']
const toolLabel = computed<Record<string, string>>(() => Object.fromEntries(TOOLS.map((k) => [k, t(`grow.assistant.tool.${k}`)])))
</script>

<template>
  <div class="flex h-[calc(100dvh-8rem)] flex-col">
    <div>
      <h1 class="page-title">{{ $t('grow.assistant.title') }}</h1>
      <p class="text-sm text-muted">{{ $t('grow.assistant.intro', { shop: shop?.name ?? '' }) }}</p>
    </div>
    <div ref="scroller" class="card mt-5 flex-1 space-y-4 overflow-y-auto p-5">
      <div v-if="!messages.length" class="grid h-full place-items-center">
        <div class="max-w-lg text-center">
          <div class="mx-auto grid size-14 place-items-center rounded-2xl bg-brand-500 text-2xl text-white">✦</div>
          <div class="mt-4 font-bold">{{ $t('grow.assistant.empty') }}</div>
          <div class="mt-4 flex flex-wrap justify-center gap-2">
            <button v-for="s in suggestions" :key="s" class="chip border border-line bg-white px-3 py-1.5 text-left text-xs font-medium hover:border-ink" @click="send(s)">{{ s }}</button>
          </div>
        </div>
      </div>
      <div v-for="(m, i) in messages" :key="i" class="flex" :class="m.role === 'user' ? 'justify-end' : 'justify-start'">
        <div class="max-w-[85%]">
          <div v-if="m.actions?.length" class="mb-2 flex flex-wrap gap-1.5">
            <span v-for="(a, j) in m.actions" :key="j" class="chip bg-paper text-muted">⚙ {{ toolLabel[a.tool] ?? a.tool }}</span>
          </div>
          <div v-if="m.role === 'user'" class="rounded-2xl rounded-br-md bg-ink px-4 py-2.5 text-sm text-white">{{ m.content }}</div>
          <!-- eslint-disable-next-line vue/no-v-html -->
          <div v-else class="rounded-2xl rounded-bl-md bg-paper px-4 py-3 text-sm leading-relaxed" v-html="md(m.content)" />
        </div>
      </div>
      <div v-if="busy" class="flex gap-1 px-2"><span v-for="d in 3" :key="d" class="size-2 animate-bounce rounded-full bg-muted" :style="{ animationDelay: `${d * 120}ms` }" /></div>
    </div>
    <p v-if="error" class="mt-2 text-sm text-brand-700">{{ error }}</p>
    <form class="mt-3 flex gap-2" @submit.prevent="send()">
      <input v-model="input" class="input rounded-full" :placeholder="$t('grow.assistant.placeholder')" :disabled="busy" >
      <button class="btn-primary" :disabled="busy || !input.trim()">{{ $t('grow.assistant.send') }}</button>
    </form>
  </div>
</template>
