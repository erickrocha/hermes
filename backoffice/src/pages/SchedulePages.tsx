import { useEffect, useState, type ChangeEvent } from 'react'
import { Link } from 'react-router-dom'
import { useDispatch, useSelector } from 'react-redux'
import { useTranslation } from 'react-i18next'
import { parseCsv } from '../csv'
import type { FieldDef } from '../features/fields'
import { usePanel, useResourceState } from '../features/hooks'
import { loadLookup, type LookupOption } from '../features/lookupSlice'
import { extraTrips, type Row } from '../features/resources'
import type { AppDispatch, RootState } from '../store'
import { PageHeader } from '../components/UI'
import { PanelSection } from '../components/molecules/PanelSection'
import { FilterBar } from '../components/organisms/FilterBar'

// C-024 (HRMS-603…610): a escala efetiva do dia, calculada pelo servidor, e a importação de viagens por CSV com pré-visualização.
const today = () => new Date().toISOString().slice(0, 10)
const dayFilter: FieldDef[] = [{ name: 'date', labelKey: 'date', type: 'date', required: true, defaultValue: today() }]
const NUMERIC = new Set(['freightValueCents'])
const BOOLEAN = new Set<string>()

const labelOf = (options: LookupOption[] | undefined, uuid: unknown) => options?.find((o) => o.value === uuid)?.label ?? (uuid ? String(uuid) : '—')
const hhmm = (value: unknown) => String(value ?? '').slice(11, 16) || '—'

export function SchedulePage() {
  const { t } = useTranslation(); const dispatch = useDispatch<AppDispatch>(); const lookups = useSelector((state: RootState) => state.lookup)
  const [date, setDate] = useState(today())
  useEffect(() => { (['vehicle', 'driver'] as const).forEach((kind) => dispatch(loadLookup(kind))) }, [dispatch])
  const schedule = usePanel<{ items: Row[]; unclassifiedDemands: number; malformedDemands: number }>('/schedule/effective', { date })
  const data = schedule.data
  return <>
    <PageHeader title={t('schedule')} subtitle={t('scheduleText')} />
    <FilterBar fields={dayFilter} onApply={(p) => setDate(String(p.date ?? today()))} applyKey="show" />
    <PanelSection titleKey="schedule" loading={schedule.loading} error={schedule.error} empty={!data || data.items.length === 0}>
      {data && <div className="table-card"><table>
        <thead><tr><th>{t('scheduleItem')}</th><th>{t('startTime')}</th><th>{t('endTime')}</th><th>{t('driver')}</th><th>{t('vehicle')}</th><th>{t('crewFrom')}</th><th>{t('alerts')}</th></tr></thead>
        <tbody>{data.items.map((item, i) => <tr key={`${String(item.demandUuid)}-${i}`}>
          <td><b>{String(item.name ?? '—')}</b><small>{t(`scheduleKind${String(item.kind)}`, { defaultValue: String(item.kind) })}</small></td><td>{hhmm(item.start)}</td><td>{hhmm(item.end)}</td>
          <td>{labelOf(lookups.driver, item.driverUuid)}</td><td>{labelOf(lookups.vehicle, item.vehicleUuid)}</td><td>{t(`crewFrom${String(item.resolvedFrom)}`, { defaultValue: String(item.resolvedFrom) })}</td>
          <td>{[...((item.alerts as string[] | undefined) ?? []).map((a) => t(`scheduleAlert${a}`, { defaultValue: a })), item.displacedByTrip ? t('displacedByTrip') : ''].filter(Boolean).join(' · ') || '—'}</td></tr>)}</tbody></table></div>}
    </PanelSection>
    {data && (data.unclassifiedDemands > 0 || data.malformedDemands > 0) && <p className="form-hint" role="status">{t('scheduleSkipped', { unclassified: data.unclassifiedDemands, malformed: data.malformedDemands })}</p>}</>
}

const readText = (file: File) => new Promise<string>((resolve, reject) => { const reader = new FileReader(); reader.onload = () => resolve(String(reader.result)); reader.onerror = () => reject(reader.error); reader.readAsText(file) })

/// Linhas do CSV viram viagens: o cabeçalho usa os nomes dos campos da API; colunas vazias somem e os números viram números.
export function tripsFromCsv(text: string): Record<string, unknown>[] {
  const { headers, rows } = parseCsv(text)
  return rows.map((cells) => Object.fromEntries(headers.flatMap((header, i) => {
    const cell = cells[i]?.trim() ?? ''
    if (cell === '') return []
    return [[header, NUMERIC.has(header) ? Number(cell) : BOOLEAN.has(header) ? cell === 'true' : cell]]
  })))
}

export function TripImportPage() {
  const { t } = useTranslation(); const dispatch = useDispatch<AppDispatch>(); const state = useResourceState(extraTrips)
  const [trips, setTrips] = useState<Record<string, unknown>[]>([]); const [preview, setPreview] = useState<{ created: number; updated: number; rows: Row[] } | null>(null)
  const [done, setDone] = useState<Row | null>(null); const [fileError, setFileError] = useState('')
  const choose = async (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0]; setPreview(null); setDone(null); setFileError('')
    if (!file) return
    try {
      const parsed = tripsFromCsv(await readText(file)); setTrips(parsed)
      if (parsed.length === 0) { setFileError('importEmpty'); return }
      const result = await dispatch(extraTrips.act({ method: 'post', path: '/extra-trip/import/preview', body: parsed }))
      if (extraTrips.act.fulfilled.match(result)) setPreview(result.payload as never)
    } catch { setFileError('loadError') }
  }
  const apply = async () => { const result = await dispatch(extraTrips.act({ method: 'post', path: '/extra-trip/import', body: trips })); if (extraTrips.act.fulfilled.match(result)) { setDone(result.payload as Row); setPreview(null) } }
  return <>
    <PageHeader title={t('importTrips')} subtitle={t('importTripsText')} actions={<Link className="btn secondary" to="/extra-trips">{t('back')}</Link>} />
    <div className="page-form-card">
      <label>{t('csvFile')}<input type="file" accept=".csv,text/csv" onChange={choose} /></label>
      {fileError && <p className="form-error" role="alert">{t(fileError)}</p>}
      {state.error && <p className="form-error" role="alert">{t(state.error, { defaultValue: state.error })}</p>}
      {preview && <><p className="form-hint" role="status">{t('importPreview', { created: preview.created, updated: preview.updated })}</p>
        <div className="table-card"><table><thead><tr><th>#</th><th>{t('orderCode')}</th><th>{t('tripDate')}</th><th>{t('importAction')}</th></tr></thead>
          <tbody>{preview.rows.map((r) => <tr key={String(r.row)}><td>{String(r.row)}</td><td>{String(r.orderCode)}</td><td>{String(r.tripDate)}</td><td>{t(`importAction${String(r.action)}`)}</td></tr>)}</tbody></table></div>
        <div className="form-actions split"><Link className="btn secondary" to="/extra-trips">{t('cancel')}</Link><button className="btn primary" disabled={state.saving} onClick={apply}>{t('importConfirm')}</button></div></>}
      {done && <p className="form-hint" role="status">{t('importDone', { created: Number(done.created ?? 0), updated: Number(done.updated ?? 0) })}</p>}
    </div></>
}
