import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { Provider } from 'react-redux'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import '../../i18n'
import { api, readTarget, TARGET_KEY } from '../../api'
import { logout, store } from '../../store'
import { signIn, signInWith, sessionWith } from '../../test/render'
import { TargetTenantSelect } from './TargetTenantSelect'

const page = (items: unknown[]) => ({ data: { items, page: 0, pageSize: 100, totalItems: items.length, totalPages: 1 } })
const child = { id: 7, uuid: 'child-uuid', businessName: 'Filial Norte', taxId: '1', countryCode: 'BR' }
const draw = () => render(<Provider store={store}><TargetTenantSelect /></Provider>)

// C-036 (`DS-1`): the tenant the console acts in is chosen here, and every later request names it.
describe('TargetTenantSelect', () => {
  beforeEach(() => { vi.restoreAllMocks(); store.dispatch(logout()); localStorage.clear() })

  it('is not offered to an account that may not act inside a descendant', () => {
    signIn('TenantOwner')
    const { container } = draw()
    expect(container).toBeEmptyDOMElement()
  })

  it('lists the descendants, and choosing one stores it, reads the permissions again and brings the console back on request', async () => {
    const get = vi.spyOn(api, 'get').mockImplementation((async (url: string) => {
      if (url === '/access-profile/mine') return { data: { scope: 'Tenant', permissions: ['vehicle.read'] } }
      return page([{ id: 2, uuid: 't-42', businessName: 'Matriz', taxId: '2', countryCode: 'BR' }, child])
    }) as never)
    signInWith(sessionWith(['tenant.child.data.read', 'tenant.read'], 'Tenant', { tenantUuid: 't-42' }))
    draw()
    const select = await screen.findByRole('combobox', { name: /acting in tenant|agindo no tenant/i })
    await screen.findByRole('option', { name: 'Filial Norte' })
    expect(screen.queryByRole('option', { name: 'Matriz' })).not.toBeInTheDocument()

    fireEvent.change(select, { target: { value: 'child-uuid' } })
    await waitFor(() => expect(store.getState().auth.target).toEqual({ uuid: 'child-uuid', name: 'Filial Norte' }))
    expect(readTarget()).toEqual({ uuid: 'child-uuid', name: 'Filial Norte' })
    expect(get).toHaveBeenCalledWith('/access-profile/mine')
    // the account still may act in a descendant while it is inside one, so the way back stays offered
    expect(store.getState().auth.session?.canTarget).toBe(true)

    fireEvent.change(select, { target: { value: '' } })
    await waitFor(() => expect(store.getState().auth.target).toBeNull())
    expect(localStorage.getItem(TARGET_KEY)).toBeNull()
  })

  it('forgets the target when the session ends', async () => {
    localStorage.setItem(TARGET_KEY, JSON.stringify({ uuid: 'child-uuid', name: 'Filial Norte' }))
    signIn('SystemAdministrator')
    store.dispatch(logout())
    expect(store.getState().auth.target).toBeNull()
    expect(localStorage.getItem(TARGET_KEY)).toBeNull()
  })
})
