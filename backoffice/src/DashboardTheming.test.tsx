import { render, screen, waitFor } from '@testing-library/react'
import { Provider } from 'react-redux'
import { MemoryRouter } from 'react-router-dom'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import './i18n'
import { api } from './api'
import App from './App'
import { store } from './store'
import { applyTheme } from './theme'
import { emptyPage } from './types'
import type { Session, Tenant } from './types'

const owner: Session = { accessToken: 't', tokenType: 'Bearer', expireIn: 3600, email: 'owner@transmega.com', uuid: 'owner-uuid', name: 'Owner', userId: 2, role: 'TenantOwner', tenantId: 42 }
const transmega: Tenant = { id: 42, businessName: 'Transmega Logística', companyName: 'Transmega', taxId: '11222333000181', email: '', phone: '', website: '', addressLine1: '', addressLine2: '', locality: '', administrativeArea: '', postalCode: '', countryCode: 'BR' }

const renderDashboard = () => render(
  <Provider store={store}>
    <MemoryRouter initialEntries={['/']}><App /></MemoryRouter>
  </Provider>
)

describe('dashboard and tenant theming against the live store', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    localStorage.setItem('hermes.session', JSON.stringify(owner))
    store.dispatch({ type: 'auth/login/fulfilled', payload: owner })
  })
  afterEach(() => {
    applyTheme(null)
    localStorage.removeItem('hermes.session')
  })

  // DEF-BO-02: the theme used to be derived from whichever tenant happened to
  // be sitting in an already-loaded tenant *list*, so a tenant owner saw their
  // own identity only by accident -- and never once the dashboard stopped
  // loading that list (DEF-BO-07).
  it('themes the console from the session tenant, without needing a tenant list', async () => {
    vi.spyOn(api, 'get').mockImplementation((async (url: string) => {
      if (url === '/tenant/42') return { data: transmega }
      return { data: { ...emptyPage(), totalItems: 29 } }
    }) as never)

    renderDashboard()

    await waitFor(() => expect(document.documentElement.style.getPropertyValue('--accent-primary').trim()).toBe('#ff5b00'))
    expect(store.getState().data.tenants.items).toHaveLength(0)
  })

  // DEF-BO-07: the count cards asked for `{page:0,pageSize:1}` and wrote the
  // answer into the same slot the Tenants screen reads, so opening the
  // dashboard left the list showing "1-1 of 29 - Page 1 of 29".
  it('counts records without overwriting the page the list screens read', async () => {
    const totals: Record<string, number> = { '/tenant': 29, '/user': 7 }
    vi.spyOn(api, 'get').mockImplementation((async (url: string) => {
      if (url === '/tenant/42') return { data: transmega }
      return { data: { ...emptyPage(), totalItems: totals[url] ?? 0, pageSize: 1 } }
    }) as never)

    renderDashboard()

    expect(await screen.findByText('29')).toBeInTheDocument()
    expect(await screen.findByText('7')).toBeInTheDocument()
    const listPage = store.getState().data.tenants
    expect(listPage.items).toHaveLength(0)
    expect(listPage.totalItems).toBe(0)
    expect(store.getState().data.counts.tenants).toBe(29)
  })
})
