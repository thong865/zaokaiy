import { fileURLToPath } from 'node:url'
import { layerLocales } from '../_layer'

// commerce core — pages, components and translations for the 'commerce' backend core.
export default defineNuxtConfig({
  alias: { '#commerce': fileURLToPath(new URL('./app', import.meta.url)) },
  i18n: { locales: layerLocales(import.meta.url) },
})
