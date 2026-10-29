import { act, cleanup, fireEvent, render, renderHook, screen, waitFor, within } from '@testing-library/react'
import { configureStore } from '@reduxjs/toolkit'
import { Provider } from 'react-redux'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { beforeEach, afterEach, describe, expect, it, vi } from 'vitest'
import '../i18n'
import { api } from '../api'
import { App } from '../App'
import { SubscriptionPage } from './ManagementPages'
import { usePagedList } from '../usePagedList'
import { emptyPage } from '../types'
import type { Session, Tenant, User } from '../types'
import { sessionAs, sessionWith, type SystemProfileKey } from '../test/session'

const session = (profile: SystemProfileKey, tenantId: number | null): Session =>
  sessionAs(profile, { userId: 9, tenantId, tenantUuid: tenantId ? 'tenant-42-uuid' : null })

const sysAdmin = session('SystemAdministrator', null)
const owner = session('TenantOwner', 42)
const member = session('TenantUser', 42)

const tenant: Tenant = { id: 42, uuid: 'tenant-42-uuid', businessName: 'Transmega Logística', companyName: 'Transmega', taxId: '11222333000181', email: 'ops@transmega.com', phone: '', website: '', addressLine1: '', addressLine2: '', locality: 'Santos', administrativeArea: 'SP', postalCode: '', countryCode: 'BR' }
const member1: User = { id: 9, name: 'Coordinator', email: 'coord@transmega.com', enabled: true, profileUuid: 'sys-user', profileKey: 'TenantUser', profileName: 'Tenant user', scope: 'Tenant', extraPermissions: [], tenantId: 42 }

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
// then lets the server refuse them. Each case below pins one surface to the
// access that opens it, because the reported defect was not a single broken
// screen -- it was that no test held the access/surface pairing anywhere.
// C-035: sessions are built from grants; the five system profiles stand in for the former roles.
describe('access-appropriate surfaces (DEF-BO-09, HRMS-414, C-035)', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    vi.spyOn(api, 'get').mockResolvedValue({ data: emptyPage() } as never)
  })

  it('offers plan and reference-data administration in the menu only to the platform', () => {
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

  // C-035: holding a Platform code inside a tenant is not enough -- the platform areas also need the platform scope.
  it('keeps platform areas closed to a tenant session even if it somehow holds the platform codes', () => {
    const { container } = renderAt('/', sessionWith(['business-plan.read', 'reference-data.province.manage-read', 'tenant.child.create']))
    const menu = within(container.querySelector('.sidebar nav') as HTMLElement)
    expect(menu.queryByRole('link', { name: /plans|planos/i })).not.toBeInTheDocument()
    expect(menu.queryByRole('link', { name: /system settings|configurações do sistema/i })).not.toBeInTheDocument()
  })

  // C-035: a platform actor has no tenant to operate in, so tenant operations stay out of their menu.
  it('keeps tenant operations (vehicles, work orders) out of the platform menu and offers them to a tenant member', () => {
    const platformMenu = within(renderAt('/', sysAdmin).container.querySelector('.sidebar nav') as HTMLElement)
    expect(platformMenu.queryByRole('link', { name: /vehicles|veículos/i })).not.toBeInTheDocument()
    expect(platformMenu.queryByRole('link', { name: /work orders|ordens de serviço/i })).not.toBeInTheDocument()
    cleanup()
    const memberMenu = within(renderAt('/', member).container.querySelector('.sidebar nav') as HTMLElement)
    expect(memberMenu.getByRole('link', { name: /vehicles|veículos/i })).toBeInTheDocument()
    expect(memberMenu.getByRole('link', { name: /work orders|ordens de serviço/i })).toBeInTheDocument()
  })

  // C-032 AC-06-S03 (HRMS-864) / C-035: the menu follows the grants, whatever profile carries them.
  it('offers Users and Profiles to whoever holds the administration permissions, and hides them from whoever does not', () => {
    const menuOf = (who: Session) => within(renderAt('/', who).container.querySelector('.sidebar nav') as HTMLElement)
    const withOwnerPermissions = menuOf(sessionWith(['user.create', 'profile.read', 'tenant.update']))
    expect(withOwnerPermissions.getByRole('link', { name: /users|usuários/i })).toBeInTheDocument()
    expect(withOwnerPermissions.getByRole('link', { name: /profiles|perfis/i })).toBeInTheDocument()
    cleanup()
    // profile.assign alone opens the user list too -- the server answers it
    const assigner = menuOf(sessionWith(['profile.assign']))
    expect(assigner.getByRole('link', { name: /users|usuários/i })).toBeInTheDocument()
    cleanup()
    const tenantUser = menuOf(sessionWith(['vehicle.read']))
    expect(tenantUser.queryByRole('link', { name: /users|usuários/i })).not.toBeInTheDocument()
    expect(tenantUser.queryByRole('link', { name: /profiles|perfis/i })).not.toBeInTheDocument()
  })

  it('sends a person to the landing page when the route needs a permission they lack', async () => {
    renderAt('/users', sessionWith(['vehicle.read']))
    await screen.findByText(/Everything looks good|Tudo parece bem/i)
    expect(screen.queryByRole('heading', { name: /^users$|^usuários$/i })).not.toBeInTheDocument()
  })

  // C-035: the principal function left with the roles; the CPF stays a tenant matter.
  it('lets the owner enter the CPF on the user form, and no longer asks for a principal function', async () => {
    renderAt('/users/new', sessionWith(['user.create']))
    expect(await screen.findByLabelText('CPF')).toBeInTheDocument()
    expect(screen.queryByLabelText(/principal function|função principal/i)).not.toBeInTheDocument()
  })

  it('shows the plan count card only to the platform', async () => {
    renderAt('/', sysAdmin)
    expect(await screen.findByText(/available plans|planos disponíveis/i)).toBeInTheDocument()
  })

  it('hides the plan count card from a tenant user', async () => {
    renderAt('/', member)
    await screen.findByText(/Everything looks good|Tudo parece bem/i)
    expect(screen.queryByText(/available plans|planos disponíveis/i)).not.toBeInTheDocument()
  })

  it('offers tenant creation and subscription management on the tenants page only to the platform', async () => {
    renderAt('/tenants', sysAdmin)
    expect(await screen.findByRole('link', { name: /new tenant|novo tenant|nova empresa/i })).toBeInTheDocument()
    expect(await screen.findByRole('link', { name: /subscription|assinatura/i })).toBeInTheDocument()
  })

  it('hides tenant creation and subscription management from a tenant owner', async () => {
    renderAt('/tenants', owner)
    await screen.findByRole('heading', { name: /tenants|empresas/i })
    expect(screen.queryByRole('link', { name: /new tenant|novo tenant|nova empresa/i })).not.toBeInTheDocument()
    expect(screen.queryByRole('link', { name: /subscription|assinatura/i })).not.toBeInTheDocument()
  })

  // C-032 (D-27 4, 8): the tenant's information and configuration are the owner's alone.
  it('keeps the tenant screens for the owner: a tenant user is bounced from them', async () => {
    renderAt('/tenants', member)
    expect(await screen.findByText(/control center|centro de controle/i)).toBeInTheDocument()
  })

  // C-032/C-035: profiles follow `profile.read`; the owner holds it, a tenant user does not.
  it('offers access profiles to the tenant owner and bounces a tenant user', async () => {
    const asOwner = renderAt('/profiles', owner)
    expect(await screen.findByRole('heading', { name: /^profiles$|^perfis$/i })).toBeInTheDocument()
    asOwner.unmount()
    renderAt('/profiles', member)
    expect(await screen.findByText(/control center|centro de controle/i)).toBeInTheDocument()
  })

  it('bounces a tenant user away from reference-data administration', async () => {
    renderAt('/system-settings', member)
    expect(await screen.findByText(/control center|centro de controle/i)).toBeInTheDocument()
  })

  // DEF-BO-04 / C-035: a tenant user holds none of the grants `GET /user` answers, so the console offers
  // neither the list nor the creation button -- and whoever may only update users gets the list without the button.
  it('offers user creation to a tenant owner, the list alone to an updater, and nothing to a tenant user', async () => {
    const asOwner = renderAt('/users', owner)
    expect(await screen.findByRole('link', { name: /new user|novo usuário/i })).toBeInTheDocument()
    asOwner.unmount()
    const asUpdater = renderAt('/users', sessionWith(['user.update']))
    await screen.findByRole('heading', { name: /^users$|^usuários$/i })
    expect(screen.queryByRole('link', { name: /new user|novo usuário/i })).not.toBeInTheDocument()
    asUpdater.unmount()
    renderAt('/users', member)
    expect(await screen.findByText(/control center|centro de controle/i)).toBeInTheDocument()
  })

  it('bounces a tenant user away from the user creation form', async () => {
    renderAt('/users/new', member)
    expect(await screen.findByText(/control center|centro de controle/i)).toBeInTheDocument()
  })

  // C-035: the topbar names the held profile -- a system one by key, a custom one by its own name.
  it('names the held profile in the topbar', async () => {
    renderAt('/', sessionWith(['vehicle.read'], 'Tenant', { profileName: 'Yard coordinator' }))
    expect(await screen.findByText('Yard coordinator')).toBeInTheDocument()
    cleanup()
    renderAt('/', owner)
    expect(await screen.findByText(/tenant owner|dono do tenant/i)).toBeInTheDocument()
  })
})

// DEF-BO-03: `/business-plan` answers with PD-028's page envelope. Reading it
// as a bare array threw inside render and took the whole console down.
describe('subscription screen reads the page envelope (DEF-BO-03)', () => {
  beforeEach(() => vi.restoreAllMocks())

  it('lists the available plans instead of crashing', async () => {
    vi.spyOn(api, 'get').mockImplementation((async (url: string) => {
      if (url === '/tenant/uuid/tenant-42-uuid') return { data: tenant }
      if (url === '/business-plan') return { data: { items: [{ id: 3, name: 'Fleet Pro', priceInCents: 149900, pricePerExtraUserInCents: 2500, availableUsers: 25, periodDays: 30 }], totalItems: 1, totalPages: 1, page: 0, pageSize: 200 } }
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
