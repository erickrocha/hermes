import { useEffect, useState } from 'react'
import { Link, useParams } from 'react-router-dom'
import { useDispatch, useSelector } from 'react-redux'
import { useTranslation } from 'react-i18next'
import { can } from '../features/access'
import type { FieldDef } from '../features/fields'
import { useAction, useResourceForm, useResourceState } from '../features/hooks'
import { parts, purchaseOrders } from '../features/resources'
import type { AppDispatch, RootState } from '../store'
import { Confirm, PageHeader } from '../components/UI'
import { DetailCard } from '../components/molecules/DetailCard'
import { StateView } from '../components/molecules/StateView'
import { ResourceFormPage } from '../components/organisms/ResourceFormPage'
import { useMoney } from '../components/organisms/ResourceRoutes'

// C-030 EPIC-SP-01…03 (HRMS-800…802): a peça com o seu estoque derivado do livro, e o pedido de compra com as suas transições.
const entryFields: FieldDef[] = [
  { name: 'quantity', labelKey: 'quantity', type: 'number', required: true, min: 0.0001 },
  { name: 'unitValueCents', labelKey: 'unitValue', type: 'money', min: 0 }, { name: 'totalValueCents', labelKey: 'totalValue', type: 'money', min: 0 },
  { name: 'supplier', labelKey: 'supplier', type: 'text' }, { name: 'invoiceNumber', labelKey: 'invoice', type: 'text' }, { name: 'entryDate', labelKey: 'entryDate', type: 'date' },
]
const adjustFields: FieldDef[] = [{ name: 'newBalance', labelKey: 'newBalance', type: 'number', required: true, min: 0 }]
const orderedFields: FieldDef[] = [{ name: 'orderedAt', labelKey: 'orderedAt', type: 'date' }, { name: 'expectedDeliveryDate', labelKey: 'expectedDelivery', type: 'date' }]

export function PartDetailPage() {
  const { uuid } = useParams(); const { t } = useTranslation(); const money = useMoney(); const dispatch = useDispatch<AppDispatch>()
  const session = useSelector((state: RootState) => state.auth.session); const state = useResourceState(parts); const part = state.current as Record<string, unknown> | null
  useEffect(() => { if (uuid) dispatch(parts.fetchOne(uuid)) }, [dispatch, uuid])
  const cents = (key: string) => (part?.[key] === undefined || part?.[key] === null ? undefined : money(Number(part[key])))
  return <>
    <PageHeader title={part ? `${part.code} — ${part.name}` : t('parts')} actions={<>
      <Link className="btn secondary" to="/parts">{t('back')}</Link>
      {can(session, 'part.stock-entry') && <Link className="btn primary" to={`/parts/${uuid}/entry`}>{t('stockEntry')}</Link>}
      {can(session, 'part.stock-adjust') && <Link className="btn secondary" to={`/parts/${uuid}/adjustment`}>{t('stockAdjust')}</Link>}</>} />
    <StateView loading={state.loading} error={state.error}>
      {part && <DetailCard rows={[
        { label: t('currentStock'), value: `${String(part.currentStock ?? 0)} ${String(part.unit)}` }, { label: t('minimumStock'), value: String(part.minimumStock) },
        { label: t('movingAverage'), value: cents('movingAverageCostCents') }, { label: t('lastPurchasePrice'), value: cents('lastPurchasePriceCents') }, { label: t('unitValue'), value: cents('unitValueCents') },
        { label: t('category'), value: String(part.category ?? '') }, { label: t('application'), value: String(part.application ?? '') }, { label: t('supplier'), value: String(part.defaultSupplier ?? '') }, { label: t('location'), value: String(part.location ?? '') },
      ]} />}
    </StateView></>
}

export function PartEntryPage() {
  const { uuid } = useParams()
  return <ResourceFormPage titleKey="stockEntry" fields={entryFields} backTo={`/parts/${uuid}`} controller={useResourceForm(parts, uuid, '/entry', 'post')} />
}
export function PartAdjustPage() {
  const { uuid } = useParams()
  return <ResourceFormPage titleKey="stockAdjust" fields={adjustFields} backTo={`/parts/${uuid}`} controller={useResourceForm(parts, uuid, '/adjustment', 'post')} />
}

export function PurchaseOrderDetailPage() {
  const { uuid } = useParams(); const { t } = useTranslation(); const dispatch = useDispatch<AppDispatch>(); const run = useAction(purchaseOrders)
  const session = useSelector((state: RootState) => state.auth.session); const state = useResourceState(purchaseOrders); const order = state.current as Record<string, unknown> | null
  const [confirming, setConfirming] = useState<'mark-purchased' | 'cancel' | null>(null)
  useEffect(() => { if (uuid) dispatch(purchaseOrders.fetchOne(uuid)) }, [dispatch, uuid])
  const status = String(order?.status ?? '')
  const apply = async () => { const action = confirming; setConfirming(null); if (action && uuid && await run('put', `/purchase-order/uuid/${uuid}/${action}`)) dispatch(purchaseOrders.fetchOne(uuid)) }
  const mayMove = can(session, 'purchase-order.mark-ordered')
  return <>
    <PageHeader title={order ? String(order.number) : t('purchaseOrders')} actions={<>
      <Link className="btn secondary" to="/purchase-orders">{t('back')}</Link>
      {order && status === 'Requested' && mayMove && <Link className="btn primary" to={`/purchase-orders/${uuid}/ordered`}>{t('markOrdered')}</Link>}
      {order && status === 'Ordered' && can(session, 'purchase-order.mark-purchased') && <button className="btn primary" onClick={() => setConfirming('mark-purchased')}>{t('markPurchased')}</button>}
      {order && status !== 'Cancelled' && status !== 'Purchased' && can(session, 'purchase-order.cancel') && <button className="btn secondary danger-text" onClick={() => setConfirming('cancel')}>{t('cancelOrder')}</button>}</>} />
    <StateView loading={state.loading} error={order ? '' : state.error}>
      {order && <DetailCard rows={[
        { label: t('status'), value: <span className="badge blue">{t(`poStatus${status}`)}</span> }, { label: t('quantity'), value: String(order.quantity) },
        { label: t('supplier'), value: String(order.suggestedSupplier ?? '') }, { label: t('observation'), value: String(order.observation ?? '') },
        { label: t('orderedAt'), value: String(order.orderedAt ?? '') }, { label: t('expectedDelivery'), value: String(order.expectedDeliveryDate ?? '') },
        { label: t('workOrders'), value: order.workOrderUuid ? <Link to={`/work-orders/${order.workOrderUuid}`}>{t('detail')}</Link> : '' },
      ]} />}
    </StateView>
    {state.error && order && <p className="form-error" role="alert">{t(state.error, { defaultValue: state.error })}</p>}
    {confirming && <Confirm title={t(confirming === 'cancel' ? 'cancelOrderTitle' : 'markPurchasedTitle')} text={t(confirming === 'cancel' ? 'cancelOrderText' : 'markPurchasedText')} onCancel={() => setConfirming(null)} onConfirm={apply} />}</>
}

export function PurchaseOrderOrderedPage() {
  const { uuid } = useParams()
  return <ResourceFormPage titleKey="markOrdered" fields={orderedFields} backTo={`/purchase-orders/${uuid}`} controller={useResourceForm(purchaseOrders, uuid, '/mark-ordered', 'put')} />
}
