import { useEffect, useMemo, useState } from 'react'
import { Controller, FormProvider, useForm } from 'react-hook-form'
import { yupResolver } from '@hookform/resolvers/yup'
import * as yup from 'yup'
import { Building2, CheckCircle2, CircleOff, CreditCard, Pencil, Plus, RefreshCw, Settings, Trash2 } from 'lucide-react'
import { can, has, isPlatform } from '../features/access'
import { useDispatch, useSelector } from 'react-redux'
import { Link, useNavigate, useParams } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { localeCountryCode } from '../i18n'
import type { AppDispatch, RootState } from '../store'
import { loadPlans, loadTenants, loadUsers } from '../store'
import { fetchPlan, fetchSubscription, fetchTenant, fetchUser, removePlan, saveBusinessPlan, saveManagedUser, saveSubscription, saveTenant, sendUserInvite, toggleUser } from '../features/managementSlice'
import type { BusinessPlan, Tenant, User } from '../types'
import type { ColumnDef } from '@tanstack/react-table'
import { DataTable, Pagination } from '../components/DataTable'
import { usePagedList } from '../usePagedList'
import { Combobox } from '../components/Combobox'
import { AddressFields } from '../components/organisms/AddressFields'
import { TaxIdField } from '../components/molecules/TaxIdField'
import { digitsOnly, isValidCnpj, maskPhone } from '../features/masks'
import { Confirm, Empty, Form, Loading, PageHeader, SearchBox } from '../components/UI'
import { canCreateTenants, canCreateUsers, canManageSubscription } from '../features/routeAccess'
import { fetchAccessOptions } from '../features/profileSlice'
import type { AccessProfile, Permission } from '../types'
import { UserAccessFields, UserProfilePicker } from './ProfilePages'

export { canCreateUsers, canSeeProfiles, canSeeTenants, canSeeUsers } from '../features/routeAccess'

const money = (cents: number, locale: string) => new Intl.NumberFormat(locale, { style: 'currency', currency: 'BRL' }).format(cents / 100)
// EPIC-BO-03-S02 (HRMS-412): the API's unit is integer cents, but a person
// types and reads currency amounts -- these two keep the editor's inputs in
// that human unit while the form state (and the request body) stays in
// cents, so a price can no longer be off by a factor of a hundred.
export const centsToAmount = (cents: number) => (cents / 100).toFixed(2)
export const amountToCents = (amount: string) => Math.round((Number.parseFloat(amount || '0') || 0) * 100)
const tenantName = (tenant: Tenant) => tenant.companyName || tenant.businessName
const blankTenant = (): Tenant => ({ businessName: '', companyName: '', taxId: '', email: '', phone: '', website: '', addressLine1: '', addressLine2: '', locality: '', administrativeArea: '', postalCode: '', countryCode: 'BR' })
const blankPlan: BusinessPlan = { name: '', priceInCents: 0, pricePerExtraUserInCents: 0, availableUsers: 1, periodDays: 30 }
export function TenantsPage() {
  const { t } = useTranslation(); const { tenants, loading } = useSelector((s: RootState) => s.data); const session = useSelector((s: RootState) => s.auth.session)!
  const { search, setSearch, onPageChange, reload } = usePagedList(loadTenants)
  return <><PageHeader title={t('tenants')} subtitle={isPlatform(session) ? t('sysSummary') : t('ownerSummary')} actions={<><button className="btn secondary" onClick={reload}><RefreshCw size={16} />{t('refresh')}</button>{canCreateTenants(session) && <Link className="btn primary" to="/tenants/new"><Plus size={17} />{t('newTenant')}</Link>}</>} /><div className="toolbar"><SearchBox value={search} onChange={setSearch} /></div>{loading && !tenants.items.length ? <Loading /> : !tenants.items.length ? <Empty /> : <><div className="tenant-grid">{tenants.items.map((tenant) => <article className="tenant-card" key={tenant.id}><div className="tenant-card-head"><span><Building2 /></span></div><h3>{tenantName(tenant)}</h3><p>{tenant.businessName}</p><dl><div><dt>{t('taxId')}</dt><dd>{tenant.taxId}</dd></div><div><dt>{t('email')}</dt><dd>{tenant.email || '—'}</dd></div><div><dt>{t('city')}</dt><dd>{[tenant.locality, tenant.administrativeArea].filter(Boolean).join(' · ') || '—'}</dd></div></dl><footer>{tenant.uuid && <Link to={`/tenants/${tenant.uuid}/edit`}><Pencil size={15} />{t('edit')}</Link>}{tenant.uuid && can(session, 'tenant-rule-settings.update') && <Link to={`/tenants/${tenant.uuid}/manage`}><Settings size={15} />{t('manage')}</Link>}{tenant.uuid && can(session, 'tenant-rule-settings.update') && <Link to={`/tenants/${tenant.uuid}/tracking`}><Settings size={15} />{t('trackingSettings')}</Link>}{canManageSubscription(session) && tenant.uuid && <Link to={`/tenants/${tenant.uuid}/subscription`}><CreditCard size={15} />{t('subscription')}</Link>}</footer></article>)}</div><Pagination page={tenants} onPageChange={onPageChange} /></>}</>
}

export function TenantEditorPage() {
  // HRMS-204/OBS-TP-05: tenants are addressed by their public uuid, in the
  // console's own URL as well as in the API call, so no browser address bar
  // or API path carries the sequential internal id.
  const { uuid } = useParams()
  const id = uuid
  const navigate = useNavigate()
  const dispatch = useDispatch<AppDispatch>()
  const { t, i18n } = useTranslation()
  const localeCountry = localeCountryCode(i18n.resolvedLanguage || i18n.language)
  const [loading, setLoading] = useState(Boolean(id))
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState('')
  const schema = yup.object({
    businessName: yup.string().required(t('genericError')),
    companyName: yup.string().required(t('genericError')),
    taxId: yup.string().required(t('genericError')).test(
      'valid-cnpj',
      t('invalidCnpj'),
      (value, context) => context.parent.countryCode !== 'BR' || isValidCnpj(value || '')
    ),
    email: yup.string().email(t('genericError')).optional(),
    website: yup.string().url(t('genericError')).optional(),
    countryCode: yup.string().oneOf(['BR', 'US']).required(t('genericError')),
  })
  const form = useForm<Tenant>({ resolver: yupResolver(schema) as never, defaultValues: blankTenant() })
  const { control, register, handleSubmit, reset, watch, formState: { errors: validationErrors } } = form
  const countryCode = watch('countryCode') || ''

  useEffect(() => {
    if (!id) return
    setLoading(true)
    dispatch(fetchTenant(id)).unwrap()
      .then((data) => reset({ ...data, countryCode: data.countryCode || localeCountry }))
      .catch(() => setError(t('loadError')))
      .finally(() => setLoading(false))
  }, [dispatch, id, localeCountry, reset, t])

  const save = handleSubmit(async (tenant) => {
    setSaving(true)
    setError('')
    try {
      // Masks are presentation: the document, the phone and the postal code travel and are stored as digits only.
      await dispatch(saveTenant({ uuid: id, tenant: { ...tenant, taxId: digitsOnly(tenant.taxId || ''), phone: digitsOnly(tenant.phone || ''), postalCode: digitsOnly(tenant.postalCode || '') } })).unwrap()
      navigate('/tenants')
    } catch (cause) {
      setError(String(cause || t('genericError')))
    } finally {
      setSaving(false)
    }
  })

  if (loading) return <Loading />

  return (
    <>
      <PageHeader title={id ? t('editTenant') : t('newTenant')} />
      <section className="page-form-card">
        <FormProvider {...form}>
          <Form onSubmit={save} error={error} saving={saving} onCancel={() => navigate('/tenants')}>
            <div className="form-row tenant-business-row">
              <label>{t('businessName')}<input required {...register('businessName')} /></label>
            </div>
            <div className="form-row tenant-tax-row">
              <TaxIdField control={control} name="taxId" id="tenant-tax-id" required kind="company" countryCode={countryCode} label={countryCode === 'US' ? t('taxIdEin') : t('taxIdCnpj')} error={validationErrors.taxId?.message} />
              <label>{t('companyName')}<input required {...register('companyName')} /></label>
            </div>
            <div className="form-row tenant-contact-row">
              <label>{t('phone')}<Controller control={control} name="phone" render={({ field }) => <input type="tel" autoComplete="tel" value={maskPhone(field.value || '', countryCode)} onBlur={field.onBlur} onChange={(event) => field.onChange(maskPhone(event.target.value, countryCode))} ref={field.ref} />} /></label>
              <label>{t('email')}<input type="email" autoComplete="email" {...register('email')} /></label>
              <label>{t('website')}<input type="url" {...register('website')} /></label>
            </div>
            <AddressFields />
          </Form>
        </FormProvider>
      </section>
    </>
  )
}

export { isValidCnpj, maskCnpj, maskPhone, maskPostalCode, maskTaxId } from '../features/masks'
export function SubscriptionPage() {
  // DEF-BO-03: `/business-plan` returns PD-028's page envelope, not a bare
  // array. Reading it as an array made `plans.flatMap` throw and -- with no
  // error boundary above it -- unmounted the whole console, so the operator
  // saw a blank screen instead of the plan picker.
  const { uuid } = useParams(); const tenantId = uuid; const navigate = useNavigate(); const dispatch = useDispatch<AppDispatch>(); const { t, i18n } = useTranslation(); const [tenant, setTenant] = useState<Tenant | null>(null); const [plans, setPlans] = useState<BusinessPlan[]>([]); const [loading, setLoading] = useState(true); const [saving, setSaving] = useState(false); const [error, setError] = useState('')
  const schema = yup.object({ businessPlanId: yup.number().nullable().required(t('genericError')) })
  const { control, handleSubmit, setValue, formState: { errors: validationErrors } } = useForm<{ businessPlanId: number | null }>({ resolver: yupResolver(schema) as never, defaultValues: { businessPlanId: null } })
  useEffect(() => { if (!tenantId) return; dispatch(fetchSubscription(tenantId)).unwrap().then(({ tenant: loadedTenant, plans: available, currentPlan }) => { setTenant(loadedTenant); setPlans(available); setValue('businessPlanId', currentPlan?.id ?? available[0]?.id ?? null) }).catch(() => setError(t('loadError'))).finally(() => setLoading(false)) }, [dispatch, setValue, t, tenantId])
  const save = handleSubmit(async ({ businessPlanId }) => { if (!tenantId || !businessPlanId) return; setSaving(true); setError(''); try { await dispatch(saveSubscription({ uuid: tenantId, businessPlanId })).unwrap(); navigate('/tenants') } catch (cause) { setError(String(cause || t('genericError'))) } finally { setSaving(false) } })
  if (loading) return <Loading />
  return <><PageHeader title={`${t('subscription')} · ${tenant ? tenantName(tenant) : ''}`} /><section className="page-form-card narrow"><Form onSubmit={save} error={error} saving={saving} onCancel={() => navigate('/tenants')}><label className="span-2">{t('plans')}<Controller control={control} name="businessPlanId" render={({ field }) => <Combobox filterable value={field.value} options={plans.flatMap((plan) => plan.id ? [{ value: plan.id, label: `${plan.name} · ${money(plan.priceInCents, i18n.language)}` }] : [])} onChange={field.onChange} />} />{validationErrors.businessPlanId?.message && <small className="form-error">{validationErrors.businessPlanId.message}</small>}</label></Form></section></>
}

export function PlansPage() {
  const { t, i18n } = useTranslation(); const dispatch = useDispatch<AppDispatch>(); const { plans, loading } = useSelector((s: RootState) => s.data); const [removing, setRemoving] = useState<BusinessPlan | null>(null); const [error, setError] = useState('')
  const { search, setSearch, onPageChange, reload } = usePagedList(loadPlans)
  const remove = async () => { if (!removing?.uuid) return; try { await dispatch(removePlan(removing.uuid)).unwrap(); setRemoving(null); reload() } catch (cause) { setError(String(cause || t('genericError'))); setRemoving(null) } }
  const columns = useMemo<ColumnDef<BusinessPlan, unknown>[]>(() => [
    { header: () => t('planName'), accessorKey: 'name', cell: ({ row }) => <b>{row.original.name}</b> },
    { header: () => t('basePrice'), accessorKey: 'priceInCents', cell: ({ row }) => money(row.original.priceInCents, i18n.language) },
    { header: () => t('pricePerExtraUser'), accessorKey: 'pricePerExtraUserInCents', cell: ({ row }) => money(row.original.pricePerExtraUserInCents, i18n.language) },
    { header: () => t('includedUsers'), accessorKey: 'availableUsers' },
    { header: () => t('periodDays'), accessorKey: 'periodDays' },
    { id: 'actions', header: () => t('actions'), cell: ({ row }) => <div className="row-actions">{row.original.uuid && <Link to={`/plans/${row.original.uuid}/edit`}><Pencil size={16} /></Link>}<button className="danger-text" onClick={() => setRemoving(row.original)}><Trash2 size={16} /></button></div> },
  ], [t, i18n.language])
  return <><PageHeader title={t('plans')} actions={<><button className="btn secondary" onClick={reload}><RefreshCw size={16} />{t('refresh')}</button><Link className="btn primary" to="/plans/new"><Plus size={17} />{t('newPlan')}</Link></>} /><div className="toolbar"><SearchBox value={search} onChange={setSearch} />{error && <span className="inline-error">{error}</span>}</div><DataTable columns={columns} page={plans} loading={loading} onPageChange={onPageChange} />{removing && <Confirm title={t('deletePlanTitle')} text={t('deletePlanText')} onCancel={() => setRemoving(null)} onConfirm={remove} />}</>
}

export function PlanEditorPage() {
  const { uuid } = useParams(); const navigate = useNavigate(); const dispatch = useDispatch<AppDispatch>(); const { t } = useTranslation(); const [loading, setLoading] = useState(Boolean(uuid)); const [saving, setSaving] = useState(false); const [error, setError] = useState('')
  const schema = yup.object({ name: yup.string().required(t('genericError')), priceInCents: yup.number().min(0).required(t('genericError')), pricePerExtraUserInCents: yup.number().min(0).required(t('genericError')), availableUsers: yup.number().min(1).required(t('genericError')), periodDays: yup.number().min(1).required(t('genericError')) })
  const { control, register, handleSubmit, reset, formState: { errors: validationErrors } } = useForm<BusinessPlan>({ resolver: yupResolver(schema), defaultValues: { ...blankPlan } })
  useEffect(() => { if (!uuid) return; dispatch(fetchPlan(uuid)).unwrap().then(reset).catch(() => setError(t('loadError'))).finally(() => setLoading(false)) }, [dispatch, reset, uuid, t])
  const save = handleSubmit(async (plan) => { setSaving(true); setError(''); try { await dispatch(saveBusinessPlan({ uuid, plan })).unwrap(); navigate('/plans') } catch (cause) { setError(String(cause || t('genericError'))) } finally { setSaving(false) } })
  if (loading) return <Loading />
  return <><PageHeader title={uuid ? t('editPlan') : t('newPlan')} /><section className="page-form-card"><Form onSubmit={save} error={error} saving={saving} onCancel={() => navigate('/plans')}><label className="span-2">{t('planName')}<input required {...register('name')} />{validationErrors.name?.message && <small className="form-error">{validationErrors.name.message}</small>}</label><label>{t('basePrice')}<Controller control={control} name="priceInCents" render={({ field }) => <input type="number" min="0" step="0.01" value={centsToAmount(field.value)} onChange={(event) => field.onChange(amountToCents(event.target.value))} />} />{validationErrors.priceInCents?.message && <small className="form-error">{validationErrors.priceInCents.message}</small>}</label><label>{t('pricePerExtraUser')}<Controller control={control} name="pricePerExtraUserInCents" render={({ field }) => <input type="number" min="0" step="0.01" value={centsToAmount(field.value)} onChange={(event) => field.onChange(amountToCents(event.target.value))} />} />{validationErrors.pricePerExtraUserInCents?.message && <small className="form-error">{validationErrors.pricePerExtraUserInCents.message}</small>}</label><label>{t('includedUsers')}<input type="number" min="1" {...register('availableUsers', { valueAsNumber: true })} />{validationErrors.availableUsers?.message && <small className="form-error">{validationErrors.availableUsers.message}</small>}</label><label>{t('periodDays')}<input type="number" min="1" {...register('periodDays', { valueAsNumber: true })} />{validationErrors.periodDays?.message && <small className="form-error">{validationErrors.periodDays.message}</small>}</label></Form></section></>
}

export function UsersPage() {
  const { t } = useTranslation(); const dispatch = useDispatch<AppDispatch>(); const { users, tenants, loading } = useSelector((s: RootState) => s.data); const session = useSelector((s: RootState) => s.auth.session)!; const [confirming, setConfirming] = useState<User | null>(null); const [error, setError] = useState('')
  const { search, setSearch, onPageChange, reload } = usePagedList(loadUsers)
  // Os nomes de tenant vêm da página de tenants já carregada; um tenant fora
  // dela cai em t('global') em vez de disparar uma busca por linha.
  useEffect(() => { dispatch(loadTenants({ page: 0, pageSize: 200 })) }, [dispatch])
  const getTenantName = (tenantId?: number | null) => { const found = tenants.items.find((tenant) => tenant.id === tenantId); return found?.companyName || found?.businessName || t('global') }
  const toggle = async () => { if (!confirming?.uuid) return; if (confirming.id === session.userId) { setError(t('selfDeactivate')); setConfirming(null); return } try { await dispatch(toggleUser({ user: confirming, enabled: !confirming.enabled })).unwrap(); setConfirming(null); reload() } catch (cause) { setError(String(cause || t('genericError'))) } }
  const columns = useMemo<ColumnDef<User, unknown>[]>(() => [
    { header: () => t('name'), accessorKey: 'name', cell: ({ row }) => <><b>{row.original.name || '—'}</b><small>{row.original.email}</small></> },
    { header: () => t('profile'), accessorKey: 'profileName', cell: ({ row }) => <><span className="badge blue">{row.original.profileKey ? t(`profile${row.original.profileKey}`) : row.original.profileName || '—'}</span>{row.original.extraPermissions?.length > 0 && <small>{t('extraPermissionsCount', { count: row.original.extraPermissions.length })}</small>}</> },
    { id: 'tenant', header: () => t('tenant'), cell: ({ row }) => getTenantName(row.original.tenantId) },
    { header: () => t('status'), accessorKey: 'enabled', cell: ({ row }) => <span className={`badge ${row.original.enabled ? 'green' : 'gray'}`}>{row.original.enabled ? <CheckCircle2 size={13} /> : <CircleOff size={13} />}{row.original.enabled ? t('active') : t('inactive')}</span> },
    { id: 'actions', header: () => t('actions'), cell: ({ row }) => <div className="row-actions">{row.original.uuid && <Link to={`/users/${row.original.uuid}/edit`}><Pencil size={16} /></Link>}<button className={row.original.enabled ? 'danger-text' : 'success-text'} disabled={row.original.id === session.userId} onClick={() => setConfirming(row.original)}>{row.original.enabled ? <CircleOff size={16} /> : <CheckCircle2 size={16} />}</button></div> },
  ], [t, tenants, session.userId])
  return <><PageHeader title={t('users')} actions={<><button className="btn secondary" onClick={reload}><RefreshCw size={16} />{t('refresh')}</button>{canCreateUsers(session) && <Link className="btn primary" to="/users/new"><Plus size={17} />{t('newUser')}</Link>}</>} /><div className="toolbar"><SearchBox value={search} onChange={setSearch} />{error && <span className="inline-error">{error}</span>}</div><DataTable columns={columns} page={users} loading={loading} onPageChange={onPageChange} />{confirming && <Confirm title={t('deactivateTitle')} text={t('deactivateText')} onCancel={() => setConfirming(null)} onConfirm={toggle} />}</>
}

export function UserEditorPage() {
  const { uuid } = useParams(); const navigate = useNavigate(); const dispatch = useDispatch<AppDispatch>(); const { t } = useTranslation(); const tenants = useSelector((s: RootState) => s.data.tenants); const session = useSelector((s: RootState) => s.auth.session)!
  // C-035: a user is created with an access profile. The platform hands out Platform profiles (the user gets no
  // tenant) and Tenant profiles (the platform names the tenant); a tenant caller hands out Tenant profiles inside
  // their own tenant only -- so the tenant picker is the platform's alone.
  const platform = isPlatform(session)
  const [loading, setLoading] = useState(true); const [saving, setSaving] = useState(false); const [error, setError] = useState('')
  const [inviting, setInviting] = useState(false); const [invited, setInvited] = useState(false)
  const [profiles, setProfiles] = useState<AccessProfile[]>([]); const [catalogue, setCatalogue] = useState<Permission[]>([])
  const schema = yup.object({ name: yup.string().required(t('genericError')), email: yup.string().email(t('genericError')).required(t('genericError')), enabled: yup.boolean().required(), tenantId: yup.number().nullable().optional(), cpf: yup.string().nullable().optional(),
    profileUuid: uuid ? yup.string().optional() : yup.string().required(t('profileRequired')), extraPermissions: yup.array().of(yup.string().required()).required() })
  const { control, register, handleSubmit, reset, setValue, watch, formState: { errors: validationErrors } } = useForm<User>({ resolver: yupResolver(schema) as never, defaultValues: { name: '', email: '', enabled: true, tenantId: platform ? null : session.tenantId, extraPermissions: [] } })
  const profileUuid = watch('profileUuid') ?? null
  const extraPermissions = watch('extraPermissions') ?? []
  const selectedTenantId = watch('tenantId')
  const selectedProfile = profiles.find((p) => p.uuid === profileUuid)
  const tenantless = selectedProfile?.scope === 'Platform'
  // O seletor de tenant precisa das opções, não de uma página: pede um lote
  // grande em vez de paginar um <select>.
  useEffect(() => { if (platform) dispatch(loadTenants({ page: 0, pageSize: 200 })) }, [dispatch, platform])
  useEffect(() => {
    const load = uuid
      ? dispatch(fetchUser(uuid)).unwrap().then((user) => reset(user))
      : dispatch(fetchAccessOptions()).unwrap().then(({ profiles: available, catalogue: permissions }) => { setProfiles(available); setCatalogue(permissions) })
    load.catch(() => setError(t('loadError'))).finally(() => setLoading(false))
  }, [dispatch, reset, uuid, t])
  // A Tenant profile handed out by the platform needs a tenant; the first one stands in until the operator picks.
  useEffect(() => { if (!uuid && platform && selectedProfile && !tenantless && !selectedTenantId && tenants.items[0]?.id) setValue('tenantId', tenants.items[0].id) }, [platform, selectedProfile, tenantless, selectedTenantId, setValue, uuid, tenants])
  const chooseProfile = (value: string | null, profile: AccessProfile | undefined) => {
    setValue('profileUuid', value ?? undefined, { shouldDirty: true, shouldValidate: true }); setValue('extraPermissions', [], { shouldDirty: true })
    if (platform) setValue('tenantId', profile?.scope === 'Platform' ? null : selectedTenantId || tenants.items[0]?.id || null)
  }
  // EPIC-IA-07/D-07: `PUT /user/uuid/{uuid}` discards any password it is sent,
  // so the old "new password" field promised a reset that silently did nothing.
  // Re-issuing the invitation is the action the API actually offers, so that
  // is the action the form offers.
  const resendInvite = async () => { if (!uuid) return; setInviting(true); setError(''); setInvited(false); try { await dispatch(sendUserInvite(uuid)).unwrap(); setInvited(true) } catch (cause) { setError(String(cause || t('genericError'))) } finally { setInviting(false) } }
  const save = handleSubmit(async (user) => { setSaving(true); setError(''); try { await dispatch(saveManagedUser({ uuid, user })).unwrap(); navigate('/users') } catch (cause) { setError(String(cause || t('genericError'))) } finally { setSaving(false) } })
  if (loading) return <Loading />
  // EPIC-IA-07/D-07 (PD-002): a new account is never given a caller-set
  // password -- the server discards it and emails an invitation instead. A
  // password field on this form would offer a control the API no longer
  // honours (exactly what PD-015/EPIC-BO-06 warns against), on creation and
  // on edit alike, so neither has one.
  return <><PageHeader title={uuid ? t('editUser') : t('newUser')} /><section className="page-form-card narrow"><Form onSubmit={save} error={error} saving={saving} onCancel={() => navigate('/users')}>
    <label className="span-2">{t('name')}<input required {...register('name')} />{validationErrors.name?.message && <small className="form-error">{validationErrors.name.message}</small>}</label>
    <label className="span-2">{t('email')}<input type="email" required {...register('email')} />{validationErrors.email?.message && <small className="form-error">{validationErrors.email.message}</small>}</label>
    {uuid ? <div className="span-2 form-hint invite-hint"><span>{invited ? t('inviteSent') : t('resendInviteHint')}</span><button type="button" className="btn tertiary" disabled={inviting} onClick={resendInvite}><RefreshCw size={15} />{inviting ? t('sending') : t('resendInvite')}</button></div> : <p className="span-2 form-hint">{t('inviteNotice')}</p>}
    {platform && <label className="span-2">{t('tenant')}<Controller control={control} name="tenantId" render={({ field }) => <Combobox filterable ariaLabel={t('tenant')} disabled={tenantless} value={field.value || null} options={tenants.items.flatMap((tenant) => tenant.id ? [{ value: tenant.id, label: tenantName(tenant) }] : [])} onChange={field.onChange} />} />{tenantless && <small className="form-hint">{t('platformProfileNoTenant')}</small>}</label>}
    {!platform && <label>{t('cpf')}<input inputMode="numeric" {...register('cpf')} /></label>}
    <label className="checkbox span-2"><Controller control={control} name="enabled" render={({ field }) => <input type="checkbox" checked={field.value} onChange={field.onChange} />} />{t('active')}</label>
    {!uuid && <UserAccessFields profiles={profiles} catalogue={catalogue} profileUuid={profileUuid} extraPermissions={extraPermissions} onProfileChange={chooseProfile} onExtrasChange={(grants) => setValue('extraPermissions', grants, { shouldDirty: true })} error={validationErrors.profileUuid?.message} />}
    {uuid && has(session, 'profile.read') && <UserProfilePicker userUuid={uuid} />}
  </Form></section></>
}
