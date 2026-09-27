<script setup lang="ts">
import type { SocialProvider } from '~/utils/types'

definePageMeta({ layout: 'dashboard', middleware: 'seller' })
const api = useApi()
const config = useRuntimeConfig()
const { t } = useI18n()
const { shopId } = useShop()
interface Ch { id: string; provider: SocialProvider; name: string; external_id: string; secret: string; active: boolean; auto_reply: boolean; has_token: boolean; last_event_at: string | null; created_at: string }
interface Settings { meta_webhook_configured: boolean; meta_signature_check: boolean; tiktok_signature_check: boolean }
const { data: channels, refresh } = await useAsyncData('social-channels', () => api<Ch[]>(`/shops/${shopId.value}/social/channels`), { watch: [shopId], default: () => [] })
const { data: settings } = await useAsyncData('social-settings-flags', () => api<Settings>(`/shops/${shopId.value}/social/settings`), { watch: [shopId] })

const apiBase = computed(() => (import.meta.client ? new URL(config.public.apiBase, location.origin).href : config.public.apiBase).replace(/\/$/, ''))
const hooks = computed(() => ({ meta: `${apiBase.value}/webhooks/meta`, tiktok: `${apiBase.value}/webhooks/tiktok` }))

const guideSteps = (p: SocialProvider, n: number) => Array.from({ length: n }, (_, i) => t(`social.channels.guides.${p}.s${i + 1}`))
const guides = computed<Record<SocialProvider, { title: string; idLabel: string; idHelp: string; tokenHelp: string; steps: string[] }>>(() => ({
  facebook: {
    title: t('social.channels.guides.facebook.title'),
    idLabel: t('social.channels.guides.facebook.idLabel'),
    idHelp: t('social.channels.guides.facebook.idHelp'),
    tokenHelp: t('social.channels.guides.facebook.tokenHelp'),
    steps: guideSteps('facebook', 4),
  },
  whatsapp: {
    title: t('social.channels.guides.whatsapp.title'),
    idLabel: t('social.channels.guides.whatsapp.idLabel'),
    idHelp: t('social.channels.guides.whatsapp.idHelp'),
    tokenHelp: t('social.channels.guides.whatsapp.tokenHelp'),
    steps: guideSteps('whatsapp', 4),
  },
  tiktok: {
    title: t('social.channels.guides.tiktok.title'),
    idLabel: t('social.channels.guides.tiktok.idLabel'),
    idHelp: t('social.channels.guides.tiktok.idHelp'),
    tokenHelp: t('social.channels.guides.tiktok.tokenHelp'),
    steps: guideSteps('tiktok', 3),
  },
  webhook: {
    title: t('social.channels.guides.webhook.title'),
    idLabel: '',
    idHelp: '',
    tokenHelp: '',
    steps: guideSteps('webhook', 2),
  },
}))
const adding = ref<SocialProvider | null>(null)
const form = reactive({ name: '', external_id: '', access_token: '' })
const error = ref('')
async function add() {
  error.value = ''
  try {
    await api(`/shops/${shopId.value}/social/channels`, { method: 'POST', body: { provider: adding.value, ...form } })
    adding.value = null
    Object.assign(form, { name: '', external_id: '', access_token: '' })
    await refresh()
  } catch (e) {
    error.value = apiError(e)
  }
}
async function patch(c: Ch, body: Record<string, unknown>) {
  error.value = ''
  try {
    await api(`/social/channels/${c.id}`, { method: 'PATCH', body })
    await refresh()
  } catch (e) {
    error.value = apiError(e)
  }
}
async function remove(c: Ch) {
  if (!window.confirm(t('social.channels.confirmDisconnect', { name: c.name }))) return
  await api(`/social/channels/${c.id}`, { method: 'DELETE' })
  await refresh()
}
const tokenEdit = reactive<Record<string, string>>({})
const copiedKey = ref('')
async function copy(text: string, key: string) {
  await navigator.clipboard?.writeText(text)
  copiedKey.value = key
  setTimeout(() => (copiedKey.value = ''), 1500)
}
const curl = (c: Ch) => `BODY='{"id":"msg-123","user_id":"line:U123","user_name":"Nok","message":"CF A01 x2"}'
SIG=$(printf '%s' "$BODY" | openssl dgst -sha256 -hmac '${c.secret}' | sed 's/^.* //')
curl -X POST ${apiBase.value}/webhooks/ingest/${c.id} \\
  -H "Content-Type: application/json" -H "X-Zaokaiy-Signature: sha256=$SIG" -d "$BODY"`
</script>

<template>
  <div>
    <h1 class="page-title">{{ $t('social.title') }}</h1>
    <SocialNav class="mt-5" />

    <div v-if="settings && (!settings.meta_webhook_configured || !settings.meta_signature_check)" class="mb-5 rounded-2xl border border-sun-400/60 bg-sun-400/10 px-4 py-3 text-sm">
      <b>{{ $t('social.channels.serverSetup') }}</b>
      <span v-if="!settings.meta_webhook_configured"> <i18n-t keypath="social.channels.needVerifyToken" tag="span"><template #env><code class="rounded bg-white px-1">META_VERIFY_TOKEN</code></template></i18n-t></span>
      <span v-if="!settings.meta_signature_check"> <i18n-t keypath="social.channels.needAppSecret" tag="span"><template #env><code class="rounded bg-white px-1">META_APP_SECRET</code></template></i18n-t></span>
      <span> {{ $t('social.channels.needHttps') }}</span>
    </div>
    <p v-if="error" class="mb-3 text-sm text-brand-700">{{ error }}</p>

    <div v-if="channels.length" class="space-y-3">
      <div v-for="c in channels" :key="c.id" class="card p-5">
        <div class="flex flex-wrap items-center gap-3">
          <SocialProviderBadge :provider="c.provider" size="md" />
          <div>
            <div class="font-bold">{{ c.name }}</div>
            <div class="text-xs text-muted">{{ guides[c.provider].title }}<template v-if="c.external_id"> · {{ c.external_id }}</template> · {{ c.last_event_at ? $t('social.channels.lastEvent', { ago: ago(c.last_event_at) }) : $t('social.channels.noEvents') }}</div>
          </div>
          <div class="ml-auto flex flex-wrap items-center gap-3 text-sm">
            <label v-if="c.provider !== 'tiktok' && c.provider !== 'webhook'" class="flex items-center gap-1.5"><input type="checkbox" class="accent-brand-500" :checked="c.auto_reply" @change="patch(c, { auto_reply: ($event.target as HTMLInputElement).checked })"> {{ $t('social.channels.autoReply') }}</label>
            <label class="flex items-center gap-1.5"><input type="checkbox" class="accent-brand-500" :checked="c.active" @change="patch(c, { active: ($event.target as HTMLInputElement).checked })"> {{ $t('social.channels.active') }}</label>
            <button class="btn-ghost btn-sm text-brand-700" @click="remove(c)">{{ $t('social.channels.disconnect') }}</button>
          </div>
        </div>
        <div class="mt-4 grid gap-3 text-sm md:grid-cols-2">
          <div v-if="c.provider === 'facebook' || c.provider === 'whatsapp'">
            <label class="label">{{ $t('social.channels.accessToken') }}</label>
            <div class="flex gap-2">
              <input v-model="tokenEdit[c.id]" type="password" class="input py-2" :placeholder="c.has_token ? $t('social.channels.tokenSaved') : $t('social.channels.pasteToken')" >
              <button class="btn-ghost btn-sm" :disabled="!tokenEdit[c.id]" @click="patch(c, { access_token: tokenEdit[c.id] }); tokenEdit[c.id] = ''">{{ $t('common.save') }}</button>
            </div>
          </div>
          <div v-if="c.provider !== 'webhook'">
            <label class="label">{{ $t('social.channels.callbackUrl') }}</label>
            <div class="flex gap-2"><input :value="c.provider === 'tiktok' ? hooks.tiktok : hooks.meta" readonly class="input bg-paper py-2 font-mono text-xs" ><button class="btn-ghost btn-sm" @click="copy(c.provider === 'tiktok' ? hooks.tiktok : hooks.meta, c.id + 'u')">{{ copiedKey === c.id + 'u' ? '✓' : $t('common.copy') }}</button></div>
          </div>
          <template v-if="c.provider === 'webhook' || c.provider === 'tiktok'">
            <div>
              <label class="label">{{ $t('social.channels.ingestUrl') }}</label>
              <div class="flex gap-2"><input :value="`${apiBase}/webhooks/ingest/${c.id}`" readonly class="input bg-paper py-2 font-mono text-xs" ><button class="btn-ghost btn-sm" @click="copy(`${apiBase}/webhooks/ingest/${c.id}`, c.id + 'i')">{{ copiedKey === c.id + 'i' ? '✓' : $t('common.copy') }}</button></div>
            </div>
            <div>
              <label class="label">{{ $t('social.channels.signingSecret') }}</label>
              <div class="flex gap-2"><input :value="c.secret" readonly class="input bg-paper py-2 font-mono text-xs" ><button class="btn-ghost btn-sm" @click="copy(c.secret, c.id + 's')">{{ copiedKey === c.id + 's' ? '✓' : $t('common.copy') }}</button><button class="btn-ghost btn-sm" @click="patch(c, { rotate_secret: true })">{{ $t('social.channels.rotate') }}</button></div>
            </div>
            <details class="md:col-span-2">
              <summary class="cursor-pointer text-xs font-semibold text-muted">{{ $t('social.channels.curlExample') }}</summary>
              <pre class="mt-2 overflow-x-auto rounded-xl bg-ink p-3 text-[11px] text-white">{{ curl(c) }}</pre>
            </details>
          </template>
        </div>
      </div>
    </div>

    <h2 class="mb-3 mt-8 font-bold">{{ $t('social.channels.connectChannel') }}</h2>
    <div class="grid gap-3 md:grid-cols-2">
      <div v-for="(g, p) in guides" :key="p" class="card p-5" :class="adding === p && 'border-ink'">
        <div class="flex items-center gap-3">
          <SocialProviderBadge :provider="p" size="md" />
          <div class="font-bold">{{ g.title }}</div>
          <button v-if="adding !== p" class="btn-ghost btn-sm ml-auto" @click="adding = p">{{ $t('social.channels.connect') }}</button>
        </div>
        <ol class="mt-3 list-decimal space-y-1 pl-5 text-xs text-muted">
          <li v-for="(s, i) in g.steps" :key="i">{{ s }}</li>
        </ol>
        <div v-if="p !== 'webhook'" class="mt-3 flex gap-2 text-xs">
          <input :value="p === 'tiktok' ? hooks.tiktok : hooks.meta" readonly class="input bg-paper py-1.5 font-mono text-[11px]" >
          <button class="btn-ghost btn-sm" @click="copy(p === 'tiktok' ? hooks.tiktok : hooks.meta, 'g' + p)">{{ copiedKey === 'g' + p ? '✓' : $t('common.copy') }}</button>
        </div>
        <form v-if="adding === p" class="mt-4 space-y-2 border-t border-line pt-4" @submit.prevent="add">
          <input v-model="form.name" class="input py-2" :placeholder="$t('social.channels.namePlaceholder', { example: $t(`social.channels.nameExample.${p === 'whatsapp' ? 'whatsapp' : p === 'webhook' ? 'webhook' : 'page'}`) })" >
          <template v-if="g.idLabel">
            <input v-model="form.external_id" required class="input py-2 font-mono" :placeholder="g.idLabel" >
            <p class="text-[11px] text-muted">{{ g.idHelp }}</p>
          </template>
          <template v-if="p === 'facebook' || p === 'whatsapp'">
            <input v-model="form.access_token" type="password" class="input py-2" :placeholder="$t('social.channels.accessToken')" >
            <p class="text-[11px] text-muted">{{ g.tokenHelp }}. {{ $t('social.channels.tokenStored') }}</p>
          </template>
          <div class="flex justify-end gap-2">
            <button type="button" class="btn-ghost btn-sm" @click="adding = null">{{ $t('common.cancel') }}</button>
            <button class="btn-primary btn-sm">{{ $t('social.channels.connect') }}</button>
          </div>
        </form>
      </div>
    </div>
  </div>
</template>
