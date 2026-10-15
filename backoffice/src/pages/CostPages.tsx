import { useEffect, useState } from 'react'
import { Download } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { useDispatch } from 'react-redux'
import { Controller, useForm } from 'react-hook-form'
import { yupResolver } from '@hookform/resolvers/yup'
import * as yup from 'yup'
import type { AppDispatch } from '../store'
import { downloadCostReport, fetchCostVehicles, fetchSupplierClosing, fetchSuppliers, fetchVehicleCost } from '../features/costSlice'
import type { SupplierClosing, VehicleCost, VehicleOption } from '../services/costService'
import { Combobox } from '../components/Combobox'
import { Loading, PageHeader } from '../components/UI'
import type { Session } from '../types'

// C-030 EPIC-SP-07/08 (HRMS-804…810): a vehicle's cost and result over a period, and the supplier closing report,
// each with its CSV. Read-only; the office (owner and tenant user) reads them, the platform administrator has no tenant.
export const canSeeCosts = (session: Session) =>
  session.role !== 'SysAdmin' && (session.permissions ? session.permissions.includes('cost-report.vehicle.read') : session.role === 'TenantOwner' || session.role === 'TenantUser')

const isoDay = (date: Date) => date.toISOString().slice(0, 10)
const monthStart = () => { const now = new Date(); return isoDay(new Date(Date.UTC(now.getFullYear(), now.getMonth(), 1))) }

function useMoney() {
  const { i18n } = useTranslation()
  return (cents: number) => (cents / 100).toLocaleString(i18n.resolvedLanguage || i18n.language, { minimumFractionDigits: 2, maximumFractionDigits: 2 })
}

function VehicleCostTab() {
  const { t } = useTranslation(); const money = useMoney(); const dispatch = useDispatch<AppDispatch>()
  const [vehicles, setVehicles] = useState<VehicleOption[]>([])
  const [cost, setCost] = useState<VehicleCost | null>(null); const [loading, setLoading] = useState(false); const [error, setError] = useState('')
  const schema = yup.object({ vehicle: yup.string().required(t('genericError')), from: yup.string().required(t('genericError')), to: yup.string().required(t('genericError')) })
  const { control, register, handleSubmit, getValues, watch } = useForm<{ vehicle: string | null; from: string; to: string }>({ resolver: yupResolver(schema) as never, defaultValues: { vehicle: null, from: monthStart(), to: isoDay(new Date()) } })
  const vehicle = watch('vehicle')
  useEffect(() => { dispatch(fetchCostVehicles()).unwrap().then(setVehicles).catch((cause) => setError(String(cause || t('loadError')))) }, [dispatch, t])
  const run = handleSubmit(async ({ vehicle: selectedVehicle, from, to }) => {
    if (!selectedVehicle) return
    if (!vehicle) return
    setLoading(true); setError(''); setCost(null)
    try { setCost(await dispatch(fetchVehicleCost({ vehicle: selectedVehicle, params: { from, to } })).unwrap()) } catch (cause) { setError(String(cause || t('loadError'))) } finally { setLoading(false) }
  })
  const sources: [string, number][] = cost ? [[t('costPostings'), cost.bySource.workOrderPostingsCents], [t('costExternal'), cost.bySource.externalServicesCents], [t('costFuel'), cost.bySource.fuelCents], [t('costExpenses'), cost.bySource.expensesCents]] : []
  return <>
    <form className="toolbar form-inline" onSubmit={run}>
      <label>{t('vehicle')}<Controller control={control} name="vehicle" render={({ field }) => <Combobox filterable value={field.value} options={vehicles.map((v) => ({ value: v.uuid, label: v.model ? `${v.plate} · ${v.model}` : v.plate }))} onChange={field.onChange} />} /></label>
      <label>{t('periodFrom')}<input type="date" required {...register('from')} /></label><label>{t('periodTo')}<input type="date" required {...register('to')} /></label>
      <button className="btn primary" type="submit" disabled={!vehicle || loading}>{t('generate')}</button>
      <button className="btn secondary" type="button" disabled={!vehicle} onClick={() => dispatch(downloadCostReport({ path: `/cost-report/vehicle/uuid/${vehicle}/export`, params: { from: getValues('from'), to: getValues('to') } })).unwrap().catch((cause) => setError(String(cause || t('loadError'))))}><Download size={16} />{t('downloadCsv')}</button>
    </form>
    {error && <p className="form-error">{error}</p>}
    {loading && <Loading />}
    {cost && <div className="table-card">
      <table><thead><tr><th>{t('costSource')}</th><th className="num">{t('amount')}</th></tr></thead>
        <tbody>{sources.map(([label, cents]) => <tr key={label}><td>{label}</td><td className="num">{money(cents)}</td></tr>)}
          <tr><th>{t('totalCost')}</th><th className="num">{money(cost.totalCostCents)}</th></tr>
          <tr><td>{t('revenue')}</td><td className="num">{money(cost.revenueCents)}</td></tr>
          <tr><th>{t('result')}</th><th className="num">{cost.resultCents === undefined ? t('noResult') : money(cost.resultCents)}</th></tr></tbody></table>
      {!!cost.byServiceType.length && <table><thead><tr><th>{t('byServiceType')}</th><th className="num">{t('amount')}</th></tr></thead>
        <tbody>{cost.byServiceType.map((g) => <tr key={g.name}><td>{g.name || t('toBeClassified')}</td><td className="num">{money(g.amount)}</td></tr>)}</tbody></table>}
      {!!cost.servicesPerformed.length && <table><thead><tr><th>{t('servicesPerformed')}</th><th className="num">{t('quantity')}</th></tr></thead>
        <tbody>{cost.servicesPerformed.map((s) => <tr key={s.name}><td>{s.name}</td><td className="num">{s.amount}</td></tr>)}</tbody></table>}
    </div>}
  </>
}

function SupplierTab() {
  const { t } = useTranslation(); const money = useMoney(); const dispatch = useDispatch<AppDispatch>()
  const [suppliers, setSuppliers] = useState<string[]>([])
  const [closing, setClosing] = useState<SupplierClosing | null>(null); const [loading, setLoading] = useState(false); const [error, setError] = useState('')
  const schema = yup.object({ supplier: yup.string().required(t('genericError')), from: yup.string().required(t('genericError')), to: yup.string().required(t('genericError')) })
  const { control, register, handleSubmit, getValues, watch } = useForm<{ supplier: string | null; from: string; to: string }>({ resolver: yupResolver(schema) as never, defaultValues: { supplier: null, from: monthStart(), to: isoDay(new Date()) } })
  const supplier = watch('supplier')
  useEffect(() => { dispatch(fetchSuppliers()).unwrap().then(setSuppliers).catch((cause) => setError(String(cause || t('loadError')))) }, [dispatch, t])
  const run = handleSubmit(async ({ supplier: selectedSupplier, from, to }) => {
    if (!selectedSupplier) return
    setLoading(true); setError(''); setClosing(null)
    try { setClosing(await dispatch(fetchSupplierClosing({ supplier: selectedSupplier, from, to })).unwrap()) } catch (cause) { setError(String(cause || t('loadError'))) } finally { setLoading(false) }
  })
  return <>
    <form className="toolbar form-inline" onSubmit={run}>
      <label>{t('supplier')}<Controller control={control} name="supplier" render={({ field }) => <Combobox filterable value={field.value} options={suppliers.map((s) => ({ value: s, label: s }))} onChange={field.onChange} />} /></label>
      <label>{t('periodFrom')}<input type="date" required {...register('from')} /></label><label>{t('periodTo')}<input type="date" required {...register('to')} /></label>
      <button className="btn primary" type="submit" disabled={!supplier || loading}>{t('generate')}</button>
      <button className="btn secondary" type="button" disabled={!supplier} onClick={() => dispatch(downloadCostReport({ path: '/cost-report/supplier-closing/export', params: { supplier: supplier ?? '', from: getValues('from'), to: getValues('to') } })).unwrap().catch((cause) => setError(String(cause || t('loadError'))))}><Download size={16} />{t('downloadCsv')}</button>
    </form>
    {error && <p className="form-error">{error}</p>}
    {loading && <Loading />}
    {closing && (closing.lines.length === 0 ? <p className="form-hint">{t('noResults')}</p> : <div className="table-card"><table>
      <thead><tr><th>{t('date')}</th><th>{t('invoice')}</th><th>{t('service')}</th><th>{t('workOrderType')}</th><th>{t('vehicle')}</th><th className="num">{t('amount')}</th></tr></thead>
      <tbody>{closing.lines.map((l, i) => <tr key={i}><td>{l.date}</td><td>{l.invoiceNumber || '—'}</td><td>{l.service}</td><td>{l.workOrderType || '—'}</td><td>{l.plate}</td><td className="num">{money(l.invoiceCents)}</td></tr>)}
        <tr><th colSpan={5}>{t('total')}</th><th className="num">{money(closing.totalCents)}</th></tr></tbody></table></div>)}
  </>
}

export function CostReportsPage() {
  const { t } = useTranslation(); const [tab, setTab] = useState<'vehicle' | 'supplier'>('vehicle')
  return <><PageHeader title={t('costs')} subtitle={t('costsSummary')} />
    <div className="tab-bar" role="tablist">
      <button role="tab" aria-selected={tab === 'vehicle'} className={`tab ${tab === 'vehicle' ? 'active' : ''}`} onClick={() => setTab('vehicle')}>{t('vehicleCost')}</button>
      <button role="tab" aria-selected={tab === 'supplier'} className={`tab ${tab === 'supplier' ? 'active' : ''}`} onClick={() => setTab('supplier')}>{t('supplierClosing')}</button>
    </div>
    {tab === 'vehicle' ? <VehicleCostTab /> : <SupplierTab />}</>
}
