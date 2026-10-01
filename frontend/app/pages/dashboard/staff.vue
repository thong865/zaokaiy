<script setup lang="ts">
definePageMeta({ layout: 'dashboard', middleware: 'seller' })
const api = useApi()
const { t } = useI18n()
const { shopId, shop } = useShop()

interface Role { id: string; name: string; permissions: string[]; template: string | null; members: number }
interface Member { user_id: string; display_name: string; email: string | null; phone: string | null; role_id: string; role: string; active: boolean; created_at: string; last_seen_at: string | null }
interface Invite { id: string; role_id: string; role: string; note: string; created_at: string; expires_at: string; accepted_at: string | null; accepted_name: string | null; revoked_at: string | null }
interface StaffDoc { roles: Role[]; members: Member[]; invites: Invite[]; permissions: string[] }

const { data, error: loadError } = await useAsyncData('staff', () => api<StaffDoc>(`/shops/${shopId.value}/staff`), {
  watch: [shopId],
  default: (): StaffDoc => ({ roles: [], members: [], invites: [], permissions: [] }),
})
const msg = ref('')
const error = ref('')
async function run(fn: () => Promise<StaffDoc>, ok = '') {
  msg.value = error.value = ''
  try {
    data.value = await fn()
    msg.value = ok
    return true
  } catch (e) {
    error.value = apiError(e)
    return false
  }
}

// ---- members
const setRole = (m: Member, role_id: string) => run(() => api(`/shops/${shopId.value}/staff/${m.user_id}`, { method: 'PATCH', body: { role_id } }), t('common.saved'))
const setActive = (m: Member, active: boolean) => run(() => api(`/shops/${shopId.value}/staff/${m.user_id}`, { method: 'PATCH', body: { active } }), t('common.saved'))
async function removeMember(m: Member) {
  if (!confirm(t('staff.removeConfirm', { name: m.display_name }))) return
  await run(() => api(`/shops/${shopId.value}/staff/${m.user_id}`, { method: 'DELETE' }))
}

// ---- invites
const inv = reactive({ role_id: '', note: '', days: 7 })
watch(() => data.value.roles, (r) => { if (!inv.role_id) inv.role_id = r.find((x) => x.template === 'cashier')?.id ?? r[0]?.id ?? '' }, { immediate: true })
const created = ref<{ link: string; role: string; note: string } | null>(null)
const copied = ref(false)
async function invite() {
  msg.value = error.value = ''
  try {
    const r = await api<{ link: string; staff: StaffDoc }>(`/shops/${shopId.value}/staff/invites`, { method: 'POST', body: { role_id: inv.role_id, note: inv.note, days: Number(inv.days) } })
    data.value = r.staff
    created.value = { link: r.link, role: data.value.roles.find((x) => x.id === inv.role_id)?.name ?? '', note: inv.note }
    inv.note = ''
    copied.value = false
  } catch (e) {
    error.value = apiError(e)
  }
}
async function copyLink() {
  if (!created.value) return
  try {
    await navigator.clipboard.writeText(created.value.link)
    copied.value = true
  } catch {}
}
const waLink = computed(() => (created.value ? `https://wa.me/?text=${encodeURIComponent(t('staff.inviteMessage', { shop: shop.value?.name ?? '', role: created.value.role, link: created.value.link }))}` : ''))
const inviteState = (i: Invite) => (i.revoked_at ? 'revoked' : i.accepted_at ? 'used' : new Date(i.expires_at).getTime() < Date.now() ? 'expired' : 'open')
const revoke = (i: Invite) => run(() => api(`/staff-invites/${i.id}`, { method: 'DELETE' }))

// ---- roles
const editing = ref<Record<string, { name: string; permissions: string[] }>>({})
function edit(r: Role) {
  editing.value = { ...editing.value, [r.id]: { name: r.name, permissions: [...r.permissions] } }
}
function toggle(list: string[], p: string) {
  const i = list.indexOf(p)
  if (i >= 0) list.splice(i, 1)
  else list.push(p)
}
async function saveRole(r: Role) {
  const e = editing.value[r.id]
  if (!e) return
  if (await run(() => api(`/shop-roles/${r.id}`, { method: 'PATCH', body: e }), t('common.saved'))) {
    const { [r.id]: _, ...rest } = editing.value
    editing.value = rest
  }
}
async function deleteRole(r: Role) {
  if (!confirm(t('staff.deleteRoleConfirm', { name: r.name }))) return
  await run(() => api(`/shop-roles/${r.id}`, { method: 'DELETE' }))
}
const newRole = reactive({ open: false, name: '', permissions: [] as string[] })
async function createRole() {
  if (await run(() => api(`/shops/${shopId.value}/staff/roles`, { method: 'POST', body: { name: newRole.name, permissions: newRole.permissions } }), t('common.saved'))) {
    Object.assign(newRole, { open: false, name: '', permissions: [] })
  }
}
const permLabel = (p: string) => t(`staff.perm.${p}`)
</script>

<template>
  <div class="max-w-5xl">
    <h1 class="page-title">{{ $t('staff.title') }}</h1>
    <p class="mt-1 max-w-3xl text-sm text-muted">{{ $t('staff.intro') }}</p>
    <p v-if="loadError" class="mt-3 text-sm text-brand-700">{{ apiError(loadError) }}</p>
    <p v-if="error" class="mt-3 text-sm text-brand-700">{{ error }}</p>
    <p v-if="msg" class="mt-3 text-sm text-mint-500">{{ msg }}</p>

    <div class="mt-5 grid gap-5 lg:grid-cols-[1fr_22rem]">
      <!-- Members -->
      <section class="card overflow-x-auto">
        <div class="flex items-center justify-between border-b border-line px-4 py-3">
          <h2 class="font-bold">{{ $t('staff.members') }} <span class="font-normal text-muted">({{ data.members.length }})</span></h2>
        </div>
        <table v-if="data.members.length" class="table">
          <thead><tr><th>{{ $t('staff.cols.person') }}</th><th>{{ $t('staff.cols.role') }}</th><th>{{ $t('staff.cols.lastSeen') }}</th><th /></tr></thead>
          <tbody>
            <tr v-for="m in data.members" :key="m.user_id" :class="!m.active && 'opacity-60'">
              <td>
                <div class="font-semibold">{{ m.display_name }}</div>
                <div class="text-xs text-muted">{{ m.email || m.phone || '—' }}</div>
                <UBadge v-if="!m.active" color="neutral" variant="soft" size="sm" class="mt-1">{{ $t('staff.suspended') }}</UBadge>
              </td>
              <td>
                <select class="input w-44 py-1.5 text-sm" :value="m.role_id" :aria-label="$t('staff.cols.role')" @change="setRole(m, ($event.target as HTMLSelectElement).value)">
                  <option v-for="r in data.roles" :key="r.id" :value="r.id">{{ r.name }}</option>
                </select>
              </td>
              <td class="text-xs text-muted">{{ m.last_seen_at ? ago(m.last_seen_at) : $t('staff.never') }}<div>{{ $t('staff.joined', { date: fmtDate(m.created_at) }) }}</div></td>
              <td class="whitespace-nowrap text-right">
                <UButton size="xs" color="neutral" variant="ghost" :icon="m.active ? 'i-lucide-pause' : 'i-lucide-play'" @click="setActive(m, !m.active)">{{ m.active ? $t('staff.suspend') : $t('staff.restore') }}</UButton>
                <UButton size="xs" color="error" variant="ghost" icon="i-lucide-user-minus" :aria-label="$t('staff.remove')" @click="removeMember(m)" />
              </td>
            </tr>
          </tbody>
        </table>
        <EmptyState v-else class="!shadow-none" icon="i-lucide-users-round" :title="$t('staff.noMembers')" :text="$t('staff.noMembersText')" />
      </section>

      <!-- Invite -->
      <aside class="space-y-4">
        <form class="card space-y-3 p-4" @submit.prevent="invite">
          <h2 class="font-bold">{{ $t('staff.invite') }}</h2>
          <p class="text-xs text-muted">{{ $t('staff.inviteHelp') }}</p>
          <div>
            <label class="label" for="inv-role">{{ $t('staff.cols.role') }}</label>
            <select id="inv-role" v-model="inv.role_id" class="input" required>
              <option v-for="r in data.roles" :key="r.id" :value="r.id">{{ r.name }}</option>
            </select>
          </div>
          <div><label class="label" for="inv-note">{{ $t('staff.inviteNote') }}</label><UInput id="inv-note" v-model="inv.note" maxlength="120" class="w-full" :placeholder="$t('staff.inviteNotePh')" /></div>
          <div>
            <label class="label" for="inv-days">{{ $t('staff.inviteDays') }}</label>
            <select id="inv-days" v-model.number="inv.days" class="input"><option v-for="d in [1, 3, 7, 14, 30]" :key="d" :value="d">{{ $t('staff.days', { n: d }, d) }}</option></select>
          </div>
          <UButton type="submit" block icon="i-lucide-link" :disabled="!inv.role_id">{{ $t('staff.createLink') }}</UButton>
        </form>

        <div v-if="created" class="card space-y-2 border-mint-500/50 p-4">
          <div class="text-sm font-bold">{{ $t('staff.linkReady', { role: created.role }) }}</div>
          <div class="break-all rounded-xl bg-[var(--field)] p-2 font-mono text-xs">{{ created.link }}</div>
          <p class="text-xs text-muted">{{ $t('staff.linkOnce') }}</p>
          <div class="flex gap-2">
            <UButton size="sm" color="neutral" :icon="copied ? 'i-lucide-check' : 'i-lucide-copy'" @click="copyLink">{{ copied ? $t('common.copied') : $t('common.copy') }}</UButton>
            <UButton size="sm" color="success" icon="i-lucide-message-circle" :to="waLink" target="_blank">WhatsApp</UButton>
          </div>
        </div>

        <div v-if="data.invites.length" class="card p-4">
          <h3 class="text-sm font-bold">{{ $t('staff.recentInvites') }}</h3>
          <ul class="mt-2 divide-y divide-line/70 text-sm">
            <li v-for="i in data.invites" :key="i.id" class="flex items-center gap-2 py-2">
              <div class="min-w-0 flex-1">
                <div class="truncate">{{ i.role }}<span v-if="i.note" class="text-muted"> · {{ i.note }}</span></div>
                <div class="text-[11px] text-muted">
                  <template v-if="inviteState(i) === 'used'">{{ $t('staff.inviteUsed', { name: i.accepted_name ?? '—' }) }}</template>
                  <template v-else-if="inviteState(i) === 'open'">{{ $t('staff.inviteOpen', { date: fmtDate(i.expires_at) }) }}</template>
                  <template v-else>{{ $t(`staff.inviteState.${inviteState(i)}`) }}</template>
                </div>
              </div>
              <UButton v-if="inviteState(i) === 'open'" size="xs" color="neutral" variant="ghost" @click="revoke(i)">{{ $t('staff.revoke') }}</UButton>
            </li>
          </ul>
        </div>
      </aside>
    </div>

    <!-- Roles -->
    <section class="mt-6">
      <div class="flex items-center justify-between">
        <div>
          <h2 class="text-lg font-bold">{{ $t('staff.roles') }}</h2>
          <p class="text-xs text-muted">{{ $t('staff.rolesHelp') }}</p>
        </div>
        <UButton size="sm" color="neutral" variant="soft" icon="i-lucide-plus" @click="newRole.open = !newRole.open">{{ $t('staff.newRole') }}</UButton>
      </div>

      <form v-if="newRole.open" class="card mt-3 space-y-3 p-4" @submit.prevent="createRole">
        <UInput v-model="newRole.name" maxlength="60" required :placeholder="$t('staff.roleNamePh')" :aria-label="$t('staff.roleName')" />
        <div class="grid gap-1.5 sm:grid-cols-2 lg:grid-cols-3">
          <label v-for="p in data.permissions" :key="p" class="flex cursor-pointer items-start gap-2 rounded-xl border border-line px-3 py-2 text-sm">
            <input type="checkbox" class="mt-0.5 size-4 accent-brand-500" :checked="newRole.permissions.includes(p)" @change="toggle(newRole.permissions, p)">
            <span><span class="font-semibold">{{ permLabel(p) }}</span><span class="block text-xs text-muted">{{ $t(`staff.permHelp.${p}`) }}</span></span>
          </label>
        </div>
        <div class="flex justify-end gap-2"><UButton color="neutral" variant="soft" @click="newRole.open = false">{{ $t('common.cancel') }}</UButton><UButton type="submit">{{ $t('common.create') }}</UButton></div>
      </form>

      <div class="mt-3 grid gap-3 md:grid-cols-2">
        <div v-for="r in data.roles" :key="r.id" class="card p-4">
          <template v-if="editing[r.id]">
            <UInput v-model="editing[r.id]!.name" maxlength="60" class="w-full" :aria-label="$t('staff.roleName')" />
            <div class="mt-3 grid gap-1.5 sm:grid-cols-2">
              <label v-for="p in data.permissions" :key="p" class="flex cursor-pointer items-center gap-2 text-sm">
                <input type="checkbox" class="size-4 accent-brand-500" :checked="editing[r.id]!.permissions.includes(p)" @change="toggle(editing[r.id]!.permissions, p)">
                {{ permLabel(p) }}
              </label>
            </div>
            <div class="mt-3 flex justify-end gap-2">
              <UButton size="sm" color="neutral" variant="soft" @click="editing = Object.fromEntries(Object.entries(editing).filter(([k]) => k !== r.id))">{{ $t('common.cancel') }}</UButton>
              <UButton size="sm" @click="saveRole(r)">{{ $t('common.save') }}</UButton>
            </div>
          </template>
          <template v-else>
            <div class="flex items-start justify-between gap-2">
              <div>
                <div class="font-bold">{{ r.name }}</div>
                <div class="text-xs text-muted">{{ $t('staff.roleMembers', { n: r.members }, r.members) }}</div>
              </div>
              <div class="flex gap-1">
                <UButton size="xs" color="neutral" variant="ghost" icon="i-lucide-pencil" :aria-label="$t('common.edit')" @click="edit(r)" />
                <UButton size="xs" color="error" variant="ghost" icon="i-lucide-trash-2" :aria-label="$t('common.delete')" :disabled="r.members > 0" @click="deleteRole(r)" />
              </div>
            </div>
            <div class="mt-2 flex flex-wrap gap-1">
              <span v-for="p in r.permissions" :key="p" class="chip bg-[var(--field)]">{{ permLabel(p) }}</span>
              <span v-if="!r.permissions.length" class="text-xs text-muted">{{ $t('staff.noPermissions') }}</span>
            </div>
          </template>
        </div>
      </div>
      <p class="mt-3 text-xs text-muted">{{ $t('staff.ownerOnly') }}</p>
    </section>
  </div>
</template>
