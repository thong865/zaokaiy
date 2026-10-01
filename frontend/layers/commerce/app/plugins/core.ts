import { defineAsyncComponent } from 'vue'

/** Commerce core: shop products, orders, delivery, POS, social selling, sell-staff network, ads, AI. */
export default defineNuxtPlugin(() => {
  const shops = ['general', 'vehicle'] // shop types that sell products
  registerCore({
    name: 'commerce',
    order: 10,
    tile: { to: '/#shop', label: 'common.superApp.shop', hint: 'common.superApp.shopHint', icon: 'shopping-bag', color: 'bg-brand-500/12 text-brand-600' },
    onboarding: [
      { id: 'seller', label: 'dash.open.seller', hint: 'dash.open.sellerHint', vertical: 'general', kind: 'seller', order: 10 },
      { id: 'creator', label: 'dash.open.creator', hint: 'dash.open.creatorHint', vertical: 'general', kind: 'creator', order: 20 },
    ],
    nav: [
      {
        id: 'shop',
        label: 'common.dashNav.shop',
        order: 10,
        items: [
          { to: '/dashboard/products', label: 'common.dashNav.products', icon: 'package', verticals: shops, order: 10 },
          { to: '/dashboard/categories', label: 'common.dashNav.categories', icon: 'folder-tree', verticals: shops, order: 20 },
          { to: '/dashboard/inventory', label: 'common.dashNav.inventory', icon: 'archive', verticals: shops, order: 30 },
          { to: '/dashboard/media', label: 'common.dashNav.media', icon: 'image', verticals: shops, order: 40 },
          { to: '/dashboard/orders', label: 'common.dashNav.orders', icon: 'receipt', verticals: shops, order: 60 },
          { to: '/dashboard/shipping', label: 'common.dashNav.shipping', icon: 'truck', verticals: shops, order: 70 },
          { to: '/dashboard/cod', label: 'common.dashNav.cod', icon: 'banknote', verticals: shops, order: 80 },
          { to: '/pos', label: 'common.dashNav.pos', icon: 'calculator', verticals: shops, order: 90 },
          { to: '/dashboard/pos-sales', label: 'common.dashNav.posSales', icon: 'file-text', verticals: shops, order: 91 },
          { to: '/dashboard/barcodes', label: 'common.dashNav.barcodes', icon: 'scan-barcode', verticals: shops, order: 92 },
        ],
      },
      { id: 'social', label: 'common.dashNav.social', order: 20, items: [{ to: '/dashboard/social', label: 'common.dashNav.socialLive', icon: 'message-circle', verticals: shops }] },
      {
        id: 'network',
        label: 'common.dashNav.network',
        order: 30,
        items: [
          { to: '/dashboard/partners', label: 'common.dashNav.partners', icon: 'users', verticals: shops, order: 10 },
          { to: '/dashboard/marketplace', label: 'common.dashNav.marketplace', icon: 'globe', verticals: shops, order: 20 },
          { to: '/dashboard/commissions', label: 'common.dashNav.commissions', icon: 'percent', verticals: shops, order: 30 },
        ],
      },
      {
        id: 'grow',
        label: 'common.dashNav.grow',
        order: 40,
        items: [
          { to: '/dashboard/ads', label: 'common.dashNav.ads', icon: 'megaphone', verticals: shops, order: 10 },
          { to: '/dashboard/content', label: 'common.dashNav.content', icon: 'pen-line', verticals: shops, order: 20 },
          { to: '/dashboard/assistant', label: 'common.dashNav.assistant', icon: 'sparkles', verticals: shops, order: 30 },
        ],
      },
    ],
    adminNav: [
      { to: '/admin', label: 'common.adminNav.review', icon: 'circle-check', badge: 'pending', order: 10 },
      { to: '/admin/categories', label: 'common.adminNav.categories', icon: 'folder-tree', order: 20 },
      { to: '/admin/carriers', label: 'common.adminNav.carriers', icon: 'truck', order: 50 },
    ],
    overviews: { general: defineAsyncComponent(() => import('../components/commerce/CommerceOverview.vue')) },
    homeSections: [{ order: 50, component: defineAsyncComponent(() => import('../components/commerce/CommerceHomeFeed.vue')) }],
  })
})
