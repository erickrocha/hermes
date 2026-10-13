import { useEffect, useMemo, useState } from 'react'
import { Link } from 'react-router-dom'
import { useDispatch, useSelector } from 'react-redux'
import { useTranslation } from 'react-i18next'
import { Trash2 } from 'lucide-react'
import type { ColumnDef } from '@tanstack/react-table'
import { can, OFFICE, OWNER, OWNER_MECHANIC } from '../features/access'
import type { FieldDef } from '../features/fields'
import { useActionForm, usePanel, useResourceForm, useSingletonForm } from '../features/hooks'
import { loadLookup } from '../features/lookupSlice'
import { fuelEntries, fuelGaugeSettings, internalTank, ruleSettings, type Row } from '../features/resources'
import type { AppDispatch, RootState } from '../store'
import { Confirm, PageHeader } from '../components/UI'
import { DetailCard } from '../components/molecules/DetailCard'
import { PanelSection } from '../components/molecules/PanelSection'
import { FilterBar } from '../components/organisms/FilterBar'
import { ResourceFormPage } from '../components/organisms/ResourceFormPage'
import { ResourceListPage } from '../components/organisms/ResourceListPage'
import { useCell, useMoney } from '../components/organisms/ResourceRoutes'

// C-029 (HRMS-942…955): o livro de abastecimentos, o comprovante do motorista, a conciliação, o relatório, o medidor do tanque e o tanque interno.
const num = (name: string, labelKey: string, required = false, min?: number): FieldDef => ({ name, labelKey, type: 'number', required, min })
export const entryFields: FieldDef[] = [
  { name: 'vehicleUuid', labelKey: 'vehicle', type: 'lookup', lookup: 'vehicle', required: true }, { name: 'recordedAt', labelKey: 'recordedAt', type: 'datetime', required: true },
  num('volumeLiters', 'volumeLiters', true, 0.0001), { name: 'valueCents', labelKey: 'amount', type: 'money', required: true, min: 0 }, num('odometerKm', 'odometerKm', false, 0),
  { name: 'station', labelKey: 'station', type: 'text' }, { name: 'fullTank', labelKey: 'fullTank', type: 'checkbox' },
]
export const receiptFields: FieldDef[] = [
  { name: 'vehicleUuid', labelKey: 'vehicle', type: 'lookup', lookup: 'vehicle', required: true }, { name: 'vehiclePrefix', labelKey: 'prefix', type: 'text', required: true },
  { name: 'recordedAt', labelKey: 'recordedAt', type: 'datetime', required: true }, num('volumeLiters', 'volumeLiters', true, 0.0001), { name: 'valueCents', labelKey: 'amount', type: 'money', required: true, min: 0 },
  num('odometerKm', 'odometerKm', false, 0), { name: 'station', labelKey: 'station', type: 'text' }, { name: 'fullTank', labelKey: 'fullTank', type: 'checkbox' },
  { name: 'overrideOdometer', labelKey: 'overrideOdometer', type: 'checkbox' }, { name: 'driverUuid', labelKey: 'driver', type: 'lookup', lookup: 'driver' },
]
export const unifyFields: FieldDef[] = [
  { name: 'targetUuid', labelKey: 'unifyTarget', type: 'lookup', lookup: 'fuelEntry', required: true }, { name: 'sourceUuid', labelKey: 'unifySource', type: 'lookup', lookup: 'fuelEntry', required: true },
]
const restoreFields: FieldDef[] = [{ name: 'vehicleUuid', labelKey: 'vehicle', type: 'lookup', lookup: 'vehicle', required: true }]
const reportFilters: FieldDef[] = [
  { name: 'vehicleUuid', labelKey: 'vehicle', type: 'lookup', lookup: 'vehicle' }, { name: 'from', labelKey: 'periodFrom', type: 'datetime' }, { name: 'to', labelKey: 'periodTo', type: 'datetime' }, { name: 'station', labelKey: 'station', type: 'text' },
]
const gaugeFilters: FieldDef[] = [{ name: 'vehicleUuid', labelKey: 'vehicle', type: 'lookup', lookup: 'vehicle', required: true }]
export const tankFields: FieldDef[] = [
  num('capacityLiters', 'capacityLiters', true, 0.0001), num('referenceStockLiters', 'referenceStockLiters', true, 0), { name: 'referenceAt', labelKey: 'referenceAt', type: 'datetime', required: true },
  num('alertLevelLiters', 'alertLevelLiters', true, 0), num('reserveLevelLiters', 'reserveLevelLiters', true, 0),
]
export const gaugeSettingFields: FieldDef[] = [
  num('suspectMarginRatio', 'suspectMarginRatio', true, 1), num('setAsideMinExpectedLiters', 'setAsideMinExpectedLiters', true, 0), num('setAsideRatio', 'setAsideRatio', true, 0.0001), num('largeFuellingRatio', 'largeFuellingRatio', true, 0.0001),
]
export const ruleFields: FieldDef[] = [
  num('avgMaxSegmentKm', 'avgMaxSegmentKm', true, 0.0001), num('avgMinKmPerLiter', 'avgMinKmPerLiter', true, 0.0001), num('avgMaxKmPerLiter', 'avgMaxKmPerLiter', true, 0.0001),
  num('avgRecentWindowDays', 'avgRecentWindowDays', true, 1), num('avgFallbackSegments', 'avgFallbackSegments', true, 1), num('avgMedianMinSegments', 'avgMedianMinSegments', true, 1),
  num('avgMedianTolerance', 'avgMedianTolerance', true, 0.0001), num('avgMedianMinSurvivors', 'avgMedianMinSurvivors', true, 1), num('avgMinSegmentsForOwnAverage', 'avgMinSegmentsForOwnAverage', true, 1),
  num('avgPeerCapacityTolerance', 'avgPeerCapacityTolerance', true, 0.0001), num('receiptDuplicateVolumeToleranceLiters', 'receiptDuplicateVolumeToleranceLiters', true, 0),
  num('receiptDuplicateWindowMinutes', 'receiptDuplicateWindowMinutes', true, 1), num('reconciliationWindowHours', 'reconciliationWindowHours', true, 1),
  num('reconciliationVolumeToleranceLiters', 'reconciliationVolumeToleranceLiters', true, 0), num('reconciliationVolumeToleranceRatio', 'reconciliationVolumeToleranceRatio', true, 0),
  num('preventiveAttentionKmMargin', 'preventiveAttentionKmMargin', true, 0), num('preventiveAttentionDaysMargin', 'preventiveAttentionDaysMargin', true, 0), num('preventiveMaxExtensionKm', 'preventiveMaxExtensionKm', true, 0.0001),
  num('avgPeerMinCount', 'avgPeerMinCount', true, 1),
  // Garagem (C-028): as constantes `[TC]` da validade do serviço, do alerta de tanque e do monitor, também por tenant.
  num('garageValidityHours', 'garageValidityHours', true, 1), num('garageMinTripAbsenceMinutes', 'garageMinTripAbsenceMinutes', true, 0), num('garageFuelPendingBelowPercent', 'garageFuelPendingBelowPercent', true, 0),
  num('garageTripFuelExemptPercent', 'garageTripFuelExemptPercent', true, 0), num('garageAlertLowPercent', 'garageAlertLowPercent', true, 0), num('garageAlertTripPercent', 'garageAlertTripPercent', true, 0),
  num('garageMonitorUrgentMinutes', 'garageMonitorUrgentMinutes', true, 0), num('garageMonitorTripWindowMinutes', 'garageMonitorTripWindowMinutes', true, 0), num('garageMonitorLineWindowMinutes', 'garageMonitorLineWindowMinutes', true, 0),
  num('garageMonitorCardLimit', 'garageMonitorCardLimit', true, 1), num('garageMonitorMatrixRows', 'garageMonitorMatrixRows', true, 1), num('garageMonitorMatrixColumns', 'garageMonitorMatrixColumns', true, 1),
  num('garageFuellingFreshnessHours', 'garageFuellingFreshnessHours', true, 0), num('garageUtcOffsetMinutes', 'garageUtcOffsetMinutes', true),
]

export function FuelPage() {
  const { t } = useTranslation(); const dispatch = useDispatch<AppDispatch>(); const cell = useCell()
  const session = useSelector((state: RootState) => state.auth.session); const vehicles = useSelector((state: RootState) => state.lookup.vehicle)
  const [removing, setRemoving] = useState<string | null>(null); const [syncing, setSyncing] = useState(false); const [outcome, setOutcome] = useState<Row | null>(null); const [version, setVersion] = useState(0)
  useEffect(() => { dispatch(loadLookup('vehicle')) }, [dispatch])
  const extra = useMemo(() => ({ r: version }), [version])
  const mayWrite = can(session, 'fuel-entry.create', [...OWNER_MECHANIC])
  const plate = (uuid: unknown) => vehicles?.find((v) => v.value === uuid)?.label ?? String(uuid ?? '—')
  const columns = useMemo<ColumnDef<Row, unknown>[]>(() => [
    { id: 'recordedAt', header: () => t('recordedAt'), cell: ({ row }) => cell({ key: 'recordedAt', labelKey: '', kind: 'datetime' }, row.original.recordedAt) },
    { id: 'vehicle', header: () => t('vehicle'), cell: ({ row }) => plate(row.original.vehicleUuid) },
    { id: 'volumeLiters', header: () => t('volumeLiters'), cell: ({ row }) => String(row.original.volumeLiters) },
    { id: 'valueCents', header: () => t('amount'), cell: ({ row }) => cell({ key: 'valueCents', labelKey: '', kind: 'money' }, row.original.valueCents) },
    { id: 'odometerKm', header: () => t('odometerKm'), cell: ({ row }) => cell({ key: 'odometerKm', labelKey: '' }, row.original.odometerKm) },
    { id: 'fullTank', header: () => t('fullTank'), cell: ({ row }) => (row.original.fullTank ? t('yes') : t('no')) },
    { id: 'origin', header: () => t('origin'), cell: ({ row }) => cell({ key: 'origin', labelKey: '', kind: 'badge', prefix: 'fuelOrigin' }, row.original.origin) },
    ...(can(session, 'fuel-entry.delete', [...OWNER_MECHANIC]) ? [{ id: 'actions', header: () => t('actions'), cell: ({ row }: { row: { original: Row } }) => <div className="row-actions">
      <button className="danger-text" aria-label={t('delete')} onClick={() => setRemoving(String(row.original.uuid))}><Trash2 size={16} /></button></div> } as ColumnDef<Row, unknown>] : []),
  // eslint-disable-next-line react-hooks/exhaustive-deps
  ], [t, vehicles, session])
  const remove = async () => { const uuid = removing; setRemoving(null); if (uuid) { await dispatch(fuelEntries.remove(uuid)); setVersion((n) => n + 1) } }
  const sync = async () => { setSyncing(false); const result = await dispatch(fuelEntries.act({ method: 'post', path: '/fuel-entry/sync' })); if (fuelEntries.act.fulfilled.match(result)) setOutcome(result.payload as Row); setVersion((n) => n + 1) }
  return <>
    <ResourceListPage resource={fuelEntries} titleKey="fuel" subtitleKey="fuelSummary" columns={columns} extra={extra} newTo={mayWrite ? '/fuel/new' : undefined} newKey="newFuelEntry"
      toolbar={<div className="inline-actions">
        {can(session, 'fuel.report.read', [...OFFICE]) && <Link className="btn secondary" to="/fuel/report">{t('fuelReport')}</Link>}
        {can(session, 'fuel.tank-gauge.read', [...OFFICE]) && <Link className="btn secondary" to="/fuel/gauge">{t('tankGauge')}</Link>}
        {can(session, 'internal-tank.read', [...OFFICE]) && <Link className="btn secondary" to="/fuel/tank">{t('internalTank')}</Link>}
        {can(session, 'fuel-entry.register-receipt', [...OWNER_MECHANIC, 'Driver']) && <Link className="btn secondary" to="/fuel/receipt">{t('registerReceipt')}</Link>}
        {can(session, 'fuel-entry.unify', [...OWNER_MECHANIC]) && <Link className="btn secondary" to="/fuel/unify">{t('unifyEntries')}</Link>}
        {can(session, 'fuel-entry.restore', [...OWNER_MECHANIC]) && <Link className="btn secondary" to="/fuel/restore">{t('restoreEntry')}</Link>}
        {can(session, 'fuel-entry.sync-provider', [...OWNER_MECHANIC]) && <button className="btn secondary" onClick={() => setSyncing(true)}>{t('syncProvider')}</button>}
        {can(session, 'fuel-gauge-settings.update', [...OWNER]) && <Link className="btn secondary" to="/fuel/settings">{t('fuelSettings')}</Link>}</div>} />
    {outcome && <p className="form-hint" role="status">{t('syncOutcome', { fetched: Number(outcome.fetched), imported: Number(outcome.imported), updated: Number(outcome.updated), reconciled: Number(outcome.reconciled), unmatched: Number(outcome.unmatched) })}</p>}
    {removing && <Confirm title={t('deleteTitle')} text={t('deleteText')} onCancel={() => setRemoving(null)} onConfirm={remove} />}
    {syncing && <Confirm title={t('syncProvider')} text={t('syncProviderText')} onCancel={() => setSyncing(false)} onConfirm={sync} />}</>
}

export function FuelEntryNewPage() { return <ResourceFormPage titleKey="newFuelEntry" fields={entryFields} backTo="/fuel" controller={useResourceForm(fuelEntries)} /> }
export function FuelReceiptPage() { return <ResourceFormPage titleKey="registerReceipt" fields={receiptFields} backTo="/fuel" controller={useResourceForm(fuelEntries, undefined, '/receipt', 'post')} /> }
export function FuelUnifyPage() { return <ResourceFormPage titleKey="unifyEntries" subtitleKey="unifyEntriesText" fields={unifyFields} backTo="/fuel" controller={useResourceForm(fuelEntries, undefined, '/unify', 'post')} /> }
export function FuelRestorePage() {
  const controller = useActionForm(fuelEntries, (body) => ({ method: 'post', path: `/fuel-entry/restore?vehicleUuid=${encodeURIComponent(String(body.vehicleUuid))}` }))
  return <ResourceFormPage titleKey="restoreEntry" subtitleKey="restoreEntryText" fields={restoreFields} backTo="/fuel" controller={controller} />
}

export function FuelReportPage() {
  const { t } = useTranslation(); const money = useMoney(); const cell = useCell()
  const [params, setParams] = useState<Record<string, unknown> | null>(null)
  const report = usePanel<{ entries: Row[]; totalLiters: number; totalValueCents: number }>(params ? '/fuel-entry/report' : null, (params ?? undefined) as never)
  return <>
    <PageHeader title={t('fuelReport')} subtitle={t('fuelReportText')} actions={<Link className="btn secondary" to="/fuel">{t('back')}</Link>} />
    <FilterBar fields={reportFilters} onApply={setParams} />
    {params && <PanelSection titleKey="fuelReport" loading={report.loading} error={report.error} empty={!report.data || report.data.entries.length === 0}>
      {report.data && <div className="table-card"><table>
        <thead><tr><th>{t('recordedAt')}</th><th>{t('station')}</th><th className="num">{t('volumeLiters')}</th><th className="num">{t('amount')}</th></tr></thead>
        <tbody>{report.data.entries.map((e) => <tr key={String(e.uuid)}><td>{cell({ key: 'recordedAt', labelKey: '', kind: 'datetime' }, e.recordedAt)}</td><td>{String(e.station ?? '—')}</td><td className="num">{String(e.volumeLiters)}</td><td className="num">{money(Number(e.valueCents))}</td></tr>)}
          <tr><th colSpan={2}>{t('total')}</th><th className="num">{report.data.totalLiters}</th><th className="num">{money(report.data.totalValueCents)}</th></tr></tbody></table></div>}
    </PanelSection>}</>
}

export function FuelGaugePage() {
  const { t } = useTranslation()
  const [vehicle, setVehicle] = useState<string | null>(null)
  const query = vehicle ? { vehicleUuid: vehicle } : undefined
  const gauge = usePanel<Row>(vehicle ? '/fuel-entry/gauge' : null, query)
  const average = usePanel<Row>(vehicle ? '/fuel-entry/average' : null, query)
  return <>
    <PageHeader title={t('tankGauge')} subtitle={t('tankGaugeText')} actions={<Link className="btn secondary" to="/fuel">{t('back')}</Link>} />
    <FilterBar fields={gaugeFilters} onApply={(p) => setVehicle(String(p.vehicleUuid))} />
    {vehicle && <>
      <PanelSection titleKey="tankGauge" loading={gauge.loading} error={gauge.error} empty={!gauge.data}>
        {gauge.data && (gauge.data.available ? <div className="stat-grid">
          <div className="stat-card"><span>{t('tankPercent')}</span><b>{Number(gauge.data.percent).toFixed(0)} %</b></div>
          <div className="stat-card"><span>{t('remainingLiters')}</span><b>{Number(gauge.data.remainingLiters).toFixed(0)} l</b></div>
          <div className="stat-card"><span>{t('rangeKm')}</span><b>{Number(gauge.data.rangeKm).toFixed(0)} km</b></div>
          {!!gauge.data.estimated && <div className="stat-card"><span>{t('estimated')}</span><b>{t('yes')}</b></div>}</div>
          : <p className="form-hint">{t(`gaugeReason${String(gauge.data.reason)}`, { defaultValue: String(gauge.data.reason ?? '') })}</p>)}
      </PanelSection>
      <PanelSection titleKey="consumptionAverage" loading={average.loading} error={average.error} empty={!average.data}>
        {average.data && <DetailCard rows={[
          { label: t('kmPerLiter'), value: average.data.kmPerLiter === undefined ? t('notEnoughSegments') : String(average.data.kmPerLiter) }, { label: t('segments'), value: String(average.data.segments) },
          { label: t('effectiveKmPerLiter'), value: String(average.data.effectiveKmPerLiter ?? '') }, { label: t('effectiveSource'), value: String(average.data.effectiveSource ?? '') },
        ]} />}
      </PanelSection></>}</>
}

export function InternalTankPage() {
  const { t } = useTranslation(); const session = useSelector((state: RootState) => state.auth.session)
  const tank = usePanel<Row>('/internal-tank'); const stock = usePanel<Row>('/internal-tank/stock')
  return <>
    <PageHeader title={t('internalTank')} subtitle={t('internalTankText')} actions={<>
      <Link className="btn secondary" to="/fuel">{t('back')}</Link>
      {can(session, 'internal-tank.update', [...OWNER]) && <Link className="btn primary" to="/fuel/tank/edit">{t('configure')}</Link>}</>} />
    <PanelSection titleKey="internalTank" loading={tank.loading} error={tank.error ? 'tankNotConfigured' : ''} empty={!tank.data}>
      {tank.data && <DetailCard rows={[
        { label: t('capacityLiters'), value: String(tank.data.capacityLiters) }, { label: t('referenceStockLiters'), value: String(tank.data.referenceStockLiters) },
        { label: t('referenceAt'), value: String(tank.data.referenceAt ?? '').slice(0, 16).replace('T', ' ') }, { label: t('alertLevelLiters'), value: String(tank.data.alertLevelLiters) }, { label: t('reserveLevelLiters'), value: String(tank.data.reserveLevelLiters) },
      ]} />}
    </PanelSection>
    {!tank.error && <PanelSection titleKey="tankStock" loading={stock.loading} error={stock.error} empty={!stock.data}>
      {stock.data && (stock.data.currentStockLiters === undefined || stock.data.currentStockLiters === null
        ? <p className="form-hint">{t('stockWithheld')}</p>
        : <div className="stat-grid"><div className="stat-card"><span>{t('currentStock')}</span><b>{String(stock.data.currentStockLiters)} l</b></div>
          {!!stock.data.deliveryAlert && <div className="stat-card"><span>{t('deliveryAlert')}</span><b>{t('yes')}</b></div>}</div>)}
    </PanelSection>}</>
}

export function InternalTankConfigPage() { return <ResourceFormPage titleKey="configureTank" fields={tankFields} backTo="/fuel/tank" controller={useSingletonForm(internalTank, '/internal-tank')} /> }
export function FuelSettingsPage() {
  const { t } = useTranslation()
  return <>
    <ResourceFormPage titleKey="fuelSettings" subtitleKey="fuelSettingsText" fields={gaugeSettingFields} backTo="/fuel" controller={useSingletonForm(fuelGaugeSettings, '/fuel-gauge-settings')} />
    <p className="form-hint"><Link to="/fuel/rules">{t('ruleSettings')}</Link></p></>
}
export function RuleSettingsPage() { return <ResourceFormPage titleKey="ruleSettings" subtitleKey="ruleSettingsText" fields={ruleFields} backTo="/fuel/settings" controller={useSingletonForm(ruleSettings, '/tenant-rule-settings')} /> }

