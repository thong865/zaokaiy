<script setup lang="ts">
/** QR code as an inline SVG (generated locally — nothing is sent to a QR service). */
import QRCode from 'qrcode'

const props = defineProps<{ value: string; label?: string }>()
const svg = ref('')
watchEffect(async () => {
  svg.value = await QRCode.toString(props.value, { type: 'svg', margin: 1, errorCorrectionLevel: 'M', color: { dark: '#100e0c', light: '#ffffff' } })
})
</script>

<template>
  <!-- eslint-disable-next-line vue/no-v-html -- SVG produced by the qrcode library from our own URL -->
  <div role="img" :aria-label="label ?? value" class="[&>svg]:h-full [&>svg]:w-full" v-html="svg" />
</template>
