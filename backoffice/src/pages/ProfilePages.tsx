import { useEffect, useMemo, useState } from 'react'
import { Controller, useForm } from 'react-hook-form'
import { yupResolver } from '@hookform/resolvers/yup'
import * as yup from 'yup'
import { Pencil, Plus } from 'lucide-react'
import { Link, useNavigate, useParams } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { useDispatch } from 'react-redux'
import type { AppDispatch } from '../store'
import { fetchProfileEditor, fetchProfiles, fetchUserProfile, saveAccessProfile, saveUserProfile } from '../features/profileSlice'
import type { AccessProfile, Permission } from '../types'
import { Empty, Form, Loading, PageHeader } from '../components/UI'

// C-032 (D-27, HRMS-851/852/862/863): the tenant owner's access profiles -- a named set of permissions
// for a tenant user, a driver or a mechanic -- and the profile of each user.
export const profileRoles: AccessProfile['baseRole'][] = ['TenantUser', 'Driver', 'Mechanic']
const blank: AccessProfile = { name: '', baseRole: 'Mechanic', active: true, permissions: [] }

/** Permissions grouped by module, in catalogue order; the form shows one group per module. */
export const groupByModule = (permissions: Permission[]) => permissions.reduce<Record<string, Permission[]>>((groups, p) => ({ ...groups, [p.module]: [...(groups[p.module] ?? []), p] }), {})

export function ProfilesPage() {
  const { t } = useTranslation(); const dispatch = useDispatch<AppDispatch>(); const [items, setItems] = useState<AccessProfile[] | null>(null); const [error, setError] = useState('')
  useEffect(() => { dispatch(fetchProfiles()).unwrap().then(setItems).catch((cause) => setError(String(cause || t('loadError')))) }, [dispatch, t])
  return <><PageHeader title={t('profiles')} subtitle={t('profilesSummary')} actions={<Link className="btn primary" to="/profiles/new"><Plus size={17} />{t('newProfile')}</Link>} />
    {error && <p className="form-error">{error}</p>}
    {!items ? <Loading /> : !items.length ? <Empty /> : <div className="table-card"><table><thead><tr><th>{t('name')}</th><th>{t('role')}</th><th>{t('permissions')}</th><th>{t('status')}</th><th /></tr></thead>
      <tbody>{items.map((p) => <tr key={p.uuid}><td>{p.name}</td><td><span className="badge blue">{t(`role${p.baseRole}`)}</span></td><td>{p.permissions.length}</td><td>{p.active ? t('active') : t('inactive')}</td><td><Link to={`/profiles/${p.uuid}/edit`}><Pencil size={15} />{t('edit')}</Link></td></tr>)}</tbody></table></div>}</>
}

export function ProfileEditorPage() {
  const { uuid } = useParams(); const navigate = useNavigate(); const { t } = useTranslation(); const dispatch = useDispatch<AppDispatch>()
  const [catalogue, setCatalogue] = useState<Permission[]>([]); const [loading, setLoading] = useState(true); const [saving, setSaving] = useState(false); const [error, setError] = useState('')
  const schema = yup.object({ name: yup.string().required(t('genericError')), baseRole: yup.mixed<AccessProfile['baseRole']>().oneOf(profileRoles).required(t('genericError')), active: yup.boolean().required(), permissions: yup.array().of(yup.string().required()).required() })
  const { control, register, handleSubmit, reset, setValue, watch, formState: { errors: validationErrors } } = useForm<AccessProfile>({ resolver: yupResolver(schema), defaultValues: { ...blank } })
  useEffect(() => {
    dispatch(fetchProfileEditor(uuid)).unwrap()
      .then(({ catalogue: permissions, profile }) => { setCatalogue(permissions); if (profile) reset(profile) }).catch((cause) => setError(String(cause || t('loadError')))).finally(() => setLoading(false))
  }, [dispatch, reset, uuid, t])
  const groups = useMemo(() => groupByModule(catalogue), [catalogue])
  const held = new Set(watch('permissions') ?? [])
  const toggle = (code: string) => setValue('permissions', held.has(code) ? [...held].filter((permission) => permission !== code) : [...held, code], { shouldDirty: true })
  // the role's built-in default is the starting point: a profile is "this role, minus/plus a few things"
  const startFromDefault = () => setValue('permissions', catalogue.filter((permission) => permission.defaultFor.includes(watch('baseRole'))).map((permission) => permission.code), { shouldDirty: true })
  const save = handleSubmit(async (profile) => { setSaving(true); setError(''); try { await dispatch(saveAccessProfile({ uuid, profile })).unwrap(); navigate('/profiles') } catch (cause) { setError(String(cause || t('genericError'))) } finally { setSaving(false) } })
  if (loading) return <Loading />
  return <><PageHeader title={uuid ? t('editProfile') : t('newProfile')} /><section className="page-form-card"><Form onSubmit={save} error={error} saving={saving} onCancel={() => navigate('/profiles')}>
    <label className="span-2">{t('name')}<input required {...register('name')} />{validationErrors.name?.message && <small className="form-error">{validationErrors.name.message}</small>}</label>
    <label>{t('role')}<Controller control={control} name="baseRole" render={({ field }) => <select {...field} onChange={(event) => field.onChange(event.target.value)}>{profileRoles.map((role) => <option key={role} value={role}>{t(`role${role}`)}</option>)}</select>} /></label>
    <label className="checkbox"><Controller control={control} name="active" render={({ field }) => <input type="checkbox" checked={field.value} onChange={field.onChange} />} />{t('active')}</label>
    <div className="span-2"><button type="button" className="btn secondary" onClick={startFromDefault}>{t('startFromDefault')}</button></div>
    {Object.entries(groups).map(([module, perms]) => <fieldset className="span-2 permission-group" key={module}><legend>{module}</legend>
      {perms.map((permission) => <label className="checkbox" key={permission.code}><input type="checkbox" checked={held.has(permission.code)} onChange={() => toggle(permission.code)} />{permission.code}{permission.own ? ` · ${t('ownOnly')}` : ''}</label>)}</fieldset>)}
  </Form></section></>
}

/** On a user's form: which profile the user holds, changeable by the owner. Hidden for roles that take none. */
export function UserProfilePicker({ userUuid, role }: { userUuid: string; role: string }) {
  const { t } = useTranslation(); const dispatch = useDispatch<AppDispatch>(); const [profiles, setProfiles] = useState<AccessProfile[]>([]); const [current, setCurrent] = useState(''); const [message, setMessage] = useState(''); const eligible = (profileRoles as string[]).includes(role)
  useEffect(() => {
    if (!eligible) return
    dispatch(fetchUserProfile(userUuid)).unwrap()
      .then(({ profiles: available, current: held }) => { setProfiles(available); setCurrent(held.profileUuid ?? '') }).catch(() => setMessage(t('loadError')))
  }, [dispatch, userUuid, eligible, t])
  if (!eligible) return null
  const change = async (profileUuid: string) => { setMessage(''); try { await dispatch(saveUserProfile({ userUuid, profileUuid })).unwrap(); setCurrent(profileUuid); setMessage(t('profileSaved')) } catch (cause) { setMessage(String(cause || t('genericError'))) } }
  return <label className="span-2">{t('profile')}<select value={current} onChange={(e) => change(e.target.value)}>
    <option value="">{t('roleDefault')}</option>{profiles.filter((p) => p.active && p.baseRole === role).map((p) => <option key={p.uuid} value={p.uuid}>{p.name}</option>)}</select>{message && <small>{message}</small>}</label>
}
