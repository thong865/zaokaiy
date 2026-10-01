<script setup lang="ts">
/**
 * Extension point for plug-in modules (frontend `layers/<module>`). A layer registers components
 * for a slot name in its app.config:
 *   zkModules: { slots: { 'gallery.suggest': [{ module: 'image_suggest', component: 'ImageSuggestStrip' }] } }
 * The component must be global (file name `*.global.vue`). Entries show only while the backend
 * module is enabled (GET /api/modules); an entry without `module` (front-end-only layer) always shows.
 * Attributes and listeners are passed through.
 */
defineOptions({ inheritAttrs: false })
const props = defineProps<{ name: string }>()
interface SlotEntry { module?: string; component: string }
const cfg = useAppConfig() as { zkModules?: { slots?: Record<string, SlotEntry[]> } }
const { enabled } = useModules()
const entries = computed(() => (cfg.zkModules?.slots?.[props.name] ?? []).filter((e) => !e.module || enabled(e.module)))
const resolve = (n: string) => resolveComponent(n)
</script>

<template>
  <component :is="resolve(e.component)" v-for="e in entries" :key="e.component" v-bind="$attrs" />
</template>
