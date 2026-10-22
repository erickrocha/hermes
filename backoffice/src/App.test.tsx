import { fireEvent, render, screen } from '@testing-library/react'
import { configureStore } from '@reduxjs/toolkit'
import { Provider } from 'react-redux'
import { MemoryRouter } from 'react-router-dom'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import './i18n'
import { api } from './api'
import { App } from './App'
import { emptyPage } from './types'
import type { Session } from './types'

const sysAdminSession: Session = { accessToken: 't', tokenType: 'Bearer', expireIn: 3600, email: 'root@hermes.dev', uuid: 'sys-uuid', name: 'Root', userId: 1, role: 'SysAdmin', tenantId: null }
const tenantOwnerSession: Session = { accessToken: 't', tokenType: 'Bearer', expireIn: 3600, email: 'owner@transmega.com', uuid: 'owner-uuid', name: 'Owner', userId: 2, role: 'TenantOwner', tenantId: 42 }
const tenantUserSession: Session = { accessToken: 't', tokenType: 'Bearer', expireIn: 3600, email: 'driver.coord@transmega.com', uuid: 'user-uuid', name: 'Coordinator', userId: 3, role: 'TenantUser', tenantId: 42 }

function sessionStore(session: Session | null) {
  return configureStore({
    reducer: {
      auth: (state = { session, loading: false, error: '' }) => state,
      data: (state = { tenants: emptyPage(), plans: emptyPage(), users: emptyPage(), provinces: [], cities: [], countries: ['BR', 'US'], counts: { tenants: 0, users: 0, plans: 0 }, currentTenant: null, loading: false, error: '' }) => state,
    },
  })
}

function renderAt(path: string, session: Session | null) {
  return render(
    <Provider store={sessionStore(session)}>
      <MemoryRouter initialEntries={[path]}><App /></MemoryRouter>
    </Provider>
  )
}

// EPIC-BO-02-S03 / EPIC-BO-04-S02 (HRMS-403, HRMS-401, PD-015): plan
// administration belongs to the vendor, not the tenant, and the server
// refuses it regardless of what the console shows -- this only covers the
// console half. PD-015's own postmortem is that the restriction once lived
// only here, so this must assert the *route*, not just a hidden nav link.
describe('App route guards (EPIC-BO-02, EPIC-BO-04)', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    vi.spyOn(api, 'get').mockResolvedValue({ data: [] } as never)
  })

  it('lets a SysAdmin reach plan administration', async () => {
    renderAt('/plans', sysAdminSession)
    expect(await screen.findByRole('link', { name: /new plan|novo plano/i })).toBeInTheDocument()
  })

  it('bounces a tenant owner away from plan administration back to the dashboard', async () => {
    renderAt('/plans', tenantOwnerSession)
    expect(screen.queryByRole('button', { name: /new plan|novo plano/i })).not.toBeInTheDocument()
    expect(await screen.findByText(/control center|centro de controle/i)).toBeInTheDocument()
  })

  it('bounces a tenant user away from plan administration back to the dashboard', async () => {
    renderAt('/plans', tenantUserSession)
    expect(await screen.findByText(/control center|centro de controle/i)).toBeInTheDocument()
  })

  it('bounces a tenant owner away from creating a tenant (SysAdmin-only)', async () => {
    renderAt('/tenants/new', tenantOwnerSession)
    await screen.findByText(/control center|centro de controle/i)
    expect(screen.queryByText(/business name|razão social/i)).not.toBeInTheDocument()
  })

  it('bounces a tenant owner away from a tenant subscription screen (SysAdmin-only)', async () => {
    renderAt('/tenants/1/subscription', tenantOwnerSession)
    expect(await screen.findByText(/control center|centro de controle/i)).toBeInTheDocument()
  })

  it('sends an unauthenticated visitor to the login screen', async () => {
    renderAt('/plans', null)
    expect(await screen.findByRole('button', { name: /sign in|entrar/i })).toBeInTheDocument()
  })

  it('collapses and expands navigation groups', async () => {
    renderAt('/', tenantOwnerSession)
    const fleetGroup = await screen.findByRole('button', { name: /fleet|frota/i })
    const vehicleLink = screen.getByRole('link', { name: /^(vehicles|veículos)$/i })

    expect(fleetGroup).toHaveAttribute('aria-expanded', 'true')
    expect(vehicleLink).toBeInTheDocument()

    fireEvent.click(fleetGroup)
    expect(fleetGroup).toHaveAttribute('aria-expanded', 'false')
    expect(screen.queryByRole('link', { name: /^(vehicles|veículos)$/i })).not.toBeInTheDocument()

    fireEvent.click(fleetGroup)
    expect(fleetGroup).toHaveAttribute('aria-expanded', 'true')
    expect(screen.getByRole('link', { name: /^(vehicles|veículos)$/i })).toBeInTheDocument()
  })

})
