// What this module plugs into the core app (merged into app.config by Nuxt).
export default defineAppConfig({
  zkModules: {
    dashboardNav: [
      { module: 'promo', group: 'grow', to: '/dashboard/promotions', labelKey: 'promo.nav.promotions', icon: 'ticket-percent' },
      { module: 'promo', group: 'grow', after: '/dashboard/promotions', to: '/dashboard/loyalty', labelKey: 'promo.nav.loyalty', icon: 'gem' },
    ],
    adminNav: [{ module: 'promo', to: '/admin/promotions', labelKey: 'promo.nav.admin', icon: 'ticket-percent' }],
    slots: {
      // cart summary: coupon code + points per shop
      'cart.promo': [{ module: 'promo', component: 'PromoCartBox' }],
      // chat-order checkout (/c/:token): coupon code + points
      'checkout.promo': [{ module: 'promo', component: 'PromoOrderBox' }],
      // POS bill: member phone → points and coupons
      'pos.promo': [{ module: 'promo', component: 'PromoPosMember' }],
      // account page: "My rewards"
      'account.sections': [{ module: 'promo', component: 'PromoAccountLink' }],
    },
  },
})
