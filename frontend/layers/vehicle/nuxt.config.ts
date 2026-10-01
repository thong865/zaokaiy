import { fileURLToPath } from 'node:url'
import { layerLocales } from '../_layer'

// vehicle core — pages, components and translations for the 'vehicle' backend core.
export default defineNuxtConfig({
  alias: { '#vehicle': fileURLToPath(new URL('./app', import.meta.url)) },
  i18n: { locales: layerLocales(import.meta.url) },
})
