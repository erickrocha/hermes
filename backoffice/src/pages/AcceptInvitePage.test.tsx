import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import '../i18n'
import { api } from '../api'
import { AcceptInvitePage } from './AuthPages'

// EPIC-BO-04 client half of EPIC-IA-07/D-07: this is the page the emailed
// invitation link actually opens -- no session exists yet, only the token
// in the URL.
describe('AcceptInvitePage', () => {
  beforeEach(() => vi.restoreAllMocks())

  it('rejects a link with no token before ever calling the API', () => {
    const post = vi.spyOn(api, 'post')
    render(
      <MemoryRouter initialEntries={['/accept-invite']}>
        <Routes><Route path="/accept-invite" element={<AcceptInvitePage />} /></Routes>
      </MemoryRouter>
    )
    expect(screen.getByText(/invalid invitation link|link de convite inválido/i)).toBeInTheDocument()
    expect(post).not.toHaveBeenCalled()
  })

  it('submits the token from the URL together with the chosen password', async () => {
    const post = vi.spyOn(api, 'post').mockResolvedValue({ data: {} } as never)
    render(
      <MemoryRouter initialEntries={['/accept-invite?token=abc.def.ghi']}>
        <Routes><Route path="/accept-invite" element={<AcceptInvitePage />} /></Routes>
      </MemoryRouter>
    )
    fireEvent.change(screen.getByLabelText(/^new password$|^nova senha$/i), { target: { value: 'a-strong-password' } })
    fireEvent.change(screen.getByLabelText(/confirm new password|confirmar nova senha/i), { target: { value: 'a-strong-password' } })
    fireEvent.click(screen.getByRole('button', { name: /activate account|ativar conta/i }))

    await waitFor(() => expect(post).toHaveBeenCalledWith('/accept-invite', { token: 'abc.def.ghi', newPassword: 'a-strong-password' }))
    expect(await screen.findByText(/account activated|conta ativada/i)).toBeInTheDocument()
  })

  it('rejects a password shorter than 8 characters without calling the API', () => {
    const post = vi.spyOn(api, 'post')
    render(
      <MemoryRouter initialEntries={['/accept-invite?token=abc.def.ghi']}>
        <Routes><Route path="/accept-invite" element={<AcceptInvitePage />} /></Routes>
      </MemoryRouter>
    )
    fireEvent.change(screen.getByLabelText(/^new password$|^nova senha$/i), { target: { value: 'short' } })
    fireEvent.change(screen.getByLabelText(/confirm new password|confirmar nova senha/i), { target: { value: 'short' } })
    fireEvent.click(screen.getByRole('button', { name: /activate account|ativar conta/i }))
    expect(post).not.toHaveBeenCalled()
  })
})
