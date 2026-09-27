<script setup lang="ts">
/** EN / ລາວ toggle. `dark` for dark headers (admin). Choice is kept in the zk_lang cookie. */
withDefaults(defineProps<{ dark?: boolean }>(), { dark: false })
const { locale, locales, setLocale } = useI18n()
</script>

<template>
  <div class="inline-flex shrink-0 rounded-full border p-0.5 text-xs font-bold" :class="dark ? 'border-white/20' : 'border-line bg-white'" role="group" :aria-label="$t('common.language')">
    <button
      v-for="l in locales"
      :key="l.code"
      type="button"
      class="rounded-full px-2.5 py-1 transition"
      :class="locale === l.code ? (dark ? 'bg-white text-ink' : 'bg-ink text-white') : dark ? 'text-white/70 hover:text-white' : 'text-muted hover:text-ink'"
      :lang="l.code"
      :aria-pressed="locale === l.code"
      @click="setLocale(l.code)"
    >
      {{ l.code === 'lo' ? 'ລາວ' : 'EN' }}
    </button>
  </div>
</template>
