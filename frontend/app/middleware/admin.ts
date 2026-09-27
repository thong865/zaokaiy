export default defineNuxtRouteMiddleware(async (to) => {
  const { loggedIn, user, fetchMe } = useAuth()
  if (!loggedIn.value) return navigateTo({ path: '/login', query: { next: to.fullPath } })
  if (!user.value) await fetchMe()
  if (user.value?.role !== 'admin') return abortNavigation(createError({ statusCode: 403, statusMessage: 'Admins only' }))
})
