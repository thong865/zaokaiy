export default defineNuxtPlugin(async () => {
  const { token, user, fetchMe } = useAuth()
  if (token.value && !user.value) await fetchMe()
})
