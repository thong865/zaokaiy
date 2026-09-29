<script setup lang="ts">
import type { SignInResponse } from '~/utils/types'

/**
 * Two steps: phone number → 6-digit code sent on WhatsApp. Emits `done` with the session.
 * `link` connects the number to the signed-in account; `askName` shows a name field (sign-up).
 */
const props = withDefaults(defineProps<{ link?: boolean; askName?: boolean }>(), { link: false, askName: false })
const emit = defineEmits<{ done: [SignInResponse]; cancel: [] }>()
const { t, locale } = useI18n()
const { whatsappSend, whatsappVerify } = useAuth()

const COUNTRIES = [
  ['+856', 'LA'], ['+66', 'TH'], ['+84', 'VN'], ['+855', 'KH'], ['+95', 'MM'], ['+86', 'CN'],
  ['+65', 'SG'], ['+60', 'MY'], ['+81', 'JP'], ['+82', 'KR'], ['+1', 'US'], ['+44', 'GB'], ['+61', 'AU'],
] as const
const cc = ref<string>('+856')
const national = ref('')
const code = ref('')
const name = ref('')
const step = ref<'phone' | 'code'>('phone')
const sentTo = ref('')
const devCode = ref('')
const busy = ref(false)
const error = ref('')
const wait = ref(0)
const captcha = ref('')
const human = ref<{ reset: () => void; pending: boolean } | null>(null)
let timer: ReturnType<typeof setInterval> | undefined

/** "+856" + "020 5555 1234" → "+8562055551234" (drops the trunk 0). */
const phone = computed(() => cc.value + national.value.replace(/\D/g, '').replace(/^0+/, ''))

function countdown(secs: number) {
  wait.value = secs
  clearInterval(timer)
  timer = setInterval(() => {
    wait.value = Math.max(0, wait.value - 1)
    if (!wait.value) clearInterval(timer)
  }, 1000)
}
onBeforeUnmount(() => clearInterval(timer))

async function send() {
  error.value = ''
  busy.value = true
  try {
    const r = await whatsappSend(phone.value, locale.value, captcha.value || undefined)
    sentTo.value = r.phone
    devCode.value = r.dev_code || ''
    step.value = 'code'
    code.value = ''
    countdown(r.resend_in)
  } catch (e) {
    error.value = apiError(e)
  } finally {
    busy.value = false
    human.value?.reset() // single-use: fetch a fresh token for "resend"
  }
}

async function verify() {
  error.value = ''
  busy.value = true
  try {
    emit('done', await whatsappVerify(sentTo.value, code.value, { display_name: name.value, link: props.link }))
  } catch (e) {
    error.value = apiError(e)
  } finally {
    busy.value = false
  }
}

// Submit automatically once 6 digits are in (typed or pasted from the WhatsApp copy button).
watch(code, (v) => {
  const d = v.replace(/\D/g, '').slice(0, 6)
  if (d !== v) code.value = d
  else if (d.length === 6 && !busy.value) verify()
})
</script>

<template>
  <div class="space-y-3">
    <form v-if="step === 'phone'" class="space-y-3" @submit.prevent="send">
      <label class="label" for="wa-phone">{{ t('auth.whatsapp.phone') }}</label>
      <div class="flex gap-2">
        <select v-model="cc" class="input w-28 shrink-0" :aria-label="t('auth.whatsapp.country')">
          <option v-for="[c, iso] in COUNTRIES" :key="c" :value="c">{{ iso }} {{ c }}</option>
        </select>
        <UInput id="wa-phone" v-model="national" inputmode="tel" autocomplete="tel-national" :placeholder="cc === '+856' ? '20 5555 1234' : t('auth.whatsapp.phonePlaceholder')" required />
      </div>
      <UInput v-if="askName" v-model="name" :placeholder="t('auth.whatsapp.namePlaceholder')" autocomplete="name" />
      <p class="text-xs text-muted">{{ t('auth.whatsapp.hint') }}</p>
      <p v-if="error" class="text-sm text-brand-700">{{ error }}</p>
      <div class="flex gap-2">
        <UButton color="neutral" variant="soft" type="button" @click="emit('cancel')">{{ t('common.back') }}</UButton>
        <UButton type="submit" class="flex-1" :disabled="busy || human?.pending || national.replace(/\D/g, '').length < 6">{{ busy ? t('auth.whatsapp.sending') : t('auth.whatsapp.send') }}</UButton>
      </div>
    </form>

    <form v-else class="space-y-3" @submit.prevent="verify">
      <p class="text-sm">{{ t('auth.whatsapp.sentTo', { phone: sentTo }) }}</p>
      <UInput v-model="code" class="text-center font-mono text-2xl tracking-[0.5em]" inputmode="numeric" autocomplete="one-time-code" maxlength="6" :aria-label="t('auth.whatsapp.code')" placeholder="••••••" autofocus required />
      <p v-if="devCode" class="rounded-lg bg-sun-400/15 px-3 py-2 text-xs text-amber-800">{{ t('auth.whatsapp.devCode', { code: devCode }) }}</p>
      <p v-if="error" class="text-sm text-brand-700">{{ error }}</p>
      <UButton type="submit" class="w-full" :disabled="busy || code.length !== 6">{{ busy ? t('auth.whatsapp.verifying') : t('auth.whatsapp.verify') }}</UButton>
      <div class="flex justify-between text-xs">
        <button type="button" class="text-muted underline hover:text-ink" @click="step = 'phone'">{{ t('auth.whatsapp.changeNumber') }}</button>
        <button type="button" class="font-semibold underline disabled:text-muted disabled:no-underline" :disabled="wait > 0 || busy || human?.pending" @click="send">
          {{ wait > 0 ? t('auth.whatsapp.resendIn', { s: wait }) : t('auth.whatsapp.resend') }}
        </button>
      </div>
    </form>
    <!-- Shared by "send" and "resend"; kept outside the steps so it survives the step switch. -->
    <AuthTurnstile ref="human" v-model="captcha" action="whatsapp" />
  </div>
</template>
