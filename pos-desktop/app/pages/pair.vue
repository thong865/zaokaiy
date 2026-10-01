<script setup lang="ts">
import { openUrl } from '@tauri-apps/plugin-opener'

const { t, info, refresh, lang, setLang, status } = usePos()
const cfg = useRuntimeConfig()

const form = reactive({
  api: info.value?.api_base || (cfg.public.defaultApiBase as string) || '',
  name: info.value?.device?.name || '',
})
type Step = 'form' | 'code' | 'done'
const step = ref<Step>('form')
const code = ref('')
const approveUrl = ref('')
const expiresAt = ref(0)
const expired = ref(false)
const error = ref('')
const busy = ref(false)
const shopName = ref('')
let timer: ReturnType<typeof setTimeout> | undefined

const webBase = computed(() => approveUrl.value.replace(/\?.*$/, ''))

async function start() {
  error.value = ''
  busy.value = true
  try {
    const r = await call<{ code: string; approve_url: string; expires_at: string; interval: number }>('pair_start', {
      apiBase: form.api,
      webBase: null,
      name: form.name || 'POS',
    })
    code.value = r.code
    approveUrl.value = r.approve_url
    expiresAt.value = new Date(r.expires_at).getTime()
    expired.value = false
    step.value = 'code'
    poll((r.interval || 3) * 1000)
  } catch (e) {
    error.value = (e as Error).message
  } finally {
    busy.value = false
  }
}

function poll(every: number) {
  clearTimeout(timer)
  timer = setTimeout(async () => {
    if (step.value !== 'code') return
    try {
      const r = await call<{ status: string; shop?: { name: string } }>('pair_poll')
      if (r.status === 'approved') {
        shopName.value = r.shop?.name ?? ''
        step.value = 'done'
        await refresh()
        setTimeout(() => navigateTo('/'), 1200)
        return
      }
      if (r.status === 'expired' || Date.now() > expiresAt.value) {
        expired.value = true
        return
      }
      error.value = ''
    } catch (e) {
      error.value = (e as Error).message // e.g. offline: keep trying
    }
    poll(every)
  }, every)
}
onBeforeUnmount(() => clearTimeout(timer))

const canCancel = computed(() => !!status.value?.paired)
</script>

<template>
  <div class="grid h-full place-items-center overflow-auto p-6">
    <div class="w-full max-w-lg">
      <div class="mb-6 flex items-center justify-between">
        <div class="flex items-center gap-3">
          <img src="/favicon.svg" alt="" class="size-10" />
          <span class="text-lg font-extrabold">{{ t('app.name') }}</span>
        </div>
        <div class="flex gap-1">
          <UButton v-if="canCancel" variant="ghost" color="neutral" size="sm" icon="i-lucide-arrow-left" to="/settings">{{ t('pair.back') }}</UButton>
          <UButton variant="ghost" color="neutral" size="sm" @click="setLang(lang === 'en' ? 'lo' : 'en')">{{ lang === 'en' ? 'ລາວ' : 'EN' }}</UButton>
        </div>
      </div>

      <div class="card p-7">
        <template v-if="step === 'form'">
          <h1 class="text-xl font-extrabold">{{ t('pair.title') }}</h1>
          <p class="mt-1 text-sm text-muted">{{ t('pair.lead') }}</p>
          <form class="mt-6 space-y-4" @submit.prevent="start">
            <UFormField :label="t('pair.server')" :help="t('pair.serverHint')">
              <UInput v-model="form.api" size="lg" class="w-full" placeholder="https://your-domain.com/api" autofocus />
            </UFormField>
            <UFormField :label="t('pair.name')">
              <UInput v-model="form.name" size="lg" class="w-full" :placeholder="t('pair.namePh')" maxlength="80" />
            </UFormField>
            <UAlert v-if="error" color="error" variant="soft" :description="error" icon="i-lucide-circle-alert" />
            <UButton type="submit" size="xl" block :loading="busy" :disabled="!form.api" icon="i-lucide-link">{{ t('pair.start') }}</UButton>
          </form>
        </template>

        <template v-else-if="step === 'code'">
          <h1 class="text-xl font-extrabold">{{ t('pair.codeTitle') }}</h1>
          <div class="my-6 rounded-2xl bg-field py-6 text-center font-mono text-5xl font-extrabold tracking-[0.2em] select-all">{{ code }}</div>
          <ol class="space-y-2 text-sm">
            <li class="flex gap-3"><span class="icon-tile size-6 text-xs">1</span><span>{{ t('pair.step1') }}</span></li>
            <li class="flex gap-3"><span class="icon-tile size-6 text-xs">2</span><span>{{ t('pair.step2', { url: webBase }) }}</span></li>
            <li class="flex gap-3"><span class="icon-tile size-6 text-xs">3</span><span>{{ t('pair.step3') }}</span></li>
          </ol>
          <UAlert v-if="error && !expired" class="mt-4" color="warning" variant="soft" :description="error" icon="i-lucide-wifi-off" />
          <div v-if="expired" class="mt-6 space-y-3">
            <UAlert color="warning" variant="soft" :title="t('pair.expired')" icon="i-lucide-timer-off" />
            <UButton block size="lg" icon="i-lucide-rotate-ccw" @click="start">{{ t('pair.restart') }}</UButton>
          </div>
          <div v-else class="mt-6 flex items-center gap-3">
            <UButton size="lg" icon="i-lucide-external-link" @click="openUrl(approveUrl)">{{ t('pair.open') }}</UButton>
            <span class="flex items-center gap-2 text-sm text-muted"><UIcon name="i-lucide-loader-circle" class="size-4 animate-spin" />{{ t('pair.waiting') }}</span>
          </div>
        </template>

        <template v-else>
          <div class="py-8 text-center">
            <UIcon name="i-lucide-circle-check" class="size-14 text-mint-500" />
            <p class="mt-3 text-lg font-bold">{{ t('pair.done', { shop: shopName }) }}</p>
          </div>
        </template>
      </div>
    </div>
  </div>
</template>
