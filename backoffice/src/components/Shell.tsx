import { Fragment, useEffect, useState, type ReactNode } from 'react'
import { type LucideIcon, Building2, ChevronDown, Coins, CreditCard, Gauge, History, LogOut, Menu, Receipt, Settings, ShieldCheck, SlidersHorizontal, Users, X } from 'lucide-react'
import { NavLink, useNavigate } from 'react-router-dom'
import { useDispatch, useSelector } from 'react-redux'
import { useTranslation } from 'react-i18next'
import type { AppDispatch, RootState } from '../store'
import type { Session } from '../types'
import { loadCurrentTenant, loadPermissions, logout } from '../store'
import { Breadcrumbs } from './UI'
import { LanguagePicker } from './LanguagePicker'
import { ThemePicker } from './ThemePicker'
import { applyThemeByName, getStoredTheme } from '../theme'
import { NotificationBell } from './molecules/NotificationBell'
import { TargetTenantSelect } from './molecules/TargetTenantSelect'
import { operationNav } from '../features/navigation'
import { canSeeBilling, canSeeCosts, canSeeCrossTenantActions, canSeePlans, canSeeProfiles, canSeeReferenceData, canSeeTenants, canSeeUsers } from '../features/routeAccess'
import { isPlatform } from '../features/access'

/// The topbar names the held profile: a system profile is translated by key, a custom one shows its own name.
export const profileLabel = (session: Session, t: (key: string) => string) =>
  session.profileKey ? t(`profile${session.profileKey}`) : session.profileName || t(`scope${session.scope}`)

const Logo = () => <div className="brand"><span className="brand-mark"><ShieldCheck size={21} /></span><span><b>hermes</b><small>BACKOFFICE</small></span></div>

export function Shell({ children }: { children: ReactNode }) {
  const [mobile, setMobile] = useState(false)
  const [userMenu, setUserMenu] = useState(false)
  const [collapsedGroups, setCollapsedGroups] = useState<Record<string, boolean>>({})
  const { t } = useTranslation()
  const session = useSelector((s: RootState) => s.auth.session)!
  const tenant = useSelector((s: RootState) => s.data.currentTenant)
  const target = useSelector((s: RootState) => s.auth.target)
  const dispatch = useDispatch<AppDispatch>()
  const navigate = useNavigate()
  const tenantName = tenant?.companyName || tenant?.businessName
  // EPIC-BO-01-S02 (HRMS-408): the console takes the signed-in tenant's
  // theme. DEF-BO-02: the tenant is fetched here for its own sake instead of
  // being picked out of whatever tenant list some other screen happened to
  // load -- that made the identity appear only by accident, and not at all
  // once the dashboard stopped listing tenants (DEF-BO-07). A platform actor has no
  // tenant of their own, so they keep the vendor's default identity.
  useEffect(() => { if (session.tenantUuid) dispatch(loadCurrentTenant(session.tenantUuid)) }, [dispatch, session.tenantUuid])
  useEffect(() => { applyThemeByName(getStoredTheme()) }, [])
  useEffect(() => { dispatch(loadPermissions()) }, [dispatch, session.userId])
  const userLabel = session.name || session.email || t('user')
  const links: { to: string; label: string; icon: LucideIcon; group?: string }[] = [
    { to: '/home', label: t('dashboard'), icon: Gauge },
    ...(canSeeTenants(session) ? [{ to: '/tenants', label: t('tenants'), icon: Building2 }] : []),
    ...(canSeePlans(session) ? [{ to: '/plans', label: t('plans'), icon: CreditCard }] : []),
    ...(canSeeUsers(session) ? [{ to: '/users', label: t('users'), icon: Users }] : []),
    ...operationNav.filter((item) => item.visible(session)).map((item) => ({ to: item.to, label: t(item.labelKey), icon: item.icon, group: item.groupKey })),
    ...(canSeeCosts(session) ? [{ to: '/costs', label: t('costs'), icon: Coins }] : []),
    ...(canSeeBilling(session) ? [{ to: '/billing', label: t('billing'), icon: Receipt }] : []),
    ...(canSeeProfiles(session) ? [{ to: '/profiles', label: t('profiles'), icon: ShieldCheck }] : []),
    ...(canSeeCrossTenantActions(session) ? [{ to: '/cross-tenant-actions', label: t('crossTenantActions'), icon: History }] : []),
    // PD-027: dados de referência são globais da plataforma, como o catálogo
    // de planos -- só quem tem alcance de plataforma administra.
    ...(canSeeReferenceData(session) ? [{ to: '/system-settings', label: t('systemSettings'), icon: SlidersHorizontal }] : []),
  ]
  const leave = () => { dispatch(logout()); navigate('/login') }
  return <div className="app-shell">
    {mobile && <button className="sidebar-backdrop" aria-label={t('close')} onClick={() => setMobile(false)} />}
    <aside className={`sidebar ${mobile ? 'open' : ''}`}>
      <div className="sidebar-brand"><Logo /><button className="mobile-close" onClick={() => setMobile(false)}><X /></button></div>
      <nav>{links.map(({ to, label, icon: Icon, group }, index) => {
        const startsGroup = Boolean(group && group !== links[index - 1]?.group)
        const collapsed = group ? collapsedGroups[group] ?? false : false
        return <Fragment key={to}>
          {startsGroup && group && <button
            type="button"
            className="nav-group"
            aria-expanded={!collapsed}
            onClick={() => setCollapsedGroups((current) => ({ ...current, [group]: !current[group] }))}
          >
            <span>{t(group)}</span><ChevronDown size={15} aria-hidden="true" />
          </button>}
          {(!group || !collapsed) && <NavLink end={to === '/'} to={to} onClick={() => setMobile(false)}><Icon size={19} /><span>{label}</span></NavLink>}
        </Fragment>
      })}</nav>
      <div className="sidebar-foot"><span>Hermes</span><small>v0.1</small></div>
    </aside>
    <div className="app-main">
      <header className="topbar">
        <button className="menu-button" onClick={() => setMobile(true)} aria-label={t('menu')}><Menu /></button>
        <div className="tenant-context"><span>{tenant?.companyName || tenant?.businessName || (isPlatform(session) ? t('platform') : 'Hermes')}</span><small>{profileLabel(session, t)}</small></div>
        <div className="topbar-actions">
          <TargetTenantSelect />
          <NotificationBell session={session} />
          <ThemePicker />
          <LanguagePicker />
          <div className="user-menu-wrap">
            <button className="user-trigger" onClick={() => setUserMenu(!userMenu)}><span className="avatar">{userLabel.slice(0, 1).toUpperCase()}</span><span className="user-copy"><b>{userLabel}</b><small>{session.email || ''}</small></span><ChevronDown size={16} /></button>
            {userMenu && <div className="user-popover"><button onClick={() => { navigate('/settings'); setUserMenu(false) }}><Settings size={17} />{t('settings')}</button><button className="danger-text" onClick={leave}><LogOut size={17} />{t('logout')}</button></div>}
          </div>
        </div>
      </header>
      {target && <div className="acting-banner" role="status">{t('actingInTenant', { name: target.name })}</div>}
      <main className="content"><Breadcrumbs />{children}</main>
    </div>
  </div>
}
