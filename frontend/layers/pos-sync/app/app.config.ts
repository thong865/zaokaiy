// Navigation entry this module adds to the dashboard (merged into app.config by Nuxt).
export default defineAppConfig({
  zkModules: {
    dashboardNav: [{ module: 'pos_sync', group: 'shop', after: '/dashboard/pos-sales', to: '/dashboard/pos-devices', labelKey: 'possync.nav', icon: 'monitor-smartphone' }],
  },
})
