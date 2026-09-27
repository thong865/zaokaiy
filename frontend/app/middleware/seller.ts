/** Dashboard guard: must be logged in; loads the seller's shops. Pages other than
 *  the dashboard home require at least one shop. */
export default defineNuxtRouteMiddleware(async (to) => {
  const { loggedIn } = useAuth()
  if (!loggedIn.value) return navigateTo({ path: '/login', query: { next: to.fullPath } })
  const { loadShops, shops } = useShop()
  try {
    await loadShops()
  } catch {
    return navigateTo('/login')
  }
  if (!shops.value.length && to.path !== '/dashboard') return navigateTo('/dashboard')
})
