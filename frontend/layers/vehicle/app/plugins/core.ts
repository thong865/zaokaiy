import { defineAsyncComponent } from 'vue'

/** Vehicle core: showroom storefront, spec sheets on listings, buyer requests. Builds on commerce. */
export default defineNuxtPlugin(() => {
  registerCore({
    name: 'vehicle',
    order: 20,
    tile: { to: '/vehicles', label: 'vehicle.nav.vehicles', hint: 'common.superApp.vehiclesHint', icon: 'car', color: 'bg-sky-500/12 text-sky-600' },
    onboarding: [{ id: 'vehicle', label: 'vehicle.store.openVehicle', hint: 'vehicle.store.openVehicleHint', vertical: 'vehicle', business: true, order: 30 }],
    nav: [{ id: 'shop', label: 'common.dashNav.shop', order: 10, items: [{ to: '/dashboard/leads', label: 'vehicle.nav.leads', icon: 'phone', verticals: ['vehicle'], order: 45 }] }],
    storefronts: { vehicle: defineAsyncComponent(() => import('../components/vehicle/VehicleStorefront.vue')) },
    // A dealer's dashboard is the commerce one (products, orders, leads in the menu).
    overviews: { vehicle: defineAsyncComponent(() => import('#commerce/components/commerce/CommerceOverview.vue')) },
    productDetail: { key: 'vehicle', component: defineAsyncComponent(() => import('../components/vehicle/VehicleProductDetail.vue')) },
  })
})
