import { fireEvent, screen, waitFor } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { api } from '../api'
import { renderPage, session, signIn } from '../test/render'
import { WorkOrderDetailPage, WorkOrderItemPage, WorkOrderNewPage, WorkOrdersPage, canSeeWorkOrders } from './WorkOrderPages'

const order = { uuid: 'w1', number: 'OS-1', vehicleUuid: 'v1', odometerKm: 1200, description: 'Brake pads', status: 'Open', openedAt: '2026-10-01T10:00:00', totalCostCents: 2500, items: [{ uuid: 'i1', description: 'Change pads', status: 'Pending' }] }

describe('work orders (C-026, HRMS-700…705)', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    vi.spyOn(api, 'get').mockImplementation(async (url: string) => {
      if (url === '/work-order') return { data: { items: [order], page: 0, pageSize: 25, totalItems: 1, totalPages: 1 } } as never
      if (url === '/work-order/uuid/w1') return { data: order } as never
      if (url === '/vehicle') return { data: { items: [{ uuid: 'v1', plate: 'ABC1D23', model: 'Bus' }], page: 0, pageSize: 200, totalItems: 1, totalPages: 1 } } as never
      throw new Error(`unexpected ${url}`)
    })
  })

  // C-035: the screen follows `work-order.read`, which every tenant system profile holds (the Driver included, as
  // the server's catalogue has it); the platform administrator has no tenant to read work orders in.
  it('offers the screen to whoever holds work-order.read inside a tenant, not to the platform administrator', () => {
    for (const profile of ['TenantOwner', 'TenantUser', 'Mechanic', 'Driver'] as const) expect(canSeeWorkOrders(session(profile))).toBe(true)
    expect(canSeeWorkOrders(session('SystemAdministrator'))).toBe(false)
    expect(canSeeWorkOrders(session('Mechanic', ['vehicle.read']))).toBe(false)
  })

  it('lists the orders; the mechanic sees the new-order button, the tenant user does not', async () => {
    signIn('Mechanic')
    const { unmount } = renderPage(<WorkOrdersPage />)
    expect(await screen.findByText('OS-1')).toBeInTheDocument()
    expect(screen.getByRole('link', { name: /new work order|nova ordem|nueva orden/i })).toBeInTheDocument()
    unmount()
    signIn('TenantUser')
    renderPage(<WorkOrdersPage />)
    expect(await screen.findByText('OS-1')).toBeInTheDocument()
    expect(screen.queryByRole('link', { name: /new work order|nova ordem|nueva orden/i })).toBeNull()
  })

  it('shows an error state when the list cannot be loaded', async () => {
    vi.spyOn(api, 'get').mockRejectedValue(new Error('down'))
    signIn('Mechanic')
    renderPage(<WorkOrdersPage />)
    expect(await screen.findByRole('alert')).toBeInTheDocument()
  })

  it('validates the new-order form and posts the trimmed payload', async () => {
    const post = vi.spyOn(api, 'post').mockResolvedValue({ data: { uuid: 'w2' } } as never)
    signIn('Mechanic')
    renderPage(<WorkOrderNewPage />, '/work-orders/new', '/work-orders/new')
    fireEvent.click(await screen.findByRole('button', { name: /^save$|^salvar$|^guardar$/i }))
    expect((await screen.findAllByText(/required field|campo obrigat/i)).length).toBeGreaterThan(0)
    expect(post).not.toHaveBeenCalled()
    fireEvent.change(await screen.findByLabelText(/vehicle|veículo|vehículo/i), { target: { value: 'v1' } })
    fireEvent.change(screen.getByLabelText(/odometer|odômetro|odómetro/i), { target: { value: '1200' } })
    fireEvent.change(screen.getByLabelText(/^description|^descrição|^descripción/i), { target: { value: '  Brake pads  ' } })
    fireEvent.click(screen.getByRole('button', { name: /^save$|^salvar$|^guardar$/i }))
    await waitFor(() => expect(post).toHaveBeenCalledWith('/work-order', { vehicleUuid: 'v1', odometerKm: 1200, description: 'Brake pads' }))
  })

  it('puts cancel on the left and save on the right', async () => {
    signIn('Mechanic')
    renderPage(<WorkOrderNewPage />, '/work-orders/new', '/work-orders/new')
    const cancel = await screen.findByRole('link', { name: /cancel|cancelar/i })
    const save = screen.getByRole('button', { name: /^save$|^salvar$|^guardar$/i })
    expect(cancel.compareDocumentPosition(save) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy()
  })

  it('adds an item on its own page, not in a modal', async () => {
    const post = vi.spyOn(api, 'post').mockResolvedValue({ data: {} } as never)
    signIn('Mechanic')
    renderPage(<WorkOrderItemPage />, '/work-orders/w1/items/new', '/work-orders/:uuid/items/new')
    fireEvent.change(await screen.findByLabelText(/^description|^descrição|^descripción/i), { target: { value: 'Check lights' } })
    fireEvent.click(screen.getByRole('button', { name: /^save$|^salvar$|^guardar$/i }))
    await waitFor(() => expect(post).toHaveBeenCalledWith('/work-order/uuid/w1/item', { description: 'Check lights' }))
    expect(screen.queryByRole('dialog')).toBeNull()
  })

  it('lets the mechanic open the item page but not conclude; the owner can conclude', async () => {
    signIn('Mechanic')
    const { unmount } = renderPage(<WorkOrderDetailPage />, '/work-orders/w1', '/work-orders/:uuid')
    expect(await screen.findByText('Change pads')).toBeInTheDocument()
    expect(screen.getByRole('link', { name: /new item|novo item|nuevo/i })).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /^conclude$|^concluir$/i })).toBeNull()
    unmount()
    signIn('TenantOwner')
    renderPage(<WorkOrderDetailPage />, '/work-orders/w1', '/work-orders/:uuid')
    expect(await screen.findByRole('button', { name: /^conclude$|^concluir$/i })).toBeInTheDocument()
  })
})
