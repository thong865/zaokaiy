import { readdirSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

/** Every JSON file in i18n/locales/<code>/ is one namespace (the file holds a single top-level key = file name). */
const localeFiles = (code: string) =>
  readdirSync(fileURLToPath(new URL(`./i18n/locales/${code}`, import.meta.url)))
    .filter((f) => f.endsWith('.json'))
    .sort()
    .map((f) => `${code}/${f}`)

export default defineNuxtConfig({
  compatibilityDate: '2026-09-01',
  devtools: { enabled: true },
  modules: ['@nuxt/ui', '@nuxtjs/i18n'],
  // Nuxt UI (components, colour mode, icons). Theme: app.config.ts (Vuesax-style) + assets/css/main.css tokens.
  ui: {
    fonts: false, // Plus Jakarta Sans / Noto Sans Lao are loaded in app.head
    theme: { colors: ['primary', 'secondary', 'success', 'info', 'warning', 'error'] },
  },
  colorMode: { preference: 'system', fallback: 'light', storage: 'cookie', storageKey: 'zk_theme' },
  icon: { serverBundle: { collections: ['lucide'] }, clientBundle: { scan: true, sizeLimitKb: 512 } },
  i18n: {
    strategy: 'no_prefix',
    // Public site URL for SEO link tags — override with NUXT_PUBLIC_I18N_BASE_URL.
    baseUrl: 'http://localhost:3000',
    defaultLocale: 'en',
    locales: [
      { code: 'en', language: 'en-US', name: 'English', files: localeFiles('en') },
      { code: 'lo', language: 'lo-LA', name: 'ລາວ', files: localeFiles('lo') },
    ],
    detectBrowserLanguage: { useCookie: true, cookieKey: 'zk_lang', redirectOn: 'root', fallbackLocale: 'en' },
  },
  css: ['~/assets/css/main.css'],
  runtimeConfig: {
    // Override at runtime with env vars NUXT_API_BASE_SERVER / NUXT_PUBLIC_API_BASE.
    // Server-side (SSR) base URL for the Rust API — inside Docker this is the service name.
    // Empty = same as public apiBase.
    apiBaseServer: '',
    public: {
      apiBase: 'http://localhost:8080/api',
      appName: 'zaokaiy',
    },
  },
  app: {
    pageTransition: { name: 'page', mode: 'out-in' },
    head: {
      htmlAttrs: { lang: 'en' },
      title: 'zaokaiy — sell, resell, create',
      meta: [
        { name: 'viewport', content: 'width=device-width, initial-scale=1' },
        { name: 'description', content: 'Open your shop, let creators sell for you, and grow with AI.' },
      ],
      link: [
        { rel: 'icon', type: 'image/svg+xml', href: '/favicon.svg' },
        { rel: 'preconnect', href: 'https://fonts.googleapis.com' },
        { rel: 'preconnect', href: 'https://fonts.gstatic.com', crossorigin: '' },
        {
          rel: 'stylesheet',
          href: 'https://fonts.googleapis.com/css2?family=Plus+Jakarta+Sans:wght@400;500;600;700;800&family=Noto+Sans+Lao:wght@400;500;600;700&family=Noto+Sans+Thai:wght@400;500;600;700&display=swap',
        },
      ],
    },
  },
})
