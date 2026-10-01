<script setup lang="ts">
/** Menu editor: sections and dishes (names in English + Lao, price, photo, spice level, sold-out switch). */
import { localName, type MenuItem } from '#restaurant/utils/restaurant'

definePageMeta({ layout: 'dashboard', middleware: 'seller' })
const { data, refresh, api, shopId } = useRestaurant()
const { locale } = useI18n()
const cur = computed(() => data.value?.shop.currency ?? 'LAK')
const error = ref('')
async function run(fn: () => Promise<unknown>) {
  error.value = ''
  try {
    await fn()
    await refresh()
    return true
  } catch (e) {
    error.value = apiError(e)
    return false
  }
}

// sections
const newSection = reactive({ name: '', name_lo: '' })
const addSection = () => run(() => api(`/shops/${shopId.value}/restaurant/sections`, { method: 'POST', body: newSection })).then((ok) => ok && Object.assign(newSection, { name: '', name_lo: '' }))
const renameSection = (id: string, name: string, name_lo: string) => run(() => api(`/restaurant/sections/${id}`, { method: 'PATCH', body: { name, name_lo } }))
const removeSection = (id: string) => run(() => api(`/restaurant/sections/${id}`, { method: 'DELETE' }))

// dishes
const editing = ref<string | null>(null) // item id or 'new'
const form = reactive({ name: '', name_lo: '', description: '', price: '' as string | number, section_id: '' as string, image_url: '', spicy: 0, available: true })
function openItem(i?: MenuItem, section?: string) {
  editing.value = i?.id ?? 'new'
  Object.assign(form, i
    ? { name: i.name, name_lo: i.name_lo, description: i.description, price: i.price_cents / 100, section_id: i.section_id ?? '', image_url: i.image_url, spicy: i.spicy, available: i.available }
    : { name: '', name_lo: '', description: '', price: '', section_id: section ?? data.value?.sections[0]?.id ?? '', image_url: '', spicy: 0, available: true })
}
async function saveItem() {
  const body = { ...form, price_cents: Math.round(Number(form.price || 0) * 100), section_id: form.section_id || null, price: undefined }
  const ok = await run(() =>
    editing.value === 'new'
      ? api(`/shops/${shopId.value}/restaurant/items`, { method: 'POST', body })
      : api(`/restaurant/items/${editing.value}`, { method: 'PATCH', body }),
  )
  if (ok) editing.value = null
}
const toggle = (i: MenuItem) => run(() => api(`/restaurant/items/${i.id}`, { method: 'PATCH', body: { available: !i.available } }))
const removeItem = (i: MenuItem) => run(() => api(`/restaurant/items/${i.id}`, { method: 'DELETE' }))
const groups = computed(() => {
  const d = data.value
  if (!d) return []
  const g = d.sections.map((s) => ({ section: s, items: d.items.filter((i) => i.section_id === s.id) }))
  const loose = d.items.filter((i) => !i.section_id)
  return loose.length ? [...g, { section: null, items: loose }] : g
})
const modalOpen = computed({ get: () => !!editing.value, set: (v) => !v && (editing.value = null) })
</script>

<template>
  <div v-if="data">
    <div class="flex flex-wrap items-end justify-between gap-3">
      <div>
        <h1 class="page-title">{{ $t('restaurant.menuEditor.title') }}</h1>
        <p class="text-sm text-muted">{{ $t('restaurant.menuEditor.subtitle') }}</p>
      </div>
      <div class="flex gap-2">
        <UButton color="neutral" variant="soft" :to="`/s/${data.shop.slug}`" target="_blank" icon="i-lucide-external-link">{{ $t('restaurant.board.openMenu') }}</UButton>
        <UButton icon="i-lucide-plus" @click="openItem()">{{ $t('restaurant.menuEditor.addDish') }}</UButton>
      </div>
    </div>
    <p v-if="error && !editing" class="mt-3 text-sm text-brand-700" role="alert">{{ error }}</p>

    <form class="card mt-6 flex flex-wrap items-end gap-3 p-4" @submit.prevent="addSection">
      <div class="min-w-40 flex-1"><label class="label" for="sec-name">{{ $t('restaurant.menuEditor.newSection') }}</label><UInput id="sec-name" v-model="newSection.name" required maxlength="80" :placeholder="$t('restaurant.menuEditor.sectionPlaceholder')" /></div>
      <div class="min-w-40 flex-1"><label class="label" for="sec-lo">{{ $t('restaurant.menuEditor.nameLo') }}</label><UInput id="sec-lo" v-model="newSection.name_lo" maxlength="80" /></div>
      <UButton type="submit" color="neutral">{{ $t('restaurant.menuEditor.addSection') }}</UButton>
    </form>

    <EmptyState v-if="!data.items.length && !data.sections.length" class="mt-6" :title="$t('restaurant.menuEditor.emptyTitle')" :text="$t('restaurant.menuEditor.emptyText')" />
    <section v-for="g in groups" :key="g.section?.id ?? 'none'" class="card mt-4 p-4">
      <div class="flex flex-wrap items-center gap-2">
        <template v-if="g.section">
          <input :value="g.section.name" class="min-w-0 flex-1 rounded-lg bg-transparent text-lg font-bold outline-none focus:bg-[var(--field)]" :aria-label="$t('common.name')" @change="renameSection(g.section.id, ($event.target as HTMLInputElement).value, g.section.name_lo)">
          <input :value="g.section.name_lo" class="w-40 rounded-lg bg-transparent text-sm text-muted outline-none focus:bg-[var(--field)]" :placeholder="$t('restaurant.menuEditor.nameLo')" :aria-label="$t('restaurant.menuEditor.nameLo')" @change="renameSection(g.section.id, g.section.name, ($event.target as HTMLInputElement).value)">
          <UButton size="sm" color="neutral" variant="soft" icon="i-lucide-plus" @click="openItem(undefined, g.section.id)">{{ $t('restaurant.menuEditor.dish') }}</UButton>
          <UButton size="sm" color="neutral" variant="ghost" icon="i-lucide-trash-2" :aria-label="$t('common.delete')" @click="removeSection(g.section.id)" />
        </template>
        <h2 v-else class="text-lg font-bold">{{ $t('restaurant.menu.other') }}</h2>
      </div>
      <ul class="mt-3 divide-y divide-line">
        <li v-for="i in g.items" :key="i.id" class="flex items-center gap-3 py-2.5">
          <ProductThumb :src="i.image_url || null" :name="i.name" class="size-12 shrink-0" rounded="rounded-xl" />
          <button type="button" class="min-w-0 flex-1 text-left" @click="openItem(i)">
            <div class="truncate font-semibold" :class="!i.available && 'text-muted line-through'">{{ localName(i, locale) }}</div>
            <div class="truncate text-xs text-muted">{{ money(i.price_cents, cur) }}<template v-if="i.spicy"> · {{ '🌶'.repeat(i.spicy) }}</template></div>
          </button>
          <USwitch :model-value="i.available" :label="$t('restaurant.menuEditor.available')" @update:model-value="toggle(i)" />
          <UButton size="sm" color="neutral" variant="ghost" icon="i-lucide-trash-2" :aria-label="$t('common.delete')" @click="removeItem(i)" />
        </li>
        <li v-if="!g.items.length" class="py-3 text-sm text-muted">{{ $t('restaurant.menuEditor.noDishes') }}</li>
      </ul>
    </section>

    <UModal v-model:open="modalOpen" :title="editing === 'new' ? $t('restaurant.menuEditor.addDish') : $t('restaurant.menuEditor.editDish')">
      <template #body>
        <form class="space-y-3" @submit.prevent="saveItem">
          <div class="grid gap-3 sm:grid-cols-2">
            <div><label class="label" for="it-name">{{ $t('common.name') }}</label><UInput id="it-name" v-model="form.name" required maxlength="80" /></div>
            <div><label class="label" for="it-lo">{{ $t('restaurant.menuEditor.nameLo') }}</label><UInput id="it-lo" v-model="form.name_lo" maxlength="80" /></div>
          </div>
          <div><label class="label" for="it-desc">{{ $t('common.description') }}</label><UTextarea id="it-desc" v-model="form.description" :rows="2" maxlength="500" /></div>
          <div class="grid gap-3 sm:grid-cols-2">
            <div><label class="label" for="it-price">{{ $t('restaurant.menuEditor.price', { cur }) }}</label><UInput id="it-price" v-model.number="form.price" type="number" min="0" step="any" required /></div>
            <div>
              <label class="label" for="it-sec">{{ $t('restaurant.menuEditor.section') }}</label>
              <select id="it-sec" v-model="form.section_id" class="input">
                <option value="">{{ $t('restaurant.menu.other') }}</option>
                <option v-for="s in data.sections" :key="s.id" :value="s.id">{{ localName(s, locale) }}</option>
              </select>
            </div>
          </div>
          <div><label class="label" for="it-img">{{ $t('restaurant.menuEditor.image') }}</label><UInput id="it-img" v-model="form.image_url" type="url" placeholder="https://…" /></div>
          <div class="flex flex-wrap items-center gap-4">
            <div>
              <span class="label">{{ $t('restaurant.menu.spicy') }}</span>
              <div class="flex gap-1">
                <button v-for="n in [0, 1, 2, 3]" :key="n" type="button" class="rounded-lg px-2.5 py-1 text-sm" :class="form.spicy === n ? 'bg-brand-500 text-white' : 'bg-[var(--field)]'" @click="form.spicy = n">{{ n ? '🌶'.repeat(n) : '–' }}</button>
              </div>
            </div>
            <USwitch v-model="form.available" :label="$t('restaurant.menuEditor.available')" />
          </div>
          <p v-if="error" class="text-sm text-brand-700" role="alert">{{ error }}</p>
          <UButton type="submit" block>{{ $t('common.save') }}</UButton>
        </form>
      </template>
    </UModal>
  </div>
</template>
