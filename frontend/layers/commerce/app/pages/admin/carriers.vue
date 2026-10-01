<script setup lang="ts">
import type { Carrier } from '~/utils/types'

definePageMeta({ layout: 'admin', middleware: 'admin' })
const api = useApi()
const { t } = useI18n()
const { refresh: refreshPublic } = useCarriers()
const { data: list, refresh } = await useAsyncData('admin-carriers', () => api<Carrier[]>('/admin/carriers'), { default: () => [] as Carrier[] })

type Editable = Omit<Carrier, 'code'>
const fields: (keyof Editable)[] = ['name', 'name_lo', 'website', 'tracking_url', 'phone', 'supports_cod', 'active', 'sort']
const rows = reactive<Record<string, Editable>>({})
watch(list, (l) => {
  for (const c of l) rows[c.code] = { name: c.name, name_lo: c.name_lo, website: c.website, tracking_url: c.tracking_url, phone: c.phone, supports_cod: c.supports_cod, active: c.active, sort: c.sort }
}, { immediate: true })
const dirty = (c: Carrier) => !!rows[c.code] && fields.some((f) => rows[c.code]![f] !== c[f])

const error = ref('')
const msg = ref('')
const savingCode = ref('')
async function save(c: Carrier) {
  const r = rows[c.code]
  if (!r) return
  error.value = msg.value = ''
  savingCode.value = c.code
  try {
    const body = Object.fromEntries(fields.filter((f) => r[f] !== c[f]).map((f) => [f, f === 'sort' ? Number(r[f]) || 0 : r[f]]))
    const updated = await api<Carrier>(`/admin/carriers/${c.code}`, { method: 'PATCH', body })
    const i = list.value.findIndex((x) => x.code === c.code)
    if (i >= 0) list.value.splice(i, 1, updated)
    msg.value = t('common.saved')
    refreshPublic()
  } catch (e) {
    error.value = apiError(e)
  } finally {
    savingCode.value = ''
  }
}

const blank = () => ({ code: '', name: '', name_lo: '', website: '', tracking_url: '', phone: '', supports_cod: true, active: true, sort: 100 })
const nf = reactive(blank())
const codeOk = computed(() => /^[a-z0-9_]+$/.test(nf.code))
const addError = ref('')
const addMsg = ref('')
async function add() {
  addError.value = addMsg.value = ''
  if (!codeOk.value) {
    addError.value = t('ship.admin.codeInvalid')
    return
  }
  try {
    await api<Carrier>('/admin/carriers', { method: 'POST', body: { ...nf, sort: Number(nf.sort) || 0 } })
    Object.assign(nf, blank())
    addMsg.value = t('ship.admin.added')
    await refresh()
    refreshPublic()
  } catch (e) {
    addError.value = apiError(e)
  }
}
</script>

<template>
  <div>
    <h1 class="page-title">{{ $t('ship.admin.title') }}</h1>
    <p class="mt-1 max-w-3xl text-sm text-muted">{{ $t('ship.admin.intro') }}</p>
    <p class="mt-2 max-w-3xl rounded-xl bg-paper p-3 text-xs text-muted">{{ $t('ship.admin.trackingHint') }}</p>

    <p v-if="error" class="mt-3 text-sm text-brand-700">{{ error }}</p>
    <p v-if="msg" class="mt-3 text-sm text-mint-500">{{ msg }}</p>
    <div class="card mt-4 overflow-x-auto">
      <table class="table min-w-[1100px]">
        <thead>
          <tr>
            <th class="text-left">{{ $t('ship.admin.cols.code') }}</th>
            <th class="text-left">{{ $t('ship.admin.cols.name') }}</th>
            <th class="text-left">{{ $t('ship.admin.cols.nameLo') }}</th>
            <th class="text-left">{{ $t('ship.admin.cols.website') }}</th>
            <th class="text-left">{{ $t('ship.admin.cols.trackingUrl') }}</th>
            <th class="text-left">{{ $t('ship.admin.cols.phone') }}</th>
            <th>{{ $t('ship.admin.cols.cod') }}</th>
            <th>{{ $t('ship.admin.cols.active') }}</th>
            <th class="w-20">{{ $t('ship.admin.cols.sort') }}</th>
            <th />
          </tr>
        </thead>
        <tbody>
          <tr v-for="c in list" :key="c.code" :class="!c.active && 'opacity-60'">
            <template v-if="rows[c.code]">
              <td class="font-mono text-xs font-bold">{{ c.code }}</td>
              <td><UInput size="md" v-model="rows[c.code]!.name" :aria-label="$t('ship.admin.cols.name')" /></td>
              <td><UInput size="md" v-model="rows[c.code]!.name_lo" :aria-label="$t('ship.admin.cols.nameLo')" /></td>
              <td><UInput size="md" v-model="rows[c.code]!.website" type="url" placeholder="https://" :aria-label="$t('ship.admin.cols.website')" /></td>
              <td><UInput size="sm" v-model="rows[c.code]!.tracking_url" class="font-mono" :placeholder="$t('ship.admin.trackingPlaceholder')" :aria-label="$t('ship.admin.cols.trackingUrl')" /></td>
              <td><UInput size="md" v-model="rows[c.code]!.phone" :aria-label="$t('ship.admin.cols.phone')" /></td>
              <td class="text-center"><input v-model="rows[c.code]!.supports_cod" type="checkbox" class="size-4 accent-brand-500" :aria-label="$t('ship.admin.cols.cod')"></td>
              <td class="text-center"><input v-model="rows[c.code]!.active" type="checkbox" class="size-4 accent-brand-500" :aria-label="$t('ship.admin.cols.active')"></td>
              <td><UInput size="md" v-model.number="rows[c.code]!.sort" type="number" :aria-label="$t('ship.admin.cols.sort')" /></td>
              <td><UButton size="sm" type="submit" :disabled="!dirty(c) || savingCode === c.code" @click="save(c)">{{ savingCode === c.code ? $t('common.saving') : $t('common.save') }}</UButton></td>
            </template>
          </tr>
        </tbody>
      </table>
    </div>

    <form class="card mt-6 max-w-3xl space-y-4 p-5 sm:p-6" @submit.prevent="add">
      <div class="font-bold">{{ $t('ship.admin.add') }}</div>
      <div class="grid gap-3 sm:grid-cols-3">
        <div>
          <label class="label">{{ $t('ship.admin.code') }}</label>
          <UInput v-model.trim="nf.code" class="font-mono" required maxlength="40" pattern="[a-z0-9_]+" placeholder="anousith" />
          <p class="mt-1 text-xs text-muted">{{ $t('ship.admin.codeHint') }}</p>
        </div>
        <div><label class="label">{{ $t('ship.admin.cols.name') }}</label><UInput v-model="nf.name" required /></div>
        <div><label class="label">{{ $t('ship.admin.cols.nameLo') }}</label><UInput v-model="nf.name_lo" /></div>
        <div><label class="label">{{ $t('ship.admin.cols.website') }}</label><UInput v-model="nf.website" type="url" placeholder="https://" /></div>
        <div class="sm:col-span-2"><label class="label">{{ $t('ship.admin.cols.trackingUrl') }}</label><UInput size="sm" v-model="nf.tracking_url" class="font-mono" :placeholder="$t('ship.admin.trackingPlaceholder')" /></div>
        <div><label class="label">{{ $t('ship.admin.cols.phone') }}</label><UInput v-model="nf.phone" /></div>
        <div><label class="label">{{ $t('ship.admin.cols.sort') }}</label><UInput v-model.number="nf.sort" type="number" /></div>
        <div class="flex flex-col justify-end gap-2 text-sm">
          <label class="flex items-center gap-2"><input v-model="nf.supports_cod" type="checkbox" class="size-4 accent-brand-500"> {{ $t('ship.admin.supportsCod') }}</label>
          <label class="flex items-center gap-2"><input v-model="nf.active" type="checkbox" class="size-4 accent-brand-500"> {{ $t('ship.admin.cols.active') }}</label>
        </div>
      </div>
      <div class="flex flex-wrap items-center gap-3">
        <UButton type="submit">{{ $t('ship.admin.add') }}</UButton>
        <span v-if="addError" class="text-sm text-brand-700">{{ addError }}</span>
        <span v-if="addMsg" class="text-sm text-mint-500">{{ addMsg }}</span>
      </div>
    </form>
  </div>
</template>
