import { useEffect, useMemo, useState } from 'react'
import { Link, useParams } from 'react-router-dom'
import { useDispatch, useSelector } from 'react-redux'
import { useTranslation } from 'react-i18next'
import { Plus } from 'lucide-react'
import type { ColumnDef } from '@tanstack/react-table'
import { can, OWNER, OWNER_MECHANIC } from '../features/access'
import type { FieldDef } from '../features/fields'
import { useResourceForm, useResourceState } from '../features/hooks'
import { workOrders, type Row } from '../features/resources'
import type { AppDispatch, RootState } from '../store'
import { Confirm, PageHeader } from '../components/UI'
import { StateView } from '../components/molecules/StateView'
import { ResourceFormPage } from '../components/organisms/ResourceFormPage'
import { ChildList } from '../components/organisms/ChildList'
import { ResourceListPage } from '../components/organisms/ResourceListPage'
import { useMoney } from '../components/organisms/ResourceRoutes'
import type { Session } from '../types'

// C-026 EPIC-MT-01/02 (HRMS-700…705): ordens de serviço. Abrir, acrescentar item e retirar peça são do proprietário e do mecânico;
// concluir e cancelar são só do proprietário. Cada botão segue a permissão que o servidor impõe.
const READERS = ['TenantOwner', 'TenantUser', 'Mechanic'] as const
export const canSeeWorkOrders = (session: Session) => can(session, 'work-order.read', [...READERS])

const fields: FieldDef[] = [
  { name: 'vehicleUuid', labelKey: 'vehicle', type: 'lookup', lookup: 'vehicle', required: true },
  { name: 'odometerKm', labelKey: 'odometerKm', type: 'number', required: true, min: 0 },
  { name: 'description', labelKey: 'description', type: 'text', required: true },
  { name: 'serviceType', labelKey: 'serviceType', type: 'text' }, { name: 'responsible', labelKey: 'responsible', type: 'text' }, { name: 'observation', labelKey: 'observation', type: 'text' },
]
const itemFields: FieldDef[] = [{ name: 'description', labelKey: 'description', type: 'text', required: true }, { name: 'observation', labelKey: 'observation', type: 'text' }]
const issueFields: FieldDef[] = [
  { name: 'partUuid', labelKey: 'part', type: 'lookup', lookup: 'part', required: true }, { name: 'quantity', labelKey: 'quantity', type: 'number', required: true, min: 0.0001 },
]

const statusColor = (status: string) => (status === 'Concluded' ? 'green' : status === 'Cancelled' ? 'gray' : 'blue')
function Status({ status }: { status: string }) { const { t } = useTranslation(); return <span className={`badge ${statusColor(status)}`}>{t(`woStatus${status}`)}</span> }

export function WorkOrdersPage() {
  const { t } = useTranslation(); const session = useSelector((state: RootState) => state.auth.session)
  const columns = useMemo<ColumnDef<Row, unknown>[]>(() => [
    { id: 'number', header: () => t('number'), cell: ({ row }) => <Link to={`/work-orders/${row.original.uuid}`}><b>{String(row.original.number)}</b></Link> },
    { id: 'description', header: () => t('description'), cell: ({ row }) => <><b>{String(row.original.description)}</b><small>{String(row.original.serviceType ?? '')}</small></> },
    { id: 'openedAt', header: () => t('openedAt'), cell: ({ row }) => String(row.original.openedAt ?? '').slice(0, 10) || '—' },
    { id: 'status', header: () => t('status'), cell: ({ row }) => <Status status={String(row.original.status)} /> },
  ], [t])
  return <ResourceListPage resource={workOrders} titleKey="workOrders" subtitleKey="workOrdersSummary" columns={columns}
    newTo={can(session, 'work-order.create', [...OWNER_MECHANIC]) ? '/work-orders/new' : undefined} newKey="newWorkOrder" />
}

export function WorkOrderNewPage() {
  return <ResourceFormPage titleKey="newWorkOrder" fields={fields} backTo="/work-orders" controller={useResourceForm(workOrders)} />
}

export function WorkOrderItemPage() {
  const { uuid } = useParams()
  return <ResourceFormPage titleKey="newItem" fields={itemFields} backTo={`/work-orders/${uuid}`} controller={useResourceForm(workOrders, uuid, '/item', 'post')} />
}

export function WorkOrderIssuePartPage() {
  const { uuid } = useParams()
  return <ResourceFormPage titleKey="issuePart" fields={issueFields} backTo={`/work-orders/${uuid}`} controller={useResourceForm(workOrders, uuid, '/issue-part', 'post')} />
}

export function WorkOrderDetailPage() {
  const { uuid } = useParams(); const { t } = useTranslation(); const money = useMoney(); const dispatch = useDispatch<AppDispatch>()
  const session = useSelector((state: RootState) => state.auth.session); const state = useResourceState(workOrders)
  const [confirming, setConfirming] = useState<'conclude' | 'cancel' | null>(null)
  useEffect(() => { if (uuid) dispatch(workOrders.fetchOne(uuid)) }, [dispatch, uuid])
  const order = state.current as (Row & { items?: Row[] }) | null
  const closed = order?.status === 'Concluded' || order?.status === 'Cancelled'
  const finish = async () => { const action = confirming; setConfirming(null); if (action && uuid) { await dispatch(workOrders.act({ method: 'put', path: `/work-order/uuid/${uuid}/${action}` })); dispatch(workOrders.fetchOne(uuid)) } }
  return <>
    <PageHeader title={order ? `${order.number} — ${order.description}` : t('workOrders')} subtitle={String(order?.serviceType ?? '')} actions={<>
      <Link className="btn secondary" to="/work-orders">{t('back')}</Link>
      {order && !closed && can(session, 'work-order.issue-part', [...OWNER_MECHANIC]) && <Link className="btn secondary" to={`/work-orders/${uuid}/issue-part`}>{t('issuePart')}</Link>}
      {order && !closed && can(session, 'work-order.conclude', [...OWNER]) && <button className="btn primary" onClick={() => setConfirming('conclude')}>{t('conclude')}</button>}
      {order && !closed && can(session, 'work-order.cancel', [...OWNER]) && <button className="btn secondary danger-text" onClick={() => setConfirming('cancel')}>{t('cancelWorkOrder')}</button>}</>} />
    <StateView loading={state.loading} error={state.error}>
      {order && <>
        <div className="table-card"><table><tbody>
          <tr><th>{t('status')}</th><td><Status status={String(order.status)} /></td></tr>
          <tr><th>{t('openedAt')}</th><td>{String(order.openedAt ?? '').slice(0, 10) || '—'}</td></tr>
          <tr><th>{t('odometerKm')}</th><td>{String(order.odometerKm)}</td></tr>
          {!!order.responsible && <tr><th>{t('responsible')}</th><td>{String(order.responsible)}</td></tr>}
          {!!order.observation && <tr><th>{t('observation')}</th><td>{String(order.observation)}</td></tr>}
          {order.totalCostCents !== undefined && <tr><th>{t('totalCost')}</th><td>{money(Number(order.totalCostCents))}</td></tr>}
        </tbody></table></div>
        <section className="page-section"><h2>{t('items')}</h2>
          {(order.items ?? []).length === 0 ? <p className="form-hint">{t('noItems')}</p> : <div className="table-card"><table>
            <thead><tr><th>{t('description')}</th><th>{t('status')}</th></tr></thead>
            <tbody>{(order.items ?? []).map((i) => <tr key={String(i.uuid)}><td>{String(i.description)}{!!i.observation && <small>{String(i.observation)}</small>}</td><td><Status status={String(i.status ?? 'Pending')} /></td></tr>)}</tbody></table></div>}
          {!closed && can(session, 'work-order.item.add', [...OWNER_MECHANIC]) && <div className="inline-actions"><Link className="btn secondary" to={`/work-orders/${uuid}/items/new`}><Plus size={16} />{t('newItem')}</Link></div>}
        </section>
        {can(session, 'work-order.posting.read', [...READERS]) && <ChildList titleKey="partPostings" path={`/work-order/uuid/${uuid}/postings`} columns={[{ key: 'quantity', labelKey: 'quantity', kind: 'number' }, { key: 'unitValueCents', labelKey: 'unitValue', kind: 'money' }, { key: 'totalValueCents', labelKey: 'totalValue', kind: 'money' }, { key: 'costSource', labelKey: 'costSource', kind: 'badge', prefix: 'costSource' }]} />}</>}
    </StateView>
    {confirming && <Confirm title={t(confirming === 'conclude' ? 'concludeTitle' : 'cancelWorkOrderTitle')} text={t(confirming === 'conclude' ? 'concludeText' : 'cancelWorkOrderText')} onCancel={() => setConfirming(null)} onConfirm={finish} />}</>
}
