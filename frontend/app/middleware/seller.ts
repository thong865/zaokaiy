/** Dashboard guard: must be logged in; loads the seller's shops. Pages other than
 *  the dashboard home require at least one shop. Staff only reach pages their role allows —
 *  otherwise they land on the first page they can use (the API enforces the same rules). */
export default defineNuxtRouteMiddleware(async (to) => {
  const { loggedIn } = useAuth()
  if (!loggedIn.value) return navigateTo({ path: '/login', query: { next: to.fullPath } })
  const { loadShops, shops, can } = useShop()
  try {
    await loadShops()
  } catch {
    return navigateTo('/login')
  }
  if (!shops.value.length && to.path !== '/dashboard') return navigateTo('/dashboard')
  if (shops.value.length && !can(pagePermission(to.path))) {
    const landing = LANDING_ORDER.find((p) => can(pagePermission(p)))
    if (landing && landing !== to.path) return navigateTo(landing)
    if (to.path !== '/dashboard') return navigateTo('/dashboard')
  }
})
