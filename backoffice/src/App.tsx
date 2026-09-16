import { useEffect, type ReactNode } from 'react'
import { Navigate, Route, Routes } from 'react-router-dom'
import { useDispatch, useSelector } from 'react-redux'
import type { AppDispatch, RootState } from './store'
import { logout } from './store'
import { Shell } from './components/Shell'
import { LoginPage, PasswordPage } from './pages/AuthPages'
import { DashboardPage } from './pages/DashboardPage'
import { PlanEditorPage, PlansPage, SubscriptionPage, TenantEditorPage, TenantsPage, UserEditorPage, UsersPage } from './pages/ManagementPages'
import { SettingsPage } from './pages/SettingsPage'

function Protected({ children, sysAdmin = false }: { children: ReactNode; sysAdmin?: boolean }) {
  const session = useSelector((state: RootState) => state.auth.session)
  if (!session) return <Navigate to="/login" replace />
  if (session.firstLogin) return <Navigate to="/first-access" replace />
  if (sysAdmin && session.role !== 'SysAdmin') return <Navigate to="/" replace />
  return <Shell>{children}</Shell>
}

export default function App() {
  const dispatch = useDispatch<AppDispatch>()
  const session = useSelector((state: RootState) => state.auth.session)
  useEffect(() => { const onLogout = () => dispatch(logout()); window.addEventListener('hermes:logout', onLogout); return () => window.removeEventListener('hermes:logout', onLogout) }, [dispatch])
  return <Routes>
    <Route path="/login" element={session ? <Navigate to={session.firstLogin ? '/first-access' : '/'} replace /> : <LoginPage />} />
    <Route path="/first-access" element={session ? <PasswordPage firstAccess /> : <Navigate to="/login" replace />} />
    <Route path="/" element={<Protected><DashboardPage /></Protected>} />
    <Route path="/tenants" element={<Protected><TenantsPage /></Protected>} />
    <Route path="/tenants/new" element={<Protected sysAdmin><TenantEditorPage /></Protected>} />
    <Route path="/tenants/:id/edit" element={<Protected><TenantEditorPage /></Protected>} />
    <Route path="/tenants/:id/subscription" element={<Protected sysAdmin><SubscriptionPage /></Protected>} />
    <Route path="/plans" element={<Protected sysAdmin><PlansPage /></Protected>} />
    <Route path="/plans/new" element={<Protected sysAdmin><PlanEditorPage /></Protected>} />
    <Route path="/plans/:id/edit" element={<Protected sysAdmin><PlanEditorPage /></Protected>} />
    <Route path="/users" element={<Protected><UsersPage /></Protected>} />
    <Route path="/users/new" element={<Protected><UserEditorPage /></Protected>} />
    <Route path="/users/:id/edit" element={<Protected><UserEditorPage /></Protected>} />
    <Route path="/settings" element={<Protected><SettingsPage /></Protected>} />
    <Route path="*" element={<Navigate to="/" replace />} />
  </Routes>
}
