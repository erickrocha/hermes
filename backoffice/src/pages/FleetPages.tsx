import { useEffect, useState } from 'react'
import { Link, useParams } from 'react-router-dom'
import { useDispatch, useSelector } from 'react-redux'
import { useTranslation } from 'react-i18next'
import { can, OWNER } from '../features/access'
import type { FieldDef } from '../features/fields'
import { useAction, usePanel, useResourceForm, useResourceState } from '../features/hooks'
import { customers, transportDemands, vehicles, type Row } from '../features/resources'
import type { AppDispatch, RootState } from '../store'
import { Confirm, PageHeader } from '../components/UI'
import { DetailCard } from '../components/molecules/DetailCard'
import { PanelSection } from '../components/molecules/PanelSection'
import { StateView } from '../components/molecules/StateView'
import { ChildList } from '../components/organisms/ChildList'
import { ResourceFormPage } from '../components/organisms/ResourceFormPage'

// C-021/C-024/C-025: o veículo (quilometragem, atribuição, posição), o cliente (folgas) e a demanda de transporte (alocações, escala do dia, exceções).
const originOptions = ['Manual', 'Adjustment', 'InitialRegistration']
const odometerFields: FieldDef[] = [
  { name: 'km', labelKey: 'odometerKm', type: 'number', required: true, min: 0 }, { name: 'recordedAt', labelKey: 'recordedAt', type: 'datetime' },
  { name: 'origin', labelKey: 'origin', type: 'select', options: originOptions, optionPrefix: 'kmOrigin', required: true }, { name: 'notes', labelKey: 'observation', type: 'text' },
]
const assignFields: FieldDef[] = [{ name: 'driverUuid', labelKey: 'driver', type: 'lookup', lookup: 'driver', required: true }]
const dayOffFields: FieldDef[] = [{ name: 'date', labelKey: 'date', type: 'date', required: true }, { name: 'reason', labelKey: 'reason', type: 'text' }]
const allocationFields: FieldDef[] = [
  { name: 'driverUuid', labelKey: 'driver', type: 'lookup', lookup: 'driver', required: true }, { name: 'vehicleUuid', labelKey: 'vehicle', type: 'lookup', lookup: 'vehicle', required: true },
  { name: 'daysOfWeek', labelKey: 'daysOfWeek', type: 'text' }, { name: 'startDate', labelKey: 'startDate', type: 'date', required: true }, { name: 'endDate', labelKey: 'endDate', type: 'date' }, { name: 'active', labelKey: 'activeFlag', type: 'checkbox', defaultValue: true },
]
const dailyFields: FieldDef[] = [
  { name: 'driverUuid', labelKey: 'driver', type: 'lookup', lookup: 'driver', required: true }, { name: 'vehicleUuid', labelKey: 'vehicle', type: 'lookup', lookup: 'vehicle', required: true },
  { name: 'date', labelKey: 'date', type: 'date', required: true }, { name: 'startTime', labelKey: 'startTime', type: 'time' }, { name: 'endTime', labelKey: 'endTime', type: 'time' }, { name: 'notes', labelKey: 'observation', type: 'text' },
]
const exceptionFields: FieldDef[] = [
  { name: 'date', labelKey: 'date', type: 'date', required: true }, { name: 'exceptionType', labelKey: 'exceptionType', type: 'select', options: ['Cancellation', 'Deallocation', 'Substitution'], optionPrefix: 'exceptionType', required: true },
  { name: 'newDriverUuid', labelKey: 'driver', type: 'lookup', lookup: 'driver' }, { name: 'newVehicleUuid', labelKey: 'vehicle', type: 'lookup', lookup: 'vehicle' }, { name: 'reason', labelKey: 'reason', type: 'text' },
]

export function VehicleDetailPage() {
  const { uuid } = useParams(); const { t } = useTranslation(); const dispatch = useDispatch<AppDispatch>(); const run = useAction(vehicles)
  const session = useSelector((state: RootState) => state.auth.session); const state = useResourceState(vehicles); const vehicle = state.current as Row | null
  const [ending, setEnding] = useState(false); const [refresh, setRefresh] = useState(0)
  useEffect(() => { if (uuid) dispatch(vehicles.fetchOne(uuid)) }, [dispatch, uuid])
  const assignment = usePanel<Row>(`/vehicle/uuid/${uuid}/assignment`)
  const tracking = usePanel<Row>(`/vehicle/uuid/${uuid}/tracking`)
  const endAssignment = async () => { setEnding(false); if (await run('put', `/vehicle/uuid/${uuid}/assignment/end`)) { assignment.reload(); setRefresh((n) => n + 1) } }
  const holder = assignment.error ? undefined : assignment.data
  return <>
    <PageHeader title={vehicle ? `${vehicle.plate} — ${vehicle.model}` : t('vehicles')} actions={<>
      <Link className="btn secondary" to="/vehicles">{t('back')}</Link>
      {can(session, 'vehicle.update', [...OWNER]) && <Link className="btn secondary" to={`/vehicles/${uuid}/edit`}>{t('edit')}</Link>}
      {can(session, 'vehicle.odometer.record', [...OWNER]) && <Link className="btn primary" to={`/vehicles/${uuid}/odometer`}>{t('recordOdometer')}</Link>}</>} />
    <StateView loading={state.loading} error={vehicle ? '' : state.error}>
      {vehicle && <DetailCard rows={[
        { label: t('status'), value: <span className="badge blue">{t(`vehicleStatus${String(vehicle.status)}`)}</span> }, { label: t('prefix'), value: String(vehicle.prefix ?? '') },
        { label: t('vehicleType'), value: String(vehicle.vehicleType ?? '') }, { label: t('odometerKm'), value: String(vehicle.odometerKm ?? '') },
        { label: t('tankCapacityLiters'), value: String(vehicle.tankCapacityLiters ?? '') }, { label: t('referenceKmPerLiter'), value: String(vehicle.referenceKmPerLiter ?? '') }, { label: t('garageTag'), value: String(vehicle.garageTag ?? '') },
      ]} />}
    </StateView>
    <PanelSection titleKey="currentDriver" loading={assignment.loading} empty={!holder}
      actions={<>{can(session, 'vehicle.assign-driver', [...OWNER]) && !holder && <Link className="btn secondary" to={`/vehicles/${uuid}/assign`}>{t('assignDriver')}</Link>}
        {can(session, 'vehicle.end-assignment', [...OWNER]) && holder && <button className="btn secondary danger-text" onClick={() => setEnding(true)}>{t('endAssignment')}</button>}</>}>
      {holder && <DetailCard rows={[{ label: t('driver'), value: String(holder.driverName ?? '') }, { label: t('startedAt'), value: String(holder.startedAt ?? '').slice(0, 16).replace('T', ' ') }]} />}
    </PanelSection>
    <PanelSection titleKey="tracking" loading={tracking.loading} error={tracking.error ? 'trackingUnavailable' : ''} empty={!tracking.data}>
      {tracking.data && <DetailCard rows={[
        { label: t('ignition'), value: String(tracking.data.ignition ?? '') }, { label: t('trackingSource'), value: String(tracking.data.source ?? '') },
        { label: t('reportedAt'), value: String(tracking.data.reportedAt ?? '').slice(0, 16).replace('T', ' ') }, { label: t('trustworthy'), value: tracking.data.trustworthy ? t('yes') : t('no') },
      ]} />}
    </PanelSection>
    <ChildList refresh={refresh} titleKey="assignmentHistory" path={`/vehicle/uuid/${uuid}/assignments`} columns={[{ key: 'driverName', labelKey: 'driver' }, { key: 'startedAt', labelKey: 'startedAt', kind: 'datetime' }, { key: 'endedAt', labelKey: 'endedAt', kind: 'datetime' }]} />
    <ChildList titleKey="odometerHistory" path={`/vehicle/uuid/${uuid}/km-evolutions`} columns={[{ key: 'recordedAt', labelKey: 'recordedAt', kind: 'datetime' }, { key: 'km', labelKey: 'odometerKm', kind: 'number' }, { key: 'origin', labelKey: 'origin', kind: 'badge', prefix: 'kmOrigin' }, { key: 'notes', labelKey: 'observation' }]} />
    {ending && <Confirm title={t('endAssignment')} text={t('endAssignmentText')} onCancel={() => setEnding(false)} onConfirm={endAssignment} />}</>
}

export function VehicleOdometerPage() {
  const { uuid } = useParams()
  return <ResourceFormPage titleKey="recordOdometer" fields={odometerFields} backTo={`/vehicles/${uuid}`} controller={useResourceForm(vehicles, uuid, '/km-evolution', 'post')} />
}
export function VehicleAssignPage() {
  const { uuid } = useParams()
  return <ResourceFormPage titleKey="assignDriver" fields={assignFields} backTo={`/vehicles/${uuid}`} controller={useResourceForm(vehicles, uuid, '/assignment', 'post')} />
}

export function CustomerDetailPage() {
  const { uuid } = useParams(); const { t } = useTranslation(); const dispatch = useDispatch<AppDispatch>(); const run = useAction(customers)
  const session = useSelector((state: RootState) => state.auth.session); const state = useResourceState(customers); const customer = state.current as Row | null
  const [removing, setRemoving] = useState<Row | null>(null); const [refresh, setRefresh] = useState(0)
  useEffect(() => { if (uuid) dispatch(customers.fetchOne(uuid)) }, [dispatch, uuid])
  const remove = async () => { const row = removing; setRemoving(null); if (row && await run('delete', `/customer/uuid/${uuid}/day-off/uuid/${row.uuid}`)) setRefresh((n) => n + 1) }
  return <>
    <PageHeader title={customer ? String(customer.name) : t('customers')} actions={<>
      <Link className="btn secondary" to="/customers">{t('back')}</Link>
      {can(session, 'customer.update', [...OWNER]) && <Link className="btn secondary" to={`/customers/${uuid}/edit`}>{t('edit')}</Link>}</>} />
    <StateView loading={state.loading} error={customer ? '' : state.error}>
      {customer && <DetailCard rows={[{ label: t('status'), value: t(`customerStatus${String(customer.status)}`) }, { label: t('observation'), value: String(customer.notes ?? '') }]} />}
    </StateView>
    <ChildList refresh={refresh} titleKey="customerDaysOff" path={`/customer/uuid/${uuid}/day-offs`} columns={[{ key: 'date', labelKey: 'date', kind: 'date' }, { key: 'reason', labelKey: 'reason' }]}
      addTo={can(session, 'customer.day-off.create', [...OWNER]) ? `/customers/${uuid}/day-offs/new` : undefined} addKey="newDayOff" onDelete={can(session, 'customer.day-off.delete', [...OWNER]) ? setRemoving : undefined} />
    {removing && <Confirm title={t('deleteTitle')} text={t('deleteText')} onCancel={() => setRemoving(null)} onConfirm={remove} />}</>
}
export function CustomerDayOffPage() {
  const { uuid } = useParams()
  return <ResourceFormPage titleKey="newDayOff" fields={dayOffFields} backTo={`/customers/${uuid}`} controller={useResourceForm(customers, uuid, '/day-off', 'post')} />
}

export function DemandDetailPage() {
  const { uuid } = useParams(); const { t } = useTranslation(); const dispatch = useDispatch<AppDispatch>(); const run = useAction(transportDemands)
  const session = useSelector((state: RootState) => state.auth.session); const state = useResourceState(transportDemands); const demand = state.current as Row | null
  const [removing, setRemoving] = useState<{ kind: 'daily-schedule' | 'exception'; row: Row } | null>(null); const [refresh, setRefresh] = useState(0)
  useEffect(() => { if (uuid) dispatch(transportDemands.fetchOne(uuid)) }, [dispatch, uuid])
  const remove = async () => { const target = removing; setRemoving(null); if (target && await run('delete', `/transport-demand/uuid/${uuid}/${target.kind}/uuid/${target.row.uuid}`)) setRefresh((n) => n + 1) }
  const owner = can(session, 'transport-demand.update', [...OWNER])
  return <>
    <PageHeader title={demand ? String(demand.lineName ?? demand.demandType) : t('transportDemands')} actions={<>
      <Link className="btn secondary" to="/transport-demands">{t('back')}</Link>{owner && <Link className="btn secondary" to={`/transport-demands/${uuid}/edit`}>{t('edit')}</Link>}</>} />
    <StateView loading={state.loading} error={demand ? '' : state.error}>
      {demand && <DetailCard rows={[
        { label: t('demandKind'), value: demand.demandKind ? t(`demandKind${String(demand.demandKind)}`) : '' }, { label: t('daysOfWeek'), value: String(demand.daysOfWeek ?? '') }, { label: t('specificDate'), value: String(demand.specificDate ?? '') },
        { label: t('shiftStart'), value: String(demand.shiftStart ?? '') }, { label: t('shiftEnd'), value: String(demand.shiftEnd ?? '') }, { label: t('activeFlag'), value: demand.active ? t('yes') : t('no') },
      ]} />}
    </StateView>
    <ChildList refresh={refresh} titleKey="allocations" path={`/transport-demand/uuid/${uuid}/allocations`} columns={[{ key: 'startDate', labelKey: 'startDate', kind: 'date' }, { key: 'endDate', labelKey: 'endDate', kind: 'date' }, { key: 'daysOfWeek', labelKey: 'daysOfWeek' }, { key: 'active', labelKey: 'activeFlag', kind: 'bool' }]}
      addTo={can(session, 'transport-demand.allocation.create', [...OWNER]) ? `/transport-demands/${uuid}/allocations/new` : undefined} addKey="newAllocation" />
    <ChildList refresh={refresh} titleKey="dailySchedules" path={`/transport-demand/uuid/${uuid}/daily-schedules`} columns={[{ key: 'date', labelKey: 'date', kind: 'date' }, { key: 'startTime', labelKey: 'startTime' }, { key: 'endTime', labelKey: 'endTime' }, { key: 'notes', labelKey: 'observation' }]}
      addTo={can(session, 'transport-demand.daily-schedule.create', [...OWNER]) ? `/transport-demands/${uuid}/daily-schedules/new` : undefined} addKey="newDailySchedule"
      onDelete={can(session, 'transport-demand.daily-schedule.delete', [...OWNER]) ? (row) => setRemoving({ kind: 'daily-schedule', row }) : undefined} />
    <ChildList refresh={refresh} titleKey="scheduleExceptions" path={`/transport-demand/uuid/${uuid}/exceptions`} columns={[{ key: 'date', labelKey: 'date', kind: 'date' }, { key: 'exceptionType', labelKey: 'exceptionType', kind: 'badge', prefix: 'exceptionType' }, { key: 'reason', labelKey: 'reason' }]}
      addTo={can(session, 'transport-demand.exception.create', [...OWNER]) ? `/transport-demands/${uuid}/exceptions/new` : undefined} addKey="newException"
      onDelete={can(session, 'transport-demand.exception.delete', [...OWNER]) ? (row) => setRemoving({ kind: 'exception', row }) : undefined} />
    {removing && <Confirm title={t('deleteTitle')} text={t('deleteText')} onCancel={() => setRemoving(null)} onConfirm={remove} />}</>
}
const demandChild = (titleKey: string, fields: FieldDef[], suffix: string) => function DemandChildPage() {
  const { uuid } = useParams()
  return <ResourceFormPage titleKey={titleKey} fields={fields} backTo={`/transport-demands/${uuid}`} controller={useResourceForm(transportDemands, uuid, suffix, 'post')} />
}
export const DemandAllocationPage = demandChild('newAllocation', allocationFields, '/allocation')
export const DemandDailyPage = demandChild('newDailySchedule', dailyFields, '/daily-schedule')
export const DemandExceptionPage = demandChild('newException', exceptionFields, '/exception')

