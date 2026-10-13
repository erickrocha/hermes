import { fireEvent, screen, waitFor } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { api } from '../../api'
import { resourceConfigs } from '../../features/resources'
import { renderPage, session, signIn } from '../../test/render'
import { ResourceEditRoute, ResourceListRoute, ResourceNewRoute } from './ResourceRoutes'

const config = (route: string) => resourceConfigs.find((c) => c.route === route)!
const page = (items: unknown[]) => ({ data: { items, page: 0, pageSize: 25, totalItems: items.length, totalPages: 1 } }) as never

describe('resource screens', () => {
  beforeEach(() => { vi.restoreAllMocks() })

  it('follows the permissions the server enforces for each resource', () => {
    const vehicles = config('vehicles')
    expect(vehicles.read(session('Driver'))).toBe(true)
    expect(vehicles.create!(session('TenantOwner'))).toBe(true)
    expect(vehicles.create!(session('Mechanic'))).toBe(false)
    expect(vehicles.read(session('SysAdmin'))).toBe(false)
    expect(config('parts').create!(session('Mechanic'))).toBe(true)
    expect(config('customers').create!(session('Mechanic'))).toBe(false)
    expect(config('customers').edit!(session('TenantOwner', ['customer.read']))).toBe(false)
  })

  it('lists with the empty state when there are no rows, and offers new only to who can create', async () => {
    vi.spyOn(api, 'get').mockResolvedValue(page([]))
    signIn('TenantUser')
    renderPage(<ResourceListRoute config={config('customers')} />)
    expect(await screen.findByText(/no records|nenhum registro|ningún registro|no results/i)).toBeInTheDocument()
    expect(screen.queryByRole('link', { name: /new customer|novo cliente|nuevo cliente/i })).toBeNull()
  })

  it('removes a holiday only after the confirmation', async () => {
    vi.spyOn(api, 'get').mockResolvedValue(page([{ uuid: 'h1', date: '2026-12-25', name: 'Christmas' }]))
    const del = vi.spyOn(api, 'delete').mockResolvedValue({ data: {} } as never)
    signIn('TenantOwner')
    renderPage(<ResourceListRoute config={config('holidays')} />)
    expect(await screen.findByText('Christmas')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: /delete|excluir|eliminar/i }))
    expect(del).not.toHaveBeenCalled()
    fireEvent.click(screen.getByRole('button', { name: /confirm|confirmar/i }))
    await waitFor(() => expect(del).toHaveBeenCalledWith('/holiday/uuid/h1'))
  })

  it('creates a vehicle from the form and sends only what was filled in', async () => {
    vi.spyOn(api, 'get').mockResolvedValue(page([]))
    const post = vi.spyOn(api, 'post').mockResolvedValue({ data: { uuid: 'v9' } } as never)
    signIn('TenantOwner')
    renderPage(<ResourceNewRoute config={config('vehicles')} />, '/vehicles/new', '/vehicles/new')
    fireEvent.change(await screen.findByLabelText(/^plate|^placa/i), { target: { value: 'ABC1D23' } })
    fireEvent.change(screen.getByLabelText(/^model|^modelo/i), { target: { value: 'Bus' } })
    fireEvent.change(screen.getByLabelText(/^status/i), { target: { value: 'Active' } })
    fireEvent.click(screen.getByRole('button', { name: /^save$|^salvar$|^guardar$/i }))
    await waitFor(() => expect(post).toHaveBeenCalledWith('/vehicle', { plate: 'ABC1D23', model: 'Bus', status: 'Active' }))
  })

  it('edits a customer: loads the record, then puts the change', async () => {
    vi.spyOn(api, 'get').mockResolvedValue({ data: { uuid: 'c1', name: 'Acme', status: 'Active' } } as never)
    const put = vi.spyOn(api, 'put').mockResolvedValue({ data: {} } as never)
    signIn('TenantOwner')
    renderPage(<ResourceEditRoute config={config('customers')} />, '/customers/c1/edit', '/customers/:uuid/edit')
    const name = await screen.findByDisplayValue('Acme')
    fireEvent.change(name, { target: { value: 'Acme Ltda' } })
    fireEvent.click(screen.getByRole('button', { name: /^save$|^salvar$|^guardar$/i }))
    await waitFor(() => expect(put).toHaveBeenCalledWith('/customer/uuid/c1', { name: 'Acme Ltda', status: 'Active' }))
  })

  it('shows the server message when saving fails', async () => {
    vi.spyOn(api, 'get').mockResolvedValue(page([]))
    vi.spyOn(api, 'post').mockRejectedValue(Object.assign(new Error('x'), { isAxiosError: true, response: { data: { message: 'Plate already exists' } } }))
    signIn('TenantOwner')
    renderPage(<ResourceNewRoute config={config('vehicles')} />, '/vehicles/new', '/vehicles/new')
    fireEvent.change(await screen.findByLabelText(/^plate|^placa/i), { target: { value: 'A' } })
    fireEvent.change(screen.getByLabelText(/^model|^modelo/i), { target: { value: 'B' } })
    fireEvent.change(screen.getByLabelText(/^status/i), { target: { value: 'Active' } })
    fireEvent.click(screen.getByRole('button', { name: /^save$|^salvar$|^guardar$/i }))
    expect(await screen.findByText('Plate already exists')).toBeInTheDocument()
  })
})

describe('form pages after another action of the same feature', () => {
  it('stays on the form when the slice was left in the saved state by an earlier action (e.g. a sync)', async () => {
    vi.spyOn(api, 'get').mockResolvedValue({ data: { items: [], page: 0, pageSize: 200, totalItems: 0, totalPages: 0 } } as never)
    vi.spyOn(api, 'post').mockResolvedValue({ data: {} } as never)
    signIn('TenantOwner')
    const { store } = await import('../../store')
    await store.dispatch(config('vehicles').resource.act({ method: 'post', path: '/vehicle/x' }))
    renderPage(<ResourceNewRoute config={config('vehicles')} />, '/vehicles/new', '/vehicles/new')
    expect(await screen.findByLabelText(/^plate|^placa/i)).toBeInTheDocument()
    expect(screen.queryByText('elsewhere')).toBeNull()
  })
})
