<script setup lang="ts">
/** Table QR code landing: /r/t/<token> → the restaurant's menu with the table attached. */
const route = useRoute()
const api = useApi()
const token = String(route.params.token)
const table = await api<{ shop_slug: string }>(`/restaurant/tables/${token}`).catch(() => null)
if (!table) throw createError({ statusCode: 404, statusMessage: tr('restaurant.table.notFound'), fatal: true })
await navigateTo({ path: `/s/${table.shop_slug}`, query: { table: token } }, { replace: true })
</script>

<template>
  <div />
</template>
