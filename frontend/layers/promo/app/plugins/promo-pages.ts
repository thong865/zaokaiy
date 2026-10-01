// Staff see the promotion pages when their role has "marketing" (the API enforces the same rule).
export default defineNuxtPlugin(() => {
  addPagePermissions({ '/dashboard/promotions': 'marketing', '/dashboard/loyalty': 'marketing' })
})
