import { render, screen } from '@testing-library/react'
import { Provider } from 'react-redux'
import { MemoryRouter } from 'react-router-dom'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import './i18n'
import { api } from './api'
import { App } from './App'
import { resourceConfigs } from './features/resources'
import { customRoutes } from './routes'
import { signIn } from './test/render'
import { store } from './store'

const empty = { data: { items: [], page: 0, pageSize: 25, totalItems: 0, totalPages: 0 } } as never
const at = (path: string) => render(<Provider store={store}><MemoryRouter initialEntries={[path]}><App /></MemoryRouter></Provider>)

describe('routing', () => {
  beforeEach(() => { vi.restoreAllMocks(); vi.spyOn(api, 'get').mockResolvedValue(empty) })

  it('answers an unknown address with the 404 page, inside the shell', async () => {
    signIn('TenantOwner')
    at('/nothing/here')
    expect(await screen.findByRole('heading', { name: /page not found|página não encontrada|página no encontrada/i })).toBeInTheDocument()
  })

  it('sends a signed-out visitor to the login', async () => {
    store.dispatch({ type: 'auth/logout' })
    at('/vehicles')
    expect(await screen.findByRole('button', { name: /sign in|entrar|iniciar/i })).toBeInTheDocument()
  })

  it('does not let a role open a screen it has no permission for, even by typing the address', async () => {
    signIn('Driver')
    at('/customers/new')
    expect((await screen.findAllByText(/overview|visão geral|resumen/i)).length).toBeGreaterThan(0)
    expect(screen.queryByRole('button', { name: /^save$|^salvar$|^guardar$/i })).toBeNull()
  })

  it('opens the dedicated customer form (identity + address) on the new and edit addresses', async () => {
    signIn('TenantOwner')
    const reference = async (url: string) => (url === '/province' || url.startsWith('/cities') ? { data: [] } : empty) as never
    vi.spyOn(api, 'get').mockImplementation(reference)
    const view = at('/customers/new')
    expect(await screen.findByLabelText(/legal name|razão social|razón social/i)).toBeInTheDocument()
    expect(screen.getByRole('radio', { name: /brazil|brasil/i })).toBeInTheDocument()
    view.unmount()
    vi.spyOn(api, 'get').mockImplementation(async (url: string) => (url === '/customer/uuid/abc' ? { data: { uuid: 'abc', name: 'Acme', status: 'Active' } } as never : reference(url)))
    at('/customers/abc/edit')
    expect(await screen.findByDisplayValue('Acme')).toBeInTheDocument()
    expect(screen.getByLabelText(/legal name|razão social|razón social/i)).toBeInTheDocument()
  })

  it('wires every list, form and detail route to a page that renders for the owner', async () => {
    signIn('TenantOwner')
    const paths = [...resourceConfigs.flatMap((c) => [`/${c.route}`, ...(c.create ? [`/${c.route}/new`] : [])]), ...customRoutes.map((r) => r.path.replace(/:uuid/g, 'abc'))]
    for (const path of paths) {
      const view = at(path)
      await vi.waitFor(() => expect(view.container.querySelector('.page-header, .state-box, .page-form-card, form')).not.toBeNull())
      view.unmount()
    }
    expect(paths.length).toBeGreaterThan(40)
  }, 60_000)
})
