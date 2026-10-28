import { render } from '@testing-library/react'
import type { ReactElement } from 'react'
import { Provider } from 'react-redux'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import '../i18n'
import { login, store } from '../store'
import type { Session } from '../types'
import { sessionAs, sessionWith, type SystemProfileKey } from './session'

export { sessionAs, sessionWith, systemGrants } from './session'

/// A session holding a system profile, or an explicit set of grants.
export const session = (profile: SystemProfileKey, permissions?: string[]): Session =>
  permissions ? sessionWith(permissions, profile === 'SysAdmin' ? 'Platform' : 'Tenant', { profileKey: profile }) : sessionAs(profile)

export const signIn = (profile: SystemProfileKey, permissions?: string[]) => store.dispatch({ type: login.fulfilled.type, payload: session(profile, permissions) })
export const signInWith = (who: Session) => store.dispatch({ type: login.fulfilled.type, payload: who })

/// Monta a página sob o store real e o roteador, na rota pedida.
export function renderPage(element: ReactElement, path = '/x', route = '/x') {
  return render(<Provider store={store}><MemoryRouter initialEntries={[path]}><Routes><Route path={route} element={element} /><Route path="*" element={<p>elsewhere</p>} /></Routes></MemoryRouter></Provider>)
}
