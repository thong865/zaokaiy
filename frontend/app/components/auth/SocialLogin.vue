<script setup lang="ts">
import type { SignInResponse } from '~/utils/types'

/**
 * "Continue with Google / Facebook / WhatsApp" — the methods the server has enabled.
 * In development (`nuxt dev`) methods that aren't configured are shown greyed out with the
 * backend/.env keys that turn them on, so it's obvious why a button is missing in production.
 */
const props = withDefaults(defineProps<{ redirect?: string; askName?: boolean }>(), { redirect: '/dashboard', askName: false })
const { t } = useI18n()
const { startOAuth, continueTo } = useAuth()
const { data: providers } = await useAuthProviders()
const SETUP: Record<'google' | 'facebook' | 'whatsapp', string> = {
  google: 'GOOGLE_CLIENT_ID + GOOGLE_CLIENT_SECRET',
  facebook: 'FACEBOOK_APP_ID + FACEBOOK_APP_SECRET',
  whatsapp: 'WHATSAPP_TOKEN + WHATSAPP_PHONE_NUMBER_ID (or OTP_DEV_ECHO=true for local testing)',
}
const showAll = import.meta.dev
const shown = computed(() => (['google', 'facebook', 'whatsapp'] as const).filter((p) => providers.value[p] || showAll))
const any = computed(() => shown.value.length > 0)
const missing = computed(() => shown.value.filter((p) => !providers.value[p]))
const whatsapp = ref(false)
const busy = ref<string | null>(null)
const error = ref('')

async function go(p: 'google' | 'facebook') {
  error.value = ''
  busy.value = p
  try {
    await startOAuth(p, { redirect: props.redirect })
  } catch (e) {
    error.value = apiError(e)
    busy.value = null
  }
}
async function done(r: SignInResponse) {
  await continueTo(r, props.redirect)
}
</script>

<template>
  <div v-if="any" class="flex space-y-3">
    <template v-if="!whatsapp">
      <button v-if="shown.includes('google')" type="button" class="btn-social" :disabled="!!busy || !providers.google" @click="go('google')">
        <span class="badge-social bg-[#4285F4]" aria-hidden="true">
          <UIcon name="logos:google-icon" class="size-5" />
        </span>
      </button>
      <button v-if="shown.includes('facebook')" type="button" class="btn-social" :disabled="!!busy || !providers.facebook" @click="go('facebook')">
        <span class="badge-social bg-[#1877F2]" aria-hidden="true">f</span>
      </button>
      <button v-if="shown.includes('whatsapp')" type="button" class="btn-social" :disabled="!!busy || !providers.whatsapp" @click="whatsapp = true">
        <span class="badge-social bg-[#25D366]" aria-hidden="true">
          <UIcon name="logos:whatsapp-icon" class="size-5" />
        </span>
      </button>
      <p v-if="error" class="text-sm text-brand-700">{{ error }}</p>
      <div v-if="missing.length" class="rounded-xl border border-dashed border-line bg-paper p-3 text-xs text-muted">
        <b class="text-ink">Dev mode:</b> greyed-out buttons are hidden in production until configured in <code>backend/.env</code>:
        <ul class="mt-1 list-disc pl-4">
          <li v-for="p in missing" :key="p"><span class="capitalize">{{ p }}</span>: <code>{{ SETUP[p] }}</code></li>
        </ul>
      </div>
    </template>
    <AuthWhatsAppLogin v-else :ask-name="askName" @done="done" @cancel="whatsapp = false" />
    <div class="flex items-center gap-3 pt-1 text-xs font-semibold uppercase tracking-wide text-muted">
      <span class="h-px flex-1 bg-line" />{{ t('auth.social.or') }}<span class="h-px flex-1 bg-line" />
    </div>
  </div>
</template>

<style scoped>
.btn-social {
  display: flex;
  width: 100%;
  align-items: center;
  justify-content: center;
  gap: 0.6rem;
  border-radius: 12px;
  border: 0;
  background: var(--field);
  color: var(--ink);
  padding: 0.7rem 1rem;
  font-weight: 700;
  font-size: 0.925rem;
  transition: background 0.2s, transform 0.2s, box-shadow 0.2s;
}
.btn-social:hover:not(:disabled) {
  background: var(--surface);
  transform: translateY(-2px);
  box-shadow: 0 10px 24px -12px rgb(0 0 0 / 0.25);
}
.btn-social:disabled {
  opacity: 0.6;
}
.badge-social {
  display: grid;
  place-items: center;
  width: 1.4rem;
  height: 1.4rem;
  border-radius: 9999px;
  color: #fff;
  font-size: 0.8rem;
  font-weight: 800;
  line-height: 1;
}
</style>
