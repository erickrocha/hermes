import { useEffect, useState } from 'react'
import { Download } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { api, apiMessage } from '../api'
import { Combobox } from '../components/Combobox'
import { Loading, PageHeader } from '../components/UI'
import type { Page, Session } from '../types'

// C-030 EPIC-SP-07/08 (HRMS-804…810): a vehicle's cost and result over a period, and the supplier closing report,
// each with its CSV. Read-only; the office (owner and tenant user) reads them, the platform administrator has no tenant.
export const canSeeCosts = (session: Session) =>
  session.role !== 'SysAdmin' && (session.permissions ? session.permissions.includes('cost-report.vehicle.read') : session.role === 'TenantOwner' || session.role === 'TenantUser')

interface Named { name: string; amount: number }
interface VehicleCost {
  plate: string; bySource: { workOrderPostingsCents: number; externalServicesCents: number; fuelCents: number; expensesCents: number }
  totalCostCents: number; byServiceType: Named[]; servicesPerformed: Named[]; revenueCents: number; resultCents?: number
}
interface SupplierClosing { lines: { date: string; invoiceNumber?: string; service: string; workOrderType?: string; plate: string; invoiceCents: number }[]; totalCents: number }
interface VehicleOption { uuid: string; plate: string; model?: string }

const isoDay = (date: Date) => date.toISOString().slice(0, 10)
const monthStart = () => { const now = new Date(); return isoDay(new Date(Date.UTC(now.getFullYear(), now.getMonth(), 1))) }

function useMoney() {
  const { i18n } = useTranslation()
  return (cents: number) => (cents / 100).toLocaleString(i18n.resolvedLanguage || i18n.language, { minimumFractionDigits: 2, maximumFractionDigits: 2 })
}

async function download(url: string, params: Record<string, string>) {
  const response = await api.get<Blob>(url, { params, responseType: 'blob' })
  const name = /filename="([^"]+)"/.exec(String(response.headers['content-disposition'] ?? ''))?.[1] ?? 'report.csv'
  const link = document.createElement('a')
  link.href = URL.createObjectURL(response.data)
  link.download = name
  link.click()
  URL.revokeObjectURL(link.href)
}

function Period({ from, to, setFrom, setTo }: { from: string; to: string; setFrom: (v: string) => void; setTo: (v: string) => void }) {
  const { t } = useTranslation()
  return <><label>{t('periodFrom')}<input type="date" required value={from} onChange={(e) => setFrom(e.target.value)} /></label><label>{t('periodTo')}<input type="date" required value={to} onChange={(e) => setTo(e.target.value)} /></label></>
}

function VehicleCostTab() {
  const { t } = useTranslation(); const money = useMoney()
  const [vehicles, setVehicles] = useState<VehicleOption[]>([]); const [vehicle, setVehicle] = useState<string | null>(null)
  const [from, setFrom] = useState(monthStart()); const [to, setTo] = useState(isoDay(new Date()))
  const [cost, setCost] = useState<VehicleCost | null>(null); const [loading, setLoading] = useState(false); const [error, setError] = useState('')
  useEffect(() => { api.get<Page<VehicleOption>>('/vehicle', { params: { page: 0, pageSize: 200 } }).then(({ data }) => setVehicles(data.items)).catch((cause) => setError(apiMessage(cause, t('loadError')))) }, [t])
  const query = { from, to }
  const run = async () => {
    if (!vehicle) return
    setLoading(true); setError(''); setCost(null)
    try { setCost((await api.get<VehicleCost>(`/cost-report/vehicle/uuid/${vehicle}`, { params: query })).data) } catch (cause) { setError(apiMessage(cause, t('loadError'))) } finally { setLoading(false) }
  }
  const sources: [string, number][] = cost ? [[t('costPostings'), cost.bySource.workOrderPostingsCents], [t('costExternal'), cost.bySource.externalServicesCents], [t('costFuel'), cost.bySource.fuelCents], [t('costExpenses'), cost.bySource.expensesCents]] : []
  return <>
    <form className="toolbar form-inline" onSubmit={(e) => { e.preventDefault(); void run() }}>
      <label>{t('vehicle')}<Combobox filterable value={vehicle} options={vehicles.map((v) => ({ value: v.uuid, label: v.model ? `${v.plate} · ${v.model}` : v.plate }))} onChange={(value) => setVehicle(value as string | null)} /></label>
      <Period from={from} to={to} setFrom={setFrom} setTo={setTo} />
      <button className="btn primary" type="submit" disabled={!vehicle || loading}>{t('generate')}</button>
      <button className="btn secondary" type="button" disabled={!vehicle} onClick={() => download(`/cost-report/vehicle/uuid/${vehicle}/export`, query).catch((cause) => setError(apiMessage(cause, t('loadError'))))}><Download size={16} />{t('downloadCsv')}</button>
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
  const { t } = useTranslation(); const money = useMoney()
  const [suppliers, setSuppliers] = useState<string[]>([]); const [supplier, setSupplier] = useState<string | null>(null)
  const [from, setFrom] = useState(monthStart()); const [to, setTo] = useState(isoDay(new Date()))
  const [closing, setClosing] = useState<SupplierClosing | null>(null); const [loading, setLoading] = useState(false); const [error, setError] = useState('')
  useEffect(() => { api.get<string[]>('/cost-report/suppliers').then(({ data }) => setSuppliers(data)).catch((cause) => setError(apiMessage(cause, t('loadError')))) }, [t])
  const query = { supplier: supplier ?? '', from, to }
  const run = async () => {
    if (!supplier) return
    setLoading(true); setError(''); setClosing(null)
    try { setClosing((await api.get<SupplierClosing>('/cost-report/supplier-closing', { params: query })).data) } catch (cause) { setError(apiMessage(cause, t('loadError'))) } finally { setLoading(false) }
  }
  return <>
    <form className="toolbar form-inline" onSubmit={(e) => { e.preventDefault(); void run() }}>
      <label>{t('supplier')}<Combobox filterable value={supplier} options={suppliers.map((s) => ({ value: s, label: s }))} onChange={(value) => setSupplier(value as string | null)} /></label>
      <Period from={from} to={to} setFrom={setFrom} setTo={setTo} />
      <button className="btn primary" type="submit" disabled={!supplier || loading}>{t('generate')}</button>
      <button className="btn secondary" type="button" disabled={!supplier} onClick={() => download('/cost-report/supplier-closing/export', query).catch((cause) => setError(apiMessage(cause, t('loadError'))))}><Download size={16} />{t('downloadCsv')}</button>
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
