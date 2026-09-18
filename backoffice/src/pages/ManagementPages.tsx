import { useEffect, useMemo, useState, type FormEvent } from 'react'
import { Building2, CheckCircle2, CircleOff, CreditCard, Pencil, Plus, RefreshCw, Trash2 } from 'lucide-react'
import { useDispatch, useSelector } from 'react-redux'
import { Link, useNavigate, useParams } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { api, apiMessage } from '../api'
import { localeCountryCode } from '../i18n'
import type { AppDispatch, RootState } from '../store'
import { clearCities, loadCities, loadPlans, loadProvinces, loadTenants, loadUsers } from '../store'
import type { BusinessPlan, Tenant, User } from '../types'
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

export function TenantsPage() {
  const { t } = useTranslation(); const dispatch = useDispatch<AppDispatch>(); const { tenants, loading } = useSelector((s: RootState) => s.data); const session = useSelector((s: RootState) => s.auth.session)!; const [search, setSearch] = useState('')
  useEffect(() => { dispatch(loadTenants()) }, [dispatch])
  const filtered = tenants.filter((tenant) => `${tenant.businessName} ${tenant.companyName} ${tenant.taxId} ${tenant.email}`.toLowerCase().includes(search.toLowerCase()))
  return <><PageHeader title={t('tenants')} subtitle={session.role === 'SysAdmin' ? t('sysSummary') : t('ownerSummary')} actions={<><button className="btn secondary" onClick={() => dispatch(loadTenants())}><RefreshCw size={16} />{t('refresh')}</button>{session.role === 'SysAdmin' && <Link className="btn primary" to="/tenants/new"><Plus size={17} />{t('newTenant')}</Link>}</>} /><div className="toolbar"><SearchBox value={search} onChange={setSearch} /></div>{loading && !tenants.length ? <Loading /> : !filtered.length ? <Empty /> : <div className="tenant-grid">{filtered.map((tenant) => <article className="tenant-card" key={tenant.id}><div className="tenant-card-head"><span><Building2 /></span><small>#{tenant.id}</small></div><h3>{tenantName(tenant)}</h3><p>{tenant.businessName}</p><dl><div><dt>{t('taxId')}</dt><dd>{tenant.taxId}</dd></div><div><dt>{t('email')}</dt><dd>{tenant.email || '—'}</dd></div><div><dt>{t('city')}</dt><dd>{[tenant.locality, tenant.administrativeArea].filter(Boolean).join(' · ') || '—'}</dd></div></dl><footer>{tenant.id && <Link to={`/tenants/${tenant.id}/edit`}><Pencil size={15} />{t('edit')}</Link>}{session.role === 'SysAdmin' && tenant.id && <Link to={`/tenants/${tenant.id}/subscription`}><CreditCard size={15} />{t('subscription')}</Link>}</footer></article>)}</div>}</>
}

export function TenantEditorPage() {
  const { id } = useParams()
  const navigate = useNavigate()
  const dispatch = useDispatch<AppDispatch>()
  const { t, i18n } = useTranslation()
  const { provinces, cities } = useSelector((s: RootState) => s.data)
  const localeCountry = localeCountryCode(i18n.resolvedLanguage || i18n.language)
  const [form, setForm] = useState<Tenant>(() => blankTenant(localeCountry))
  const [loading, setLoading] = useState(Boolean(id))
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState('')

  useEffect(() => {
    if (!id) {
      dispatch(loadProvinces(form.countryCode || localeCountry))
      return
    }
    setLoading(true)
    api.get<Tenant>(`/tenant/${id}`)
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
      if (id) await api.put(`/tenant/${id}`, form)
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
            <label>{t('country')}</label>
            <div className="radio-group" role="radiogroup" aria-label={t('country')}>
              <label className="radio-option">
                <input
                  type="radio"
                  name="countryCode"
                  value="BR"
                  checked={(form.countryCode || 'BR') === 'BR'}
                  onChange={() => handleCountryChange('BR')}
                />
                <span>BR · {t('brazil')}</span>
              </label>
              <label className="radio-option">
                <input
                  type="radio"
                  name="countryCode"
                  value="US"
                  checked={form.countryCode === 'US'}
                  onChange={() => handleCountryChange('US')}
                />
                <span>US · {t('unitedStates')}</span>
              </label>
            </div>
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
  const { id } = useParams(); const tenantId = Number(id); const navigate = useNavigate(); const { t, i18n } = useTranslation(); const [tenant, setTenant] = useState<Tenant | null>(null); const [plans, setPlans] = useState<BusinessPlan[]>([]); const [businessPlanId, setBusinessPlanId] = useState<number | null>(null); const [loading, setLoading] = useState(true); const [saving, setSaving] = useState(false); const [error, setError] = useState('')
  useEffect(() => { Promise.all([api.get<Tenant>(`/tenant/${tenantId}`), api.get<BusinessPlan[]>('/business-plan'), api.get<BusinessPlan | null>(`/tenant/${tenantId}/plan`).catch(() => ({ data: null }))]).then(([tenantResult, plansResult, planResult]) => { setTenant(tenantResult.data); setPlans(plansResult.data); setBusinessPlanId(planResult.data?.id ?? plansResult.data[0]?.id ?? null) }).catch(() => setError(t('loadError'))).finally(() => setLoading(false)) }, [t, tenantId])
  const save = async (event: FormEvent) => { event.preventDefault(); if (!businessPlanId) return; setSaving(true); setError(''); try { await api.post(`/tenant/${tenantId}/plan`, { businessPlanId }); navigate('/tenants') } catch (cause) { setError(apiMessage(cause, t('genericError'))) } finally { setSaving(false) } }
  if (loading) return <Loading />
  return <><PageHeader title={`${t('subscription')} · ${tenant ? tenantName(tenant) : ''}`} /><section className="page-form-card narrow"><Form onSubmit={save} error={error} saving={saving} onCancel={() => navigate('/tenants')}><label className="span-2">{t('plans')}<Combobox filterable value={businessPlanId} options={plans.flatMap((plan) => plan.id ? [{ value: plan.id, label: `${plan.name} · ${money(plan.priceInCents, i18n.language)}` }] : [])} onChange={(value) => setBusinessPlanId(value)} /></label></Form></section></>
}

export function PlansPage() {
  const { t, i18n } = useTranslation(); const dispatch = useDispatch<AppDispatch>(); const { plans, loading } = useSelector((s: RootState) => s.data); const [search, setSearch] = useState(''); const [removing, setRemoving] = useState<BusinessPlan | null>(null); const [error, setError] = useState('')
  useEffect(() => { dispatch(loadPlans()) }, [dispatch]); const filtered = plans.filter((plan) => plan.name.toLowerCase().includes(search.toLowerCase()))
  const remove = async () => { if (!removing?.id) return; try { await api.delete(`/business-plan/${removing.id}`); setRemoving(null); dispatch(loadPlans()) } catch (cause) { setError(apiMessage(cause, t('genericError'))); setRemoving(null) } }
  return <><PageHeader title={t('plans')} actions={<><button className="btn secondary" onClick={() => dispatch(loadPlans())}><RefreshCw size={16} />{t('refresh')}</button><Link className="btn primary" to="/plans/new"><Plus size={17} />{t('newPlan')}</Link></>} /><div className="toolbar"><SearchBox value={search} onChange={setSearch} />{error && <span className="inline-error">{error}</span>}</div>{loading && !plans.length ? <Loading /> : !filtered.length ? <Empty /> : <div className="table-card"><div className="table-scroll"><table><thead><tr><th>{t('planName')}</th><th>{t('basePrice')}</th><th>{t('includedUsers')}</th><th>{t('periodDays')}</th><th>{t('actions')}</th></tr></thead><tbody>{filtered.map((plan) => <tr key={plan.id}><td><b>{plan.name}</b></td><td>{money(plan.priceInCents, i18n.language)}</td><td>{plan.availableUsers}</td><td>{plan.periodDays}</td><td><div className="row-actions">{plan.id && <Link to={`/plans/${plan.id}/edit`}><Pencil size={16} /></Link>}<button className="danger-text" onClick={() => setRemoving(plan)}><Trash2 size={16} /></button></div></td></tr>)}</tbody></table></div></div>}{removing && <Confirm title={t('deletePlanTitle')} text={t('deletePlanText')} onCancel={() => setRemoving(null)} onConfirm={remove} />}</>
}

export function PlanEditorPage() {
  const { id } = useParams(); const navigate = useNavigate(); const { t } = useTranslation(); const [form, setForm] = useState<BusinessPlan>({ ...blankPlan }); const [loading, setLoading] = useState(Boolean(id)); const [saving, setSaving] = useState(false); const [error, setError] = useState('')
  useEffect(() => { if (!id) return; api.get<BusinessPlan>(`/business-plan/${id}`).then(({ data }) => setForm(data)).catch(() => setError(t('loadError'))).finally(() => setLoading(false)) }, [id, t])
  const save = async (event: FormEvent) => { event.preventDefault(); setSaving(true); setError(''); try { if (id) await api.put(`/business-plan/${id}`, form); else await api.post('/business-plan', form); navigate('/plans') } catch (cause) { setError(apiMessage(cause, t('genericError'))) } finally { setSaving(false) } }
  if (loading) return <Loading />
  return <><PageHeader title={id ? t('editPlan') : t('newPlan')} /><section className="page-form-card"><Form onSubmit={save} error={error} saving={saving} onCancel={() => navigate('/plans')}><label className="span-2">{t('planName')}<input required value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} /></label><label>{t('basePrice')}<input type="number" min="0" step="0.01" value={centsToAmount(form.priceInCents)} onChange={(e) => setForm({ ...form, priceInCents: amountToCents(e.target.value) })} /></label><label>{t('includedUsers')}<input type="number" min="1" value={form.availableUsers} onChange={(e) => setForm({ ...form, availableUsers: Number(e.target.value) })} /></label><label>{t('periodDays')}<input type="number" min="1" value={form.periodDays} onChange={(e) => setForm({ ...form, periodDays: Number(e.target.value) })} /></label><label>{t('paymentDate')}<input type="date" value={form.paymentDate} onChange={(e) => setForm({ ...form, paymentDate: e.target.value })} /></label></Form></section></>
}

export function UsersPage() {
  const { t } = useTranslation(); const dispatch = useDispatch<AppDispatch>(); const { users, tenants, loading } = useSelector((s: RootState) => s.data); const session = useSelector((s: RootState) => s.auth.session)!; const [search, setSearch] = useState(''); const [confirming, setConfirming] = useState<User | null>(null); const [error, setError] = useState('')
  useEffect(() => { dispatch(loadUsers()); dispatch(loadTenants()) }, [dispatch]); const filtered = useMemo(() => users.filter((user) => `${user.name} ${user.email}`.toLowerCase().includes(search.toLowerCase())), [users, search]); const getTenantName = (tenantId?: number | null) => tenants.find((tenant) => tenant.id === tenantId)?.companyName || tenants.find((tenant) => tenant.id === tenantId)?.businessName || t('global')
  const toggle = async () => { if (!confirming?.id) return; if (confirming.id === session.userId) { setError(t('selfDeactivate')); setConfirming(null); return } try { await api.put(`/user/${confirming.id}`, { ...confirming, password: undefined, enabled: !confirming.enabled }); setConfirming(null); dispatch(loadUsers()) } catch (cause) { setError(apiMessage(cause, t('genericError'))) } }
  return <><PageHeader title={t('users')} actions={<><button className="btn secondary" onClick={() => dispatch(loadUsers())}><RefreshCw size={16} />{t('refresh')}</button><Link className="btn primary" to="/users/new"><Plus size={17} />{t('newUser')}</Link></>} /><div className="toolbar"><SearchBox value={search} onChange={setSearch} />{error && <span className="inline-error">{error}</span>}</div>{loading && !users.length ? <Loading /> : !filtered.length ? <Empty /> : <div className="table-card"><div className="table-scroll"><table><thead><tr><th>{t('name')}</th><th>{t('role')}</th><th>{t('tenant')}</th><th>{t('status')}</th><th>{t('actions')}</th></tr></thead><tbody>{filtered.map((user) => <tr key={user.id}><td><b>{user.name || '—'}</b><small>{user.email}</small></td><td><span className="badge blue">{user.role}</span></td><td>{getTenantName(user.tenantId)}</td><td><span className={`badge ${user.enabled ? 'green' : 'gray'}`}>{user.enabled ? <CheckCircle2 size={13} /> : <CircleOff size={13} />}{user.enabled ? t('active') : t('inactive')}</span></td><td><div className="row-actions">{user.id && <Link to={`/users/${user.id}/edit`}><Pencil size={16} /></Link>}<button className={user.enabled ? 'danger-text' : 'success-text'} disabled={user.id === session.userId} onClick={() => setConfirming(user)}>{user.enabled ? <CircleOff size={16} /> : <CheckCircle2 size={16} />}</button></div></td></tr>)}</tbody></table></div></div>}{confirming && <Confirm title={t('deactivateTitle')} text={t('deactivateText')} onCancel={() => setConfirming(null)} onConfirm={toggle} />}</>
}

export function UserEditorPage() {
  const { id } = useParams(); const navigate = useNavigate(); const dispatch = useDispatch<AppDispatch>(); const { t } = useTranslation(); const tenants = useSelector((s: RootState) => s.data.tenants); const session = useSelector((s: RootState) => s.auth.session)!
  // PD-019: a tenant owner creates tenant users in their own tenant only --
  // a platform administrator creates tenants and tenant owners, never a
  // tenant user directly (EPIC-IA-04).
  const [form, setForm] = useState<User>({ name: '', email: '', password: '', enabled: true, role: session.role === 'TenantOwner' ? 'TenantUser' : 'TenantOwner', tenantId: session.role === 'TenantOwner' ? session.tenantId : null }); const [loading, setLoading] = useState(Boolean(id)); const [saving, setSaving] = useState(false); const [error, setError] = useState('')
  useEffect(() => { dispatch(loadTenants()) }, [dispatch])
  useEffect(() => { if (!id) return; api.get<User>(`/user/${id}`).then(({ data }) => setForm({ ...data, password: '' })).catch(() => setError(t('loadError'))).finally(() => setLoading(false)) }, [id, t])
  useEffect(() => { if (!id && session.role === 'SysAdmin' && form.role === 'TenantOwner' && !form.tenantId && tenants[0]?.id) setForm((value) => ({ ...value, tenantId: tenants[0].id })) }, [form.role, form.tenantId, id, session.role, tenants])
  const save = async (event: FormEvent) => { event.preventDefault(); setSaving(true); setError(''); const payload = { ...form }; if (!payload.password) delete payload.password; try { if (id) await api.put(`/user/${id}`, payload); else await api.post('/user', payload); navigate('/users') } catch (cause) { setError(apiMessage(cause, t('genericError'))) } finally { setSaving(false) } }
  if (loading) return <Loading />
  // EPIC-IA-07/D-07 (PD-002): a new account is never given a caller-set
  // password -- the server discards it and emails an invitation instead. A
  // password field on this form for *creation* would offer a control the
  // API no longer honours (exactly what PD-015/EPIC-BO-06 warns against).
  // Editing an existing user still supports an admin-driven reset.
  return <><PageHeader title={id ? t('editUser') : t('newUser')} /><section className="page-form-card narrow"><Form onSubmit={save} error={error} saving={saving} onCancel={() => navigate('/users')}><label className="span-2">{t('name')}<input required value={form.name || ''} onChange={(e) => setForm({ ...form, name: e.target.value })} /></label><label className="span-2">{t('email')}<input type="email" required value={form.email} onChange={(e) => setForm({ ...form, email: e.target.value })} /></label>{id ? <label className="span-2">{t('newPassword')}<input type="password" value={form.password || ''} onChange={(e) => setForm({ ...form, password: e.target.value })} /></label> : <p className="span-2 form-hint">{t('inviteNotice')}</p>}{session.role === 'SysAdmin' && <><label>{t('role')}<Combobox value={form.role} options={[{ value: 'TenantOwner', label: 'TenantOwner' }, { value: 'SysAdmin', label: 'SysAdmin' }]} onChange={(value) => value && setForm({ ...form, role: value as User['role'], tenantId: value === 'SysAdmin' ? null : form.tenantId || tenants[0]?.id })} /></label><label>{t('tenant')}<Combobox filterable disabled={form.role === 'SysAdmin'} value={form.tenantId || null} options={tenants.flatMap((tenant) => tenant.id ? [{ value: tenant.id, label: tenantName(tenant) }] : [])} onChange={(value) => setForm({ ...form, tenantId: value })} /></label></>}<label className="checkbox span-2"><input type="checkbox" checked={form.enabled} onChange={(e) => setForm({ ...form, enabled: e.target.checked })} />{t('active')}</label></Form></section></>
}
