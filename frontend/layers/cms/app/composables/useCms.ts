import type { CmsCollection, CmsItem, CmsSection } from '../utils/cms'
import { cmsCollection } from '../utils/cms'

const FIELDS = ['path', 'title', 'description', 'date', 'updated', 'cover', 'author', 'tags', 'pinned', 'order'] as const

const listIn = (collection: CmsCollection, section: CmsSection) =>
  queryCollection(collection)
    .where('path', 'LIKE', `/${section}/%`)
    .where('draft', '=', false)
    .select(...FIELDS)
    .all() as Promise<CmsItem[]>

/** Pinned first; news newest first; policies and guides by `order`, then newest. */
function compare(section: CmsSection) {
  return (a: CmsItem, b: CmsItem) =>
    Number(b.pinned) - Number(a.pinned) ||
    (section === 'news' ? 0 : (a.order ?? 100) - (b.order ?? 100)) ||
    (b.date ?? '').localeCompare(a.date ?? '') ||
    a.title.localeCompare(b.title)
}

/**
 * Published pages of a section in the UI language. Pages that exist only in English are listed
 * too (shown in English) so a missing translation never hides a page.
 */
export function useCmsList(section: CmsSection, opts: { limit?: number } = {}) {
  const { locale } = useI18n()
  return useAsyncData(
    () => `cms-list-${locale.value}-${section}-${opts.limit ?? 'all'}`,
    async () => {
      const lang = locale.value
      const own = await listIn(cmsCollection(lang), section)
      const fallback = cmsCollection(lang) === 'cms_en' ? [] : await listIn('cms_en', section)
      const seen = new Set(own.map((p) => p.path))
      const items = [...own, ...fallback.filter((p) => !seen.has(p.path))].sort(compare(section))
      return opts.limit ? items.slice(0, opts.limit) : items
    },
    { default: () => [] as CmsItem[] },
  )
}

/** One page in the UI language, or the English page (`fallback: true`) when it isn't translated. */
export function useCmsPage(path: MaybeRefOrGetter<string>) {
  const { locale } = useI18n()
  return useAsyncData(
    () => `cms-page-${locale.value}-${toValue(path)}`,
    async () => {
      const p = toValue(path).replace(/\/+$/, '')
      const own = await queryCollection(cmsCollection(locale.value)).path(p).first()
      if (own && !own.draft) return { page: own, fallback: false }
      if (cmsCollection(locale.value) !== 'cms_en') {
        const en = await queryCollection('cms_en').path(p).first()
        if (en && !en.draft) return { page: en, fallback: true }
      }
      return null
    },
  )
}

/** Page or 404 — for the [slug] pages. */
export async function useCmsPageOr404(path: MaybeRefOrGetter<string>) {
  const res = await useCmsPage(path)
  if (!res.data.value) throw createError({ statusCode: 404, statusMessage: 'Page not found', fatal: true })
  return res
}
