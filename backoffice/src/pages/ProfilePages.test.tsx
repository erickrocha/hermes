import { fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { Provider } from 'react-redux'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { api } from '../api'
import { store } from '../store'
import '../i18n'
import { signIn, signInWith, sessionWith } from '../test/render'
import type { AccessProfile, Permission } from '../types'
import { ProfileEditorPage, ProfilesPage, UserProfilePicker, extrasCatalogueFor, grantableFor, profilesFor, sortProfiles, systemGrantsFrom } from './ProfilePages'
import { groupByModule } from '../components/molecules/PermissionPicker'

const perm = (code: string, module: string, extra: Partial<Permission> = {}): Permission => ({ code, module, scope: 'Tenant', own: false, defaultFor: [], ...extra })
const catalogue: Permission[] = [
  perm('vehicle.read', 'Fleet', { own: true, defaultFor: ['TenantUser', 'Driver', 'Mechanic'] }),
  perm('vehicle.create', 'Fleet'),
  perm('work-order.read', 'Maintenance', { defaultFor: ['TenantUser', 'Mechanic'] }),
  perm('business-plan.read', 'Platform', { scope: 'Platform' }),
]
const system: AccessProfile = { uuid: 'sys-driver', name: 'Driver', scope: 'Tenant', system: true, key: 'Driver', active: true, permissions: ['vehicle.read@own'] }
const custom: AccessProfile = { uuid: 'profile-1', tenantId: 42, name: 'Fleet operator', scope: 'Tenant', system: false, active: true, permissions: ['vehicle.read'] }
const platformProfile: AccessProfile = { uuid: 'sys-admin', name: 'Platform administrator', scope: 'Platform', system: true, key: 'SysAdmin', active: true, permissions: ['business-plan.read'] }
const inactive: AccessProfile = { ...custom, uuid: 'profile-2', name: 'Retired', active: false }

const at = (path: string, route: string, element: React.ReactElement) =>
  render(<Provider store={store}><MemoryRouter initialEntries={[path]}><Routes><Route path={route} element={element} /><Route path="*" element={<p>elsewhere</p>} /></Routes></MemoryRouter></Provider>)

// C-035: the profile form shows one group per module, in catalogue order; a profile's scope decides what it may hold.
describe('profile form helpers', () => {
  it('groups permissions by module and keeps catalogue order inside a group', () => {
    const g = groupByModule([perm('work-order.read', 'Maintenance'), perm('part.read', 'Stock & purchasing'), perm('work-order.create', 'Maintenance')])
    expect(Object.keys(g)).toEqual(['Maintenance', 'Stock & purchasing'])
    expect(g.Maintenance.map((p) => p.code)).toEqual(['work-order.read', 'work-order.create'])
  })

  it('lets a Tenant profile hold Tenant codes only and a Platform profile everything', () => {
    expect(grantableFor(catalogue, 'Tenant').map((p) => p.code)).not.toContain('business-plan.read')
    expect(grantableFor(catalogue, 'Platform').map((p) => p.code)).toContain('business-plan.read')
  })

  it('derives a system profile the way the server does: the Driver gets @own where the catalogue allows it', () => {
    expect(systemGrantsFrom(catalogue, 'Driver')).toEqual(['vehicle.read@own'])
    expect(systemGrantsFrom(catalogue, 'Mechanic')).toEqual(['vehicle.read', 'work-order.read'])
  })

  it('offers a tenant caller active Tenant profiles only, and the platform every active one', () => {
    const all = [system, custom, platformProfile, inactive]
    expect(profilesFor(sessionWith([]), all).map((p) => p.uuid)).toEqual(['sys-driver', 'profile-1'])
    expect(profilesFor(sessionWith([], 'Platform'), all).map((p) => p.uuid)).toEqual(['sys-driver', 'profile-1', 'sys-admin'])
  })

  it('offers as extras what the profile does not already grant in full, within the profile scope', () => {
    expect(extrasCatalogueFor(catalogue, custom).map((p) => p.code)).toEqual(['vehicle.create', 'work-order.read'])
    // an @own grant can still be widened to the full permission
    expect(extrasCatalogueFor(catalogue, system).map((p) => p.code)).toContain('vehicle.read')
    expect(extrasCatalogueFor(catalogue, undefined)).toEqual([])
  })

  it('lists system profiles first', () => {
    expect(sortProfiles([custom, system]).map((p) => p.uuid)).toEqual(['sys-driver', 'profile-1'])
  })
})

describe('ProfilesPage', () => {
  beforeEach(() => { vi.restoreAllMocks(); signIn('TenantOwner') })

  it('loads profiles through the Redux feature thunk and marks the system ones read-only', async () => {
    vi.spyOn(api, 'get').mockResolvedValue({ data: { items: [custom, system] } } as never)
    const { container } = at('/profiles', '/profiles', <ProfilesPage />)
    expect(await screen.findByText('Fleet operator')).toBeInTheDocument()
    expect(api.get).toHaveBeenCalledWith('/access-profile', { params: { page: 0, pageSize: 200 } })
    const rows = container.querySelectorAll('tbody tr')
    expect(within(rows[0] as HTMLElement).getByText(/system profile|perfil do sistema/i)).toBeInTheDocument()
    expect(within(rows[0] as HTMLElement).getByRole('link', { name: /view|ver/i })).toBeInTheDocument()
    expect(within(rows[1] as HTMLElement).getByRole('link', { name: /edit|editar/i })).toBeInTheDocument()
  })

  it('hides the new-profile button from whoever cannot create profiles', async () => {
    signInWith(sessionWith(['profile.read']))
    vi.spyOn(api, 'get').mockResolvedValue({ data: { items: [custom] } } as never)
    at('/profiles', '/profiles', <ProfilesPage />)
    await screen.findByText('Fleet operator')
    expect(screen.queryByRole('link', { name: /new profile|novo perfil/i })).not.toBeInTheDocument()
  })
})

describe('ProfileEditorPage', () => {
  beforeEach(() => { vi.restoreAllMocks() })
  const mockEditor = (profile: AccessProfile | null) => vi.spyOn(api, 'get').mockImplementation((async (url: string) => {
    if (url === '/access-profile/catalogue') return { data: catalogue }
    return { data: profile }
  }) as never)

  it('shows a system profile read-only: no save button, scope fixed, picker disabled', async () => {
    signIn('TenantOwner'); mockEditor(system)
    at('/profiles/sys-driver/edit', '/profiles/:uuid/edit', <ProfileEditorPage />)
    expect(await screen.findByText(/built-in profile|perfil embutido/i)).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /^save$|^salvar$/i })).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: /^back$|^voltar$/i })).toBeInTheDocument()
    expect(screen.getByRole('checkbox', { name: /^vehicle\.read$/ })).toBeDisabled()
  })

  it('keeps a tenant caller on Tenant scope and sends grants with @own when "own records only" is toggled', async () => {
    signIn('TenantOwner'); mockEditor(null)
    const post = vi.spyOn(api, 'post').mockResolvedValue({ data: {} } as never)
    at('/profiles/new', '/profiles/new', <ProfileEditorPage />)
    await screen.findByRole('checkbox', { name: /^vehicle\.read$/ })
    expect(screen.queryByRole('combobox', { name: /scope|alcance/i })).not.toBeInTheDocument()
    expect(screen.queryByText('business-plan.read')).not.toBeInTheDocument()
    fireEvent.change(screen.getByLabelText(/^name|^nome/i), { target: { value: 'Yard driver' } })
    fireEvent.click(screen.getByRole('checkbox', { name: /^vehicle\.read$/ }))
    fireEvent.click(screen.getByRole('checkbox', { name: /vehicle\.read .*own/i }))
    fireEvent.click(screen.getByRole('button', { name: /^save$|^salvar$/i }))
    await waitFor(() => expect(post).toHaveBeenCalledWith('/access-profile', expect.objectContaining({ name: 'Yard driver', scope: 'Tenant', permissions: ['vehicle.read@own'] })))
  })

  it('lets the platform choose the scope and offers Platform codes for a Platform profile', async () => {
    signIn('SysAdmin'); mockEditor(null)
    at('/profiles/new', '/profiles/new', <ProfileEditorPage />)
    const scope = await screen.findByRole('combobox', { name: /scope|alcance/i })
    fireEvent.change(scope, { target: { value: 'Platform' } })
    expect(await screen.findByText('business-plan.read')).toBeInTheDocument()
  })

  it('copies a system profile as a starting point', async () => {
    signIn('TenantOwner'); mockEditor(null)
    at('/profiles/new', '/profiles/new', <ProfileEditorPage />)
    const start = await screen.findByRole('combobox', { name: /start from|começar por/i })
    fireEvent.change(start, { target: { value: 'Mechanic' } })
    expect((screen.getByRole('checkbox', { name: /^work-order\.read$/ }) as HTMLInputElement).checked).toBe(true)
    expect((screen.getByRole('checkbox', { name: /^vehicle\.create$/ }) as HTMLInputElement).checked).toBe(false)
  })
})

// C-035: the assign flow -- `PUT /access-profile/user/uuid/{uuid}` with `AssignProfileJson { profileUuid, extraPermissions }`.
describe('UserProfilePicker', () => {
  beforeEach(() => {
    vi.restoreAllMocks(); signIn('TenantOwner')
    vi.spyOn(api, 'get').mockImplementation((async (url: string) => {
      if (url === '/access-profile/catalogue') return { data: catalogue }
      if (url === '/access-profile') return { data: { items: [system, custom] } }
      return { data: { scope: 'Tenant', profileUuid: 'profile-1', profileName: 'Fleet operator', extraPermissions: ['work-order.read'], permissions: ['vehicle.read', 'work-order.read'] } }
    }) as never)
  })

  it('shows the held profile and extras, and applies a change through the assign endpoint', async () => {
    const put = vi.spyOn(api, 'put').mockResolvedValue({ data: {} } as never)
    at('/users/u-1/edit', '/users/:uuid/edit', <UserProfilePicker userUuid="u-1" />)
    expect(await screen.findByPlaceholderText(/Fleet operator/)).toBeInTheDocument()
    const extras = within(screen.getByTestId('extraPermissions'))
    expect((extras.getByRole('checkbox', { name: /^work-order\.read$/ }) as HTMLInputElement).checked).toBe(true)
    // what the profile already grants is not offered again
    expect(extras.queryByRole('checkbox', { name: /^vehicle\.read$/ })).not.toBeInTheDocument()
    fireEvent.click(extras.getByRole('checkbox', { name: /^vehicle\.create$/ }))
    fireEvent.click(screen.getByRole('button', { name: /apply access|aplicar acesso/i }))
    await waitFor(() => expect(put).toHaveBeenCalledWith('/access-profile/user/uuid/u-1', { profileUuid: 'profile-1', extraPermissions: ['work-order.read', 'vehicle.create'] }))
    expect(await screen.findByText(/profile saved|perfil salvo/i)).toBeInTheDocument()
  })
})
