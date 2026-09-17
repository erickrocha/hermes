import { render, screen } from '@testing-library/react'
import { configureStore } from '@reduxjs/toolkit'
import { Provider } from 'react-redux'
import { MemoryRouter } from 'react-router-dom'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import './i18n'
import { api } from './api'
import App from './App'
import type { Session } from './types'

const sysAdminSession: Session = { accessToken: 't', tokenType: 'Bearer', expireIn: 3600, email: 'root@hermes.dev', uuid: 'sys-uuid', name: 'Root', userId: 1, role: 'SysAdmin', tenantId: null, firstLogin: false }
const tenantOwnerSession: Session = { accessToken: 't', tokenType: 'Bearer', expireIn: 3600, email: 'owner@transmega.com', uuid: 'owner-uuid', name: 'Owner', userId: 2, role: 'TenantOwner', tenantId: 42, firstLogin: false }
const tenantUserSession: Session = { accessToken: 't', tokenType: 'Bearer', expireIn: 3600, email: 'driver.coord@transmega.com', uuid: 'user-uuid', name: 'Coordinator', userId: 3, role: 'TenantUser', tenantId: 42, firstLogin: false }

function sessionStore(session: Session | null) {
  return configureStore({
    reducer: {
      auth: (state = { session, loading: false, error: '' }) => state,
      data: (state = { tenants: [], plans: [], users: [], provinces: [], cities: [], loading: false, error: '' }) => state,
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

  it('lets a SysAdmin reach plan administration', () => {
    renderAt('/plans', sysAdminSession)
    expect(screen.getByRole('link', { name: /new plan|novo plano/i })).toBeInTheDocument()
  })

  it('bounces a tenant owner away from plan administration back to the dashboard', () => {
    renderAt('/plans', tenantOwnerSession)
    expect(screen.queryByRole('button', { name: /new plan|novo plano/i })).not.toBeInTheDocument()
    expect(screen.getByText(/control center|centro de controle/i)).toBeInTheDocument()
  })

  it('bounces a tenant user away from plan administration back to the dashboard', () => {
    renderAt('/plans', tenantUserSession)
    expect(screen.getByText(/control center|centro de controle/i)).toBeInTheDocument()
  })

  it('bounces a tenant owner away from creating a tenant (SysAdmin-only)', () => {
    renderAt('/tenants/new', tenantOwnerSession)
    expect(screen.queryByText(/business name|razão social/i)).not.toBeInTheDocument()
  })

  it('bounces a tenant owner away from a tenant subscription screen (SysAdmin-only)', () => {
    renderAt('/tenants/1/subscription', tenantOwnerSession)
    expect(screen.getByText(/control center|centro de controle/i)).toBeInTheDocument()
  })

  it('sends an unauthenticated visitor to the login screen', () => {
    renderAt('/plans', null)
    expect(screen.getByRole('button', { name: /sign in|entrar/i })).toBeInTheDocument()
  })

  it('sends a first-login session straight to setting a password, bypassing every other route', () => {
    renderAt('/plans', { ...sysAdminSession, firstLogin: true })
    expect(screen.queryByText(/plans|planos/i)).not.toBeInTheDocument()
  })
})
