import { render, screen } from '@testing-library/react'
import { Provider } from 'react-redux'
import { MemoryRouter } from 'react-router-dom'
import { describe, expect, it, vi } from 'vitest'
import { api } from '../api'
import { store } from '../store'
import '../i18n'
import { ProfilesPage, groupByModule, profileRoles } from './ProfilePages'

// C-032 (D-27): the profile form shows one group per module, in catalogue order; only the roles below
// the owner take a profile.
describe('profile form helpers', () => {
  it('groups permissions by module and keeps catalogue order inside a group', () => {
    const g = groupByModule([
      { code: 'work-order.read', module: 'Maintenance', own: false, defaultFor: [] },
      { code: 'part.read', module: 'Stock & purchasing', own: false, defaultFor: [] },
      { code: 'work-order.create', module: 'Maintenance', own: false, defaultFor: ['Mechanic'] },
    ])
    expect(Object.keys(g)).toEqual(['Maintenance', 'Stock & purchasing'])
    expect(g.Maintenance.map((p) => p.code)).toEqual(['work-order.read', 'work-order.create'])
  })

  it('offers profiles for tenant users, drivers and mechanics only', () => {
    expect(profileRoles).toEqual(['TenantUser', 'Driver', 'Mechanic'])
  })
})

describe('ProfilesPage', () => {
  it('loads profiles through the Redux feature thunk', async () => {
    vi.spyOn(api, 'get').mockResolvedValue({ data: { items: [{ uuid: 'profile-1', name: 'Fleet operator', baseRole: 'Mechanic', active: true, permissions: ['vehicle.read'] }] } } as never)

    render(<Provider store={store}><MemoryRouter><ProfilesPage /></MemoryRouter></Provider>)

    expect(await screen.findByText('Fleet operator')).toBeInTheDocument()
    expect(api.get).toHaveBeenCalledWith('/access-profile', { params: { page: 0, pageSize: 200 } })
  })
})
