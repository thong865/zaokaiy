import { defineAsyncComponent } from 'vue'

/** Restaurant core: menu storefront with QR table ordering, kitchen board, food home section. */
export default defineNuxtPlugin(() => {
  const only = ['restaurant']
  registerCore({
    name: 'restaurant',
    order: 30,
    tile: { to: '/food', label: 'common.superApp.food', hint: 'common.superApp.foodHint', icon: 'utensils-crossed', color: 'bg-amber-500/15 text-amber-600' },
    onboarding: [{ id: 'restaurant', label: 'restaurant.open.label', hint: 'restaurant.open.hint', vertical: 'restaurant', order: 40 }],
    pages: {
      '/dashboard/restaurant/orders': 'orders',
      '/dashboard/restaurant/menu': 'products',
      '/dashboard/restaurant/tables': 'settings',
      '/dashboard/restaurant/settings': 'settings',
    },
    nav: [
      {
        id: 'restaurant',
        label: 'common.verticals.restaurant',
        order: 15,
        items: [
          { to: '/dashboard/restaurant/orders', label: 'restaurant.nav.kitchen', icon: 'chef-hat', verticals: only, order: 10 },
          { to: '/dashboard/restaurant/menu', label: 'restaurant.nav.menu', icon: 'book-open', verticals: only, order: 20 },
          { to: '/dashboard/restaurant/tables', label: 'restaurant.nav.tables', icon: 'qr-code', verticals: only, order: 30 },
          { to: '/dashboard/restaurant/settings', label: 'restaurant.nav.settings', icon: 'clock', verticals: only, order: 40 },
        ],
      },
    ],
    storefronts: { restaurant: defineAsyncComponent(() => import('../components/restaurant/RestaurantMenu.vue')) },
    overviews: { restaurant: defineAsyncComponent(() => import('../components/restaurant/RestaurantOverview.vue')) },
    homeSections: [{ order: 20, component: defineAsyncComponent(() => import('../components/restaurant/RestaurantHomeStrip.vue')) }],
  })
})
