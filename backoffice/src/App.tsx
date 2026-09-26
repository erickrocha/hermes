import { useEffect, type ReactNode } from 'react'
import { Navigate, Route, Routes, useLocation } from 'react-router-dom'
import { useDispatch, useSelector } from 'react-redux'
import type { AppDispatch, RootState } from './store'
import { logout } from './store'
import { applyTheme } from './theme'
import { Shell } from './components/Shell'
import { ErrorBoundary } from './components/ErrorBoundary'
import { AcceptInvitePage, LoginPage } from './pages/AuthPages'
import { DashboardPage } from './pages/DashboardPage'
import { PlanEditorPage, PlansPage, SubscriptionPage, TenantEditorPage, TenantsPage, UserEditorPage, UsersPage, canCreateUsers, canReachAdministration } from './pages/ManagementPages'
import { SettingsPage } from './pages/SettingsPage'
import { SystemSettingsPage } from './pages/SystemSettingsPage'

function Protected({ children, sysAdmin = false, allow }: { children: ReactNode; sysAdmin?: boolean; allow?: (role: string) => boolean }) {
  const session = useSelector((state: RootState) => state.auth.session)
  const location = useLocation()
  if (!session) return <Navigate to="/login" replace />
  if (sysAdmin && session.role !== 'SysAdmin') return <Navigate to="/" replace />
  // DEF-BO-04: a role check that only lives in the page's action bar is a
  // suggestion -- the URL is still reachable by hand. The guard and the
  // button share one predicate so they cannot disagree.
  if (allow && !allow(session.role)) return <Navigate to="/" replace />
  // U-1: keyed by route so navigating away from a page that threw clears the
  // failure instead of stranding the operator on the error card.
  return <Shell><ErrorBoundary key={location.pathname}>{children}</ErrorBoundary></Shell>
}

export default function App() {
  const dispatch = useDispatch<AppDispatch>()
  const session = useSelector((state: RootState) => state.auth.session)
  useEffect(() => { const onLogout = () => dispatch(logout()); window.addEventListener('hermes:logout', onLogout); return () => window.removeEventListener('hermes:logout', onLogout) }, [dispatch])
  // EPIC-BO-01-S02: a signed-out session (or one still on /login) never
  // keeps a previous tenant's theme -- Shell is what applies one, and it
  // isn't mounted here to un-apply it itself.
  useEffect(() => { if (!session) applyTheme(null) }, [session])
  return <Routes>
    <Route path="/login" element={session ? <Navigate to="/" replace /> : <LoginPage />} />
    <Route path="/accept-invite" element={<AcceptInvitePage />} />
    <Route path="/" element={<Protected><DashboardPage /></Protected>} />
    <Route path="/system-settings" element={<Protected sysAdmin><SystemSettingsPage /></Protected>} />
    <Route path="/tenants" element={<Protected allow={canReachAdministration}><TenantsPage /></Protected>} />
    <Route path="/tenants/new" element={<Protected sysAdmin><TenantEditorPage /></Protected>} />
    <Route path="/tenants/:uuid/edit" element={<Protected allow={canReachAdministration}><TenantEditorPage /></Protected>} />
    <Route path="/tenants/:uuid/subscription" element={<Protected sysAdmin><SubscriptionPage /></Protected>} />
    <Route path="/plans" element={<Protected sysAdmin><PlansPage /></Protected>} />
    <Route path="/plans/new" element={<Protected sysAdmin><PlanEditorPage /></Protected>} />
    <Route path="/plans/:uuid/edit" element={<Protected sysAdmin><PlanEditorPage /></Protected>} />
    <Route path="/users" element={<Protected allow={canReachAdministration}><UsersPage /></Protected>} />
    <Route path="/users/new" element={<Protected allow={canCreateUsers}><UserEditorPage /></Protected>} />
    <Route path="/users/:uuid/edit" element={<Protected allow={canReachAdministration}><UserEditorPage /></Protected>} />
    <Route path="/settings" element={<Protected><SettingsPage /></Protected>} />
    <Route path="*" element={<Navigate to="/" replace />} />
  </Routes>
}
