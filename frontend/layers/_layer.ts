import { readdirSync, existsSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

/**
 * i18n locales of one layer: every JSON file in <layer>/i18n/locales/<code>/ is a namespace
 * (the file holds a single top-level key = file name). @nuxtjs/i18n merges layers by locale code.
 */
export function layerLocales(layerUrl: string) {
  const dir = (code: string) => fileURLToPath(new URL(`./i18n/locales/${code}`, layerUrl))
  const files = (code: string) => (existsSync(dir(code)) ? readdirSync(dir(code)).filter((f) => f.endsWith('.json')).sort().map((f) => `${code}/${f}`) : [])
  return [
    { code: 'en', files: files('en') },
    { code: 'lo', files: files('lo') },
  ]
}
