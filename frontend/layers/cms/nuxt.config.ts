/**
 * Plug-in layer: site content — terms & policies, news and guides, written as Markdown files in
 * `content/<lang>/` (see content.config.ts). No API: pages are built by Nuxt Content.
 * Nuxt loads every folder in `layers/` automatically — delete this folder to remove it.
 */
export default defineNuxtConfig({
  modules: ['@nuxt/content'],
  content: {
    // node:sqlite (Node 22.13+) — no native add-on to build in the Docker image
    experimental: { sqliteConnector: 'native' },
    build: {
      markdown: {
        // right-side navigator: ## and ### headings
        toc: { depth: 3, searchDepth: 3 },
      },
    },
  },
  i18n: {
    locales: [
      { code: 'en', files: ['en/cms.json'] },
      { code: 'lo', files: ['lo/cms.json'] },
    ],
  },
})
