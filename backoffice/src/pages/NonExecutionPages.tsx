import { useCallback, useEffect, useMemo, useState } from 'react'
import { AlertTriangle, Play, Repeat } from 'lucide-react'
import { Link, useParams } from 'react-router-dom'
import { useDispatch, useSelector } from 'react-redux'
import { useTranslation } from 'react-i18next'
import { PageHeader } from '../components/UI'
import { Pagination } from '../components/DataTable'
import { StateView } from '../components/molecules/StateView'
import { ResourceFormPage, type FormController } from '../components/organisms/ResourceFormPage'
import { can } from '../features/access'
import type { FieldDef } from '../features/fields'
import { evaluateNow, fetchInbox, justifyOccurrence, recordManualOccurrence, resetForm } from '../features/nonExecutionSlice'
import type { NonExecutionOccurrence } from '../services/nonExecutionService'
import type { AppDispatch, RootState } from '../store'
export { canSeeNonExecutions } from '../features/routeAccess'

const PAGE_SIZE = 25

const when = (value?: string | null) => (value ? new Date(value).toLocaleString() : '—')

function Occurrence({ occurrence, mayJustify }: { occurrence: NonExecutionOccurrence; mayJustify: boolean }) {
  const { t } = useTranslation()
  return <li className="non-execution-occurrence">
    <b>{t(`nonExecutionKind_${occurrence.kind}`)}</b>
    <small>{t('nonExecutionWindow')}: {when(occurrence.windowStart)} → {when(occurrence.windowEnd)}</small>
    {occurrence.kind !== 'DidNotAppear' && <small>{t('nonExecutionArrived')}: {when(occurrence.arrivedAt)} · {t('nonExecutionDeparted')}: {when(occurrence.departedAt)}
      {occurrence.dwellMinutes != null && ` · ${t('nonExecutionDwell')}: ${occurrence.dwellMinutes}`}
      {occurrence.minutesInWindow != null && ` · ${t('nonExecutionInWindow')}: ${occurrence.minutesInWindow}`}</small>}
    {mayJustify && <Link className="btn tertiary" to={`/non-executions/${occurrence.uuid}/justify`}>{t('justifyNonExecution')}</Link>}
  </li>
}

/// EPIC-MT-04 (TRM-269): a caixa de pendências, agrupada por veículo, uma linha por ordem de serviço.
export function NonExecutionsPage() {
  const { t } = useTranslation()
  const dispatch = useDispatch<AppDispatch>()
  const session = useSelector((state: RootState) => state.auth.session)
  const { page, loading, error, evaluating, evaluation } = useSelector((state: RootState) => state.nonExecutions)
  const [index, setIndex] = useState(0)
  useEffect(() => { void dispatch(resetForm()) }, [dispatch])
  useEffect(() => { void dispatch(fetchInbox({ page: index, pageSize: PAGE_SIZE })) }, [dispatch, index])
  const mayJustify = can(session, 'non-execution.justify')

  return <>
    <PageHeader title={t('nonExecutions')} subtitle={t('nonExecutionsText')} actions={<>
      {can(session, 'non-execution.record-manual') && <Link className="btn secondary" to="/non-executions/manual">{t('recordManualNonExecution')}</Link>}
      {can(session, 'non-execution.evaluate') && <button type="button" className="btn primary" disabled={evaluating} onClick={() => void dispatch(evaluateNow())}><Play size={16} />{evaluating ? t('evaluating') : t('evaluateNow')}</button>}
    </>} />
    {evaluation && <p className="form-hint" role="status">{t('nonExecutionEvaluated', { occurrences: evaluation.occurrencesRecorded, concluded: evaluation.plansConcluded })}</p>}
    <StateView loading={loading && !page.items.length} error={error} empty={!loading && !error && !page.items.length}>
      <div className="non-execution-list">{page.items.map((group) => <section key={group.vehicle.uuid} className="non-execution-vehicle">
        <h2><AlertTriangle size={18} />{group.vehicle.plate}{group.vehicle.model ? ` · ${group.vehicle.model}` : ''}</h2>
        <ul>{group.workOrders.map((order) => <li key={order.workOrderUuid} className="non-execution-order">
          <div className="non-execution-order-head">
            <Link to={`/work-orders/${order.workOrderUuid}`}>{order.description}</Link>
            <span className="badge blue">{order.status}</span>
            <span className="badge gray" title={t('nonExecutionUnresolvedTimes', { count: order.totalOccurrences })}><Repeat size={13} />{t('nonExecutionUnresolvedTimes', { count: order.totalOccurrences })}</span>
          </div>
          <ul>{order.occurrences.map((occurrence) => <Occurrence key={occurrence.uuid} occurrence={occurrence} mayJustify={mayJustify} />)}</ul>
        </li>)}</ul>
      </section>)}</div>
      <Pagination page={page} onPageChange={(request) => setIndex(request.page)} />
    </StateView>
  </>
}

function useFormController(submit: (body: Record<string, unknown>) => unknown): FormController {
  const dispatch = useDispatch<AppDispatch>()
  const { saving, saved, error } = useSelector((state: RootState) => state.nonExecutions)
  const reset = useCallback(() => { dispatch(resetForm()) }, [dispatch])
  return { loading: false, saving, error, saved, current: null, reset, submit }
}

const justificationField: FieldDef = { name: 'justification', labelKey: 'nonExecutionJustification', type: 'textarea', required: true, maxLength: 1000 }

/// TRM-270: justificar nunca muda o status da ordem de serviço; é uma página, não um modal.
export function NonExecutionJustifyPage() {
  const { uuid = '' } = useParams()
  const dispatch = useDispatch<AppDispatch>()
  const controller = useFormController((body) => dispatch(justifyOccurrence({ uuid, justification: String(body.justification) })))
  const fields = useMemo(() => [justificationField], [])
  return <ResourceFormPage titleKey="justifyNonExecution" subtitleKey="justifyNonExecutionText" fields={fields} backTo="/non-executions" controller={controller} />
}

/// TRM-272: só com evidência de presença; sem ela o servidor recusa e a mensagem aparece no formulário.
export function NonExecutionManualPage() {
  const dispatch = useDispatch<AppDispatch>()
  const controller = useFormController((body) => dispatch(recordManualOccurrence({ workOrderUuid: String(body.workOrderUuid), justification: String(body.justification) })))
  const fields = useMemo<FieldDef[]>(() => [{ name: 'workOrderUuid', labelKey: 'nonExecutionWorkOrder', type: 'lookup', lookup: 'workOrder', required: true }, justificationField], [])
  return <ResourceFormPage titleKey="recordManualNonExecution" subtitleKey="recordManualNonExecutionText" fields={fields} backTo="/non-executions" controller={controller} />
}
