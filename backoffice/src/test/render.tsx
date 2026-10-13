import { render } from '@testing-library/react'
import type { ReactElement } from 'react'
import { Provider } from 'react-redux'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import '../i18n'
import { login, store } from '../store'
import type { Session } from '../types'

export const session = (role: Session['role'], permissions?: string[]): Session =>
  ({ accessToken: 't', tokenType: 'Bearer', expireIn: 3600, email: 'a@b.c', uuid: 'u', name: 'n', userId: 1, role, tenantId: 42, tenantUuid: 't-42', permissions })

export const signIn = (role: Session['role'], permissions?: string[]) => store.dispatch({ type: login.fulfilled.type, payload: session(role, permissions) })

/// Monta a página sob o store real e o roteador, na rota pedida.
export function renderPage(element: ReactElement, path = '/x', route = '/x') {
  return render(<Provider store={store}><MemoryRouter initialEntries={[path]}><Routes><Route path={route} element={element} /><Route path="*" element={<p>elsewhere</p>} /></Routes></MemoryRouter></Provider>)
}
