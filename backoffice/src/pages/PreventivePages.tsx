import { useEffect, useState } from 'react'
import { Link, useNavigate, useParams } from 'react-router-dom'
import { useDispatch, useSelector } from 'react-redux'
import { useTranslation } from 'react-i18next'
import { can, OWNER_MECHANIC } from '../features/access'
import type { FieldDef } from '../features/fields'
import { useAction, useResourceForm, useResourceState } from '../features/hooks'
import { preventivePlans, type Row } from '../features/resources'
import type { AppDispatch, RootState } from '../store'
import { Confirm, PageHeader } from '../components/UI'
import { DetailCard } from '../components/molecules/DetailCard'
import { StateView } from '../components/molecules/StateView'
import { ChildList } from '../components/organisms/ChildList'
import { ResourceFormPage } from '../components/organisms/ResourceFormPage'

// C-027 (HRMS-706…713): o plano preventivo com o status derivado, a ordem de serviço sob demanda, a prorrogação e os alertas.
const extensionFields: FieldDef[] = [
  { name: 'workOrderItemUuid', labelKey: 'workOrderItem', type: 'text', required: true }, { name: 'inspectionKm', labelKey: 'inspectionKm', type: 'number', min: 0 },
  { name: 'grantedKm', labelKey: 'grantedKm', type: 'number', required: true, min: 0.0001 }, { name: 'description', labelKey: 'description', type: 'text', required: true },
]
const alertFields: FieldDef[] = [
  { name: 'atKm', labelKey: 'atKm', type: 'number', required: true, min: 0.0001 }, { name: 'title', labelKey: 'title', type: 'text', required: true },
  { name: 'inspectionModel', labelKey: 'inspectionModel', type: 'lookup', lookup: 'inspectionModelName', required: true },
]

export function PreventivePlanDetailPage() {
  const { uuid } = useParams(); const { t } = useTranslation(); const navigate = useNavigate(); const dispatch = useDispatch<AppDispatch>(); const run = useAction(preventivePlans)
  const session = useSelector((state: RootState) => state.auth.session); const state = useResourceState(preventivePlans); const plan = state.current as Row | null
  const [discharging, setDischarging] = useState<Row | null>(null); const [refresh, setRefresh] = useState(0); const [opened, setOpened] = useState('')
  useEffect(() => { if (uuid) dispatch(preventivePlans.fetchOne(uuid)) }, [dispatch, uuid])
  const status = String(plan?.status ?? '')
  const openOrder = async () => {
    const result = await dispatch(preventivePlans.act({ method: 'post', path: `/preventive-plan/uuid/${uuid}/work-order` }))
    if (preventivePlans.act.fulfilled.match(result)) { const order = result.payload as Row; if (order.workOrderUuid) navigate(`/work-orders/${order.workOrderUuid}`); else setOpened(String(order.number ?? '')) }
  }
  const discharge = async () => { const row = discharging; setDischarging(null); if (row && await run('post', `/preventive-plan/alert/${row.uuid}/discharge`)) setRefresh((n) => n + 1) }
  const mayAct = can(session, 'preventive-plan.open-work-order', [...OWNER_MECHANIC])
  return <>
    <PageHeader title={plan ? String(plan.planName) : t('preventivePlans')} actions={<>
      <Link className="btn secondary" to="/preventive-plans">{t('back')}</Link>
      {can(session, 'preventive-plan.update', [...OWNER_MECHANIC]) && <Link className="btn secondary" to={`/preventive-plans/${uuid}/edit`}>{t('edit')}</Link>}
      {plan && status !== 'Ok' && mayAct && <button className="btn primary" disabled={state.saving} onClick={openOrder}>{t('openPreventiveOrder')}</button>}
      {plan && can(session, 'preventive-plan.extend', [...OWNER_MECHANIC]) && <Link className="btn secondary" to={`/preventive-plans/${uuid}/extension`}>{t('extendPlan')}</Link>}</>} />
    {state.error && plan && <p className="form-error" role="alert">{t(state.error, { defaultValue: state.error })}</p>}
    {opened && <p className="form-hint" role="status">{opened}</p>}
    <StateView loading={state.loading} error={plan ? '' : state.error}>
      {plan && <DetailCard rows={[
        { label: t('status'), value: <span className="badge blue">{t(`planStatus${status}`)}</span> }, { label: t('controlType'), value: t(`controlType${String(plan.controlType)}`) },
        { label: t('intervalKm'), value: String(plan.intervalKm ?? '') }, { label: t('intervalDays'), value: String(plan.intervalDays ?? '') },
        { label: t('lastServiceKm'), value: String(plan.lastServiceKm ?? '') }, { label: t('lastServiceDate'), value: String(plan.lastServiceDate ?? '') }, { label: t('extensionLimitKm'), value: String(plan.extensionLimitKm ?? '') },
        { label: t('workOrders'), value: plan.lastWorkOrderUuid ? <Link to={`/work-orders/${plan.lastWorkOrderUuid}`}>{t('detail')}</Link> : '' },
      ]} />}
    </StateView>
    <ChildList refresh={refresh} titleKey="pendingAlerts" path={`/preventive-plan/uuid/${uuid}/alert`} columns={[{ key: 'atKm', labelKey: 'atKm', kind: 'number' }, { key: 'title', labelKey: 'title' }, { key: 'inspectionModel', labelKey: 'inspectionModel' }]}
      addTo={can(session, 'preventive-plan.alert.read', [...OWNER_MECHANIC]) && session?.role !== 'TenantUser' ? `/preventive-plans/${uuid}/alerts/new` : undefined} addKey="newAlert" actionKey="dischargeAlert"
      onDelete={can(session, 'preventive-plan.alert.discharge', [...OWNER_MECHANIC]) ? setDischarging : undefined} />
    {discharging && <Confirm title={t('dischargeAlert')} text={t('dischargeAlertText')} onCancel={() => setDischarging(null)} onConfirm={discharge} />}</>
}

export function PreventiveExtensionPage() {
  const { uuid } = useParams()
  return <ResourceFormPage titleKey="extendPlan" subtitleKey="extendPlanText" fields={extensionFields} backTo={`/preventive-plans/${uuid}`} controller={useResourceForm(preventivePlans, uuid, '/extension', 'post')} />
}
export function PreventiveAlertPage() {
  const { uuid } = useParams()
  return <ResourceFormPage titleKey="newAlert" fields={alertFields} backTo={`/preventive-plans/${uuid}`} controller={useResourceForm(preventivePlans, uuid, '/alert', 'post')} />
}
