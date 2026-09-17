import { useEffect, useState, type ReactNode } from 'react'
import { Building2, ChevronDown, CreditCard, Gauge, Languages, LogOut, Menu, Settings, ShieldCheck, Users, X } from 'lucide-react'
import { NavLink, useNavigate } from 'react-router-dom'
import { useDispatch, useSelector } from 'react-redux'
import { useTranslation } from 'react-i18next'
import type { AppDispatch, RootState } from '../store'
import { logout } from '../store'
import { applyTheme, themeForTenant } from '../theme'
import { Breadcrumbs } from './UI'

const Logo = () => <div className="brand"><span className="brand-mark"><ShieldCheck size={21} /></span><span><b>hermes</b><small>BACKOFFICE</small></span></div>

export function Shell({ children }: { children: ReactNode }) {
  const [mobile, setMobile] = useState(false)
  const [userMenu, setUserMenu] = useState(false)
  const { t, i18n } = useTranslation()
  const session = useSelector((s: RootState) => s.auth.session)!
  const tenants = useSelector((s: RootState) => s.data.tenants)
  const dispatch = useDispatch<AppDispatch>()
  const navigate = useNavigate()
  const tenant = tenants.find((item) => item.id === session.tenantId)
  const tenantName = tenant?.companyName || tenant?.businessName
  // EPIC-BO-01-S02 (HRMS-408): the console takes the signed-in tenant's
  // theme, derived here rather than at App-level because this is where the
  // tenant record (loaded elsewhere into the store) is actually resolved.
  useEffect(() => { applyTheme(themeForTenant(tenantName)) }, [tenantName])
  const userLabel = session.name || session.email || t('user')
  const links = [
    { to: '/', label: t('dashboard'), icon: Gauge },
    { to: '/tenants', label: t('tenants'), icon: Building2 },
    ...(session.role === 'SysAdmin' ? [{ to: '/plans', label: t('plans'), icon: CreditCard }] : []),
    { to: '/users', label: t('users'), icon: Users },
  ]
  const leave = () => { dispatch(logout()); navigate('/login') }
  return <div className="app-shell">
    {mobile && <button className="sidebar-backdrop" aria-label={t('close')} onClick={() => setMobile(false)} />}
    <aside className={`sidebar ${mobile ? 'open' : ''}`}>
      <div className="sidebar-brand"><Logo /><button className="mobile-close" onClick={() => setMobile(false)}><X /></button></div>
      <nav>{links.map(({ to, label, icon: Icon }) => <NavLink end={to === '/'} key={to} to={to} onClick={() => setMobile(false)}><Icon size={19} /><span>{label}</span></NavLink>)}</nav>
      <div className="sidebar-foot"><span>Hermes</span><small>v0.1</small></div>
    </aside>
    <div className="app-main">
      <header className="topbar">
        <button className="menu-button" onClick={() => setMobile(true)} aria-label={t('menu')}><Menu /></button>
        <div className="tenant-context"><span>{tenant?.companyName || tenant?.businessName || (session.role === 'SysAdmin' ? 'SysAdmin' : 'Hermes')}</span><small>{session.role}</small></div>
        <div className="topbar-actions">
          <button className="language-button" onClick={() => i18n.changeLanguage(i18n.language.startsWith('pt') ? 'en' : 'pt-BR')} title={t('language')}><Languages size={18} /><span>{i18n.language.startsWith('pt') ? 'PT' : 'EN'}</span></button>
          <div className="user-menu-wrap">
            <button className="user-trigger" onClick={() => setUserMenu(!userMenu)}><span className="avatar">{userLabel.slice(0, 1).toUpperCase()}</span><span className="user-copy"><b>{userLabel}</b><small>{session.email || ''}</small></span><ChevronDown size={16} /></button>
            {userMenu && <div className="user-popover"><button onClick={() => { navigate('/settings'); setUserMenu(false) }}><Settings size={17} />{t('settings')}</button><button className="danger-text" onClick={leave}><LogOut size={17} />{t('logout')}</button></div>}
          </div>
        </div>
      </header>
      <main className="content"><Breadcrumbs />{children}</main>
    </div>
  </div>
}
