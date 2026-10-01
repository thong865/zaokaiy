// What this layer plugs into the core app (merged into app.config by Nuxt).
// No `module` key: the pages are Markdown files, so they show without a backend module.
export default defineAppConfig({
  zkModules: {
    slots: {
      // footer column: news, guides, terms & policies
      'footer.links': [{ component: 'CmsFooterLinks' }],
      // home page: latest news + guides
      'home.sections': [{ component: 'CmsHomeStrip' }],
    },
  },
})
