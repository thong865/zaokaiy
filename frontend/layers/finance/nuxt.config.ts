import { fileURLToPath } from 'node:url'
import { layerLocales } from '../_layer'

// finance core — pages, components and translations for the 'finance' backend core.
export default defineNuxtConfig({
  alias: { '#finance': fileURLToPath(new URL('./app', import.meta.url)) },
  i18n: { locales: layerLocales(import.meta.url) },
})
