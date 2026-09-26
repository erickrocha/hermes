import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { configureStore } from '@reduxjs/toolkit'
import { Provider } from 'react-redux'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import '../i18n'
import { api } from '../api'
import { store } from '../store'
import { emptyPage } from '../types'
import type { Session, Tenant } from '../types'
import { PlanEditorPage, UserEditorPage, amountToCents, canCreateUsers, canReachAdministration, centsToAmount } from './ManagementPages'

// EPIC-BO-03-S02 (HRMS-412): the API's price fields are integer cents; a
// human types and reads currency amounts. These two conversions are the
// entire fix -- get them wrong and every price is off by a factor of 100.
describe('cents/amount conversion', () => {
  it('renders cents as a two-decimal currency amount', () => {
    expect(centsToAmount(12345)).toBe('123.45')
    expect(centsToAmount(0)).toBe('0.00')
  })

  it('parses a typed amount back into cents', () => {
    expect(amountToCents('123.45')).toBe(12345)
    expect(amountToCents('100')).toBe(10000)
    expect(amountToCents('')).toBe(0)
  })
})

describe('PlanEditorPage price editing', () => {
  beforeEach(() => { vi.restoreAllMocks() })

  it('shows a loaded plan price in currency units, not raw cents', async () => {
    vi.spyOn(api, 'get').mockResolvedValue({ data: { id: 1, uuid: 'plan-uuid-1', name: 'Pro', priceInCents: 12345, availableUsers: 5, periodDays: 30, paymentDate: '2026-01-01' } } as never)

    render(
      <Provider store={store}>
        <MemoryRouter initialEntries={['/plans/plan-uuid-1/edit']}>
          <Routes><Route path="/plans/:uuid/edit" element={<PlanEditorPage />} /></Routes>
        </MemoryRouter>
      </Provider>
    )

    const priceInput = await screen.findByDisplayValue('123.45')
    expect(priceInput).toBeInTheDocument()
  })

  it('converts a typed currency amount back into cents on save', async () => {
    vi.spyOn(api, 'get').mockResolvedValue({ data: { id: 1, uuid: 'plan-uuid-1', name: 'Pro', priceInCents: 0, availableUsers: 5, periodDays: 30, paymentDate: '2026-01-01' } } as never)
    const put = vi.spyOn(api, 'put').mockResolvedValue({ data: {} } as never)

    render(
      <Provider store={store}>
        <MemoryRouter initialEntries={['/plans/plan-uuid-1/edit']}>
          <Routes><Route path="/plans/:uuid/edit" element={<PlanEditorPage />} /></Routes>
        </MemoryRouter>
      </Provider>
    )

    const priceInput = await screen.findByDisplayValue('0.00')
    fireEvent.change(priceInput, { target: { value: '50.00' } })
    fireEvent.click(screen.getByRole('button', { name: /save|salvar/i }))

    await waitFor(() => expect(put).toHaveBeenCalledWith('/business-plan/uuid/plan-uuid-1', expect.objectContaining({ priceInCents: 5000 })))
  })
})

const sysAdminSession: Session = { accessToken: 't', tokenType: 'Bearer', expireIn: 3600, email: 'root@hermes.dev', uuid: 'sys-uuid', name: 'Root', userId: 1, role: 'SysAdmin', tenantId: null }
const tenantOwnerSession: Session = { accessToken: 't', tokenType: 'Bearer', expireIn: 3600, email: 'owner@transmega.com', uuid: 'owner-uuid', name: 'Owner', userId: 2, role: 'TenantOwner', tenantId: 42 }
const oneTenant: Tenant = { id: 42, businessName: 'Transmega', companyName: 'Transmega', taxId: '52998224725', countryCode: 'BR' }

// A minimal store shaped like RootState -- it doesn't need to react to the
// thunks UserEditorPage dispatches (loadTenants), only to hold the session
// and tenant list the page reads via useSelector.
function sessionStore(session: Session, tenants: Tenant[] = []) {
  const tenantPage = { ...emptyPage<Tenant>(), items: tenants, totalItems: tenants.length, totalPages: tenants.length ? 1 : 0 }
  return configureStore({
    reducer: {
      auth: (state = { session, loading: false, error: '' }) => state,
      data: (state = { tenants: tenantPage, plans: emptyPage(), users: emptyPage(), provinces: [], cities: [], loading: false, error: '' }) => state,
    },
  })
}

// EPIC-BO-02-S02 / PD-019 (HRMS-405, HRMS-113..116): a platform administrator
// creates tenants and tenant owners -- never a tenant user directly -- and a
// tenant owner creates tenant users in their own tenant only. The console
// must offer exactly those actions, not the superset the API would refuse.
describe('UserEditorPage role-conditional creation (EPIC-BO-02)', () => {
  beforeEach(() => { vi.restoreAllMocks(); vi.spyOn(api, 'get').mockResolvedValue({ data: [] } as never) })

  it('offers a SysAdmin only TenantOwner and SysAdmin as roles to create, never TenantUser', () => {
    render(
      <Provider store={sessionStore(sysAdminSession, [oneTenant])}>
        <MemoryRouter initialEntries={['/users/new']}>
          <Routes><Route path="/users/new" element={<UserEditorPage />} /></Routes>
        </MemoryRouter>
      </Provider>
    )
    fireEvent.click(screen.getByRole('button', { name: /^role$/i }))
    const options = screen.getAllByRole('option').map((option) => option.textContent)
    // EPIC-IA-09-S06 (HRMS-134): role names are shown translated, not as enum values.
    expect(options).toEqual(expect.arrayContaining(['Tenant owner', 'Platform administrator']))
    expect(options).not.toContain('Tenant user')
    expect(options).not.toContain('Driver')
  })

  it('gives a tenant owner a role picker limited to tenant user, driver and mechanic, and no tenant picker -- the new user lands in their own tenant', async () => {
    const post = vi.spyOn(api, 'post').mockResolvedValue({ data: {} } as never)
    render(
      <Provider store={sessionStore(tenantOwnerSession, [oneTenant])}>
        <MemoryRouter initialEntries={['/users/new']}>
          <Routes><Route path="/users/new" element={<UserEditorPage />} /></Routes>
        </MemoryRouter>
      </Provider>
    )
    // EPIC-IA-09-S03 (HRMS-131, D-22): a tenant owner staffs their own tenant.
    fireEvent.click(screen.getByRole('button', { name: /^role$/i }))
    const roles = screen.getAllByRole('option').map((option) => option.textContent)
    expect(roles).toEqual(['Tenant user', 'Driver', 'Mechanic'])
    expect(screen.queryByRole('button', { name: /^tenant$/i })).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('option', { name: 'Driver' }))
    // EPIC-IA-07/D-07: no password field on creation -- the account is
    // invite-only, never given a caller-set password.
    expect(screen.queryByLabelText(/temporary password|senha tempor|new password|nova senha/i)).not.toBeInTheDocument()

    fireEvent.change(screen.getByLabelText(/name|nome/i), { target: { value: 'New Driver Coordinator' } })
    fireEvent.change(screen.getByLabelText(/e-?mail/i), { target: { value: 'coordinator@transmega.com' } })
    fireEvent.click(screen.getByRole('button', { name: /save|salvar/i }))

    await waitFor(() => expect(post).toHaveBeenCalledWith('/user', expect.objectContaining({ role: 'Driver', tenantId: 42 })))
    expect(post).not.toHaveBeenCalledWith('/user', expect.objectContaining({ password: expect.anything() }))
  })
})

// EPIC-BO-07-S02/S03 (HRMS-416, HRMS-417, D-22): the console's role guards answer for five roles.
describe('operational roles in the console', () => {
  it('keeps drivers and mechanics out of tenant and user administration', () => {
    for (const role of ['SysAdmin', 'TenantOwner', 'TenantUser']) expect(canReachAdministration(role)).toBe(true)
    for (const role of ['Driver', 'Mechanic']) {
      expect(canReachAdministration(role)).toBe(false)
      expect(canCreateUsers(role)).toBe(false)
    }
  })
})
