<script setup lang="ts">
import type { CategoryNode } from '~/utils/types'

/** Cascading category → sub-category → … picker. Emits the deepest selected id. */
const props = withDefaults(defineProps<{ nodes: CategoryNode[]; placeholder?: string; requireLeaf?: boolean }>(), {
  requireLeaf: false,
})
const model = defineModel<string | null>({ default: null })

const byId = computed(() => new Map(props.nodes.map((n) => [n.id, n])))
const chain = computed(() => {
  const out: string[] = []
  let cur = model.value ? byId.value.get(model.value) : undefined
  while (cur) {
    out.unshift(cur.id)
    cur = cur.parent_id ? byId.value.get(cur.parent_id) : undefined
  }
  return out
})
const levels = computed(() => {
  const out: { parent: string | null; options: CategoryNode[]; value: string }[] = []
  let parent: string | null = null
  for (let depth = 0; depth < MAX_CATEGORY_DEPTH; depth++) {
    const options = props.nodes.filter((n) => n.parent_id === parent)
    if (!options.length) break
    const value = chain.value[depth] ?? ''
    out.push({ parent, options, value })
    if (!value) break
    parent = value
  }
  return out
})
function pick(depth: number, value: string) {
  model.value = value || (depth > 0 ? levels.value[depth]!.parent : null)
}
const current = computed(() => (model.value ? byId.value.get(model.value) : undefined))
const needsDeeper = computed(() => props.requireLeaf && !!current.value && current.value.child_count > 0)
defineExpose({ needsDeeper })
</script>

<template>
  <div>
    <div class="grid gap-2">
      <select v-for="(lvl, d) in levels" :key="`${d}-${lvl.parent}`" :value="lvl.value" class="input" @change="pick(d, ($event.target as HTMLSelectElement).value)">
        <option value="">{{ d === 0 ? (placeholder ?? $t('category.select.placeholder')) : $t('category.select.allOf', { name: byId.get(lvl.parent!)?.name }) }}</option>
        <option v-for="o in lvl.options" :key="o.id" :value="o.id">{{ o.name }}</option>
      </select>
    </div>
    <p v-if="current" class="mt-1 text-xs" :class="needsDeeper ? 'text-amber-700' : 'text-muted'">
      {{ current.path }}<template v-if="needsDeeper"> — {{ $t('category.select.pickDeeper') }}</template>
    </p>
  </div>
</template>
