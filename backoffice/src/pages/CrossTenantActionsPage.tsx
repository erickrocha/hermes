import { useMemo } from 'react'
import { RefreshCw } from 'lucide-react'
import type { ColumnDef } from '@tanstack/react-table'
import { useSelector } from 'react-redux'
import { useTranslation } from 'react-i18next'
import { PageHeader } from '../components/UI'
import { DataTable } from '../components/DataTable'
import { loadCrossTenantActions } from '../features/crossTenantActionSlice'
import type { RootState } from '../store'
import type { CrossTenantAction } from '../types'
import { usePagedList } from '../usePagedList'

/// C-036 (`HRMS-877`): what a tenant did inside its descendants, and what was done in this one -- the trail that
/// makes support with write access acceptable. Read-only.
export function CrossTenantActionsPage() {
  const { t } = useTranslation()
  const { page, loading, error } = useSelector((state: RootState) => state.crossTenantActions)
  const { onPageChange, reload } = usePagedList(loadCrossTenantActions)
  const columns = useMemo<ColumnDef<CrossTenantAction, unknown>[]>(() => [
    { header: t('when'), accessorKey: 'occurredAt', cell: ({ row }) => new Date(row.original.occurredAt).toLocaleString() },
    { header: t('actor'), accessorKey: 'actorEmail' },
    { header: t('kind'), accessorKey: 'kind', cell: ({ row }) => <span className={`badge ${row.original.kind === 'write' ? 'yellow' : 'gray'}`}>{t(`kind${row.original.kind}`)}</span> },
    { header: t('operation'), cell: ({ row }) => <><b>{row.original.method}</b> {row.original.path}</> },
    { header: t('fromTenant'), accessorKey: 'actorTenantId' },
    { header: t('inTenant'), accessorKey: 'affectedTenantId' },
  ], [t])
  return <>
    <PageHeader title={t('crossTenantActions')} subtitle={t('crossTenantActionsText')} actions={<button className="btn secondary" disabled={loading} onClick={reload}><RefreshCw size={16} />{t('refresh')}</button>} />
    {error && <p className="form-error" role="alert">{t(error, { defaultValue: error })}</p>}
    <DataTable columns={columns} page={page} loading={loading} onPageChange={onPageChange} />
  </>
}
