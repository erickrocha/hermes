import { useEffect, useState } from 'react'
import { Link, useParams } from 'react-router-dom'
import { useDispatch, useSelector } from 'react-redux'
import { useTranslation } from 'react-i18next'
import { can, OFFICE, OWNER_MECHANIC } from '../features/access'
import type { FieldDef } from '../features/fields'
import { useAction, useActionForm, usePanel, useResourceForm, useResourceState } from '../features/hooks'
import { loadLookup } from '../features/lookupSlice'
import { garageAttendances, type Row } from '../features/resources'
import type { AppDispatch, RootState } from '../store'
import { Confirm, PageHeader } from '../components/UI'
import { DetailCard } from '../components/molecules/DetailCard'
import { PanelSection } from '../components/molecules/PanelSection'
import { StateView } from '../components/molecules/StateView'
import { ResourceFormPage } from '../components/organisms/ResourceFormPage'

// C-028 (HRMS-956…970): a triagem do pátio, a fila, a lista de chamada à base e o painel do monitor de parede.
const READERS = ['TenantOwner', 'TenantUser', 'Mechanic'] as const
const openFields: FieldDef[] = [{ name: 'vehicleUuid', labelKey: 'vehicle', type: 'lookup', lookup: 'vehicle', required: true }, { name: 'manualPriority', labelKey: 'manualPriority', type: 'number', min: 0 }]
const callFields: FieldDef[] = [{ name: 'vehicleUuid', labelKey: 'vehicle', type: 'lookup', lookup: 'vehicle', required: true }]
const rows = (data: unknown): Row[] => (Array.isArray(data) ? data : []) as Row[]

function useVehicleLabel() {
  const dispatch = useDispatch<AppDispatch>(); const vehicles = useSelector((state: RootState) => state.lookup.vehicle)
  useEffect(() => { dispatch(loadLookup('vehicle')) }, [dispatch])
  return (uuid: unknown, prefix?: unknown) => String(prefix || vehicles?.find((v) => v.value === uuid)?.label || uuid || '—')
}

export function GaragePage() {
  const { t } = useTranslation(); const session = useSelector((state: RootState) => state.auth.session); const label = useVehicleLabel()
  const queue = usePanel<Row[]>('/garage-attendance/queue')
  return <>
    <PageHeader title={t('garage')} subtitle={t('garageSummary')} actions={<>
      {can(session, 'garage.triage.open', [...OWNER_MECHANIC]) && <Link className="btn primary" to="/garage/open">{t('openTriage')}</Link>}
      {can(session, 'garage.call.create', [...OWNER_MECHANIC]) && <Link className="btn secondary" to="/garage/call">{t('callToBase')}</Link>}
      {can(session, 'garage.call.cancel', [...OWNER_MECHANIC]) && <Link className="btn secondary" to="/garage/call/cancel">{t('cancelCall')}</Link>}
      {can(session, 'garage.monitor.read', [...READERS]) && <Link className="btn secondary" to="/garage/monitor">{t('garageMonitor')}</Link>}
      {session?.role === 'TenantOwner' && <Link className="btn secondary" to="/garage/devices">{t('garageDevices')}</Link>}
      {can(session, 'garage.service-catalogue.read', [...OFFICE]) && <Link className="btn secondary" to="/garage-services">{t('garageServices')}</Link>}</>} />
    <PanelSection titleKey="yardQueue" loading={queue.loading} error={queue.error} empty={rows(queue.data).length === 0}>
      <div className="table-card"><table>
        <thead><tr><th>{t('vehicle')}</th><th>{t('manualPriority')}</th><th>{t('nextDeparture')}</th><th>{t('requiredPending')}</th><th>{t('alerts')}</th></tr></thead>
        <tbody>{rows(queue.data).map((entry) => <tr key={String(entry.vehicleUuid)}>
          <td>{entry.attendanceUuid ? <Link to={`/garage/triage/${entry.attendanceUuid}`}><b>{label(entry.vehicleUuid, entry.prefix)}</b></Link> : label(entry.vehicleUuid, entry.prefix)}</td>
          <td>{String(entry.manualPriority ?? '—')}</td><td>{String(entry.nextDeparture ?? '—').slice(0, 16).replace('T', ' ')}</td>
          <td>{((entry.requiredPending as string[] | undefined) ?? []).join(', ') || '—'}</td>
          <td>{[entry.fillTankAlert && t('fillTankAlert'), entry.calledByManager && t('calledByManager')].filter(Boolean).join(' · ') || '—'}</td></tr>)}</tbody></table></div>
    </PanelSection></>
}

export function GarageOpenPage() {
  return <ResourceFormPage titleKey="openTriage" fields={openFields} backTo="/garage" controller={useResourceForm(garageAttendances)} />
}
export function GarageCallPage() {
  const controller = useActionForm(garageAttendances, (body) => ({ method: 'post', path: '/garage-call', body }))
  return <ResourceFormPage titleKey="callToBase" subtitleKey="callToBaseText" fields={callFields} backTo="/garage" controller={controller} />
}

export function GarageCallCancelPage() {
  const controller = useActionForm(garageAttendances, (body) => ({ method: 'post', path: '/garage-call/cancel', body }))
  return <ResourceFormPage titleKey="cancelCall" fields={callFields} backTo="/garage" controller={controller} />
}

export function GarageTriagePage() {
  const { uuid } = useParams(); const { t } = useTranslation(); const dispatch = useDispatch<AppDispatch>(); const run = useAction(garageAttendances)
  const session = useSelector((state: RootState) => state.auth.session); const label = useVehicleLabel(); const state = useResourceState(garageAttendances)
  const [checkout, setCheckout] = useState(false)
  useEffect(() => { if (uuid) dispatch(garageAttendances.fetchOne(uuid)) }, [dispatch, uuid])
  const triage = state.current as (Row & { services?: Row[] }) | null
  const mark = async (service: Row, to: string) => { if (await run('put', `/garage-attendance/uuid/${uuid}/service/${service.uuid}`, { state: to })) dispatch(garageAttendances.fetchOne(String(uuid))) }
  const finish = async () => { setCheckout(false); if (await run('post', `/garage-attendance/uuid/${uuid}/checkout`)) dispatch(garageAttendances.fetchOne(String(uuid))) }
  const open = triage?.status === 'Open'; const mayMark = can(session, 'garage.triage.mark-service', [...OWNER_MECHANIC])
  return <>
    <PageHeader title={triage ? label(triage.vehicleUuid) : t('garage')} actions={<>
      <Link className="btn secondary" to="/garage">{t('back')}</Link>
      {open && can(session, 'garage.triage.check-out', [...OWNER_MECHANIC]) && <button className="btn primary" onClick={() => setCheckout(true)}>{t('checkOut')}</button>}</>} />
    {state.error && triage && <p className="form-error" role="alert">{t(state.error, { defaultValue: state.error })}</p>}
    <StateView loading={state.loading && !triage} error={triage ? '' : state.error}>
      {triage && <>
        <DetailCard rows={[{ label: t('status'), value: t(`triageStatus${String(triage.status)}`, { defaultValue: String(triage.status) }) }, { label: t('checkedInAt'), value: String(triage.checkedInAt ?? '').slice(0, 16).replace('T', ' ') },
          { label: t('manualPriority'), value: String(triage.manualPriority ?? '') }, { label: t('releasedAt'), value: String(triage.releasedAt ?? '').slice(0, 16).replace('T', ' ') }]} />
        <PanelSection titleKey="garageServices" loading={false} empty={(triage.services ?? []).length === 0}>
          <div className="table-card"><table><thead><tr><th>{t('name')}</th><th>{t('serviceGroup')}</th><th>{t('serviceState')}</th>{open && mayMark && <th>{t('actions')}</th>}</tr></thead>
            <tbody>{(triage.services ?? []).map((service) => <tr key={String(service.uuid)}>
              <td>{String(service.name)}{!!service.requiredForDeparture && <small>{t('requiredForDeparture')}</small>}</td><td>{t(`serviceGroup${String(service.serviceGroup)}`)}</td>
              <td><span className="badge blue">{t(`serviceState${String(service.effectiveState)}`)}</span></td>
              {open && mayMark && <td><div className="inline-actions">{['Performed', 'NotNeeded', 'Pending'].map((to) => <button key={to} className="btn tertiary" disabled={service.state === to || state.saving} onClick={() => mark(service, to)}>{t(`serviceState${to}`)}</button>)}</div></td>}</tr>)}</tbody></table></div>
        </PanelSection></>}
    </StateView>
    {checkout && <Confirm title={t('checkOut')} text={t('checkOutText')} onCancel={() => setCheckout(false)} onConfirm={finish} />}</>
}

export function GarageMonitorPage() {
  const { t } = useTranslation(); const label = useVehicleLabel()
  const monitor = usePanel<Row[]>('/garage-attendance/monitor'); const cards = usePanel<{ cards: Row[]; notShown: number }>('/garage-attendance/monitor/cards')
  const callList = usePanel<Row[]>('/garage-attendance/call-list'); const matrix = usePanel<{ columns: Row[]; rows: Row[]; rowsNotShown: number }>('/garage-attendance/monitor/matrix')
  return <>
    <PageHeader title={t('garageMonitor')} subtitle={t('garageMonitorText')} actions={<Link className="btn secondary" to="/garage">{t('back')}</Link>} />
    <PanelSection titleKey="monitorReadiness" loading={monitor.loading} error={monitor.error} empty={rows(monitor.data).length === 0}>
      <div className="table-card"><table><thead><tr><th>{t('vehicle')}</th><th>{t('readiness')}</th><th>{t('nextDeparture')}</th></tr></thead>
        <tbody>{rows(monitor.data).map((m) => { const entry = m.entry as Row; return <tr key={String(entry.vehicleUuid)}><td>{label(entry.vehicleUuid, entry.prefix)}</td>
          <td><span className={`badge ${m.urgent ? 'gray' : 'blue'}`}>{t(`readiness${String(m.readiness ?? 'None')}`, { defaultValue: String(m.readiness ?? '—') })}{m.urgent ? ` · ${t('urgent')}` : ''}</span></td>
          <td>{String(entry.nextDeparture ?? '—').slice(0, 16).replace('T', ' ')}</td></tr> })}</tbody></table></div>
    </PanelSection>
    <PanelSection titleKey="vehicleCards" loading={cards.loading} error={cards.error} empty={!cards.data || cards.data.cards.length === 0}>
      {cards.data && <div className="stat-grid">{cards.data.cards.map((c) => <div className="stat-card" key={String(c.vehicleUuid)}><span>{label(c.vehicleUuid, c.prefix)}</span>
        <b>{c.tankPercent === undefined || c.tankPercent === null ? '—' : `${Number(c.tankPercent).toFixed(0)} %`}</b>{!!c.needsRefuel && <small>{t('needsRefuel')}</small>}</div>)}
        {cards.data.notShown > 0 && <div className="stat-card"><span>{t('notShown')}</span><b>{cards.data.notShown}</b></div>}</div>}
    </PanelSection>
    <PanelSection titleKey="callList" loading={callList.loading} error={callList.error} empty={rows(callList.data).length === 0}>
      <div className="table-card"><table><thead><tr><th>{t('vehicle')}</th><th>{t('nextDeparture')}</th><th>{t('calledByManager')}</th><th>{t('tankCalls')}</th></tr></thead>
        <tbody>{rows(callList.data).map((c) => <tr key={String(c.vehicleUuid)}><td>{label(c.vehicleUuid, c.prefix)}</td><td>{String(c.nextDeparture ?? '—').slice(0, 16).replace('T', ' ')}</td>
          <td>{c.calledByManager ? t('yes') : t('no')}</td><td>{c.tankCalls ? t('yes') : t('no')}</td></tr>)}</tbody></table></div>
    </PanelSection>
    <PanelSection titleKey="serviceMatrix" loading={matrix.loading} error={matrix.error} empty={!matrix.data || matrix.data.rows.length === 0}>
      {matrix.data && <div className="table-card"><table><thead><tr><th>{t('vehicle')}</th>{matrix.data.columns.map((c) => <th key={String(c.serviceModelUuid)}>{String(c.name)}</th>)}</tr></thead>
        <tbody>{matrix.data.rows.map((r) => <tr key={String(r.vehicleUuid)}><td>{label(r.vehicleUuid, r.prefix)}</td>
          {((r.cells as (Row | null)[]) ?? []).map((cell, i) => <td key={i}>{cell ? t(`serviceState${String(cell.effectiveState)}`) : '—'}</td>)}</tr>)}</tbody></table></div>}
    </PanelSection></>
}
