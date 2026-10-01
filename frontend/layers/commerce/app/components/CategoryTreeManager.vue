<script setup lang="ts">
import type { CategoryNode } from '~/utils/types'

/**
 * Tree editor for categories & sub-categories. Works for both the marketplace tree (admin)
 * and a shop's own tree (seller) — the only difference is where new categories are POSTed.
 */
const props = defineProps<{
  nodes: CategoryNode[]
  createPath: string
  scope: 'shop' | 'marketplace'
  shopId?: string
}>()
const emit = defineEmits<{ changed: [] }>()
const api = useApi()
const { t } = useI18n()
const error = ref('')
const notice = ref('')
function flash(m: string) {
  notice.value = m
  setTimeout(() => (notice.value = ''), 2000)
}

// ---- collapse ----
const collapsed = ref<Set<string>>(new Set())
const parentOf = computed(() => new Map(props.nodes.map((n) => [n.id, n.parent_id])))
function hidden(n: CategoryNode) {
  let p = n.parent_id
  while (p) {
    if (collapsed.value.has(p)) return true
    p = parentOf.value.get(p) ?? null
  }
  return false
}
function toggle(id: string) {
  const s = new Set(collapsed.value)
  s.has(id) ? s.delete(id) : s.add(id)
  collapsed.value = s
}
const allCollapsed = computed(() => props.nodes.filter((n) => n.child_count).every((n) => collapsed.value.has(n.id)))
function toggleAll() {
  collapsed.value = allCollapsed.value ? new Set() : new Set(props.nodes.filter((n) => n.child_count).map((n) => n.id))
}

// ---- add ----
const adding = ref<string | null | undefined>(undefined) // undefined = closed, null = top level
const newName = ref('')
async function add() {
  if (!newName.value.trim()) return
  error.value = ''
  try {
    await api(props.createPath, { method: 'POST', body: { name: newName.value, parent_id: adding.value ?? null } })
    if (adding.value) {
      const s = new Set(collapsed.value)
      s.delete(adding.value)
      collapsed.value = s
    }
    newName.value = ''
    flash(t('category.tree.added'))
    emit('changed')
  } catch (e) {
    error.value = apiError(e)
  }
}
function startAdd(parent: string | null) {
  adding.value = parent
  newName.value = ''
  editing.value = null
  nextTick(() => document.getElementById('cat-new-input')?.focus())
}

// ---- edit ----
const editing = ref<CategoryNode | null>(null)
const draft = reactive({ name: '', slug: '', description: '', image_url: '', parent_id: '' as string, active: true })
function startEdit(n: CategoryNode) {
  editing.value = n
  adding.value = undefined
  Object.assign(draft, { name: n.name, slug: n.slug, description: n.description, image_url: n.image_url ?? '', parent_id: n.parent_id ?? '', active: n.active })
}
// Valid parents: not itself/descendants, and the moved subtree must fit in MAX depth.
const parentOptions = computed(() => {
  const n = editing.value
  if (!n) return []
  const sub = subtreeIds(props.nodes, n.id)
  const height = Math.max(...props.nodes.filter((x) => sub.has(x.id)).map((x) => x.depth)) - n.depth
  return props.nodes.filter((p) => !sub.has(p.id) && p.depth + 1 + height < MAX_CATEGORY_DEPTH)
})
async function saveEdit() {
  const n = editing.value
  if (!n) return
  error.value = ''
  try {
    const body: Record<string, unknown> = {
      name: draft.name,
      slug: draft.slug,
      description: draft.description,
      image_url: draft.image_url,
      active: draft.active,
    }
    if ((draft.parent_id || null) !== n.parent_id) body.parent_id = draft.parent_id || null
    await api(`/categories/${n.id}`, { method: 'PATCH', body })
    editing.value = null
    flash(t('common.saved'))
    emit('changed')
  } catch (e) {
    error.value = apiError(e)
  }
}

// ---- reorder ----
const siblings = (n: CategoryNode) => props.nodes.filter((x) => x.parent_id === n.parent_id)
async function move(n: CategoryNode, dir: -1 | 1) {
  const sib = siblings(n)
  const i = sib.findIndex((x) => x.id === n.id)
  const j = i + dir
  if (j < 0 || j >= sib.length) return
  const ids = sib.map((x) => x.id)
  ;[ids[i], ids[j]] = [ids[j]!, ids[i]!]
  try {
    await api('/categories/reorder', { method: 'POST', body: { ids } })
    emit('changed')
  } catch (e) {
    error.value = apiError(e)
  }
}

// ---- delete ----
const deleting = ref<CategoryNode | null>(null)
const reassignTo = ref('')
const deleteError = ref('')
async function remove(n: CategoryNode, reassign?: string) {
  error.value = deleteError.value = ''
  try {
    await api(`/categories/${n.id}`, { method: 'DELETE', query: reassign ? { reassign_to: reassign } : undefined })
    deleting.value = null
    flash(t('category.tree.deleted'))
    emit('changed')
  } catch (e) {
    const status = (e as { status?: number }).status
    if (status === 409 && props.scope === 'marketplace' && n.child_count === 0) {
      deleting.value = n
      deleteError.value = apiError(e)
    } else if (deleting.value) {
      deleteError.value = apiError(e)
    } else {
      error.value = apiError(e)
    }
  }
}
function askDelete(n: CategoryNode) {
  if (props.scope === 'shop' && n.product_count > 0) {
    if (!window.confirm(t('category.tree.confirmDeleteWithProducts', { name: n.name, n: n.product_count }))) return
  } else if (!window.confirm(t('category.tree.confirmDelete', { name: n.name }))) return
  remove(n)
}
const reassignOptions = computed(() => props.nodes.filter((x) => x.id !== deleting.value?.id && x.active))
</script>

<template>
  <div>
    <div class="flex flex-wrap items-center gap-2">
      <UButton size="sm" type="button" @click="startAdd(null)">{{ $t('category.tree.addCategory') }}</UButton>
      <UButton color="neutral" variant="soft" size="sm" v-if="nodes.some((n) => n.child_count)" type="button" @click="toggleAll">{{ allCollapsed ? $t('category.tree.expandAll') : $t('category.tree.collapseAll') }}</UButton>
      <span class="ml-auto text-xs text-muted">{{ $t('category.tree.count', { n: nodes.length }, nodes.length) }} · {{ $t('category.tree.upToLevels', { n: MAX_CATEGORY_DEPTH }) }}</span>
    </div>
    <p v-if="error" class="mt-3 text-sm text-brand-700">{{ error }}</p>

    <form v-if="adding === null" class="card mt-3 flex gap-2 p-3" @submit.prevent="add">
      <UInput id="cat-new-input" v-model="newName" maxlength="80" :placeholder="$t('category.tree.newTopPh')" />
      <UButton color="neutral" size="sm" type="submit">{{ $t('common.add') }}</UButton>
      <UButton color="neutral" variant="soft" size="sm" type="button" @click="adding = undefined">{{ $t('common.cancel') }}</UButton>
    </form>

    <div v-if="nodes.length" class="card mt-3 divide-y divide-line/70 overflow-hidden">
      <template v-for="n in nodes" :key="n.id">
        <div v-if="!hidden(n)" class="group">
          <div class="flex items-center gap-2 px-3 py-2.5 hover:bg-paper" :style="{ paddingLeft: `${12 + n.depth * 28}px` }">
            <button
              type="button"
              class="grid size-6 shrink-0 place-items-center rounded text-muted transition hover:bg-line"
              :class="!n.child_count && 'invisible'"
              :aria-label="collapsed.has(n.id) ? $t('category.tree.expand') : $t('category.tree.collapse')"
              @click="toggle(n.id)"
            >
              <span class="text-xs transition" :class="!collapsed.has(n.id) && 'rotate-90'">▶</span>
            </button>
            <span v-if="n.depth > 0" class="text-line">└</span>
            <div class="min-w-0 flex-1">
              <div class="flex items-center gap-2">
                <span class="truncate font-semibold" :class="!n.active && 'text-muted line-through'">{{ n.name }}</span>
                <span v-if="!n.active" class="chip bg-line text-muted">{{ $t('category.tree.hidden') }}</span>
                <span class="chip bg-paper text-muted" :title="$t('category.tree.productCountTitle', { direct: n.product_count, total: n.total_count })">{{ $t('category.tree.productCount', { n: n.total_count }, n.total_count) }}</span>
              </div>
              <div class="truncate text-[11px] text-muted">/{{ n.slug }}<template v-if="n.child_count"> · {{ $t('category.tree.subCount', { n: n.child_count }, n.child_count) }}</template></div>
            </div>
            <div class="flex shrink-0 items-center gap-1 opacity-60 transition group-hover:opacity-100">
              <UButton color="neutral" variant="soft" size="sm" v-if="n.depth < MAX_CATEGORY_DEPTH - 1" type="button" @click="startAdd(n.id)">{{ $t('category.tree.addSub') }}</UButton>
              <UButton color="neutral" variant="soft" size="sm" type="button" class="px-2" :aria-label="$t('category.tree.moveUp')" @click="move(n, -1)">↑</UButton>
              <UButton color="neutral" variant="soft" size="sm" type="button" class="px-2" :aria-label="$t('category.tree.moveDown')" @click="move(n, 1)">↓</UButton>
              <UButton color="neutral" variant="soft" size="sm" type="button" @click="startEdit(n)">{{ $t('common.edit') }}</UButton>
              <UButton color="neutral" variant="soft" size="sm" type="button" class="px-2 text-brand-700" :aria-label="$t('common.delete')" @click="askDelete(n)">✕</UButton>
            </div>
          </div>

          <form v-if="adding === n.id" class="flex gap-2 bg-paper px-3 py-2" :style="{ paddingLeft: `${40 + (n.depth + 1) * 28}px` }" @submit.prevent="add">
            <UInput id="cat-new-input" v-model="newName" maxlength="80" :placeholder="$t('category.tree.newSubPh', { name: n.name })" />
            <UButton color="neutral" size="sm" type="submit">{{ $t('common.add') }}</UButton>
            <UButton color="neutral" variant="soft" size="sm" type="button" @click="adding = undefined">{{ $t('common.cancel') }}</UButton>
          </form>

          <form v-if="editing?.id === n.id" class="grid gap-3 bg-paper p-4 sm:grid-cols-2" @submit.prevent="saveEdit">
            <div><label class="label">{{ $t('common.name') }}</label><UInput v-model="draft.name" required maxlength="80" /></div>
            <div><label class="label">{{ $t('category.tree.slug') }}</label><UInput size="sm" v-model="draft.slug" class="font-mono" /></div>
            <div>
              <label class="label">{{ $t('category.tree.parent') }}</label>
              <select v-model="draft.parent_id" class="input">
                <option value="">{{ $t('category.tree.topLevel') }}</option>
                <option v-for="p in parentOptions" :key="p.id" :value="p.id">{{ '— '.repeat(p.depth) }}{{ p.name }}</option>
              </select>
            </div>
            <div class="flex items-end">
              <label class="flex items-center gap-2 text-sm"><input v-model="draft.active" type="checkbox" class="size-4 accent-brand-500"> {{ $t('category.tree.visible') }}</label>
            </div>
            <div class="sm:col-span-2"><label class="label">{{ $t('common.description') }}</label><UTextarea v-model="draft.description" :rows="2" /></div>
            <div class="sm:col-span-2">
              <MediaField v-if="scope === 'shop' && shopId" v-model="draft.image_url" :shop-id="shopId" kind="image" :label="$t('category.tree.image')" />
              <template v-else><label class="label">{{ $t('category.tree.imageUrl') }}</label><UInput v-model="draft.image_url" placeholder="https://…" /></template>
            </div>
            <div class="flex justify-end gap-2 sm:col-span-2">
              <UButton color="neutral" variant="soft" size="sm" type="button" @click="editing = null">{{ $t('common.cancel') }}</UButton>
              <UButton size="sm" type="submit">{{ $t('common.save') }}</UButton>
            </div>
          </form>
        </div>
      </template>
    </div>
    <EmptyState v-else class="mt-3" :title="$t('category.tree.emptyTitle')" :text="scope === 'shop' ? $t('category.tree.emptyShop') : $t('category.tree.emptyMarketplace')" />

    <!-- Marketplace delete with products: reassign -->
    <Teleport to="body">
      <div v-if="deleting" class="fixed inset-0 z-50 grid place-items-center bg-night/40 p-4" @click.self="deleting = null">
        <form class="card w-full max-w-md space-y-4 p-6 shadow-2xl" @submit.prevent="remove(deleting!, reassignTo)">
          <div class="text-lg font-bold">{{ $t('category.tree.deleteTitle', { name: deleting.name }) }}</div>
          <p class="text-sm text-muted">{{ deleteError }}</p>
          <div>
            <label class="label">{{ $t('category.tree.moveProductsTo') }}</label>
            <select v-model="reassignTo" required class="input">
              <option value="" disabled>{{ $t('category.tree.chooseCategory') }}</option>
              <option v-for="o in reassignOptions" :key="o.id" :value="o.id">{{ o.path }}</option>
            </select>
          </div>
          <p class="text-xs text-muted">{{ $t('category.tree.deleteTip') }}</p>
          <div class="flex justify-end gap-2">
            <UButton color="neutral" variant="soft" type="button" @click="deleting = null">{{ $t('common.cancel') }}</UButton>
            <UButton color="neutral" variant="ghost" type="submit" class="bg-brand-700 text-white hover:bg-brand-600" :disabled="!reassignTo">{{ $t('category.tree.moveAndDelete') }}</UButton>
          </div>
        </form>
      </div>
    </Teleport>
    <div v-if="notice" class="fixed bottom-6 left-1/2 z-50 -translate-x-1/2 rounded-full bg-ink px-4 py-2 text-sm font-semibold text-paper shadow-lg">{{ notice }}</div>
  </div>
</template>
