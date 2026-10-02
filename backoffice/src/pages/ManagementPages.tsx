import { useEffect, useMemo, useState, type FormEvent } from 'react'
import { Building2, CheckCircle2, CircleOff, CreditCard, Pencil, Plus, RefreshCw, Trash2 } from 'lucide-react'
import { useDispatch, useSelector } from 'react-redux'
import { Link, useNavigate, useParams } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { api, apiMessage } from '../api'
import { localeCountryCode } from '../i18n'
import type { AppDispatch, RootState } from '../store'
import { clearCities, loadCities, loadCountries, loadPlans, loadProvinces, loadTenants, loadUsers } from '../store'
import type { BusinessPlan, Page, Tenant, User } from '../types'
import type { ColumnDef } from '@tanstack/react-table'
import { DataTable, Pagination } from '../components/DataTable'
import { usePagedList } from '../usePagedList'
import { Combobox } from '../components/Combobox'
import { Confirm, Empty, Form, Loading, PageHeader, SearchBox } from '../components/UI'

const money = (cents: number, locale: string) => new Intl.NumberFormat(locale, { style: 'currency', currency: 'BRL' }).format(cents / 100)
// EPIC-BO-03-S02 (HRMS-412): the API's unit is integer cents, but a person
// types and reads currency amounts -- these two keep the editor's inputs in
// that human unit while the form state (and the request body) stays in
// cents, so a price can no longer be off by a factor of a hundred.
export const centsToAmount = (cents: number) => (cents / 100).toFixed(2)
export const amountToCents = (amount: string) => Math.round((Number.parseFloat(amount || '0') || 0) * 100)
const today = () => new Date().toISOString().slice(0, 10)
const tenantName = (tenant: Tenant) => tenant.companyName || tenant.businessName
const blankTenant = (countryCode: string): Tenant => ({ businessName: '', companyName: '', taxId: '', email: '', phone: '', website: '', addressLine1: '', addressLine2: '', locality: '', administrativeArea: '', postalCode: '', countryCode })
const blankPlan: BusinessPlan = { name: '', priceInCents: 0, availableUsers: 1, periodDays: 30, paymentDate: today() }
// DEF-BO-04 (PD-019/EPIC-IA-04): only a SysAdmin and a TenantOwner may create
// accounts. A TenantUser was still shown "New user", and the form only failed
// on submit -- offering a control the API refuses is exactly what PD-015
// warns against. Exported so the route guard and the page agree on one rule.
export const canCreateUsers = (role: string) => role === 'SysAdmin' || role === 'TenantOwner'
// EPIC-IA-09 (HRMS-131, D-22): a tenant owner staffs their own tenant with these roles.
export const tenantOwnerCreatableRoles: User['role'][] = ['TenantUser', 'Driver', 'Mechanic']
// EPIC-BO-07-S02 (HRMS-416, D-22): drivers and mechanics reach no tenant or user administration.
export const canReachAdministration = (role: string) => role !== 'Driver' && role !== 'Mechanic'

export function TenantsPage() {
  const { t } = useTranslation(); const { tenants, loading } = useSelector((s: RootState) => s.data); const session = useSelector((s: RootState) => s.auth.session)!
  const { search, setSearch, onPageChange, reload } = usePagedList(loadTenants)
  return <><PageHeader title={t('tenants')} subtitle={session.role === 'SysAdmin' ? t('sysSummary') : t('ownerSummary')} actions={<><button className="btn secondary" onClick={reload}><RefreshCw size={16} />{t('refresh')}</button>{session.role === 'SysAdmin' && <Link className="btn primary" to="/tenants/new"><Plus size={17} />{t('newTenant')}</Link>}</>} /><div className="toolbar"><SearchBox value={search} onChange={setSearch} /></div>{loading && !tenants.items.length ? <Loading /> : !tenants.items.length ? <Empty /> : <><div className="tenant-grid">{tenants.items.map((tenant) => <article className="tenant-card" key={tenant.id}><div className="tenant-card-head"><span><Building2 /></span></div><h3>{tenantName(tenant)}</h3><p>{tenant.businessName}</p><dl><div><dt>{t('taxId')}</dt><dd>{tenant.taxId}</dd></div><div><dt>{t('email')}</dt><dd>{tenant.email || '—'}</dd></div><div><dt>{t('city')}</dt><dd>{[tenant.locality, tenant.administrativeArea].filter(Boolean).join(' · ') || '—'}</dd></div></dl><footer>{tenant.uuid && <Link to={`/tenants/${tenant.uuid}/edit`}><Pencil size={15} />{t('edit')}</Link>}{session.role === 'SysAdmin' && tenant.uuid && <Link to={`/tenants/${tenant.uuid}/subscription`}><CreditCard size={15} />{t('subscription')}</Link>}</footer></article>)}</div><Pagination page={tenants} onPageChange={onPageChange} /></>}</>
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
  const { provinces, cities, countries } = useSelector((s: RootState) => s.data)
  const localeCountry = localeCountryCode(i18n.resolvedLanguage || i18n.language)
  // DEF-RD-08: ISO codes alone ("BR", "AR") are not what an operator reads, so
  // the browser names them in the console's language. `Intl.DisplayNames` is
  // the platform's own table -- translating country names by hand in our
  // bundles would rot the moment a new country is imported.
  const countryOptions = useMemo(() => {
    const language = i18n.resolvedLanguage || i18n.language || 'pt-BR'
    let names: Intl.DisplayNames | null = null
    try { names = new Intl.DisplayNames([language], { type: 'region' }) } catch { names = null }
    return countries
      .map((code) => ({ code, label: (names?.of(code) ?? code) || code }))
      .sort((a, b) => a.label.localeCompare(b.label, language))
  }, [countries, i18n.resolvedLanguage, i18n.language])
  const [form, setForm] = useState<Tenant>(() => blankTenant(localeCountry))
  const [loading, setLoading] = useState(Boolean(id))
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState('')

  useEffect(() => {
    dispatch(loadCountries())
  }, [dispatch])

  useEffect(() => {
    if (!id) {
      dispatch(loadProvinces(form.countryCode || localeCountry))
      return
    }
    setLoading(true)
    api.get<Tenant>(`/tenant/uuid/${id}`)
      .then(({ data }) => {
        const country = data.countryCode || localeCountry
        setForm({ ...data, countryCode: country })
        dispatch(loadProvinces(country))
      })
      .catch(() => setError(t('loadError')))
      .finally(() => setLoading(false))
  }, [dispatch, id, localeCountry, t])

  useEffect(() => {
    if (!form.administrativeArea) {
      dispatch(clearCities())
      return
    }
    const province = provinces.find(
      (item) => item.acronym === form.administrativeArea || item.name === form.administrativeArea
    )
    if (province) {
      dispatch(loadCities(province.id))
    }
  }, [dispatch, form.administrativeArea, provinces])

  const handleCountryChange = (selectedCountry: string) => {
    if (form.countryCode === selectedCountry) return
    setForm((prev) => ({
      ...prev,
      countryCode: selectedCountry,
      administrativeArea: '',
      locality: '',
    }))
    dispatch(clearCities())
    dispatch(loadProvinces(selectedCountry))
  }

  const save = async (event: FormEvent) => {
    event.preventDefault()
    setSaving(true)
    setError('')
    try {
      if (id) await api.put(`/tenant/uuid/${id}`, form)
      else await api.post('/tenant', form)
      navigate('/tenants')
    } catch (cause) {
      setError(apiMessage(cause, t('genericError')))
    } finally {
      setSaving(false)
    }
  }

  if (loading) return <Loading />

  return (
    <>
      <PageHeader title={id ? t('editTenant') : t('newTenant')} />
      <section className="page-form-card">
        <Form onSubmit={save} error={error} saving={saving} onCancel={() => navigate('/tenants')}>
          <div className="span-2">
            <label htmlFor="tenant-country">{t('country')}</label>
            {/* DEF-RD-08 (PD-027): the country list is the set that actually
                has reference data, so importing a country's provinces is all
                it takes to make it selectable. The hardcoded BR/US radios
                meant PD-027's import could never reach this form. */}
            <select
              id="tenant-country"
              required
              value={form.countryCode || ''}
              onChange={(e) => handleCountryChange(e.target.value)}
            >
              {!countryOptions.some((option) => option.code === (form.countryCode || '')) && (
                <option value={form.countryCode || ''}>{form.countryCode || t('select')}</option>
              )}
              {countryOptions.map(({ code, label }) => (
                <option key={code} value={code}>{code} · {label}</option>
              ))}
            </select>
          </div>
          <label>
            {t('businessName')}
            <input required value={form.businessName} onChange={(e) => setForm({ ...form, businessName: e.target.value })} />
          </label>
          <label>
            {t('companyName')}
            <input required value={form.companyName || ''} onChange={(e) => setForm({ ...form, companyName: e.target.value })} />
          </label>
          <label>
            {t('taxId')}
            <input required value={form.taxId} onChange={(e) => setForm({ ...form, taxId: e.target.value })} />
          </label>
          <label>
            {t('email')}
            <input type="email" value={form.email || ''} onChange={(e) => setForm({ ...form, email: e.target.value })} />
          </label>
          <label>
            {t('phone')}
            <input value={form.phone || ''} onChange={(e) => setForm({ ...form, phone: e.target.value })} />
          </label>
          <label>
            {t('website')}
            <input type="url" value={form.website || ''} onChange={(e) => setForm({ ...form, website: e.target.value })} />
          </label>
          <label className="span-2">
            {t('address')}
            <input value={form.addressLine1 || ''} onChange={(e) => setForm({ ...form, addressLine1: e.target.value })} />
          </label>
          <label>
            {t('address2')}
            <input value={form.addressLine2 || ''} onChange={(e) => setForm({ ...form, addressLine2: e.target.value })} />
          </label>
          <label>
            {t('province')}
            <Combobox
              filterable
              value={form.administrativeArea || null}
              options={provinces.map((province) => ({ value: province.acronym, label: `${province.name} (${province.acronym})` }))}
              onChange={(value) => {
                setForm((prev) => ({ ...prev, administrativeArea: value || '', locality: '' }))
                if (!value) dispatch(clearCities())
              }}
            />
          </label>
          <label>
            {t('city')}
            <Combobox
              filterable
              disabled={!form.administrativeArea || !cities.length}
              value={form.locality || null}
              options={cities.map((city) => ({ value: city.name, label: city.name }))}
              onChange={(value) => setForm((prev) => ({ ...prev, locality: value || '' }))}
            />
          </label>
          <label>
            {t('postalCode')}
            <input value={form.postalCode || ''} onChange={(e) => setForm({ ...form, postalCode: e.target.value })} />
          </label>
        </Form>
      </section>
    </>
  )
}

export function SubscriptionPage() {
  // DEF-BO-03: `/business-plan` returns PD-028's page envelope, not a bare
  // array. Reading it as an array made `plans.flatMap` throw and -- with no
  // error boundary above it -- unmounted the whole console, so the operator
  // saw a blank screen instead of the plan picker.
  const { uuid } = useParams(); const tenantId = uuid; const navigate = useNavigate(); const { t, i18n } = useTranslation(); const [tenant, setTenant] = useState<Tenant | null>(null); const [plans, setPlans] = useState<BusinessPlan[]>([]); const [businessPlanId, setBusinessPlanId] = useState<number | null>(null); const [loading, setLoading] = useState(true); const [saving, setSaving] = useState(false); const [error, setError] = useState('')
  useEffect(() => { Promise.all([api.get<Tenant>(`/tenant/uuid/${tenantId}`), api.get<Page<BusinessPlan>>('/business-plan', { params: { page: 0, pageSize: 200 } }), api.get<BusinessPlan | null>(`/tenant/uuid/${tenantId}/plan`).catch(() => ({ data: null }))]).then(([tenantResult, plansResult, planResult]) => { const available = plansResult.data.items ?? []; setTenant(tenantResult.data); setPlans(available); setBusinessPlanId(planResult.data?.id ?? available[0]?.id ?? null) }).catch(() => setError(t('loadError'))).finally(() => setLoading(false)) }, [t, tenantId])
  const save = async (event: FormEvent) => { event.preventDefault(); if (!businessPlanId) return; setSaving(true); setError(''); try { await api.post(`/tenant/uuid/${tenantId}/plan`, { businessPlanId }); navigate('/tenants') } catch (cause) { setError(apiMessage(cause, t('genericError'))) } finally { setSaving(false) } }
  if (loading) return <Loading />
  return <><PageHeader title={`${t('subscription')} · ${tenant ? tenantName(tenant) : ''}`} /><section className="page-form-card narrow"><Form onSubmit={save} error={error} saving={saving} onCancel={() => navigate('/tenants')}><label className="span-2">{t('plans')}<Combobox filterable value={businessPlanId} options={plans.flatMap((plan) => plan.id ? [{ value: plan.id, label: `${plan.name} · ${money(plan.priceInCents, i18n.language)}` }] : [])} onChange={(value) => setBusinessPlanId(value)} /></label></Form></section></>
}

export function PlansPage() {
  const { t, i18n } = useTranslation(); const { plans, loading } = useSelector((s: RootState) => s.data); const [removing, setRemoving] = useState<BusinessPlan | null>(null); const [error, setError] = useState('')
  const { search, setSearch, onPageChange, reload } = usePagedList(loadPlans)
  const remove = async () => { if (!removing?.uuid) return; try { await api.delete(`/business-plan/uuid/${removing.uuid}`); setRemoving(null); reload() } catch (cause) { setError(apiMessage(cause, t('genericError'))); setRemoving(null) } }
  const columns = useMemo<ColumnDef<BusinessPlan, unknown>[]>(() => [
    { header: () => t('planName'), accessorKey: 'name', cell: ({ row }) => <b>{row.original.name}</b> },
    { header: () => t('basePrice'), accessorKey: 'priceInCents', cell: ({ row }) => money(row.original.priceInCents, i18n.language) },
    { header: () => t('includedUsers'), accessorKey: 'availableUsers' },
    { header: () => t('periodDays'), accessorKey: 'periodDays' },
    { id: 'actions', header: () => t('actions'), cell: ({ row }) => <div className="row-actions">{row.original.uuid && <Link to={`/plans/${row.original.uuid}/edit`}><Pencil size={16} /></Link>}<button className="danger-text" onClick={() => setRemoving(row.original)}><Trash2 size={16} /></button></div> },
  ], [t, i18n.language])
  return <><PageHeader title={t('plans')} actions={<><button className="btn secondary" onClick={reload}><RefreshCw size={16} />{t('refresh')}</button><Link className="btn primary" to="/plans/new"><Plus size={17} />{t('newPlan')}</Link></>} /><div className="toolbar"><SearchBox value={search} onChange={setSearch} />{error && <span className="inline-error">{error}</span>}</div><DataTable columns={columns} page={plans} loading={loading} onPageChange={onPageChange} />{removing && <Confirm title={t('deletePlanTitle')} text={t('deletePlanText')} onCancel={() => setRemoving(null)} onConfirm={remove} />}</>
}

export function PlanEditorPage() {
  const { uuid } = useParams(); const navigate = useNavigate(); const { t } = useTranslation(); const [form, setForm] = useState<BusinessPlan>({ ...blankPlan }); const [loading, setLoading] = useState(Boolean(uuid)); const [saving, setSaving] = useState(false); const [error, setError] = useState('')
  useEffect(() => { if (!uuid) return; api.get<BusinessPlan>(`/business-plan/uuid/${uuid}`).then(({ data }) => setForm(data)).catch(() => setError(t('loadError'))).finally(() => setLoading(false)) }, [uuid, t])
  const save = async (event: FormEvent) => { event.preventDefault(); setSaving(true); setError(''); try { if (uuid) await api.put(`/business-plan/uuid/${uuid}`, form); else await api.post('/business-plan', form); navigate('/plans') } catch (cause) { setError(apiMessage(cause, t('genericError'))) } finally { setSaving(false) } }
  if (loading) return <Loading />
  return <><PageHeader title={uuid ? t('editPlan') : t('newPlan')} /><section className="page-form-card"><Form onSubmit={save} error={error} saving={saving} onCancel={() => navigate('/plans')}><label className="span-2">{t('planName')}<input required value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} /></label><label>{t('basePrice')}<input type="number" min="0" step="0.01" value={centsToAmount(form.priceInCents)} onChange={(e) => setForm({ ...form, priceInCents: amountToCents(e.target.value) })} /></label><label>{t('includedUsers')}<input type="number" min="1" value={form.availableUsers} onChange={(e) => setForm({ ...form, availableUsers: Number(e.target.value) })} /></label><label>{t('periodDays')}<input type="number" min="1" value={form.periodDays} onChange={(e) => setForm({ ...form, periodDays: Number(e.target.value) })} /></label><label>{t('paymentDate')}<input type="date" value={form.paymentDate} onChange={(e) => setForm({ ...form, paymentDate: e.target.value })} /></label></Form></section></>
}

export function UsersPage() {
  const { t } = useTranslation(); const dispatch = useDispatch<AppDispatch>(); const { users, tenants, loading } = useSelector((s: RootState) => s.data); const session = useSelector((s: RootState) => s.auth.session)!; const [confirming, setConfirming] = useState<User | null>(null); const [error, setError] = useState('')
  const { search, setSearch, onPageChange, reload } = usePagedList(loadUsers)
  // Os nomes de tenant vêm da página de tenants já carregada; um tenant fora
  // dela cai em t('global') em vez de disparar uma busca por linha.
  useEffect(() => { dispatch(loadTenants({ page: 0, pageSize: 200 })) }, [dispatch])
  const getTenantName = (tenantId?: number | null) => { const found = tenants.items.find((tenant) => tenant.id === tenantId); return found?.companyName || found?.businessName || t('global') }
  const toggle = async () => { if (!confirming?.uuid) return; if (confirming.id === session.userId) { setError(t('selfDeactivate')); setConfirming(null); return } try { await api.put(`/user/uuid/${confirming.uuid}`, { ...confirming, password: undefined, enabled: !confirming.enabled }); setConfirming(null); reload() } catch (cause) { setError(apiMessage(cause, t('genericError'))) } }
  const columns = useMemo<ColumnDef<User, unknown>[]>(() => [
    { header: () => t('name'), accessorKey: 'name', cell: ({ row }) => <><b>{row.original.name || '—'}</b><small>{row.original.email}</small></> },
    { header: () => t('role'), accessorKey: 'role', cell: ({ row }) => <span className="badge blue">{t(`role${row.original.role}`)}</span> },
    { id: 'tenant', header: () => t('tenant'), cell: ({ row }) => getTenantName(row.original.tenantId) },
    { header: () => t('status'), accessorKey: 'enabled', cell: ({ row }) => <span className={`badge ${row.original.enabled ? 'green' : 'gray'}`}>{row.original.enabled ? <CheckCircle2 size={13} /> : <CircleOff size={13} />}{row.original.enabled ? t('active') : t('inactive')}</span> },
    { id: 'actions', header: () => t('actions'), cell: ({ row }) => <div className="row-actions">{row.original.uuid && <Link to={`/users/${row.original.uuid}/edit`}><Pencil size={16} /></Link>}<button className={row.original.enabled ? 'danger-text' : 'success-text'} disabled={row.original.id === session.userId} onClick={() => setConfirming(row.original)}>{row.original.enabled ? <CircleOff size={16} /> : <CheckCircle2 size={16} />}</button></div> },
  ], [t, tenants, session.userId])
  return <><PageHeader title={t('users')} actions={<><button className="btn secondary" onClick={reload}><RefreshCw size={16} />{t('refresh')}</button>{canCreateUsers(session.role) && <Link className="btn primary" to="/users/new"><Plus size={17} />{t('newUser')}</Link>}</>} /><div className="toolbar"><SearchBox value={search} onChange={setSearch} />{error && <span className="inline-error">{error}</span>}</div><DataTable columns={columns} page={users} loading={loading} onPageChange={onPageChange} />{confirming && <Confirm title={t('deactivateTitle')} text={t('deactivateText')} onCancel={() => setConfirming(null)} onConfirm={toggle} />}</>
}

export function UserEditorPage() {
  const { uuid } = useParams(); const navigate = useNavigate(); const dispatch = useDispatch<AppDispatch>(); const { t } = useTranslation(); const tenants = useSelector((s: RootState) => s.data.tenants); const session = useSelector((s: RootState) => s.auth.session)!
  // PD-019: a tenant owner creates tenant users in their own tenant only --
  // a platform administrator creates tenants and tenant owners, never a
  // tenant user directly (EPIC-IA-04).
  const [form, setForm] = useState<User>({ name: '', email: '', password: '', enabled: true, role: session.role === 'TenantOwner' ? 'TenantUser' : 'TenantOwner', tenantId: session.role === 'TenantOwner' ? session.tenantId : null }); const [loading, setLoading] = useState(Boolean(uuid)); const [saving, setSaving] = useState(false); const [error, setError] = useState('')
  const [inviting, setInviting] = useState(false); const [invited, setInvited] = useState(false)
  // O seletor de tenant precisa das opções, não de uma página: pede um lote
  // grande em vez de paginar um <select>.
  useEffect(() => { dispatch(loadTenants({ page: 0, pageSize: 200 })) }, [dispatch])
  useEffect(() => { if (!uuid) return; api.get<User>(`/user/uuid/${uuid}`).then(({ data }) => setForm({ ...data, password: '' })).catch(() => setError(t('loadError'))).finally(() => setLoading(false)) }, [uuid, t])
  useEffect(() => { if (!uuid && session.role === 'SysAdmin' && form.role === 'TenantOwner' && !form.tenantId && tenants.items[0]?.id) setForm((value) => ({ ...value, tenantId: tenants.items[0].id })) }, [form.role, form.tenantId, uuid, session.role, tenants])
  // EPIC-IA-07/D-07: `PUT /user/uuid/{uuid}` discards any password it is sent,
  // so the old "new password" field promised a reset that silently did nothing.
  // Re-issuing the invitation is the action the API actually offers, so that
  // is the action the form offers.
  const resendInvite = async () => { if (!uuid) return; setInviting(true); setError(''); setInvited(false); try { await api.post(`/user/uuid/${uuid}/invite`); setInvited(true) } catch (cause) { setError(apiMessage(cause, t('genericError'))) } finally { setInviting(false) } }
  const save = async (event: FormEvent) => { event.preventDefault(); setSaving(true); setError(''); const payload = { ...form }; delete payload.password; try { if (uuid) await api.put(`/user/uuid/${uuid}`, payload); else await api.post('/user', payload); navigate('/users') } catch (cause) { setError(apiMessage(cause, t('genericError'))) } finally { setSaving(false) } }
  if (loading) return <Loading />
  // EPIC-IA-07/D-07 (PD-002): a new account is never given a caller-set
  // password -- the server discards it and emails an invitation instead. A
  // password field on this form would offer a control the API no longer
  // honours (exactly what PD-015/EPIC-BO-06 warns against), on creation and
  // on edit alike, so neither has one.
  return <><PageHeader title={uuid ? t('editUser') : t('newUser')} /><section className="page-form-card narrow"><Form onSubmit={save} error={error} saving={saving} onCancel={() => navigate('/users')}><label className="span-2">{t('name')}<input required value={form.name || ''} onChange={(e) => setForm({ ...form, name: e.target.value })} /></label><label className="span-2">{t('email')}<input type="email" required value={form.email} onChange={(e) => setForm({ ...form, email: e.target.value })} /></label>{uuid ? <div className="span-2 form-hint invite-hint"><span>{invited ? t('inviteSent') : t('resendInviteHint')}</span><button type="button" className="btn tertiary" disabled={inviting} onClick={resendInvite}><RefreshCw size={15} />{inviting ? t('sending') : t('resendInvite')}</button></div> : <p className="span-2 form-hint">{t('inviteNotice')}</p>}{session.role === 'SysAdmin' && <><label>{t('role')}<Combobox value={form.role} options={[{ value: 'TenantOwner', label: t('roleTenantOwner') }, { value: 'SysAdmin', label: t('roleSysAdmin') }]} onChange={(value) => value && setForm({ ...form, role: value as User['role'], tenantId: value === 'SysAdmin' ? null : form.tenantId || tenants.items[0]?.id })} /></label><label>{t('tenant')}<Combobox filterable disabled={form.role === 'SysAdmin'} value={form.tenantId || null} options={tenants.items.flatMap((tenant) => tenant.id ? [{ value: tenant.id, label: tenantName(tenant) }] : [])} onChange={(value) => setForm({ ...form, tenantId: value })} /></label></>}{session.role === 'TenantOwner' && !uuid && <label className="span-2">{t('role')}<Combobox value={form.role} options={tenantOwnerCreatableRoles.map((role) => ({ value: role, label: t(`role${role}`) }))} onChange={(value) => value && setForm({ ...form, role: value as User['role'] })} /></label>}<label className="checkbox span-2"><input type="checkbox" checked={form.enabled} onChange={(e) => setForm({ ...form, enabled: e.target.checked })} />{t('active')}</label></Form></section></>
}
