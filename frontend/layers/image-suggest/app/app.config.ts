// What this module plugs into the core app (merged into app.config by Nuxt).
export default defineAppConfig({
  zkModules: {
    slots: {
      // product form → gallery: suggestions for the product name
      'gallery.suggest': [{ module: 'image_suggest', component: 'ImageSuggestStrip' }],
      // media library header: "share my product photos" switch
      'media.library': [{ module: 'image_suggest', component: 'ImageSuggestSharing' }],
    },
    adminNav: [{ module: 'image_suggest', to: '/admin/stock-images', labelKey: 'imgsuggest.nav.admin', icon: 'images' }],
  },
})
