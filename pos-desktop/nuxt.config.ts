/**
 * zaokaiy POS desktop — the UI of the Tauri app (single-page, no server).
 * `npm run generate` builds static files into .output/public, which Tauri bundles.
 * Everything (fonts, icons) is bundled so the till looks right with no internet.
 */
export default defineNuxtConfig({
  compatibilityDate: '2026-09-01',
  ssr: false,
  devtools: { enabled: false },
  modules: ['@nuxt/ui'],
  ui: {
    fonts: false,
    theme: { colors: ['primary', 'secondary', 'success', 'info', 'warning', 'error'] },
  },
  colorMode: { preference: 'light', fallback: 'light', storageKey: 'zkpos_theme' },
  // Icons are compiled into the bundle (no Iconify API calls when offline).
  icon: { provider: 'none', clientBundle: { scan: true, sizeLimitKb: 1024 }, serverBundle: false },
  css: [
    '@fontsource/plus-jakarta-sans/400.css',
    '@fontsource/plus-jakarta-sans/600.css',
    '@fontsource/plus-jakarta-sans/700.css',
    '@fontsource/plus-jakarta-sans/800.css',
    '@fontsource/noto-sans-lao/400.css',
    '@fontsource/noto-sans-lao/600.css',
    '@fontsource/noto-sans-lao/700.css',
    '@fontsource/noto-sans-thai/400.css',
    '@fontsource/noto-sans-thai/700.css',
    '~/assets/css/main.css',
  ],
  app: {
    head: {
      title: 'zaokaiy POS',
      htmlAttrs: { lang: 'en' },
      meta: [{ name: 'viewport', content: 'width=device-width, initial-scale=1' }],
      link: [{ rel: 'icon', type: 'image/svg+xml', href: '/favicon.svg' }],
    },
  },
  // Tauri expects a fixed dev port and must not be cleared by Vite's screen output.
  vite: {
    clearScreen: false,
    envPrefix: ['VITE_', 'TAURI_'],
    server: { strictPort: true, watch: { ignored: ['**/src-tauri/**'] } },
  },
  runtimeConfig: {
    public: {
      // Pre-filled server address on the pairing screen (NUXT_PUBLIC_DEFAULT_API_BASE at build time).
      defaultApiBase: '',
    },
  },
})
