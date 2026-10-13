import { fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import '../i18n'
import { api } from '../api'
import { CostReportsPage, canSeeCosts } from './CostPages'
import type { Session } from '../types'

const session = (role: Session['role'], permissions?: string[]): Session =>
  ({ accessToken: 't', tokenType: 'Bearer', expireIn: 3600, email: 'a@b.c', uuid: 'u', name: 'n', userId: 1, role, tenantId: 42, tenantUuid: 't-42', permissions })

const cost = {
  plate: 'ABC1D23', bySource: { workOrderPostingsCents: 12500, externalServicesCents: 45000, fuelCents: 10000, expensesCents: 7000 }, totalCostCents: 74500,
  byServiceType: [{ name: '', amount: 45000 }, { name: 'Brakes', amount: 12500 }], servicesPerformed: [{ name: 'Wash', amount: 2 }], revenueCents: 200000, resultCents: 125500,
}

describe('cost reports (C-030, HRMS-804…810)', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    vi.spyOn(api, 'get').mockImplementation(async (url: string) => {
      if (url === '/vehicle') return { data: { items: [{ uuid: 'v1', plate: 'ABC1D23', model: 'Bus' }] } } as never
      if (url === '/cost-report/suppliers') return { data: ['Oficina Sul'] } as never
      if (url.startsWith('/cost-report/vehicle/uuid/v1')) return { data: cost } as never
      if (url === '/cost-report/supplier-closing') return { data: { lines: [{ date: '2026-09-20', invoiceNumber: 'NF-3', service: 'Tyres', plate: 'ABC1D23', invoiceCents: 45000 }], totalCents: 45000 } } as never
      throw new Error(`unexpected ${url}`)
    })
  })

  it('offers the screen to the office and to nobody else', () => {
    expect(canSeeCosts(session('TenantOwner'))).toBe(true)
    expect(canSeeCosts(session('TenantUser'))).toBe(true)
    for (const role of ['Driver', 'Mechanic', 'SysAdmin'] as const) expect(canSeeCosts(session(role))).toBe(false)
    expect(canSeeCosts(session('TenantUser', ['vehicle.read']))).toBe(false)
    expect(canSeeCosts(session('TenantUser', ['cost-report.vehicle.read']))).toBe(true)
  })

  it('shows a vehicle\'s cost by source, the classification heading and the result', async () => {
    render(<MemoryRouter><CostReportsPage /></MemoryRouter>)
    fireEvent.focus(await screen.findByRole('textbox'))
    fireEvent.click(await screen.findByRole('option', { name: /ABC1D23/ }))
    fireEvent.click(screen.getByRole('button', { name: /generate|gerar|generar/i }))
    const table = (await screen.findAllByRole('table'))[0]
    await waitFor(() => expect(within(table).getByText(/745[.,]00/)).toBeInTheDocument())
    expect(within(table).getByText(/1[.,]?255[.,]00/)).toBeInTheDocument()
    expect(screen.getByText(/to be classified|a classificar|por clasificar/i)).toBeInTheDocument()
  })

  it('lists the supplier closing lines with their total', async () => {
    render(<MemoryRouter><CostReportsPage /></MemoryRouter>)
    fireEvent.click(screen.getByRole('tab', { name: /supplier closing|fechamento|cierre/i }))
    fireEvent.focus(await screen.findByRole('textbox'))
    fireEvent.click(await screen.findByRole('option', { name: 'Oficina Sul' }))
    fireEvent.click(screen.getByRole('button', { name: /generate|gerar|generar/i }))
    expect(await screen.findByText('NF-3')).toBeInTheDocument()
    expect(screen.getAllByText(/450[.,]00/).length).toBeGreaterThan(0)
  })
})
