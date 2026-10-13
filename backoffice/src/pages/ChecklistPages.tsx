import { useMemo } from 'react'
import { Link, useParams } from 'react-router-dom'
import { useSelector } from 'react-redux'
import { useTranslation } from 'react-i18next'
import { can, OWNER, OWNER_MECHANIC } from '../features/access'
import type { FieldDef } from '../features/fields'
import { usePanel, useResourceForm } from '../features/hooks'
import { checklistRuns, technicalInspections, type Row } from '../features/resources'
import type { RootState } from '../store'
import { PageHeader } from '../components/UI'
import { DetailCard } from '../components/molecules/DetailCard'
import { PanelSection } from '../components/molecules/PanelSection'
import { ResourceFormPage } from '../components/organisms/ResourceFormPage'

// C-025 (HRMS-650…654): o modelo de checklist, a execução com as respostas e a posse do veículo; C-027 (HRMS-714): a inspeção técnica.
const runFields: FieldDef[] = [
  { name: 'vehicleUuid', labelKey: 'vehicle', type: 'lookup', lookup: 'vehicle', required: true }, { name: 'driverUuid', labelKey: 'driver', type: 'lookup', lookup: 'driver', required: true },
  { name: 'odometerKm', labelKey: 'odometerKm', type: 'number', required: true, min: 0 }, { name: 'openingChecklistUuid', labelKey: 'openingChecklist', type: 'text' }, { name: 'notes', labelKey: 'observation', type: 'text' },
  { name: 'answers', labelKey: 'answers', type: 'list', required: true, fixedRows: true, itemFields: [
    { name: 'checklistTemplateItemUuid', labelKey: 'checklistItem', type: 'hidden' }, { name: 'description', labelKey: 'description', type: 'text', transient: true },
    { name: 'status', labelKey: 'answerStatus', type: 'select', options: ['Conforming', 'NonConforming'], optionPrefix: 'answerStatus', required: true }, { name: 'observation', labelKey: 'observation', type: 'text' }] },
]
const inspectionFields: FieldDef[] = [
  { name: 'vehicleUuid', labelKey: 'vehicle', type: 'lookup', lookup: 'vehicle', required: true }, { name: 'inspectionModel', labelKey: 'inspectionModel', type: 'lookup', lookup: 'inspectionModelName', required: true },
  { name: 'inspectedAt', labelKey: 'inspectedAt', type: 'date' }, { name: 'odometerKm', labelKey: 'odometerKm', type: 'number', min: 0 }, { name: 'observation', labelKey: 'observation', type: 'text' },
  { name: 'items', labelKey: 'inspectionItems', type: 'list', required: true, itemFields: [
    { name: 'description', labelKey: 'description', type: 'text', required: true }, { name: 'conforming', labelKey: 'conforming', type: 'checkbox', defaultValue: true }, { name: 'observation', labelKey: 'observation', type: 'text' }] },
]

export function ChecklistTemplateDetailPage() {
  const { uuid } = useParams(); const { t } = useTranslation(); const session = useSelector((state: RootState) => state.auth.session)
  const template = usePanel<Row & { items: Row[] }>(`/checklist-template/uuid/${uuid}`)
  const data = template.data
  return <>
    <PageHeader title={data ? String(data.name) : t('checklistTemplates')} actions={<>
      <Link className="btn secondary" to="/checklist-templates">{t('back')}</Link>
      {can(session, 'checklist-run.create', [...OWNER, 'Driver']) && <Link className="btn primary" to={`/checklist-templates/${uuid}/run`}>{t('recordRun')}</Link>}</>} />
    <PanelSection titleKey="checklistTemplates" loading={template.loading} error={template.error} empty={!data}>
      {data && <><DetailCard rows={[{ label: t('checklistType'), value: t(`checklistType${String(data.checklistType)}`) }, { label: t('activeFlag'), value: data.active ? t('yes') : t('no') }]} />
        <div className="table-card"><table><thead><tr><th>{t('description')}</th><th>{t('generatesWorkOrder')}</th></tr></thead>
          <tbody>{data.items.map((i) => <tr key={String(i.uuid)}><td>{String(i.description)}</td><td>{i.generatesWorkOrder ? t('yes') : t('no')}</td></tr>)}</tbody></table></div></>}
    </PanelSection></>
}

export function ChecklistRunNewPage() {
  const { uuid } = useParams(); const template = usePanel<Row & { items: Row[] }>(`/checklist-template/uuid/${uuid}`)
  const base = useResourceForm(checklistRuns)
  // As respostas nascem do próprio modelo: uma linha por item, todas "conforme" até o motorista dizer o contrário.
  const current = useMemo(() => (template.data ? { answers: template.data.items.map((item) => ({ checklistTemplateItemUuid: item.uuid, description: item.description, status: 'Conforming' })) } : null), [template.data])
  const controller = useMemo(() => ({ ...base, loading: template.loading, error: base.error || (template.error ? 'loadError' : ''), current, load: undefined,
    submit: (body: Record<string, unknown>) => base.submit({ ...body, checklistTemplateUuid: uuid, checklistType: template.data?.checklistType }) }), [base, template.loading, template.error, template.data, current, uuid])
  return <ResourceFormPage titleKey="recordRun" subtitleKey="recordRunText" fields={runFields} backTo={`/checklist-templates/${uuid}`} controller={controller} />
}

export function TechnicalInspectionNewPage() {
  return <ResourceFormPage titleKey="recordInspection" subtitleKey="recordInspectionText" fields={inspectionFields} backTo="/inspection-models" controller={useResourceForm(technicalInspections)} />
}

export const canRecordInspection = (session: RootState['auth']['session']) => can(session, 'technical-inspection.create', [...OWNER_MECHANIC])
