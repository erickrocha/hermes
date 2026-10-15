import { useCallback, useEffect, useState, type ReactNode } from 'react'
import { Link } from 'react-router-dom'
import { useDispatch } from 'react-redux'
import { useTranslation } from 'react-i18next'
import { Plus, RefreshCw } from 'lucide-react'
import type { ColumnDef } from '@tanstack/react-table'
import type { Params } from '../../services/resourceService'
import type { AppDispatch } from '../../store'
import { useResourceState, type Resource } from '../../features/hooks'
import type { PageRequest } from '../../types'
import { DataTable } from '../DataTable'
import { PageHeader } from '../UI'
import { StateView } from '../molecules/StateView'

interface Props<T> {
  resource: Resource<T>; titleKey: string; subtitleKey?: string; columns: ColumnDef<T, unknown>[]
  newTo?: string; newKey?: string; extra?: Params; toolbar?: ReactNode
}

/// A página de lista: estados de carregando/erro/vazio, paginação do servidor e o botão de novo por navegação.
export function ResourceListPage<T>({ resource, titleKey, subtitleKey, columns, newTo, newKey, extra, toolbar }: Props<T>) {
  const { t } = useTranslation(); const dispatch = useDispatch<AppDispatch>(); const state = useResourceState(resource)
  const [request, setRequest] = useState<PageRequest>({ page: 0, pageSize: 25 })
  const load = useCallback(() => { dispatch(resource.fetchList({ page: request.page, pageSize: request.pageSize, extra })) }, [dispatch, resource, request, extra])
  useEffect(load, [load])
  return <>
    <PageHeader title={t(titleKey)} subtitle={subtitleKey ? t(subtitleKey) : undefined} actions={<>
      <button className="btn secondary" onClick={load}><RefreshCw size={16} />{t('refresh')}</button>
      {newTo && <Link className="btn primary" to={newTo}><Plus size={17} />{t(newKey ?? 'create')}</Link>}</>} />
    {toolbar}
    <StateView loading={false} error={state.error}>
      <DataTable columns={columns} page={state.list} loading={state.loading} onPageChange={setRequest} />
    </StateView>
  </>
}
