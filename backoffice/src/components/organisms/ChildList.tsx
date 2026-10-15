import { Link } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { Plus, Trash2 } from 'lucide-react'
import { usePanel } from '../../features/hooks'
import type { ColumnSpec, Row } from '../../features/resources'
import { PanelSection } from '../molecules/PanelSection'
import { useCell } from './ResourceRoutes'

interface Props {
  titleKey: string; path: string; columns: ColumnSpec[]; addTo?: string; addKey?: string
  /** Presente: cada linha ganha um botão de remover; o pai decide o que fazer (confirmar, chamar o slice, recarregar). */
  onDelete?: (row: Row) => void
  /** Rótulo acessível do botão da linha (padrão: remover). */
  actionKey?: string
  /** Troca de valor força nova leitura (depois de uma ação do pai). */
  refresh?: number
}

const rowsOf = (data: unknown): Row[] => (Array.isArray(data) ? data : (data as { items?: Row[] } | undefined)?.items) ?? []

/// Uma sublista de um registro (histórico, alertas, itens…): carrega do próprio caminho e mostra os quatro estados.
export function ChildList({ titleKey, path, columns, addTo, addKey = 'create', onDelete, actionKey = 'delete', refresh = 0 }: Props) {
  const { t } = useTranslation(); const cell = useCell()
  const { data, loading, error } = usePanel<unknown>(path, { page: 0, pageSize: 50, r: refresh }, `${path}#${refresh}`)
  const rows = rowsOf(data)
  return <PanelSection titleKey={titleKey} loading={loading} error={error} empty={rows.length === 0}
    actions={addTo && <Link className="btn secondary" to={addTo}><Plus size={16} />{t(addKey)}</Link>}>
    <div className="table-card"><table>
      <thead><tr>{columns.map((c) => <th key={c.key}>{t(c.labelKey)}</th>)}{onDelete && <th>{t('actions')}</th>}</tr></thead>
      <tbody>{rows.map((row, i) => <tr key={String(row.uuid ?? i)}>
        {columns.map((c) => <td key={c.key}>{cell(c, row[c.key])}</td>)}
        {onDelete && <td><div className="row-actions"><button className="danger-text" aria-label={t(actionKey)} onClick={() => onDelete(row)}><Trash2 size={16} /></button></div></td>}
      </tr>)}</tbody></table></div>
  </PanelSection>
}
