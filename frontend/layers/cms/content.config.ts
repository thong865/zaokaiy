import { defineCollection, defineContentConfig, z } from '@nuxt/content'

/**
 * Site content as Markdown files in `layers/cms/content/<lang>/<section>/<slug>.md`:
 *   sections: legal (terms & policies) · news · guides
 *   one collection per language; both use the same paths (/news/<slug>), so a page missing in
 *   Lao falls back to the English file.
 */
const schema = z.object({
  /** Publish date (news, guides) — YYYY-MM-DD. */
  date: z.string().optional(),
  /** Last change (shown on policies) — YYYY-MM-DD. */
  updated: z.string().optional(),
  /** Cover image URL (or /path in public/). */
  cover: z.string().optional(),
  author: z.string().optional(),
  tags: z.array(z.string()).default([]),
  /** Shown first in lists. */
  pinned: z.boolean().default(false),
  /** Sort order for policies and guides (lower first). */
  order: z.number().default(100),
  /** Drafts are left out of lists and pages. */
  draft: z.boolean().default(false),
})

const language = (code: string) =>
  defineCollection({
    type: 'page',
    source: { include: `${code}/**/*.md`, prefix: '' },
    schema,
  })

export default defineContentConfig({
  collections: {
    cms_en: language('en'),
    cms_lo: language('lo'),
  },
})
