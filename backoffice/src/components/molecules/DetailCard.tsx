import type { ReactNode } from 'react'

export interface DetailRow { label: string; value: ReactNode; hide?: boolean }

/// Os dados de um registro em pares rótulo/valor; linhas sem valor ficam de fora.
export function DetailCard({ rows }: { rows: DetailRow[] }) {
  return <div className="table-card"><table><tbody>
    {rows.filter((row) => !row.hide && row.value !== undefined && row.value !== null && row.value !== '').map((row) => <tr key={row.label}><th>{row.label}</th><td>{row.value}</td></tr>)}
  </tbody></table></div>
}
