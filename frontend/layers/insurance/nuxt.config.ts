import { fileURLToPath } from 'node:url'
import { layerLocales } from '../_layer'

// insurance core — pages, components and translations for the 'insurance' backend core.
export default defineNuxtConfig({
  alias: { '#insurance': fileURLToPath(new URL('./app', import.meta.url)) },
  i18n: { locales: layerLocales(import.meta.url) },
})
