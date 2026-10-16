import { useEffect, useMemo, useState } from 'react'
import { Check, LockKeyhole, UnlockKeyhole } from 'lucide-react'
import { useForm } from 'react-hook-form'
import { yupResolver } from '@hookform/resolvers/yup'
import * as yup from 'yup'
import { useDispatch, useSelector } from 'react-redux'
import { useTranslation } from 'react-i18next'
import type { ColumnDef } from '@tanstack/react-table'
import type { AppDispatch, RootState } from '../store'
import { loadTenants } from '../store'
import { confirmBillingCharge, fetchBillingCharges, fetchBillingSettings, saveBillingSettings, setBillingSuspension } from '../features/billingSlice'
import type { BillingCharge, PageRequest, Tenant } from '../types'

export { canSeeBilling } from '../features/routeAccess'
import { DataTable } from '../components/DataTable'
import { Combobox } from '../components/Combobox'
import { Confirm, PageHeader } from '../components/UI'

function money(cents: number, language: string) {
  return (cents / 100).toLocaleString(language, { style: 'currency', currency: 'BRL' })
}

function date(value: string, language: string) {
  return new Date(value).toLocaleDateString(language)
}

export function BillingPage() {
  const { t, i18n } = useTranslation()
  const dispatch = useDispatch<AppDispatch>()
  const session = useSelector((state: RootState) => state.auth.session)!
  const tenants = useSelector((state: RootState) => state.data.tenants.items)
  const billing = useSelector((state: RootState) => state.billing)
  const isPlatformAdmin = session.role === 'SysAdmin'
  const [tenantUuid, setTenantUuid] = useState(session.role === 'TenantOwner' ? session.tenantUuid ?? '' : '')
  const [pageRequest, setPageRequest] = useState<PageRequest>({ page: 0, pageSize: 25 })
  const [chargeToConfirm, setChargeToConfirm] = useState<BillingCharge | null>(null)
  const [suspensionToConfirm, setSuspensionToConfirm] = useState<boolean | null>(null)

  const settingsSchema = yup.object({
    graceDays: yup.number().typeError(t('genericError')).integer().min(0).required(t('genericError')),
  })
  const { register, handleSubmit, reset, formState: { errors } } = useForm<{ graceDays: number }>({
    resolver: yupResolver(settingsSchema) as never,
    defaultValues: { graceDays: 0 },
  })

  useEffect(() => {
    dispatch(fetchBillingCharges(pageRequest))
  }, [dispatch, pageRequest])

  useEffect(() => {
    if (isPlatformAdmin) dispatch(loadTenants({ page: 0, pageSize: 200 }))
  }, [dispatch, isPlatformAdmin])

  useEffect(() => {
    if (!tenantUuid) return
    dispatch(fetchBillingSettings(tenantUuid)).unwrap().then((settings) => {
      reset({ graceDays: settings.graceDays ?? 0 })
    }).catch(() => reset({ graceDays: 0 }))
  }, [dispatch, reset, tenantUuid])

  const tenantOptions = useMemo(() => tenants.flatMap((tenant: Tenant) =>
    tenant.uuid ? [{ value: tenant.uuid, label: tenant.companyName || tenant.businessName }] : []), [tenants])

  const columns = useMemo<ColumnDef<BillingCharge, unknown>[]>(() => [
    ...(isPlatformAdmin ? [{ header: () => t('tenant'), accessorKey: 'tenantName' } as ColumnDef<BillingCharge, unknown>] : []),
    { id: 'billingPeriod', header: () => t('billingPeriod'), cell: ({ row }) => `${date(row.original.periodStart, i18n.language)} – ${date(row.original.periodEnd, i18n.language)}` },
    { header: () => t('planName'), accessorKey: 'planName' },
    { id: 'activeUsers', header: () => t('activeUsers'), cell: ({ row }) => `${row.original.activeUsers} / ${row.original.includedUsers}` },
    { id: 'amount', header: () => t('amount'), cell: ({ row }) => money(row.original.totalInCents, i18n.resolvedLanguage || i18n.language) },
    { id: 'status', header: () => t('status'), cell: ({ row }) => <span className={`badge ${row.original.status === 'Paid' ? 'green' : row.original.isOverdue ? 'red' : 'yellow'}`}>
      {row.original.isOverdue && row.original.status === 'Pending' ? t('billingOverdue') : t(`billing${row.original.status}`)}
    </span> },
    ...(isPlatformAdmin ? [{
      id: 'actions',
      header: () => t('actions'),
      cell: ({ row }) => row.original.status === 'Pending'
        ? <button type="button" className="btn secondary" onClick={() => setChargeToConfirm(row.original)}><Check size={16} />{t('confirmPayment')}</button>
        : <span>—</span>,
    } as ColumnDef<BillingCharge, unknown>] : []),
  ], [i18n.language, i18n.resolvedLanguage, isPlatformAdmin, t])

  const saveSettings = handleSubmit(async ({ graceDays }) => {
    if (!tenantUuid) return
    await dispatch(saveBillingSettings({ tenantUuid, graceDays })).unwrap().catch(() => undefined)
  })

  const setSuspension = async () => {
    if (!tenantUuid || suspensionToConfirm === null) return
    await dispatch(setBillingSuspension({ tenantUuid, suspended: suspensionToConfirm })).unwrap()
      .then(() => setSuspensionToConfirm(null))
      .catch(() => undefined)
  }

  const confirmPayment = async () => {
    if (!chargeToConfirm) return
    await dispatch(confirmBillingCharge(chargeToConfirm.uuid)).unwrap()
      .then(() => setChargeToConfirm(null))
      .catch(() => undefined)
  }

  const settingsMatch = billing.settingsTenantUuid === tenantUuid ? billing.settings : null
  const isSuspended = Boolean(settingsMatch?.suspendedAt)

  return <>
    <PageHeader title={t('billing')} subtitle={t('billingSummary')} />
    {isPlatformAdmin && <div className="toolbar form-inline">
      <label>{t('tenant')}<Combobox filterable ariaLabel={t('tenant')} value={tenantUuid || null} options={tenantOptions} onChange={(value) => setTenantUuid(value ?? '')} /></label>
    </div>}
    {billing.error && <p className="form-error">{billing.error}</p>}
    <section aria-label={t('billingSettings')}>
      <h2>{t('billingSettings')}</h2>
      {!tenantUuid && isPlatformAdmin ? <p className="form-hint">{t('selectBillingTenant')}</p> : billing.loadingSettings ? <p>{t('loading')}</p> : <>
        {isPlatformAdmin && <form className="toolbar form-inline" onSubmit={saveSettings}>
          <label>{t('graceDays')}<input type="number" min="0" {...register('graceDays', { valueAsNumber: true })} />{errors.graceDays?.message && <small className="form-error">{errors.graceDays.message}</small>}</label>
          <button className="btn primary" type="submit" disabled={billing.saving}>{t('save')}</button>
        </form>}
        {!settingsMatch?.graceDays && <p className="form-hint">{t('billingGraceNotConfigured')}</p>}
        {settingsMatch?.cycleStartedAt && <p>{t('billingCycleStarted', { date: date(settingsMatch.cycleStartedAt, i18n.language) })}</p>}
        <p className={`billing-status ${isSuspended ? 'danger-text' : ''}`}>{isSuspended ? t('tenantBillingSuspended') : t('tenantBillingActive')}</p>
        {isPlatformAdmin && <button type="button" className={`btn ${isSuspended ? 'secondary' : 'danger'}`} disabled={billing.saving} onClick={() => setSuspensionToConfirm(!isSuspended)}>
          {isSuspended ? <UnlockKeyhole size={16} /> : <LockKeyhole size={16} />}{isSuspended ? t('restoreTenantAccess') : t('suspendTenantAccess')}
        </button>}
      </>}
    </section>
    <section aria-label={t('billingHistory')}>
      <h2>{t('billingHistory')}</h2>
      <DataTable columns={columns} page={billing.charges} loading={billing.loadingCharges} onPageChange={setPageRequest} />
    </section>
    {chargeToConfirm && <Confirm title={t('confirmPaymentTitle')} text={t('confirmPaymentText', { tenant: chargeToConfirm.tenantName, amount: money(chargeToConfirm.totalInCents, i18n.language) })} onCancel={() => setChargeToConfirm(null)} onConfirm={confirmPayment} />}
    {suspensionToConfirm !== null && <Confirm title={suspensionToConfirm ? t('suspendTenantTitle') : t('restoreTenantTitle')} text={suspensionToConfirm ? t('suspendTenantText') : t('restoreTenantText')} onCancel={() => setSuspensionToConfirm(null)} onConfirm={setSuspension} />}
  </>
}