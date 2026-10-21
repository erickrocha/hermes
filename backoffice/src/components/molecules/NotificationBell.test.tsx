import { screen, waitFor } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { api } from '../../api'
import { renderPage, session, signIn } from '../../test/render'
import { NotificationBell, canSeeMaintenanceNotifications } from './NotificationBell'

describe('NotificationBell', () => {
  beforeEach(() => { vi.restoreAllMocks() })

  it('follows the permission of the maintenance notifications', () => {
    expect(canSeeMaintenanceNotifications(session('TenantOwner'))).toBe(true)
    expect(canSeeMaintenanceNotifications(session('Driver'))).toBe(false)
  })

  it('renders a link to the notifications for who can read them', async () => {
    vi.spyOn(api, 'get').mockResolvedValue({ data: { items: [], page: 0, pageSize: 25, totalItems: 3, totalPages: 1, unread: 3, count: 3 } } as never)
    signIn('TenantOwner')
    const { container } = renderPage(<NotificationBell session={session('TenantOwner')} />)
    await waitFor(() => expect(container.querySelector('a')).not.toBeNull())
    expect(screen.getByRole('link')).toBeInTheDocument()
  })
})
