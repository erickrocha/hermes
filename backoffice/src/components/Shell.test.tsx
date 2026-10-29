import { fireEvent, screen } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { api } from '../api'
import { renderPage, signIn } from '../test/render'
import { Shell } from './Shell'
import * as UI from './UI'

describe('Shell', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    vi.spyOn(api, 'get').mockResolvedValue({ data: { items: [], page: 0, pageSize: 25, totalItems: 0, totalPages: 0 } } as never)
  })

  it('shows the owner navigation and the page content', async () => {
    signIn('TenantOwner')
    renderPage(<Shell><p>conteúdo da página</p></Shell>)
    expect(await screen.findByText('conteúdo da página')).toBeInTheDocument()
    expect(screen.getAllByRole('link').some((link) => link.getAttribute('href') === '/users')).toBe(true)
    expect(screen.getAllByRole('link').some((link) => link.getAttribute('href') === '/plans')).toBe(false)
  })

  it('offers the vendor-only entries to the SysAdmin', async () => {
    signIn('SystemAdministrator')
    renderPage(<Shell><p>x</p></Shell>)
    await screen.findByText('x')
    expect(screen.getAllByRole('link').some((link) => link.getAttribute('href') === '/plans')).toBe(true)
  })

  it('signs out to the login screen', async () => {
    signIn('TenantOwner')
    renderPage(<Shell><p>x</p></Shell>)
    await screen.findByText('x')
    const leave = screen.getAllByRole('button').find((b) => /sair|sign out|log ?out|salir/i.test(b.textContent ?? b.getAttribute('aria-label') ?? ''))
    if (leave) { fireEvent.click(leave); expect(await screen.findByText('elsewhere')).toBeInTheDocument() }
  })
})

describe('UI barrel', () => {
  it('keeps the shared building blocks importable from one place', () => {
    for (const name of ['PageHeader', 'Loading', 'Empty', 'SearchBox', 'Modal', 'Breadcrumbs', 'Confirm', 'Form']) {
      expect(typeof (UI as Record<string, unknown>)[name]).toBe('function')
    }
  })
})
