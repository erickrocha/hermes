import { fireEvent, render, screen } from '@testing-library/react'
import type { ColumnDef } from '@tanstack/react-table'
import { describe, expect, it, vi } from 'vitest'
import '../i18n'
import { DataTable } from './DataTable'
import type { Page } from '../types'

interface Row { id: number; name: string }

const columns: ColumnDef<Row, unknown>[] = [
  { header: () => 'Name', accessorKey: 'name' },
]

const pageOf = (page: number, totalItems: number, pageSize = 2): Page<Row> => ({
  items: Array.from({ length: Math.min(pageSize, totalItems - page * pageSize) }, (_, index) => ({
    id: page * pageSize + index,
    name: `row-${page * pageSize + index}`,
  })),
  page,
  pageSize,
  totalItems,
  totalPages: Math.ceil(totalItems / pageSize),
})

// PD-028: a tabela recebe a página que o servidor devolveu e não fatia nada.
// O que pode quebrar em silêncio é o contrário -- paginar no cliente, ou
// deixar avançar além da última página -- então é isso que estes casos prendem.
describe('DataTable server-side pagination', () => {
  it('renders exactly the rows the server returned, not a client-side slice', () => {
    render(<DataTable columns={columns} page={pageOf(0, 5)} loading={false} onPageChange={vi.fn()} />)
    expect(screen.getByText('row-0')).toBeInTheDocument()
    expect(screen.getByText('row-1')).toBeInTheDocument()
    expect(screen.queryByText('row-2')).not.toBeInTheDocument()
  })

  it('asks for the next page by index rather than fetching it itself', () => {
    const onPageChange = vi.fn()
    render(<DataTable columns={columns} page={pageOf(0, 5)} loading={false} onPageChange={onPageChange} />)
    fireEvent.click(screen.getByLabelText(/next page|próxima página|página siguiente/i))
    expect(onPageChange).toHaveBeenCalledWith({ page: 1, pageSize: 2 })
  })

  it('cannot move before the first page or past the last', () => {
    const { unmount } = render(<DataTable columns={columns} page={pageOf(0, 5)} loading={false} onPageChange={vi.fn()} />)
    expect(screen.getByLabelText(/previous page|página anterior/i)).toBeDisabled()
    unmount()

    render(<DataTable columns={columns} page={pageOf(2, 5)} loading={false} onPageChange={vi.fn()} />)
    expect(screen.getByLabelText(/next page|próxima página|página siguiente/i)).toBeDisabled()
  })

  it('reports the range against the whole table, not the loaded page', () => {
    render(<DataTable columns={columns} page={pageOf(1, 5)} loading={false} onPageChange={vi.fn()} />)
    expect(screen.getByText('3–4 of 5')).toBeInTheDocument()
  })

  it('hides the pager when everything fits on one page', () => {
    render(<DataTable columns={columns} page={pageOf(0, 2)} loading={false} onPageChange={vi.fn()} />)
    expect(screen.queryByLabelText(/next page|próxima página|página siguiente/i)).not.toBeInTheDocument()
  })
})
