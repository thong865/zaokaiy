import { fileURLToPath } from 'node:url'
import { layerLocales } from '../_layer'

// restaurant core — pages, components and translations for the 'restaurant' backend core.
export default defineNuxtConfig({
  alias: { '#restaurant': fileURLToPath(new URL('./app', import.meta.url)) },
  i18n: { locales: layerLocales(import.meta.url) },
})
