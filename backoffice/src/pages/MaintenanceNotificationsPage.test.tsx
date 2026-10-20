import { fireEvent, screen, waitFor } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { api } from '../api'
import { NotificationBell, canSeeMaintenanceNotifications } from '../components/molecules/NotificationBell'
import { customRoutes } from '../routes'
import { renderPage, session, signIn } from '../test/render'
import { MaintenanceNotificationsPage } from './MaintenanceNotificationsPage'

const unread = { uuid: 'n1', workOrderUuid: 'w1', partName: 'Brake pad', quantity: 4, read: false, createdAt: '2026-10-15T10:00:00' }
const done = { uuid: 'n2', workOrderUuid: null, partName: 'Oil filter', quantity: 1, read: true, readAt: '2026-10-15T11:00:00', createdAt: '2026-10-14T10:00:00' }
const page = (items: unknown[]) => ({ data: { items, page: 0, pageSize: 25, totalItems: items.length, totalPages: items.length ? 1 : 0 } })

describe('maintenance notices (C-026 EPIC-MT-06)', () => {
  beforeEach(() => { vi.restoreAllMocks() })

  it('is offered to the owner and the mechanic, not to the office user or the driver', () => {
    expect(canSeeMaintenanceNotifications(session('TenantOwner'))).toBe(true)
    expect(canSeeMaintenanceNotifications(session('Mechanic'))).toBe(true)
    expect(canSeeMaintenanceNotifications(session('TenantUser'))).toBe(false)
    expect(canSeeMaintenanceNotifications(session('Driver'))).toBe(false)
    expect(customRoutes.find((route) => route.path === '/maintenance-notifications')?.allow(session('Mechanic'))).toBe(true)
  })

  it('lists notices with a work-order link, highlights unread ones and marks one read', async () => {
    const get = vi.spyOn(api, 'get').mockImplementation(async (url: string) => (url === '/maintenance-notification/unread-count' ? { data: { count: 1 } } : page([unread, done])) as never)
    const put = vi.spyOn(api, 'put').mockResolvedValue({ data: { ...unread, read: true } } as never)
    signIn('Mechanic')
    renderPage(<MaintenanceNotificationsPage />)

    expect(await screen.findByText(/Part arrived: Brake pad|Peça chegou: Brake pad|Llegó la pieza: Brake pad/)).toBeInTheDocument()
    expect(screen.getByRole('link', { name: /open work order|abrir ordem|abrir orden/i })).toHaveAttribute('href', '/work-orders/w1')
    fireEvent.click(screen.getByRole('button', { name: /mark as read|marcar como lida|marcar como leída/i }))
    await waitFor(() => expect(put).toHaveBeenCalledWith('/maintenance-notification/uuid/n1/read'))
    await waitFor(() => expect(get).toHaveBeenCalledWith('/maintenance-notification/unread-count'))
  })

  it('asks only for the unread ones when the filter is on, and marks all read', async () => {
    const get = vi.spyOn(api, 'get').mockImplementation(async () => page([unread]) as never)
    const put = vi.spyOn(api, 'put').mockResolvedValue({ data: { marked: 1 } } as never)
    signIn('TenantOwner')
    renderPage(<MaintenanceNotificationsPage />)
    await screen.findByText(/Brake pad/)

    fireEvent.click(screen.getByRole('button', { name: /unread only|só não lidos|solo sin leer/i }))
    await waitFor(() => expect(get).toHaveBeenCalledWith('/maintenance-notification', { params: { page: 0, pageSize: 25, unread: true } }))
    fireEvent.click(screen.getByRole('button', { name: /mark all as read|marcar todas como lidas|marcar todas como leídas/i }))
    await waitFor(() => expect(put).toHaveBeenCalledWith('/maintenance-notification/read-all'))
  })

  it('shows the empty and the error states', async () => {
    vi.spyOn(api, 'get').mockImplementation(async () => page([]) as never)
    signIn('TenantOwner')
    const first = renderPage(<MaintenanceNotificationsPage />)
    expect(await screen.findByText(/no records found|nenhum registro|no se encontraron/i)).toBeInTheDocument()
    first.unmount()

    vi.restoreAllMocks()
    vi.spyOn(api, 'get').mockRejectedValue(new Error('boom'))
    renderPage(<MaintenanceNotificationsPage />)
    expect(await screen.findByRole('alert')).toBeInTheDocument()
  })

  it('shows the unread counter in the bell for a mechanic and no bell for a driver', async () => {
    vi.spyOn(api, 'get').mockResolvedValue({ data: { count: 3 } } as never)
    signIn('Mechanic')
    const mechanic = renderPage(<NotificationBell session={session('Mechanic')} />)
    expect(await screen.findByTestId('notification-count')).toHaveTextContent('3')
    mechanic.unmount()

    renderPage(<NotificationBell session={session('Driver')} />)
    expect(screen.queryByRole('link')).toBeNull()
  })
})
