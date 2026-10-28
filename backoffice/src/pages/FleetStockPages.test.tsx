import { fireEvent, screen, waitFor } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { api } from '../api'
import { renderPage, signIn } from '../test/render'
import { CustomerDayOffPage, CustomerDetailPage, DemandExceptionPage, VehicleDetailPage } from './FleetPages'
import { PartDetailPage, PartEntryPage, PurchaseOrderDetailPage } from './StockPages'

const page = (items: unknown[]) => ({ data: { items, page: 0, pageSize: 50, totalItems: items.length, totalPages: 1 } }) as never
const notFound = () => Promise.reject(Object.assign(new Error('nf'), { isAxiosError: true, response: { status: 404, data: { message: 'none' } } }))
const save = /^save$|^salvar$|^guardar$/i

describe('stock (C-030) and fleet detail pages', () => {
  beforeEach(() => { vi.restoreAllMocks() })

  it('shows a part with its derived stock; the mechanic can enter stock, the tenant user cannot', async () => {
    vi.spyOn(api, 'get').mockResolvedValue({ data: { uuid: 'p1', code: 'PC-1', name: 'Brake pad', unit: 'un', currentStock: 12, minimumStock: 3, movingAverageCostCents: 760 } } as never)
    signIn('Mechanic')
    const { unmount } = renderPage(<PartDetailPage />, '/parts/p1', '/parts/:uuid')
    expect(await screen.findByText(/PC-1 — Brake pad/)).toBeInTheDocument()
    expect(screen.getByText('12 un')).toBeInTheDocument(); expect(screen.getByText(/7[.,]60/)).toBeInTheDocument()
    expect(screen.getByRole('link', { name: /stock entry|entrada de estoque|entrada de stock/i })).toBeInTheDocument()
    unmount()
    signIn('TenantUser')
    renderPage(<PartDetailPage />, '/parts/p1', '/parts/:uuid')
    await screen.findByText(/PC-1 — Brake pad/)
    expect(screen.queryByRole('link', { name: /stock entry|entrada de estoque|entrada de stock/i })).toBeNull()
  })

  it('posts a stock entry with the value converted to cents', async () => {
    const post = vi.spyOn(api, 'post').mockResolvedValue({ data: {} } as never)
    signIn('Mechanic')
    renderPage(<PartEntryPage />, '/parts/p1/entry', '/parts/:uuid/entry')
    fireEvent.change(await screen.findByLabelText(/^quantity|^quantidade|^cantidad/i), { target: { value: '10' } })
    fireEvent.change(screen.getByLabelText(/^unit value|^valor unitário|^valor unitario/i), { target: { value: '6.5' } })
    fireEvent.click(screen.getByRole('button', { name: save }))
    await waitFor(() => expect(post).toHaveBeenCalledWith('/part/uuid/p1/entry', { quantity: 10, unitValueCents: 650 }))
  })

  it('moves a purchase order: ordered orders can be marked purchased only after confirming', async () => {
    vi.spyOn(api, 'get').mockResolvedValue({ data: { uuid: 'o1', number: 'PO-1', status: 'Ordered', quantity: 4 } } as never)
    const put = vi.spyOn(api, 'put').mockResolvedValue({ data: {} } as never)
    signIn('Mechanic')
    renderPage(<PurchaseOrderDetailPage />, '/purchase-orders/o1', '/purchase-orders/:uuid')
    fireEvent.click(await screen.findByRole('button', { name: /mark as purchased|marcar como comprado/i }))
    expect(put).not.toHaveBeenCalled()
    fireEvent.click(screen.getByRole('button', { name: /confirm|confirmar/i }))
    await waitFor(() => expect(put).toHaveBeenCalledWith('/purchase-order/uuid/o1/mark-purchased', undefined))
  })

  it('a vehicle without a current driver offers the assignment; the history is listed', async () => {
    vi.spyOn(api, 'get').mockImplementation(async (url: string) => {
      if (url === '/vehicle/uuid/v1') return { data: { uuid: 'v1', plate: 'ABC1D23', model: 'Bus', status: 'Active', odometerKm: 1500 } } as never
      if (url.endsWith('/assignment') || url.endsWith('/tracking')) return notFound()
      if (url.endsWith('/km-evolutions')) return page([{ uuid: 'k1', km: 1500, origin: 'Manual', recordedAt: '2026-10-01T10:00:00' }])
      return page([])
    })
    signIn('TenantOwner')
    renderPage(<VehicleDetailPage />, '/vehicles/v1', '/vehicles/:uuid')
    expect(await screen.findByText(/ABC1D23 — Bus/)).toBeInTheDocument()
    expect(await screen.findByRole('link', { name: /assign driver|atribuir motorista|asignar conductor/i })).toBeInTheDocument()
    expect((await screen.findAllByText('1500')).length).toBe(2)
    expect(screen.getByText(/tracking unavailable|rastreamento indisponível|rastreo no disponible/i)).toBeInTheDocument()
  })

  it('shows a customer with its legal name, masked document and the address as one block', async () => {
    vi.spyOn(api, 'get').mockImplementation(async (url: string) => {
      if (url === '/customer/uuid/c1') return { data: { uuid: 'c1', name: 'Acme', legalName: 'Acme Ltda', taxId: '11222333000181', countryCode: 'BR', status: 'Active', addressLine1: 'Rua A, 10', addressLine2: 'Sala 2', locality: 'Campinas', administrativeArea: 'SP', postalCode: '13000-000' } } as never
      return page([])
    })
    signIn('TenantOwner')
    renderPage(<CustomerDetailPage />, '/customers/c1', '/customers/:uuid')
    expect(await screen.findByRole('heading', { name: 'Acme' })).toBeInTheDocument()
    expect(screen.getByText('Acme Ltda')).toBeInTheDocument()
    expect(screen.getByText('11.222.333/0001-81')).toBeInTheDocument()
    const address = screen.getByText('Rua A, 10').closest('address')!
    expect(address).toHaveTextContent('Sala 2')
    expect(address).toHaveTextContent('Campinas · SP')
    expect(address).toHaveTextContent(/13000-000 · (Brazil|Brasil)/)
    expect(screen.getByRole('link', { name: /^edit$|^editar$/i })).toHaveAttribute('href', '/customers/c1/edit')
  })

  it('records a customer day off', async () => {
    const post = vi.spyOn(api, 'post').mockResolvedValue({ data: {} } as never)
    signIn('TenantOwner')
    renderPage(<CustomerDayOffPage />, '/customers/c1/day-offs/new', '/customers/:uuid/day-offs/new')
    fireEvent.change(await screen.findByLabelText(/^date|^data|^fecha/i), { target: { value: '2026-12-24' } })
    fireEvent.click(screen.getByRole('button', { name: save }))
    await waitFor(() => expect(post).toHaveBeenCalledWith('/customer/uuid/c1/day-off', { date: '2026-12-24' }))
  })

  it('requires the exception type before posting', async () => {
    vi.spyOn(api, 'get').mockResolvedValue(page([]))
    const post = vi.spyOn(api, 'post').mockResolvedValue({ data: {} } as never)
    signIn('TenantOwner')
    renderPage(<DemandExceptionPage />, '/transport-demands/d1/exceptions/new', '/transport-demands/:uuid/exceptions/new')
    fireEvent.change(await screen.findByLabelText(/^date|^data|^fecha/i), { target: { value: '2026-12-24' } })
    fireEvent.click(screen.getByRole('button', { name: save }))
    expect(await screen.findByText(/required field|campo obrigat/i)).toBeInTheDocument()
    expect(post).not.toHaveBeenCalled()
    fireEvent.change(screen.getByLabelText(/exception type|tipo de exceção|tipo de excepción/i), { target: { value: 'Cancellation' } })
    fireEvent.click(screen.getByRole('button', { name: save }))
    await waitFor(() => expect(post).toHaveBeenCalledWith('/transport-demand/uuid/d1/exception', { date: '2026-12-24', exceptionType: 'Cancellation' }))
  })
})
