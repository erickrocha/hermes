import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { Provider } from 'react-redux'
import { MemoryRouter } from 'react-router-dom'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { i18n } from './i18n'
import { publicApi } from './api'
import { App } from './App'
import { store } from './store'
import { applyThemeByName, applyTheme } from './theme'

const plans = [
  { name: 'Basic', priceInCents: 19900, pricePerExtraUserInCents: 1500, availableUsers: 5, periodDays: 30 },
  { name: 'Pro', priceInCents: 49900, pricePerExtraUserInCents: 1200, availableUsers: 20, periodDays: 30 },
]

const renderAt = (path: string) => render(<Provider store={store}><MemoryRouter initialEntries={[path]}><App /></MemoryRouter></Provider>)

describe('landing page', () => {
  beforeEach(async () => {
    vi.restoreAllMocks()
    localStorage.clear()
    store.dispatch({ type: 'auth/logout' })
    await i18n.changeLanguage('pt')
  })
  afterEach(() => { applyTheme(null); localStorage.clear() })

  it('shows the product sections, the plans from the public API and a login button', async () => {
    const get = vi.spyOn(publicApi, 'get').mockResolvedValue({ data: plans } as never)
    renderAt('/')
    expect(await screen.findByText('Basic')).toBeInTheDocument()
    expect(screen.getByText('Pro')).toBeInTheDocument()
    expect(get).toHaveBeenCalledWith('/public/business-plan')
    expect(screen.getByRole('heading', { name: 'Toda a operação da sua frota em um só lugar' })).toBeInTheDocument()
    expect(screen.getAllByRole('link', { name: /Acessar o sistema/ })[0]).toHaveAttribute('href', '/login')
    expect(screen.getAllByRole('link', { name: 'Começar agora' })).toHaveLength(2)
  })

  it('translates when the language changes', async () => {
    vi.spyOn(publicApi, 'get').mockResolvedValue({ data: plans } as never)
    renderAt('/')
    await screen.findByText('Basic')
    fireEvent.change(screen.getByLabelText(/Idioma|Language/), { target: { value: 'en' } })
    expect(await screen.findByRole('heading', { name: 'Your whole fleet operation in one place' })).toBeInTheDocument()
  })

  it('offers a retry when the plans fail and an empty message when there are none', async () => {
    const get = vi.spyOn(publicApi, 'get').mockRejectedValueOnce(new Error('down'))
    renderAt('/')
    expect(await screen.findByRole('alert')).toHaveTextContent('Não foi possível carregar os planos agora.')
    get.mockResolvedValue({ data: [] } as never)
    fireEvent.click(screen.getByRole('button', { name: 'Tentar novamente' }))
    expect(await screen.findByText('Os planos serão divulgados em breve.')).toBeInTheDocument()
  })

  it('ignores the stored theme on the landing page but applies it on the login screen', async () => {
    vi.spyOn(publicApi, 'get').mockResolvedValue({ data: plans } as never)
    localStorage.setItem('hermes.theme', 'transmega')
    applyThemeByName('transmega')
    const landing = renderAt('/')
    await screen.findByText('Basic')
    expect(document.documentElement.style.getPropertyValue('--accent-primary')).toBe('')
    landing.unmount()
    renderAt('/login')
    await waitFor(() => expect(document.documentElement.style.getPropertyValue('--accent-primary')).not.toBe(''))
  })
})
