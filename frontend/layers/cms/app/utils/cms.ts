/** Site content (Markdown in layers/cms/content/<lang>/<section>/): terms & policies, news, guides. */

export type CmsSection = 'legal' | 'news' | 'guides'
export const SECTION_ICON: Record<CmsSection, string> = { legal: 'i-lucide-scale', news: 'i-lucide-newspaper', guides: 'i-lucide-book-open' }
/** Languages that have their own content folder; the first one is the fallback. */
export const CMS_LANGS = ['en', 'lo'] as const
export type CmsLang = (typeof CMS_LANGS)[number]
export type CmsCollection = `cms_${CmsLang}`

export const cmsCollection = (locale: string): CmsCollection => `cms_${(CMS_LANGS as readonly string[]).includes(locale) ? (locale as CmsLang) : 'en'}`

/** List row. */
export interface CmsItem {
  path: string
  title: string
  description: string
  date?: string
  updated?: string
  cover?: string
  author?: string
  tags?: string[]
  pinned?: boolean
  order?: number
}
export const sectionOf = (path: string) => path.split('/')[1] as CmsSection

/** Rough reading time: counts the words in a page body (minimark: [tag, props, ...children]). */
export function readMinutes(body: unknown) {
  let words = 0
  const walk = (n: unknown) => {
    if (typeof n === 'string') words += n.split(/\s+/).filter(Boolean).length
    else if (Array.isArray(n)) n.slice(2).forEach(walk)
  }
  const value = (body as { value?: unknown[] } | null)?.value
  if (Array.isArray(value)) value.forEach(walk)
  return Math.max(1, Math.round(words / 200))
}
