/** Finance core: tax-agent mandate + VAT / e-commerce tax filings for every shop type. */
export default defineNuxtPlugin(() => {
  registerCore({
    name: 'finance',
    order: 90,
    pages: { '/dashboard/tax': 'owner' },
    nav: [{ id: 'account', label: 'common.dashNav.account', order: 90, items: [{ to: '/dashboard/tax', label: 'common.superApp.tax', icon: 'landmark', order: 50 }] }],
    adminNav: [{ to: '/admin/tax', label: 'common.superApp.tax', icon: 'landmark', order: 60 }],
  })
})
