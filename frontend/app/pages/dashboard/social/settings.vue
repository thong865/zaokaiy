<script setup lang="ts">
definePageMeta({ layout: 'dashboard', middleware: 'seller' })
const api = useApi()
const { t } = useI18n()
const { shopId, shop } = useShop()
interface S { triggers: string[]; require_trigger: boolean; hold_hours: number; max_qty: number; shipping_cents: number; reply_template: string; soldout_template: string; payment_instructions: string }
const f = reactive({ triggers: [] as string[], require_trigger: false, hold_hours: 24, max_qty: 10, shipping: 0, reply_template: '', soldout_template: '', payment_instructions: '' })
const { data } = await useAsyncData('social-settings', () => api<S>(`/shops/${shopId.value}/social/settings`), { watch: [shopId] })
watch(data, (d) => d && Object.assign(f, { ...d, shipping: d.shipping_cents / 100 }), { immediate: true })
const newTrigger = ref('')
function addTrigger() {
  const w = newTrigger.value.trim().toLowerCase()
  if (w && !f.triggers.includes(w)) f.triggers.push(w)
  newTrigger.value = ''
}
const msg = ref('')
const error = ref('')
async function save() {
  msg.value = error.value = ''
  try {
    const { shipping, ...rest } = f
    await api(`/shops/${shopId.value}/social/settings`, { method: 'PATCH', body: { ...rest, shipping_cents: Math.round(Number(shipping) * 100) } })
    msg.value = t('common.saved')
  } catch (e) {
    error.value = apiError(e)
  }
}
const cur = computed(() => shop.value?.currency ?? 'THB')
const fill = (tpl: string) =>
  tpl.replaceAll('{name}', t('social.settings.sampleName')).replaceAll('{items}', t('social.settings.sampleItems')).replaceAll('{total}', money(64800, cur.value))
    .replaceAll('{link}', 'https://yourshop/c/…').replaceAll('{hours}', String(f.hold_hours)).replaceAll('{order}', 'SO000123')
const examples = ['CF A01', 'cf a01 x2', 'A01 2', 'CFA01', 'F A01=3', 'ເອົາ A01 2', 'ຈອງ B03', 'ສັ່ງ A01 x2', 'เอา A01 2 ชิ้น', 'CF A01 x2, B03']
// Reply template presets — this is the text customers receive, so it is data (not UI translation).
const PRESETS = {
  lo: {
    reply: 'ຮັບອໍເດີແລ້ວ {name} ✅ {items} ລວມ {total} ກະລຸນາຢືນຢັນ ແລະ ຊຳລະເງິນທີ່ {link} ພາຍໃນ {hours} ຊົ່ວໂມງ',
    soldout: 'ຂໍອະໄພ {name} {items} ໝົດແລ້ວ 🙏',
  },
  th: {
    reply: 'รับออเดอร์แล้วค่ะ {name} ✅ {items} รวม {total} กรุณายืนยันและชำระเงินที่ {link} ภายใน {hours} ชม.',
    soldout: 'ขออภัยค่ะ {name} {items} หมดแล้ว 🙏',
  },
  en: {
    reply: 'Order received, {name} ✅ {items} — total {total}. Please confirm and pay at {link} within {hours} hours.',
    soldout: 'Sorry {name}, {items} is sold out 🙏',
  },
} as const
function applyPreset(lang: keyof typeof PRESETS) {
  f.reply_template = PRESETS[lang].reply
  f.soldout_template = PRESETS[lang].soldout
}
</script>

<template>
  <div class="max-w-3xl">
    <h1 class="page-title">{{ $t('social.title') }}</h1>
    <SocialNav class="mt-5" />
    <form class="space-y-6" @submit.prevent="save">
      <div class="card space-y-4 p-6">
        <div class="font-bold">{{ $t('social.settings.howRead') }}</div>
        <div>
          <label class="label">{{ $t('social.settings.triggerWords') }}</label>
          <div class="flex flex-wrap items-center gap-2">
            <span v-for="(t, i) in f.triggers" :key="t" class="chip border border-line bg-paper px-2.5 py-1 font-mono text-xs">{{ t }} <button type="button" class="ml-1 text-muted hover:text-brand-700" :aria-label="$t('social.settings.removeWord', { word: t })" @click="f.triggers.splice(i, 1)">✕</button></span>
            <input v-model="newTrigger" class="input w-36 py-1.5" :placeholder="$t('social.settings.addWord')" @keydown.enter.prevent="addTrigger" >
          </div>
          <p class="mt-2 text-xs text-muted">{{ $t('social.settings.understood') }} <span v-for="e in examples" :key="e" class="mr-1 inline-block rounded bg-paper px-1.5 font-mono">{{ e }}</span></p>
        </div>
        <label class="flex items-start gap-2 text-sm">
          <input v-model="f.require_trigger" type="checkbox" class="mt-0.5 size-4 accent-brand-500">
          <span>{{ $t('social.settings.requireTrigger') }} <span class="block text-xs text-muted">{{ $t('social.settings.requireTriggerHint', { example: 'A01 2', question: $t('social.settings.questionExample') }) }}</span></span>
        </label>
        <div class="grid gap-3 sm:grid-cols-3">
          <div><label class="label">{{ $t('social.settings.holdHours') }}</label><input v-model.number="f.hold_hours" type="number" min="1" max="336" class="input" ></div>
          <div><label class="label">{{ $t('social.settings.maxQty') }}</label><input v-model.number="f.max_qty" type="number" min="1" max="999" class="input" ></div>
          <div><label class="label">{{ $t('social.settings.shippingFee', { cur }) }}</label><input v-model="f.shipping" type="number" min="0" step="0.01" class="input" ></div>
        </div>
      </div>

      <div class="card space-y-4 p-6">
        <div class="font-bold">{{ $t('social.settings.autoReplies') }}</div>
        <p class="text-xs text-muted">{{ $t('social.settings.placeholders') }} <code>{name}</code> <code>{items}</code> <code>{total}</code> <code>{link}</code> <code>{hours}</code> <code>{order}</code></p>
        <div class="flex flex-wrap items-center gap-2 text-xs">
          <span class="text-muted">{{ $t('social.settings.presets') }}</span>
          <button type="button" class="btn-ghost btn-sm" @click="applyPreset('lo')">{{ $t('social.settings.useLao') }}</button>
          <button type="button" class="btn-ghost btn-sm" @click="applyPreset('th')">{{ $t('social.settings.useThai') }}</button>
          <button type="button" class="btn-ghost btn-sm" @click="applyPreset('en')">{{ $t('social.settings.useEnglish') }}</button>
        </div>
        <div>
          <label class="label">{{ $t('social.settings.whenReserved') }}</label>
          <textarea v-model="f.reply_template" rows="3" class="input" />
          <div class="mt-1 rounded-xl bg-paper p-2 text-xs"><b>{{ $t('social.settings.preview') }}</b> {{ fill(f.reply_template) }}</div>
        </div>
        <div>
          <label class="label">{{ $t('social.settings.whenSoldOut') }}</label>
          <textarea v-model="f.soldout_template" rows="2" class="input" />
          <div class="mt-1 rounded-xl bg-paper p-2 text-xs"><b>{{ $t('social.settings.preview') }}</b> {{ fill(f.soldout_template) }}</div>
        </div>
        <div>
          <label class="label">{{ $t('social.settings.paymentInstructions') }}</label>
          <textarea v-model="f.payment_instructions" rows="3" class="input" :placeholder="$t('social.settings.paymentPlaceholder')" />
        </div>
      </div>
      <p v-if="msg" class="text-sm text-mint-500">{{ msg }}</p>
      <p v-if="error" class="text-sm text-brand-700">{{ error }}</p>
      <button class="btn-primary">{{ $t('social.settings.save') }}</button>
    </form>
  </div>
</template>
