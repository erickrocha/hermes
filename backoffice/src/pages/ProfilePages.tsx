import { useEffect, useMemo, useState } from 'react'
import { Controller, useForm } from 'react-hook-form'
import { yupResolver } from '@hookform/resolvers/yup'
import * as yup from 'yup'
import { Eye, Lock, Pencil, Plus } from 'lucide-react'
import { Link, useNavigate, useParams } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { useDispatch, useSelector } from 'react-redux'
import type { AppDispatch, RootState } from '../store'
import { fetchProfileEditor, fetchProfiles, fetchUserProfile, saveAccessProfile, saveUserProfile } from '../features/profileSlice'
import { has, isPlatform, ownGrant } from '../features/access'
import { canEditProfiles } from '../features/routeAccess'
import type { AccessProfile, Grant, Permission, Session } from '../types'
import { Combobox } from '../components/Combobox'
import { Empty, Form, Loading, PageHeader } from '../components/UI'
import { PermissionPicker } from '../components/molecules/PermissionPicker'

// C-035/C-036: an access profile is a named set of grants -- it has no scope. The former roles are read-only system
// profiles; the catalogue the server offers is what the caller holds, which is what they may hand out.
/// The system profiles a new profile can start from -- the ones below the owner, which `defaultFor` names.
export const STARTING_POINTS = ['TenantUser', 'Driver', 'Mechanic']
const blank: AccessProfile = { name: '', system: false, active: true, permissions: [] }

/** The grants the given system profile holds, derived the way the server derives them: `@own` where the Driver's catalogue entry allows it. */
export const systemGrantsFrom = (catalogue: Permission[], key: string): Grant[] =>
  catalogue.filter((p) => p.defaultFor.includes(key)).map((p) => (key === 'Driver' && p.own ? ownGrant(p.code) : p.code))

/** The profiles a caller may hand out: active ones; the system administrator's is only offered to its own kind. */
export const profilesFor = (session: Session | null, profiles: AccessProfile[]) =>
  profiles.filter((p) => p.active && (isPlatform(session) || p.key !== 'SystemAdministrator'))

/** The catalogue for a user's extras: what the caller may grant, minus what the profile already grants in full. */
export const extrasCatalogueFor = (catalogue: Permission[], profile: AccessProfile | undefined) =>
  profile ? catalogue.filter((p) => !profile.permissions.includes(p.code)) : []

/** System profiles first, as the server lists them, then by name. */
export const sortProfiles = (profiles: AccessProfile[]) => [...profiles].sort((a, b) => Number(b.system) - Number(a.system) || a.name.localeCompare(b.name))

export const profileLabelOf = (profile: AccessProfile, t: (key: string) => string) => (profile.key ? t(`profile${profile.key}`) : profile.name)

export function ProfilesPage() {
  const { t } = useTranslation(); const dispatch = useDispatch<AppDispatch>(); const session = useSelector((s: RootState) => s.auth.session)
  const [items, setItems] = useState<AccessProfile[] | null>(null); const [error, setError] = useState('')
  useEffect(() => { dispatch(fetchProfiles()).unwrap().then((profiles) => setItems(sortProfiles(profiles))).catch((cause) => setError(String(cause || t('loadError')))) }, [dispatch, t])
  const editable = (profile: AccessProfile) => !profile.system && canEditProfiles(session!)
  return <><PageHeader title={t('profiles')} subtitle={t('profilesSummary')} actions={has(session, 'profile.create') ? <Link className="btn primary" to="/profiles/new"><Plus size={17} />{t('newProfile')}</Link> : undefined} />
    {error && <p className="form-error">{error}</p>}
    {!items ? <Loading /> : !items.length ? <Empty /> : <div className="table-card"><div className="table-scroll"><table><thead><tr><th>{t('name')}</th><th>{t('permissions')}</th><th>{t('status')}</th><th /></tr></thead>
      <tbody>{items.map((p) => <tr key={p.uuid}><td><b>{profileLabelOf(p, t)}</b>{p.system && <small><span className="badge yellow"><Lock size={12} />{t('systemProfile')} · <span className="profile-key">{p.key}</span></span></small>}</td><td>{p.permissions.length}</td><td>{p.active ? t('active') : t('inactive')}</td>
        <td>{p.uuid && <Link to={`/profiles/${p.uuid}/edit`}>{editable(p) ? <><Pencil size={15} />{t('edit')}</> : <Eye size={15} aria-label={t('view')} />}</Link>}</td></tr>)}</tbody></table></div></div>}</>
}

export function ProfileEditorPage() {
  const { uuid } = useParams(); const navigate = useNavigate(); const { t } = useTranslation(); const dispatch = useDispatch<AppDispatch>()
  const session = useSelector((s: RootState) => s.auth.session)
  const [catalogue, setCatalogue] = useState<Permission[]>([]); const [loaded, setLoaded] = useState<AccessProfile | null>(null)
  const [loading, setLoading] = useState(true); const [saving, setSaving] = useState(false); const [error, setError] = useState('')
  const schema = yup.object({ name: yup.string().required(t('genericError')), system: yup.boolean().required(), active: yup.boolean().required(), permissions: yup.array().of(yup.string().required()).required() })
  const { control, register, handleSubmit, reset, setValue, watch, formState: { errors: validationErrors } } = useForm<AccessProfile>({ resolver: yupResolver(schema) as never, defaultValues: { ...blank } })
  useEffect(() => {
    dispatch(fetchProfileEditor(uuid)).unwrap()
      .then(({ catalogue: permissions, profile }) => { setCatalogue(permissions); if (profile) { setLoaded(profile); reset(profile) } }).catch((cause) => setError(String(cause || t('loadError')))).finally(() => setLoading(false))
  }, [dispatch, reset, uuid, t])
  const permissions = watch('permissions') ?? []
  // A system profile is the server's: shown, never edited.
  const readOnly = Boolean(loaded?.system) || (uuid ? !has(session, 'profile.update') : !has(session, 'profile.create'))
  const grantable = catalogue
  const startFrom = (key: string) => { if (key) setValue('permissions', systemGrantsFrom(grantable, key), { shouldDirty: true }) }
  const save = handleSubmit(async (profile) => { setSaving(true); setError(''); try { await dispatch(saveAccessProfile({ uuid, profile })).unwrap(); navigate('/profiles') } catch (cause) { setError(String(cause || t('genericError'))) } finally { setSaving(false) } })
  if (loading) return <Loading />
  return <><PageHeader title={uuid ? (readOnly ? t('viewProfile') : t('editProfile')) : t('newProfile')} /><section className="page-form-card"><Form onSubmit={save} error={error} saving={saving} onCancel={() => navigate('/profiles')} readOnly={readOnly}>
    {loaded?.system && <p className="span-2 form-hint"><Lock size={14} /> {t('systemProfileHint')} · <span className="profile-key">{loaded.key}</span></p>}
    <label className="span-2">{t('name')}<input required readOnly={readOnly} {...register('name')} />{validationErrors.name?.message && <small className="form-error">{validationErrors.name.message}</small>}</label>
    <label className="checkbox"><Controller control={control} name="active" render={({ field }) => <input type="checkbox" disabled={readOnly} checked={field.value} onChange={field.onChange} />} />{t('active')}</label>
    {!readOnly && <label className="span-2">{t('startFrom')}<select value="" onChange={(event) => startFrom(event.target.value)}><option value="">{t('startFromPlaceholder')}</option>{STARTING_POINTS.map((key) => <option key={key} value={key}>{t(`profile${key}`)}</option>)}</select></label>}
    <PermissionPicker catalogue={grantable} value={permissions} readOnly={readOnly} onChange={(grants) => setValue('permissions', grants, { shouldDirty: true })} />
  </Form></section></>
}

export interface UserAccessFieldsProps {
  profiles: AccessProfile[]
  catalogue: Permission[]
  profileUuid: string | null
  extraPermissions: Grant[]
  onProfileChange: (profileUuid: string | null, profile: AccessProfile | undefined) => void
  onExtrasChange: (grants: Grant[]) => void
  disabled?: boolean
  error?: string
}

/** The access a user is handed: a profile, and the grants laid on top of it (never what the profile already gives). */
export function UserAccessFields({ profiles, catalogue, profileUuid, extraPermissions, onProfileChange, onExtrasChange, disabled = false, error }: UserAccessFieldsProps) {
  const { t } = useTranslation(); const session = useSelector((s: RootState) => s.auth.session)
  const offered = useMemo(() => sortProfiles(profilesFor(session, profiles)), [session, profiles])
  const selected = offered.find((p) => p.uuid === profileUuid)
  const extras = useMemo(() => extrasCatalogueFor(catalogue, selected), [catalogue, selected])
  return <div className="access-fields">
    <label className="span-2">{t('profile')}<Combobox filterable ariaLabel={t('profile')} disabled={disabled} value={profileUuid} options={offered.map((p) => ({ value: p.uuid!, label: profileLabelOf(p, t) }))} onChange={(value) => onProfileChange(value, offered.find((p) => p.uuid === value))} />
      {selected && <small className="form-hint">{selected.system ? `${t('systemProfile')} · ${selected.key}` : t('customProfile')} · {t('permissionsCount', { count: selected.permissions.length })}</small>}
      {error && <small className="form-error">{error}</small>}</label>
    {selected && <><p className="span-2 form-hint"><b>{t('extraPermissions')}</b> · {t('extraPermissionsHint')}</p>
      <PermissionPicker name="extraPermissions" catalogue={extras} value={extraPermissions} readOnly={disabled} onChange={onExtrasChange} /></>}
  </div>
}

/** On an existing user's form: the profile and extras they hold, changed through the assign flow (`PUT /access-profile/user/uuid/{uuid}`). */
export function UserProfilePicker({ userUuid }: { userUuid: string }) {
  const { t } = useTranslation(); const dispatch = useDispatch<AppDispatch>(); const session = useSelector((s: RootState) => s.auth.session)
  const [profiles, setProfiles] = useState<AccessProfile[]>([]); const [catalogue, setCatalogue] = useState<Permission[]>([])
  const [profileUuid, setProfileUuid] = useState<string | null>(null); const [extras, setExtras] = useState<Grant[]>([]); const [effective, setEffective] = useState<Grant[]>([])
  const [loading, setLoading] = useState(true); const [saving, setSaving] = useState(false); const [message, setMessage] = useState(''); const [failed, setFailed] = useState(false)
  useEffect(() => {
    dispatch(fetchUserProfile(userUuid)).unwrap()
      .then(({ profiles: available, catalogue: permissions, current }) => { setProfiles(available); setCatalogue(permissions); setProfileUuid(current.profileUuid ?? null); setExtras(current.extraPermissions); setEffective(current.permissions) })
      .catch(() => { setMessage(t('loadError')); setFailed(true) }).finally(() => setLoading(false))
  }, [dispatch, userUuid, t])
  const apply = async () => {
    if (!profileUuid) return
    setSaving(true); setMessage(''); setFailed(false)
    try { await dispatch(saveUserProfile({ userUuid, profileUuid, extraPermissions: extras })).unwrap(); setMessage(t('profileSaved')) } catch (cause) { setMessage(String(cause || t('genericError'))); setFailed(true) } finally { setSaving(false) }
  }
  if (loading) return <p className="span-2 form-hint">{t('loading')}</p>
  const assignable = has(session, 'profile.assign')
  return <fieldset className="span-2 permission-group access-section"><legend>{t('access')}</legend><div className="form-grid">
    <UserAccessFields profiles={profiles} catalogue={catalogue} profileUuid={profileUuid} extraPermissions={extras} disabled={!assignable} onProfileChange={(value) => { setProfileUuid(value); setExtras([]) }} onExtrasChange={setExtras} />
    {effective.length > 0 && <p className="span-2 form-hint">{t('effectivePermissions')}: {effective.length}</p>}
    {assignable && <div className="span-2 form-actions"><button type="button" className="btn tertiary" disabled={saving || !profileUuid} onClick={apply}>{saving ? t('saving') : t('applyAccess')}</button></div>}
    {message && <small className={failed ? 'form-error span-2' : 'form-hint span-2'}>{message}</small>}
  </div></fieldset>
}
