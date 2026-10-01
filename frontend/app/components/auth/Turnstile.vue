<script setup lang="ts">
/**
 * Cloudflare Turnstile human check. `v-model` is the token to send as `captcha` ('' until solved).
 * Most visitors never see it (interaction-only); it shows a checkbox only when Cloudflare is unsure.
 * Renders nothing when the API has no Turnstile keys — `required` is then false and forms work as before.
 * Tokens are single-use: call `reset()` after each request that used one.
 */
const props = withDefaults(defineProps<{ action?: string }>(), { action: 'auth' })
const token = defineModel<string>({ default: '' })
// Expose before the first `await`: expose() after an await in async setup is lost (parent ref gets no reset/pending).
let pendingC: { readonly value: boolean } | undefined
defineExpose({ reset, get pending() { return pendingC?.value ?? false } })
const { data: providers } = await useAuthProviders()
const siteKey = computed(() => providers.value.captcha_site_key)
const { locale } = useI18n()

const el = ref<HTMLElement | null>(null)
const failed = ref(false)
let widgetId: string | undefined

interface TurnstileApi {
  render: (el: HTMLElement, opts: Record<string, unknown>) => string
  reset: (id?: string) => void
  remove: (id?: string) => void
}
declare global {
  interface Window { turnstile?: TurnstileApi; __zkTurnstile?: Promise<TurnstileApi> }
}

function loadScript(): Promise<TurnstileApi> {
  if (window.turnstile) return Promise.resolve(window.turnstile)
  window.__zkTurnstile ??= new Promise((resolve, reject) => {
    const s = document.createElement('script')
    s.src = 'https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit'
    s.async = true
    s.onload = () => (window.turnstile ? resolve(window.turnstile) : reject(new Error('turnstile')))
    s.onerror = () => {
      window.__zkTurnstile = undefined
      reject(new Error('turnstile'))
    }
    document.head.appendChild(s)
  })
  return window.__zkTurnstile
}

onMounted(async () => {
  if (!siteKey.value || !el.value) return
  try {
    const ts = await loadScript()
    if (!el.value) return
    widgetId = ts.render(el.value, {
      sitekey: siteKey.value,
      action: props.action,
      appearance: 'interaction-only',
      theme: 'auto',
      language: locale.value === 'lo' ? 'auto' : locale.value,
      callback: (t: string) => {
        failed.value = false
        token.value = t
      },
      'expired-callback': () => (token.value = ''),
      'error-callback': () => {
        token.value = ''
        failed.value = true
      },
    })
  } catch {
    failed.value = true
  }
})
onBeforeUnmount(() => {
  if (widgetId) window.turnstile?.remove(widgetId)
})

function reset() {
  token.value = ''
  if (widgetId) window.turnstile?.reset(widgetId)
}
/** `pending`: true while a token is needed and not there yet (disable the submit button). */
const pending = computed(() => !!siteKey.value && !token.value)
pendingC = pending
</script>

<template>
  <div v-if="siteKey" class="space-y-1">
    <div ref="el" class="flex justify-center empty:hidden" />
    <p v-if="pending && !failed" class="text-center text-xs text-muted">{{ $t('auth.captcha.checking') }}</p>
    <p v-if="failed" class="text-center text-xs text-brand-700">{{ $t('auth.captcha.failed') }}</p>
  </div>
</template>
