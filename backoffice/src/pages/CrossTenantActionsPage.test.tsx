import { screen } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { api } from '../api'
import { canSeeCrossTenantActions } from '../features/routeAccess'
import { customRoutes } from '../routes'
import { renderPage, session, signIn } from '../test/render'
import { CrossTenantActionsPage } from './CrossTenantActionsPage'

const action = { actorUserId: 1, actorEmail: 'suporte@erocha.test', actorTenantId: 1, affectedTenantId: 7, kind: 'write', method: 'PUT', operation: '/vehicle/uuid/{uuid}', path: '/vehicle/uuid/abc', occurredAt: '2026-10-29T14:53:47' }
const page = (items: unknown[]) => ({ data: { items, page: 0, pageSize: 25, totalItems: items.length, totalPages: items.length ? 1 : 0 } })

// C-036 (`HRMS-877`): the affected tenant reads what the support did in its data.
describe('CrossTenantActionsPage', () => {
  beforeEach(() => { vi.restoreAllMocks() })

  it('is reached by whoever holds cross-tenant-action.read -- the owner, not the other profiles', () => {
    expect(canSeeCrossTenantActions(session('TenantOwner'))).toBe(true)
    expect(canSeeCrossTenantActions(session('Mechanic'))).toBe(false)
    expect(customRoutes.find((route) => route.path === '/cross-tenant-actions')?.allow(session('TenantOwner'))).toBe(true)
  })

  it('shows who did what, where and when, marking the writes', async () => {
    vi.spyOn(api, 'get').mockResolvedValue(page([action, { ...action, kind: 'read', method: 'GET', path: '/vehicle' }]) as never)
    signIn('TenantOwner')
    renderPage(<CrossTenantActionsPage />, '/cross-tenant-actions', '/cross-tenant-actions')
    expect(await screen.findAllByText('suporte@erocha.test')).toHaveLength(2)
    expect(screen.getByText('/vehicle/uuid/abc')).toBeInTheDocument()
    expect(screen.getByText(/^write$|^escrita$/i)).toBeInTheDocument()
    expect(screen.getByText(/^read$|^leitura$/i)).toBeInTheDocument()
  })

  it('says so when nothing was done in the tenant', async () => {
    vi.spyOn(api, 'get').mockResolvedValue(page([]) as never)
    signIn('TenantOwner')
    renderPage(<CrossTenantActionsPage />, '/cross-tenant-actions', '/cross-tenant-actions')
    expect(await screen.findByText(/no records|nenhum registro|sem registros|nothing/i)).toBeInTheDocument()
  })
})
