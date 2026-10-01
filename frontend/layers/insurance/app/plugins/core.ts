import { defineAsyncComponent } from 'vue'

/** Insurance core: plans, instant quotes and online applications; agents issue the policies. */
export default defineNuxtPlugin(() => {
  const only = ['insurance']
  registerCore({
    name: 'insurance',
    order: 40,
    tile: { to: '/insurance', label: 'common.superApp.insurance', hint: 'common.superApp.insuranceHint', icon: 'shield-plus', color: 'bg-violet-500/12 text-violet-600' },
    onboarding: [{ id: 'insurance', label: 'insurance.open.label', hint: 'insurance.open.hint', vertical: 'insurance', business: true, order: 50 }],
    pages: { '/dashboard/insurance/applications': 'orders', '/dashboard/insurance/plans': 'products' },
    nav: [
      {
        id: 'insurance',
        label: 'common.verticals.insurance',
        order: 15,
        items: [
          { to: '/dashboard/insurance/applications', label: 'insurance.nav.applications', icon: 'file-check', verticals: only, order: 10 },
          { to: '/dashboard/insurance/plans', label: 'insurance.nav.plans', icon: 'shield-plus', verticals: only, order: 20 },
        ],
      },
    ],
    storefronts: { insurance: defineAsyncComponent(() => import('../components/insurance/InsuranceStorefront.vue')) },
    overviews: { insurance: defineAsyncComponent(() => import('../components/insurance/InsuranceOverview.vue')) },
    homeSections: [{ order: 30, component: defineAsyncComponent(() => import('../components/insurance/InsuranceHomeStrip.vue')) }],
  })
})
