import { configureStore } from '@reduxjs/toolkit'
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { Provider } from 'react-redux'
import { MemoryRouter } from 'react-router-dom'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import '../i18n'
import { api } from '../api'
import { emptyPage } from '../types'
import type { BillingCharge, Session, Tenant } from '../types'
import { billingReducer } from '../features/billingSlice'
import { BillingPage, canSeeBilling } from './BillingPage'

const session = (role: Session['role'], tenantUuid: string | null = null): Session => ({
  accessToken: 'token', tokenType: 'Bearer', expireIn: 3600, email: `${role.toLowerCase()}@hermes.test`,
  uuid: `${role}-uuid`, name: role, userId: 1, role, tenantId: tenantUuid ? 42 : null, tenantUuid,
})

const pendingCharge: BillingCharge = {
  uuid: 'charge-1', tenantUuid: 'tenant-1', tenantName: 'Acme', planName: 'Pro', currency: 'BRL',
  periodStart: '2026-01-01T00:00:00', periodEnd: '2026-01-31T00:00:00', priceInCents: 10000,
  pricePerExtraUserInCents: 500, includedUsers: 10, activeUsers: 12, extraUsers: 2, graceDays: 7,
  totalInCents: 11000, status: 'Pending', isOverdue: true, paidAt: null, paidBy: null,
}

const tenant: Tenant = { id: 42, uuid: 'tenant-1', businessName: 'Acme', companyName: 'Acme Transport', taxId: '123', countryCode: 'BR' }

function renderBilling(current: Session) {
  const tenantPage = { ...emptyPage<Tenant>(), items: [tenant], totalItems: 1, totalPages: 1 }
  const testStore = configureStore({
    reducer: {
      auth: (state = { session: current, loading: false, error: '' }) => state,
      data: (state = { tenants: tenantPage, plans: emptyPage(), users: emptyPage(), provinces: [], cities: [], countries: [], counts: { tenants: 1, users: 1, plans: 1 }, currentTenant: tenant, loading: false, error: '' }) => state,
      billing: billingReducer,
    },
  })
  return render(<Provider store={testStore}><MemoryRouter><BillingPage /></MemoryRouter></Provider>)
}

describe('billing backoffice (C-015, PD-042)', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    vi.spyOn(api, 'get').mockImplementation(async (url: string) => {
      if (url === '/billing/charges') return { data: { items: [pendingCharge], page: 0, pageSize: 25, totalItems: 1, totalPages: 1 } } as never
      if (url.endsWith('/settings')) return { data: { tenantUuid: 'tenant-1', graceDays: 7, cycleStartedAt: '2026-01-01T00:00:00', pendingPlan: false, suspendedAt: null, suspendedBy: null } } as never
      if (url === '/tenant') return { data: { items: [tenant], page: 0, pageSize: 200, totalItems: 1, totalPages: 1 } } as never
      throw new Error(`unexpected GET ${url}`)
    })
    vi.spyOn(api, 'put').mockResolvedValue({ data: { tenantUuid: 'tenant-1', graceDays: 7, pendingPlan: false, suspendedAt: null } } as never)
    vi.spyOn(api, 'post').mockResolvedValue({ data: { ...pendingCharge, status: 'Paid', paidBy: 'sysadmin@hermes.test' } } as never)
  })

  it('offers billing to SysAdmin and TenantOwner only', () => {
    expect(canSeeBilling(session('SysAdmin'))).toBe(true)
    expect(canSeeBilling(session('TenantOwner', 'tenant-1'))).toBe(true)
    expect(canSeeBilling(session('TenantUser', 'tenant-1'))).toBe(false)
    expect(canSeeBilling(session('Driver', 'tenant-1'))).toBe(false)
    expect(canSeeBilling(session('Mechanic', 'tenant-1'))).toBe(false)
  })

  it('lets the tenant owner read their own billing history without platform actions', async () => {
    renderBilling(session('TenantOwner', 'tenant-1'))
    const table = await screen.findByRole('table')
    expect(within(table).getByText('Pro')).toBeInTheDocument()
    expect(within(table).getByText(/overdue|vencida/i)).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /confirm payment|confirmar pagamento/i })).not.toBeInTheDocument()
    expect(screen.queryByRole('button', { name: /suspend access|suspender acesso/i })).not.toBeInTheDocument()
  })

  it('lets SysAdmin configure grace days and explicitly confirm a payment or suspend access', async () => {
    const put = vi.mocked(api.put)
    const post = vi.mocked(api.post)
    renderBilling(session('SysAdmin'))

    const tenantPicker = await screen.findByRole('combobox', { name: /tenant/i })
    fireEvent.focus(tenantPicker.querySelector('input')!)
    fireEvent.click(await screen.findByRole('option', { name: 'Acme Transport' }))

    const graceInput = await screen.findByLabelText(/grace days|dias de tolerância/i)
    fireEvent.change(graceInput, { target: { value: '10' } })
    fireEvent.click(screen.getByRole('button', { name: /save|salvar/i }))
    await waitFor(() => expect(put).toHaveBeenCalledWith('/billing/tenant/uuid/tenant-1/settings', { graceDays: 10 }))

    fireEvent.click(screen.getByRole('button', { name: /confirm payment|confirmar pagamento/i }))
    fireEvent.click(within(screen.getByRole('dialog')).getByRole('button', { name: /^confirm|^confirmar/i }))
    await waitFor(() => expect(post).toHaveBeenCalledWith('/billing/charges/uuid/charge-1/confirm'))

    fireEvent.click(screen.getByRole('button', { name: /suspend access|suspender acesso/i }))
    fireEvent.click(within(screen.getByRole('dialog')).getByRole('button', { name: /^confirm|^confirmar/i }))
    await waitFor(() => expect(put).toHaveBeenCalledWith('/billing/tenant/uuid/tenant-1/suspension', { suspended: true }))
  })
})
