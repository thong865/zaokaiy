/** Lets utils (money, fmtDate, ago, tr …) read the active locale in the browser. */
export default defineNuxtPlugin({
  name: 'i18n-bind',
  enforce: 'post',
  setup(nuxtApp) {
    bindI18n(nuxtApp.$i18n as unknown as Parameters<typeof bindI18n>[0])
  },
})
