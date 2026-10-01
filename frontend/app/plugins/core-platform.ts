/** Platform core (accounts, shops, corporate KYC): what the shell itself adds to the registry. */
export default defineNuxtPlugin(() => {
  registerCore({
    name: 'platform',
    order: 0,
    nav: [
      { id: 'shop', label: 'common.dashNav.shop', order: 10, items: [{ to: '/dashboard', label: 'common.dashNav.overview', icon: 'layout-dashboard', order: 0 }] },
      {
        id: 'account',
        label: 'common.dashNav.account',
        order: 90,
        items: [
          { to: '/dashboard/verification', label: 'kyb.nav', icon: 'shield-check', business: true, order: 10 },
          { to: '/dashboard/staff', label: 'staff.nav', icon: 'users-round', order: 20 },
          { to: '/dashboard/settings', label: 'common.dashNav.settings', icon: 'sliders-horizontal', order: 90 },
        ],
      },
    ],
    adminNav: [
      { to: '/admin/shops', label: 'common.adminNav.shops', icon: 'store', order: 30 },
      { to: '/admin/kyb', label: 'kyb.admin.nav', icon: 'shield-check', badge: 'kyb_pending', order: 40 },
    ],
  })
})
