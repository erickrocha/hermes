import { fireEvent, screen, waitFor } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { api } from '../api'
import { operationNav } from '../features/navigation'
import { customRoutes } from '../routes'
import { renderPage, session, signIn } from '../test/render'
import { NonExecutionJustifyPage, NonExecutionManualPage, NonExecutionsPage, canSeeNonExecutions } from './NonExecutionPages'

const occurrence = { uuid: 'o1', kind: 'EnteredAndLeft', windowStart: '2026-10-10T10:00:00', windowEnd: '2026-10-10T12:00:00', arrivedAt: '2026-10-10T10:30:00', departedAt: '2026-10-10T11:20:00', dwellMinutes: 50, minutesInWindow: 50 }
const noShow = { uuid: 'o2', kind: 'DidNotAppear', windowStart: '2026-10-11T10:00:00', windowEnd: '2026-10-11T12:00:00' }
const group = { vehicle: { uuid: 'v1', plate: 'ABC1D23', model: 'Bus' }, workOrders: [{ workOrderUuid: 'w1', description: 'Brake pads', status: 'Open', totalOccurrences: 3, occurrences: [occurrence, noShow] }] }
const page = (items: unknown[]) => ({ data: { items, page: 0, pageSize: 25, totalItems: items.length, totalPages: items.length ? 1 : 0 } })

describe('non-executed maintenance (C-026 EPIC-MT-04)', () => {
  beforeEach(() => { vi.restoreAllMocks() })

  it('is read by the owner, the mechanic and the tenant user; not by the driver; the menu follows', () => {
    for (const role of ['TenantOwner', 'Mechanic', 'TenantUser'] as const) expect(canSeeNonExecutions(session(role))).toBe(true)
    expect(canSeeNonExecutions(session('Driver'))).toBe(false)
    expect(canSeeNonExecutions(session('Mechanic', ['work-order.read']))).toBe(false)
    const entry = operationNav.find((item) => item.to === '/non-executions')
    expect(entry?.visible(session('Mechanic'))).toBe(true)
    expect(entry?.visible(session('Driver'))).toBe(false)
    const route = (path: string) => customRoutes.find((candidate) => candidate.path === path)
    expect(route('/non-executions/manual')?.allow(session('TenantUser'))).toBe(false)
    expect(route('/non-executions/:uuid/justify')?.allow(session('Mechanic'))).toBe(true)
  })

  it('groups by vehicle, one row per order, with the unresolved count and the occurrences', async () => {
    vi.spyOn(api, 'get').mockResolvedValue(page([group]) as never)
    signIn('Mechanic')
    renderPage(<NonExecutionsPage />)

    expect(await screen.findByText(/ABC1D23/)).toBeInTheDocument()
    expect(screen.getByText(/3× (unresolved|sem resolver|sin resolver)/)).toBeInTheDocument()
    expect(screen.getByRole('link', { name: 'Brake pads' })).toHaveAttribute('href', '/work-orders/w1')
    expect(screen.getByText(/came and left|veio e saiu|vino y salió/i)).toBeInTheDocument()
    expect(screen.getByText(/did not appear|não apareceu|no apareció/i)).toBeInTheDocument()
    const justify = screen.getAllByRole('link', { name: /^(justify|justificar)$/i })
    expect(justify[0]).toHaveAttribute('href', '/non-executions/o1/justify')
    expect(screen.getByRole('link', { name: /record manually|registrar manualmente/i })).toHaveAttribute('href', '/non-executions/manual')
  })

  it('shows "Evaluate now" only to who may evaluate, and reports what it did', async () => {
    const get = vi.spyOn(api, 'get').mockResolvedValue(page([group]) as never)
    const post = vi.spyOn(api, 'post').mockResolvedValue({ data: { plansEvaluated: 2, occurrencesRecorded: 1, plansConcluded: 0 } } as never)
    signIn('Mechanic')
    const first = renderPage(<NonExecutionsPage />)
    await screen.findByText(/ABC1D23/)
    expect(screen.queryByRole('button', { name: /evaluate now|avaliar agora|evaluar ahora/i })).toBeNull()
    first.unmount()

    signIn('TenantOwner')
    renderPage(<NonExecutionsPage />)
    fireEvent.click(await screen.findByRole('button', { name: /evaluate now|avaliar agora|evaluar ahora/i }))
    await waitFor(() => expect(post).toHaveBeenCalledWith('/non-execution/evaluate'))
    expect(await screen.findByRole('status')).toHaveTextContent(/1/)
    await waitFor(() => expect(get.mock.calls.length).toBeGreaterThan(2))
  })

  it('shows the empty and the error states', async () => {
    vi.spyOn(api, 'get').mockResolvedValue(page([]) as never)
    signIn('TenantUser')
    const first = renderPage(<NonExecutionsPage />)
    expect(await screen.findByText(/no records found|nenhum registro|no se encontraron/i)).toBeInTheDocument()
    expect(screen.queryByRole('link', { name: /record manually|registrar manualmente/i })).toBeNull()
    first.unmount()

    vi.restoreAllMocks()
    vi.spyOn(api, 'get').mockRejectedValue(new Error('down'))
    renderPage(<NonExecutionsPage />)
    expect(await screen.findByRole('alert')).toBeInTheDocument()
  })

  it('justifies on its own page: required text, then a PUT, then back to the inbox', async () => {
    const put = vi.spyOn(api, 'put').mockResolvedValue({ data: {} } as never)
    signIn('Mechanic')
    renderPage(<NonExecutionJustifyPage />, '/non-executions/o1/justify', '/non-executions/:uuid/justify')

    fireEvent.click(screen.getByRole('button', { name: /^(save|salvar|guardar)$/i }))
    expect(await screen.findByText(/required field|campo obrigatório|campo obligatorio/i)).toBeInTheDocument()
    expect(put).not.toHaveBeenCalled()

    fireEvent.change(screen.getByLabelText(/justification|justificativa|justificación/i), { target: { value: '  waiting for the part  ' } })
    fireEvent.click(screen.getByRole('button', { name: /^(save|salvar|guardar)$/i }))
    await waitFor(() => expect(put).toHaveBeenCalledWith('/non-execution/occurrence/uuid/o1/justify', { justification: 'waiting for the part' }))
    expect(await screen.findByText('elsewhere')).toBeInTheDocument()
  })

  it('refuses a justification above 1000 characters before asking the server', async () => {
    const put = vi.spyOn(api, 'put').mockResolvedValue({ data: {} } as never)
    signIn('Mechanic')
    renderPage(<NonExecutionJustifyPage />, '/non-executions/o1/justify', '/non-executions/:uuid/justify')
    fireEvent.change(screen.getByLabelText(/justification|justificativa|justificación/i), { target: { value: 'x'.repeat(1001) } })
    fireEvent.click(screen.getByRole('button', { name: /^(save|salvar|guardar)$/i }))
    expect(await screen.findByText(/text too long|texto longo demais|texto demasiado largo/i)).toBeInTheDocument()
    expect(put).not.toHaveBeenCalled()
  })

  it('shows the server refusal when the justification fails', async () => {
    vi.spyOn(api, 'put').mockRejectedValue({ isAxiosError: true, response: { data: { message: 'This non-execution is already justified' } } })
    signIn('Mechanic')
    renderPage(<NonExecutionJustifyPage />, '/non-executions/o1/justify', '/non-executions/:uuid/justify')
    fireEvent.change(screen.getByLabelText(/justification|justificativa|justificación/i), { target: { value: 'again' } })
    fireEvent.click(screen.getByRole('button', { name: /^(save|salvar|guardar)$/i }))
    expect(await screen.findByRole('alert')).toBeInTheDocument()
  })

  it('records manually from its own page, choosing the work order; no evidence shows the refusal', async () => {
    vi.spyOn(api, 'get').mockImplementation(async (url: string) => (url === '/work-order' ? page([{ uuid: 'w1', number: 'OS-1', description: 'Brake pads' }]) : page([])) as never)
    const post = vi.spyOn(api, 'post').mockRejectedValueOnce({ isAxiosError: true, response: { data: { message: 'The tracker does not evidence it' } } }).mockResolvedValue({ data: {} } as never)
    signIn('Mechanic')
    renderPage(<NonExecutionManualPage />, '/non-executions/manual', '/non-executions/manual')

    await screen.findByRole('option', { name: /OS-1/ })
    fireEvent.change(screen.getByLabelText(/^work order|^ordem de serviço|^orden de servicio/i), { target: { value: 'w1' } })
    fireEvent.change(screen.getByLabelText(/justification|justificativa|justificación/i), { target: { value: 'left early' } })
    fireEvent.click(screen.getByRole('button', { name: /^(save|salvar|guardar)$/i }))
    expect(await screen.findByRole('alert')).toHaveTextContent(/does not evidence/)
    expect(post).toHaveBeenCalledWith('/non-execution/manual', { workOrderUuid: 'w1', justification: 'left early' })

    fireEvent.click(screen.getByRole('button', { name: /^(save|salvar|guardar)$/i }))
    expect(await screen.findByText('elsewhere')).toBeInTheDocument()
  })
})
