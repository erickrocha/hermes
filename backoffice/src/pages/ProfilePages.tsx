import { useEffect, useMemo, useState, type FormEvent } from 'react'
import { Pencil, Plus } from 'lucide-react'
import { Link, useNavigate, useParams } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { api, apiMessage } from '../api'
import type { Page } from '../types'
import { Empty, Form, Loading, PageHeader } from '../components/UI'

// C-032 (D-27, HRMS-851/852/862/863): the tenant owner's access profiles -- a named set of permissions
// for a tenant user, a driver or a mechanic -- and the profile of each user.
export interface AccessProfile { uuid?: string; name: string; baseRole: 'TenantUser' | 'Driver' | 'Mechanic'; active: boolean; permissions: string[] }
interface Permission { code: string; module: string; own: boolean; defaultFor: string[] }
interface Effective { role: string; profileUuid?: string; profileName?: string; permissions: string[] }

export const profileRoles: AccessProfile['baseRole'][] = ['TenantUser', 'Driver', 'Mechanic']
const blank: AccessProfile = { name: '', baseRole: 'Mechanic', active: true, permissions: [] }

/** Permissions grouped by module, in catalogue order; the form shows one group per module. */
export const groupByModule = (permissions: Permission[]) => permissions.reduce<Record<string, Permission[]>>((groups, p) => ({ ...groups, [p.module]: [...(groups[p.module] ?? []), p] }), {})

export function ProfilesPage() {
  const { t } = useTranslation(); const [items, setItems] = useState<AccessProfile[] | null>(null); const [error, setError] = useState('')
  useEffect(() => { api.get<Page<AccessProfile>>('/access-profile?page=0&pageSize=200').then(({ data }) => setItems(data.items)).catch((cause) => setError(apiMessage(cause, t('loadError')))) }, [t])
  return <><PageHeader title={t('profiles')} subtitle={t('profilesSummary')} actions={<Link className="btn primary" to="/profiles/new"><Plus size={17} />{t('newProfile')}</Link>} />
    {error && <p className="form-error">{error}</p>}
    {!items ? <Loading /> : !items.length ? <Empty /> : <div className="table-card"><table><thead><tr><th>{t('name')}</th><th>{t('role')}</th><th>{t('permissions')}</th><th>{t('status')}</th><th /></tr></thead>
      <tbody>{items.map((p) => <tr key={p.uuid}><td>{p.name}</td><td><span className="badge blue">{t(`role${p.baseRole}`)}</span></td><td>{p.permissions.length}</td><td>{p.active ? t('active') : t('inactive')}</td><td><Link to={`/profiles/${p.uuid}/edit`}><Pencil size={15} />{t('edit')}</Link></td></tr>)}</tbody></table></div>}</>
}

export function ProfileEditorPage() {
  const { uuid } = useParams(); const navigate = useNavigate(); const { t } = useTranslation()
  const [form, setForm] = useState<AccessProfile>({ ...blank }); const [catalogue, setCatalogue] = useState<Permission[]>([]); const [loading, setLoading] = useState(true); const [saving, setSaving] = useState(false); const [error, setError] = useState('')
  useEffect(() => {
    Promise.all([api.get<Permission[]>('/access-profile/catalogue'), uuid ? api.get<AccessProfile>(`/access-profile/uuid/${uuid}`) : Promise.resolve(null)])
      .then(([cat, profile]) => { setCatalogue(cat.data); if (profile) setForm(profile.data) }).catch((cause) => setError(apiMessage(cause, t('loadError')))).finally(() => setLoading(false))
  }, [uuid, t])
  const groups = useMemo(() => groupByModule(catalogue), [catalogue])
  const held = new Set(form.permissions)
  const toggle = (code: string) => setForm({ ...form, permissions: held.has(code) ? form.permissions.filter((c) => c !== code) : [...form.permissions, code] })
  // the role's built-in default is the starting point: a profile is "this role, minus/plus a few things"
  const startFromDefault = () => setForm({ ...form, permissions: catalogue.filter((p) => p.defaultFor.includes(form.baseRole)).map((p) => p.code) })
  const save = async (event: FormEvent) => { event.preventDefault(); setSaving(true); setError(''); try { if (uuid) await api.put(`/access-profile/uuid/${uuid}`, form); else await api.post('/access-profile', form); navigate('/profiles') } catch (cause) { setError(apiMessage(cause, t('genericError'))) } finally { setSaving(false) } }
  if (loading) return <Loading />
  return <><PageHeader title={uuid ? t('editProfile') : t('newProfile')} /><section className="page-form-card"><Form onSubmit={save} error={error} saving={saving} onCancel={() => navigate('/profiles')}>
    <label className="span-2">{t('name')}<input required value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} /></label>
    <label>{t('role')}<select value={form.baseRole} onChange={(e) => setForm({ ...form, baseRole: e.target.value as AccessProfile['baseRole'] })}>{profileRoles.map((r) => <option key={r} value={r}>{t(`role${r}`)}</option>)}</select></label>
    <label className="checkbox"><input type="checkbox" checked={form.active} onChange={(e) => setForm({ ...form, active: e.target.checked })} />{t('active')}</label>
    <div className="span-2"><button type="button" className="btn secondary" onClick={startFromDefault}>{t('startFromDefault')}</button></div>
    {Object.entries(groups).map(([module, perms]) => <fieldset className="span-2 permission-group" key={module}><legend>{module}</legend>
      {perms.map((p) => <label className="checkbox" key={p.code}><input type="checkbox" checked={held.has(p.code)} onChange={() => toggle(p.code)} />{p.code}{p.own ? ` · ${t('ownOnly')}` : ''}</label>)}</fieldset>)}
  </Form></section></>
}

/** On a user's form: which profile the user holds, changeable by the owner. Hidden for roles that take none. */
export function UserProfilePicker({ userUuid, role }: { userUuid: string; role: string }) {
  const { t } = useTranslation(); const [profiles, setProfiles] = useState<AccessProfile[]>([]); const [current, setCurrent] = useState(''); const [message, setMessage] = useState(''); const eligible = (profileRoles as string[]).includes(role)
  useEffect(() => {
    if (!eligible) return
    Promise.all([api.get<Page<AccessProfile>>('/access-profile?page=0&pageSize=200'), api.get<Effective>(`/access-profile/user/uuid/${userUuid}`)])
      .then(([list, held]) => { setProfiles(list.data.items); setCurrent(held.data.profileUuid ?? '') }).catch(() => setMessage(t('loadError')))
  }, [userUuid, eligible, t])
  if (!eligible) return null
  const change = async (profileUuid: string) => { setMessage(''); try { await api.put(`/access-profile/user/uuid/${userUuid}`, { profileUuid: profileUuid || null }); setCurrent(profileUuid); setMessage(t('profileSaved')) } catch (cause) { setMessage(apiMessage(cause, t('genericError'))) } }
  return <label className="span-2">{t('profile')}<select value={current} onChange={(e) => change(e.target.value)}>
    <option value="">{t('roleDefault')}</option>{profiles.filter((p) => p.active && p.baseRole === role).map((p) => <option key={p.uuid} value={p.uuid}>{p.name}</option>)}</select>{message && <small>{message}</small>}</label>
}
