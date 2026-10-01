// Navigation entries this module adds to the core layouts (merged into app.config by Nuxt).
export default defineAppConfig({
  zkModules: {
    dashboardNav: [{ module: 'cod_risk', group: 'shop', after: '/dashboard/cod', to: '/dashboard/cod-risk', labelKey: 'codrisk.nav.seller', icon: 'shield-alert' }],
    adminNav: [{ module: 'cod_risk', to: '/admin/cod-risk', labelKey: 'codrisk.nav.admin', icon: 'shield-alert' }],
  },
})
