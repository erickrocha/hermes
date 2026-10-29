import { fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { configureStore } from '@reduxjs/toolkit'
import { Provider } from 'react-redux'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import '../i18n'
import { api } from '../api'
import { store } from '../store'
import { emptyPage } from '../types'
import type { AccessProfile, Permission, Session, Tenant } from '../types'
import { sessionAs, sessionWith } from '../test/session'
import { PlanEditorPage, UserEditorPage, amountToCents, canCreateUsers, canSeeUsers, centsToAmount } from './ManagementPages'

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
    vi.spyOn(api, 'get').mockResolvedValue({ data: { id: 1, uuid: 'plan-uuid-1', name: 'Pro', priceInCents: 12345, pricePerExtraUserInCents: 500, availableUsers: 5, periodDays: 30 } } as never)

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
    vi.spyOn(api, 'get').mockResolvedValue({ data: { id: 1, uuid: 'plan-uuid-1', name: 'Pro', priceInCents: 0, pricePerExtraUserInCents: 0, availableUsers: 5, periodDays: 30 } } as never)
    const put = vi.spyOn(api, 'put').mockResolvedValue({ data: {} } as never)

    render(
      <Provider store={store}>
        <MemoryRouter initialEntries={['/plans/plan-uuid-1/edit']}>
          <Routes><Route path="/plans/:uuid/edit" element={<PlanEditorPage />} /></Routes>
        </MemoryRouter>
      </Provider>
    )

    const priceInput = await screen.findByLabelText(/base price/i)
    const overageInput = screen.getByLabelText(/price per extra user/i)
    fireEvent.change(priceInput, { target: { value: '50.00' } })
    fireEvent.change(overageInput, { target: { value: '5.00' } })
    fireEvent.click(screen.getByRole('button', { name: /save|salvar/i }))

    await waitFor(() => expect(put).toHaveBeenCalledWith('/business-plan/uuid/plan-uuid-1', expect.objectContaining({ priceInCents: 5000, pricePerExtraUserInCents: 500 })))
  })
})

const sysAdminSession: Session = sessionAs('SystemAdministrator', { email: 'root@hermes.dev', uuid: 'sys-uuid', name: 'Root', userId: 1 })
const tenantOwnerSession: Session = sessionAs('TenantOwner', { email: 'owner@transmega.com', uuid: 'owner-uuid', name: 'Owner', userId: 2, tenantId: 42 })
const oneTenant: Tenant = { id: 42, businessName: 'Transmega', companyName: 'Transmega', taxId: '52998224725', countryCode: 'BR' }
const perm = (code: string, module: string, extra: Partial<Permission> = {}): Permission => ({ code, module, scope: 'Tenant', own: false, defaultFor: [], ...extra })
const catalogue: Permission[] = [perm('vehicle.read', 'Fleet', { own: true }), perm('vehicle.create', 'Fleet'), perm('business-plan.read', 'Platform', { scope: 'Platform' })]
const driverProfile: AccessProfile = { uuid: 'sys-driver', name: 'Driver', system: true, key: 'Driver', active: true, permissions: ['vehicle.read@own'] }
const ownerProfile: AccessProfile = { uuid: 'sys-owner', name: 'Tenant owner', system: true, key: 'TenantOwner', active: true, permissions: ['vehicle.read', 'vehicle.create'] }
const adminProfile: AccessProfile = { uuid: 'sys-admin', name: 'Platform administrator', system: true, key: 'SystemAdministrator', active: true, permissions: ['vehicle.read', 'vehicle.create', 'business-plan.read'] }
const mockAccessOptions = () => vi.spyOn(api, 'get').mockImplementation((async (url: string) => {
  if (url === '/access-profile/catalogue') return { data: catalogue }
  if (url === '/access-profile') return { data: { items: [adminProfile, ownerProfile, driverProfile] } }
  return { data: { items: [oneTenant], page: 0, pageSize: 200, totalItems: 1, totalPages: 1 } }
}) as never)

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
const renderNewUser = (session: Session) => render(
  <Provider store={sessionStore(session, [oneTenant])}>
    <MemoryRouter initialEntries={['/users/new']}>
      <Routes><Route path="/users/new" element={<UserEditorPage />} /></Routes>
    </MemoryRouter>
  </Provider>
)
const pickProfile = async (name: RegExp) => {
  const picker = await screen.findByRole('combobox', { name: /^profile$|^perfil$/i })
  fireEvent.focus(within(picker).getByRole('textbox'))
  fireEvent.click(await screen.findByRole('option', { name }))
}

// C-035 (replaces EPIC-BO-02-S02 / PD-019): a user is created with an access profile. The platform hands out Platform
// profiles (no tenant) and Tenant profiles (naming the tenant); a tenant caller hands out Tenant profiles inside their
// own tenant. Extras are laid on top of the profile, never repeating what it already grants.
describe('UserEditorPage profile-based creation (C-035)', () => {
  beforeEach(() => { vi.restoreAllMocks(); mockAccessOptions() })

  it('lets the platform pick any active profile; the system administrator profile is placed by the server, a Tenant profile asks for a tenant', async () => {
    const post = vi.spyOn(api, 'post').mockResolvedValue({ data: {} } as never)
    renderNewUser(sysAdminSession)
    await pickProfile(/System administrator|Administrador do Sistema/)
    expect(screen.getByText(/placed in the platform's own tenant|fica no tenant da própria plataforma/i)).toBeInTheDocument()
    expect(within(screen.getByRole('combobox', { name: /^tenant$/i })).getByRole('textbox')).toBeDisabled()
    await pickProfile(/Tenant owner/)
    expect(within(screen.getByRole('combobox', { name: /^tenant$/i })).getByRole('textbox')).not.toBeDisabled()
    fireEvent.change(screen.getByLabelText(/^name|^nome/i), { target: { value: 'New Owner' } })
    fireEvent.change(screen.getByLabelText(/e-?mail/i), { target: { value: 'owner2@transmega.com' } })
    fireEvent.click(screen.getByRole('button', { name: /save|salvar/i }))
    await waitFor(() => expect(post).toHaveBeenCalledWith('/user', expect.objectContaining({ profileUuid: 'sys-owner', tenantId: 42, extraPermissions: [] })))
  })

  it('gives a tenant owner Tenant profiles only and no tenant picker -- the new user lands in their own tenant, with extras on top of the profile', async () => {
    const post = vi.spyOn(api, 'post').mockResolvedValue({ data: {} } as never)
    renderNewUser(tenantOwnerSession)
    const picker = await screen.findByRole('combobox', { name: /^profile$|^perfil$/i })
    fireEvent.focus(within(picker).getByRole('textbox'))
    const options = (await screen.findAllByRole('option')).map((option) => option.textContent)
    expect(options.some((label) => /System administrator|Administrador do Sistema/.test(label ?? ''))).toBe(false)
    expect(options.some((label) => /Driver|Motorista/.test(label ?? ''))).toBe(true)
    expect(screen.queryByRole('combobox', { name: /^tenant$/i })).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('option', { name: /Driver|Motorista/ }))
    // EPIC-IA-07/D-07: no password field on creation -- the account is invite-only.
    expect(screen.queryByLabelText(/temporary password|senha tempor|new password|nova senha/i)).not.toBeInTheDocument()
    // the Driver profile already holds vehicle.read@own; vehicle.read is still offered (widening), vehicle.create too
    const extras = within(screen.getByTestId('extraPermissions'))
    fireEvent.click(extras.getByRole('checkbox', { name: /^vehicle\.create$/ }))
    fireEvent.change(screen.getByLabelText(/^name|^nome/i), { target: { value: 'New Driver Coordinator' } })
    fireEvent.change(screen.getByLabelText(/e-?mail/i), { target: { value: 'coordinator@transmega.com' } })
    fireEvent.click(screen.getByRole('button', { name: /save|salvar/i }))
    await waitFor(() => expect(post).toHaveBeenCalledWith('/user', expect.objectContaining({ profileUuid: 'sys-driver', tenantId: 42, extraPermissions: ['vehicle.create'] })))
    expect(post).not.toHaveBeenCalledWith('/user', expect.objectContaining({ password: expect.anything() }))
    expect(post).not.toHaveBeenCalledWith('/user', expect.objectContaining({ principalFunction: expect.anything() }))
  })

  it('refuses to submit without a profile', async () => {
    const post = vi.spyOn(api, 'post').mockResolvedValue({ data: {} } as never)
    renderNewUser(tenantOwnerSession)
    await screen.findByRole('combobox', { name: /^profile$|^perfil$/i })
    fireEvent.change(screen.getByLabelText(/^name|^nome/i), { target: { value: 'Nobody' } })
    fireEvent.change(screen.getByLabelText(/e-?mail/i), { target: { value: 'nobody@transmega.com' } })
    fireEvent.click(screen.getByRole('button', { name: /save|salvar/i }))
    expect(await screen.findByText(/pick an access profile|escolha um perfil de acesso/i)).toBeInTheDocument()
    expect(post).not.toHaveBeenCalled()
  })
})

// C-035: the console's administration guards answer for permissions, not roles.
describe('administration guards', () => {
  it('opens user administration to whoever may create, update, invite or assign, and to nobody else', () => {
    for (const code of ['user.create', 'user.update', 'user.invite', 'profile.assign']) expect(canSeeUsers(sessionWith([code]))).toBe(true)
    expect(canSeeUsers(sessionWith(['vehicle.read']))).toBe(false)
    expect(canCreateUsers(sessionWith(['user.update']))).toBe(false)
    expect(canCreateUsers(sessionWith(['user.create']))).toBe(true)
    for (const key of ['Driver', 'Mechanic'] as const) { expect(canSeeUsers(sessionAs(key))).toBe(false); expect(canCreateUsers(sessionAs(key))).toBe(false) }
    for (const key of ['SystemAdministrator', 'TenantOwner'] as const) expect(canCreateUsers(sessionAs(key))).toBe(true)
  })
})
