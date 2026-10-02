import { act, fireEvent, render, renderHook, screen, waitFor, within } from '@testing-library/react'
import { configureStore } from '@reduxjs/toolkit'
import { Provider } from 'react-redux'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { beforeEach, afterEach, describe, expect, it, vi } from 'vitest'
import '../i18n'
import { api } from '../api'
import App from '../App'
import { SubscriptionPage } from './ManagementPages'
import { usePagedList } from '../usePagedList'
import { emptyPage } from '../types'
import type { Session, Tenant, User } from '../types'

const session = (role: Session['role'], tenantId: number | null): Session =>
  ({ accessToken: 't', tokenType: 'Bearer', expireIn: 3600, email: `${role}@transmega.com`, uuid: `${role}-uuid`, name: role, userId: 9, role, tenantId, tenantUuid: tenantId ? 'tenant-42-uuid' : null })

const sysAdmin = session('SysAdmin', null)
const owner = session('TenantOwner', 42)
const member = session('TenantUser', 42)

const tenant: Tenant = { id: 42, uuid: 'tenant-42-uuid', businessName: 'Transmega Logística', companyName: 'Transmega', taxId: '11222333000181', email: 'ops@transmega.com', phone: '', website: '', addressLine1: '', addressLine2: '', locality: 'Santos', administrativeArea: 'SP', postalCode: '', countryCode: 'BR' }
const member1: User = { id: 9, name: 'Coordinator', email: 'coord@transmega.com', enabled: true, role: 'TenantUser', tenantId: 42 }

const dataState = {
  tenants: { ...emptyPage<Tenant>(), items: [tenant], totalItems: 1, totalPages: 1 },
  plans: emptyPage(),
  users: { ...emptyPage<User>(), items: [member1], totalItems: 1, totalPages: 1 },
  provinces: [], cities: [], countries: ['BR'],
  counts: { tenants: 29, users: 7, plans: 3 },
  currentTenant: tenant,
  loading: false, error: '',
}

function renderAt(path: string, who: Session) {
  const store = configureStore({
    reducer: {
      auth: (state = { session: who, loading: false, error: '' }) => state,
      data: (state = dataState) => state,
    },
  })
  return render(
    <Provider store={store}>
      <MemoryRouter initialEntries={[path]}><App /></MemoryRouter>
    </Provider>
  )
}

// DEF-BO-09 (HRMS-414): the console shows every operator the same surfaces and
// then lets the server refuse them. Each case below pins one surface to one
// role, because the reported defect was not a single broken screen -- it was
// that no test held the role/surface pairing anywhere.
describe('role-appropriate surfaces (DEF-BO-09, HRMS-414)', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    vi.spyOn(api, 'get').mockResolvedValue({ data: emptyPage() } as never)
  })

  it('offers plan and reference-data administration in the menu only to a SysAdmin', () => {
    const { container } = renderAt('/', sysAdmin)
    const menu = within(container.querySelector('.sidebar nav') as HTMLElement)
    expect(menu.getByRole('link', { name: /plans|planos/i })).toBeInTheDocument()
    expect(menu.getByRole('link', { name: /system settings|configurações do sistema/i })).toBeInTheDocument()
  })

  it('hides plan and reference-data administration from a tenant owner', () => {
    const { container } = renderAt('/', owner)
    const menu = within(container.querySelector('.sidebar nav') as HTMLElement)
    expect(menu.queryByRole('link', { name: /plans|planos/i })).not.toBeInTheDocument()
    expect(menu.queryByRole('link', { name: /system settings|configurações do sistema/i })).not.toBeInTheDocument()
  })

  it('shows the plan count card only to a SysAdmin', () => {
    renderAt('/', sysAdmin)
    expect(screen.getByText(/available plans|planos disponíveis/i)).toBeInTheDocument()
  })

  it('hides the plan count card from a tenant user', () => {
    renderAt('/', member)
    expect(screen.queryByText(/available plans|planos disponíveis/i)).not.toBeInTheDocument()
  })

  it('offers tenant creation and subscription management on the tenants page only to a SysAdmin', () => {
    renderAt('/tenants', sysAdmin)
    expect(screen.getByRole('link', { name: /new tenant|novo tenant|nova empresa/i })).toBeInTheDocument()
    expect(screen.getByRole('link', { name: /subscription|assinatura/i })).toBeInTheDocument()
  })

  it('hides tenant creation and subscription management from a tenant owner', () => {
    renderAt('/tenants', owner)
    expect(screen.queryByRole('link', { name: /new tenant|novo tenant|nova empresa/i })).not.toBeInTheDocument()
    expect(screen.queryByRole('link', { name: /subscription|assinatura/i })).not.toBeInTheDocument()
  })

  it('bounces a tenant user away from reference-data administration', () => {
    renderAt('/system-settings', member)
    expect(screen.getByText(/control center|centro de controle/i)).toBeInTheDocument()
  })

  // DEF-BO-04: a TenantUser cannot create accounts, so the console must not
  // offer it -- neither as a button nor as a reachable URL.
  it('offers user creation to a tenant owner but not to a tenant user', () => {
    const asOwner = renderAt('/users', owner)
    expect(screen.getByRole('link', { name: /new user|novo usuário/i })).toBeInTheDocument()
    asOwner.unmount()
    renderAt('/users', member)
    expect(screen.queryByRole('link', { name: /new user|novo usuário/i })).not.toBeInTheDocument()
  })

  it('bounces a tenant user away from the user creation form', () => {
    renderAt('/users/new', member)
    expect(screen.getByText(/control center|centro de controle/i)).toBeInTheDocument()
  })
})

// DEF-BO-03: `/business-plan` answers with PD-028's page envelope. Reading it
// as a bare array threw inside render and took the whole console down.
describe('subscription screen reads the page envelope (DEF-BO-03)', () => {
  beforeEach(() => vi.restoreAllMocks())

  it('lists the available plans instead of crashing', async () => {
    vi.spyOn(api, 'get').mockImplementation((async (url: string) => {
      if (url === '/tenant/uuid/tenant-42-uuid') return { data: tenant }
      if (url === '/business-plan') return { data: { items: [{ id: 3, name: 'Fleet Pro', priceInCents: 149900, availableUsers: 25, periodDays: 30, paymentDate: '2025-01-10' }], totalItems: 1, totalPages: 1, page: 0, pageSize: 200 } }
      return { data: null }
    }) as never)

    const store = configureStore({ reducer: { auth: (state = { session: sysAdmin, loading: false, error: '' }) => state, data: (state = dataState) => state } })
    render(
      <Provider store={store}>
        <MemoryRouter initialEntries={['/tenants/tenant-42-uuid/subscription']}>
          <Routes><Route path="/tenants/:uuid/subscription" element={<SubscriptionPage />} /></Routes>
        </MemoryRouter>
      </Provider>
    )

    expect(await screen.findByPlaceholderText(/Fleet Pro/)).toBeInTheDocument()
  })
})

// DEF-BO-08: page and search have to move as one. Held apart, typing fired a
// request for the *old* page and another for page 0, and the slower reply won.
describe('paged list search (DEF-BO-08)', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    vi.useFakeTimers()
  })
  afterEach(() => vi.useRealTimers())

  it('issues exactly one request for a search, and it asks for the first page', async () => {
    const requests: { page: number; search?: string }[] = []
    const load = (request: { page: number; pageSize: number; search?: string }) => { requests.push({ page: request.page, search: request.search }); return { type: 'noop' } }
    const store = configureStore({ reducer: { auth: (state = { session: sysAdmin, loading: false, error: '' }) => state, data: (state = dataState) => state } })
    const wrapper = ({ children }: { children: React.ReactNode }) => <Provider store={store}>{children}</Provider>

    const { result } = renderHook(() => usePagedList(load), { wrapper })
    expect(requests).toEqual([{ page: 0, search: '' }])

    // Move to page three, then search -- the old code replayed page three
    // against the filtered set before resetting, which is the race.
    await act(async () => { result.current.onPageChange({ page: 2, pageSize: 25 }) })
    expect(requests.at(-1)).toEqual({ page: 2, search: '' })

    const before = requests.length
    await act(async () => { result.current.setSearch('transmega') })
    await act(async () => { await vi.advanceTimersByTimeAsync(400) })

    expect(requests.length - before).toBe(1)
    expect(requests.at(-1)).toEqual({ page: 0, search: 'transmega' })
  })
})

// DEF-BO-06: Spanish was loaded but unreachable, and the button was labelled
// with the language you were *not* using.
describe('language picker (DEF-BO-06)', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    vi.spyOn(api, 'get').mockResolvedValue({ data: emptyPage() } as never)
  })

  it('offers all three languages and shows the one in use', async () => {
    renderAt('/', sysAdmin)
    const picker = screen.getByLabelText(/language|idioma/i) as HTMLSelectElement
    expect(Array.from(picker.options).map((option) => option.value)).toEqual(['pt-BR', 'en', 'es'])

    fireEvent.change(picker, { target: { value: 'es' } })
    await waitFor(() => expect((screen.getByLabelText(/idioma|language/i) as HTMLSelectElement).value).toBe('es'))
  })
})
