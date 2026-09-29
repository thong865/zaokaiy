<script setup lang="ts">
/** EN / ລາວ toggle. `dark` for dark headers (admin). Choice is kept in the zk_lang cookie. */
withDefaults(defineProps<{ dark?: boolean }>(), { dark: false })
const { locale, locales, setLocale } = useI18n()
</script>

<template>
  <div class="inline-flex shrink-0 rounded-xl p-1 text-xs font-bold" :class="dark ? 'bg-white/10' : 'bg-field'" role="group" :aria-label="$t('common.language')">
    <button
      v-for="l in locales"
      :key="l.code"
      type="button"
      class="rounded-lg px-2.5 py-1 transition-all duration-200"
      :class="locale === l.code ? (dark ? 'bg-white text-night' : 'bg-surface text-ink shadow-[0_2px_8px_rgb(0_0_0/0.08)]') : dark ? 'text-white/70 hover:text-white' : 'text-muted hover:text-ink'"
      :lang="l.code"
      :aria-pressed="locale === l.code"
      @click="setLocale(l.code)"
    >
      {{ l.code === 'lo' ? 'ລາວ' : 'EN' }}
    </button>
  </div>
</template>
