// Checks: en/lo key parity, and every literal $t('…') / t('…') / tr('…') key used in app/ exists in en.
import { readdirSync, readFileSync, statSync } from 'node:fs'
import { join } from 'node:path'
const root = new URL('..', import.meta.url).pathname
const load = (code) => {
  const out = {}
  for (const f of readdirSync(join(root, 'i18n/locales', code)).filter((f) => f.endsWith('.json'))) {
    const j = JSON.parse(readFileSync(join(root, 'i18n/locales', code, f), 'utf8'))
    const ns = f.replace('.json', '')
    if (Object.keys(j).length !== 1 || !j[ns]) throw new Error(`${code}/${f}: must hold a single top-level key "${ns}"`)
    Object.assign(out, j)
  }
  return out
}
const flat = (o, p = '', acc = new Map()) => {
  for (const [k, v] of Object.entries(o)) typeof v === 'object' ? flat(v, p + k + '.', acc) : acc.set(p + k, v)
  return acc
}
const en = flat(load('en')), lo = flat(load('lo'))
let bad = 0
for (const k of en.keys()) if (!lo.has(k)) { console.log('missing in lo:', k); bad++ }
for (const k of lo.keys()) if (!en.has(k)) { console.log('missing in en:', k); bad++ }
for (const [k, v] of lo) if (typeof v === 'string' && v.trim() === '' ) { console.log('empty lo:', k); bad++ }
// Every message must compile with vue-i18n's parser (catches unescaped @ | { } — write {'@'} for a literal @).
const { baseCompile } = await import('@intlify/message-compiler')
for (const [lang, m] of [['en', en], ['lo', lo]])
  for (const [k, v] of m)
    if (typeof v === 'string')
      baseCompile(v, { onError: (e) => { console.log(`syntax error in ${lang}:${k} (${e.message}): ${v}`); bad++ } })
const prefixes = new Set([...en.keys()].flatMap((k) => k.split('.').map((_, i, a) => a.slice(0, i + 1).join('.'))))
const walk = (d) => readdirSync(d).flatMap((f) => (statSync(join(d, f)).isDirectory() ? walk(join(d, f)) : [join(d, f)]))
for (const file of walk(join(root, 'app')).filter((f) => /\.(vue|ts)$/.test(f))) {
  const src = readFileSync(file, 'utf8')
  for (const m of src.matchAll(/(?:\$t|\$te|\bt|\btr|keypath=)\(?\s*['"`]([a-zA-Z][\w-]*(?:\.[\w-]+)+)['"`]/g)) {
    const k = m[1]
    if (!en.has(k) && !prefixes.has(k)) { console.log(`unknown key ${k} in ${file.replace(root, '')}`); bad++ }
  }
}
console.log(bad ? `${bad} problem(s)` : `i18n OK — ${en.size} keys`)
process.exit(bad ? 1 : 0)
