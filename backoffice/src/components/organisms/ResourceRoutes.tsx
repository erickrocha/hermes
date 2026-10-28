import { useMemo, useState } from 'react'
import { Link, useParams } from 'react-router-dom'
import { useDispatch, useSelector } from 'react-redux'
import { useTranslation } from 'react-i18next'
import { Pencil, Trash2 } from 'lucide-react'
import type { ColumnDef } from '@tanstack/react-table'
import type { ResourceConfig, Row, ColumnSpec } from '../../features/resources'
import { useResourceForm } from '../../features/hooks'
import type { AppDispatch, RootState } from '../../store'
import { maskDocument } from '../../features/masks'
import { Confirm } from '../UI'
import { ResourceFormPage } from './ResourceFormPage'
import { ResourceListPage } from './ResourceListPage'

export function useMoney() {
  const { i18n } = useTranslation()
  return (cents: number) => (cents / 100).toLocaleString(i18n.resolvedLanguage || i18n.language, { minimumFractionDigits: 2, maximumFractionDigits: 2 })
}

/// Uma célula conforme o tipo da coluna; valor ausente vira travessão. A linha inteira entra para os tipos que dependem de outra coluna (o documento, do país).
export function useCell() {
  const { t } = useTranslation(); const money = useMoney()
  return (spec: ColumnSpec, value: unknown, row?: Row) => {
    if (value === undefined || value === null || value === '') return '—'
    switch (spec.kind) {
      case 'money': return money(Number(value))
      case 'taxId': return maskDocument(String(value), String(row?.countryCode ?? 'BR'))
      case 'date': return String(value).slice(0, 10)
      case 'datetime': return String(value).replace('T', ' ').slice(0, 16)
      case 'bool': return value ? t('yes') : t('no')
      case 'badge': return <span className="badge blue">{t(`${spec.prefix ?? ''}${String(value)}`, { defaultValue: String(value) })}</span>
      default: return String(value)
    }
  }
}

export function ResourceListRoute({ config }: { config: ResourceConfig }) {
  const { t } = useTranslation(); const dispatch = useDispatch<AppDispatch>(); const cell = useCell()
  const session = useSelector((state: RootState) => state.auth.session)
  const [removing, setRemoving] = useState<string | null>(null)
  const canEdit = config.edit?.(session) ?? false; const canRemove = config.remove?.(session) ?? false
  const columns = useMemo<ColumnDef<Row, unknown>[]>(() => [
    ...config.columns.map((spec, index): ColumnDef<Row, unknown> => ({ id: spec.key, header: () => t(spec.labelKey), cell: ({ row }) => (config.detail && index === 0 && row.original.uuid ? <Link to={`/${config.route}/${row.original.uuid}`}><b>{cell(spec, row.original[spec.key], row.original)}</b></Link> : cell(spec, row.original[spec.key], row.original)) })),
    ...(canEdit || canRemove ? [{ id: 'actions', header: () => t('actions'), cell: ({ row }: { row: { original: Row } }) => <div className="row-actions">
      {canEdit && row.original.uuid && <Link to={`/${config.route}/${row.original.uuid}/edit`} aria-label={t('edit')}><Pencil size={16} /></Link>}
      {canRemove && row.original.uuid && <button className="danger-text" aria-label={t('delete')} onClick={() => setRemoving(row.original.uuid ?? null)}><Trash2 size={16} /></button>}</div> } as ColumnDef<Row, unknown>] : []),
  // eslint-disable-next-line react-hooks/exhaustive-deps
  ], [t, config, canEdit, canRemove])
  const confirm = async () => { const uuid = removing; setRemoving(null); if (uuid) { await dispatch(config.resource.remove(uuid)); dispatch(config.resource.fetchList({ page: 0, pageSize: 25 })) } }
  return <><ResourceListPage resource={config.resource} titleKey={config.titleKey} subtitleKey={config.summaryKey} columns={columns}
      newTo={config.create?.(session) ? `/${config.route}/new` : undefined} newKey={config.newKey}
      toolbar={config.shortcuts?.some((x) => x.visible(session)) ? <div className="inline-actions">{config.shortcuts.filter((x) => x.visible(session)).map((x) => <Link key={x.to} className="btn secondary" to={x.to}>{t(x.labelKey)}</Link>)}</div> : undefined} />
    {removing && <Confirm title={t('deleteTitle')} text={t('deleteText')} onCancel={() => setRemoving(null)} onConfirm={confirm} />}</>
}

export function ResourceNewRoute({ config }: { config: ResourceConfig }) {
  const controller = useResourceForm(config.resource)
  return <ResourceFormPage titleKey={config.newKey} fields={config.fields} backTo={`/${config.route}`} controller={controller} />
}

export function ResourceEditRoute({ config }: { config: ResourceConfig }) {
  const { uuid } = useParams()
  const controller = useResourceForm(config.resource, uuid)
  return <ResourceFormPage titleKey={config.editKey} fields={config.fields} backTo={`/${config.route}`} controller={controller} editing />
}
