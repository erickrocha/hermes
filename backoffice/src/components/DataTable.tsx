import { flexRender, getCoreRowModel, useReactTable } from '@tanstack/react-table'
import type { ColumnDef } from '@tanstack/react-table'
import { ChevronLeft, ChevronRight } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import type { Page, PageRequest } from '../types'
import { Empty, Loading } from './UI'

// PD-028: a única tabela do console. Paginação é do servidor (`manualPagination`),
// então o TanStack nunca fatia linhas por conta própria -- ele recebe exatamente
// a página que a API devolveu, e `pageIndex` é o mesmo `page` base zero do
// envelope. Fatiar no cliente daria uma tabela que parece funcionar com 25
// linhas e mente com 26.
interface DataTableProps<T> {
  columns: ColumnDef<T, unknown>[]
  page: Page<T>
  loading: boolean
  onPageChange: (request: PageRequest) => void
}

/// O rodapé de paginação, separado da tabela porque nem toda lista do console é
/// uma tabela -- a de tenants é uma grade de cartões e continua sendo.
export function Pagination<T>({ page, onPageChange }: { page: Page<T>; onPageChange: (request: PageRequest) => void }) {
  const { t } = useTranslation()
  if (page.totalPages <= 1) return null

  const first = page.page * page.pageSize + 1
  const last = Math.min(first + page.items.length - 1, page.totalItems)

  return (
    <nav className="table-pagination" aria-label={t('pagination')}>
      <span className="page-range">{t('pageRange', { first, last, total: page.totalItems })}</span>
      <div className="page-controls">
        <button
          type="button"
          className="btn tertiary"
          disabled={page.page <= 0}
          aria-label={t('previousPage')}
          onClick={() => onPageChange({ page: page.page - 1, pageSize: page.pageSize })}
        >
          <ChevronLeft size={16} />
        </button>
        <span className="page-position">{t('pagePosition', { page: page.page + 1, pages: page.totalPages })}</span>
        <button
          type="button"
          className="btn tertiary"
          disabled={page.page + 1 >= page.totalPages}
          aria-label={t('nextPage')}
          onClick={() => onPageChange({ page: page.page + 1, pageSize: page.pageSize })}
        >
          <ChevronRight size={16} />
        </button>
      </div>
    </nav>
  )
}

export function DataTable<T>({ columns, page, loading, onPageChange }: DataTableProps<T>) {
  const table = useReactTable({
    data: page.items,
    columns,
    getCoreRowModel: getCoreRowModel(),
    manualPagination: true,
    pageCount: page.totalPages,
    state: { pagination: { pageIndex: page.page, pageSize: page.pageSize } },
  })

  if (loading && !page.items.length) return <Loading />
  if (!page.items.length) return <Empty />

  return (
    <div className="table-card">
      <div className="table-scroll">
        <table>
          <thead>
            {table.getHeaderGroups().map((headerGroup) => (
              <tr key={headerGroup.id}>
                {headerGroup.headers.map((header) => (
                  <th key={header.id}>
                    {header.isPlaceholder ? null : flexRender(header.column.columnDef.header, header.getContext())}
                  </th>
                ))}
              </tr>
            ))}
          </thead>
          <tbody>
            {table.getRowModel().rows.map((row) => (
              <tr key={row.id}>
                {row.getVisibleCells().map((cell) => (
                  <td key={cell.id}>{flexRender(cell.column.columnDef.cell, cell.getContext())}</td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <Pagination page={page} onPageChange={onPageChange} />
    </div>
  )
}
