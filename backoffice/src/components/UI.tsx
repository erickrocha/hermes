import { AlertTriangle, LoaderCircle, Search, X } from 'lucide-react'
import { Link, matchPath, useLocation } from 'react-router-dom'
import type { FormEvent, ReactNode } from 'react'
import { useTranslation } from 'react-i18next'

export function PageHeader({ title, subtitle, actions }: { title: string; subtitle?: string; actions?: ReactNode }) { return <div className="page-header"><div><h1>{title}</h1>{subtitle && <p>{subtitle}</p>}</div><div className="page-actions">{actions}</div></div> }
export function Loading() { const { t } = useTranslation(); return <div className="state-box"><LoaderCircle className="spin" /><p>{t('loading')}</p></div> }
export function Empty() { const { t } = useTranslation(); return <div className="state-box"><Search /><p>{t('noResults')}</p></div> }
export function SearchBox({ value, onChange }: { value: string; onChange: (value: string) => void }) { const { t } = useTranslation(); return <label className="search-box"><Search size={17} /><input value={value} onChange={(e) => onChange(e.target.value)} placeholder={t('search')} /></label> }
export function Breadcrumbs() {
  const { t } = useTranslation(); const { pathname } = useLocation()
  const routes = [
    { pattern: '/tenants/new', parent: '/tenants', parentLabel: 'tenants', label: 'newTenant' },
    { pattern: '/tenants/:uuid/edit', parent: '/tenants', parentLabel: 'tenants', label: 'editTenant' },
    { pattern: '/tenants/:uuid/subscription', parent: '/tenants', parentLabel: 'tenants', label: 'subscription' },
    { pattern: '/plans/new', parent: '/plans', parentLabel: 'plans', label: 'newPlan' },
    { pattern: '/plans/:uuid/edit', parent: '/plans', parentLabel: 'plans', label: 'editPlan' },
    { pattern: '/users/new', parent: '/users', parentLabel: 'users', label: 'newUser' },
    { pattern: '/users/:uuid/edit', parent: '/users', parentLabel: 'users', label: 'editUser' },
  ].find((route) => matchPath(route.pattern, pathname))
  const lists: Record<string, string> = { '/tenants': 'tenants', '/plans': 'plans', '/users': 'users', '/settings': 'settings' }
  const current = routes?.label || lists[pathname]
  return <nav className="breadcrumbs" aria-label={t('breadcrumbs')}><ol><li>{pathname === '/' ? <span aria-current="page">{t('dashboard')}</span> : <Link to="/">{t('dashboard')}</Link>}</li>{routes?.parent && <li><Link to={routes.parent}>{t(routes.parentLabel)}</Link></li>}{current && pathname !== '/' && <li><span aria-current="page">{t(current)}</span></li>}</ol></nav>
}
export function Modal({ title, children, onClose, wide = false }: { title: string; children: ReactNode; onClose: () => void; wide?: boolean }) { const { t } = useTranslation(); return <div className="modal-backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}><section className={`modal ${wide ? 'wide' : ''}`} role="dialog" aria-modal="true" aria-label={title}><header><h2>{title}</h2><button onClick={onClose} aria-label={t('close')}><X /></button></header>{children}</section></div> }
export function Confirm({ title, text, onCancel, onConfirm }: { title: string; text: string; onCancel: () => void; onConfirm: () => void }) { const { t } = useTranslation(); return <Modal title={title} onClose={onCancel}><div className="confirm-body"><span className="warning-icon"><AlertTriangle /></span><p>{text}</p></div><div className="modal-actions"><button className="btn secondary" onClick={onCancel}>{t('cancel')}</button><button className="btn danger" onClick={onConfirm}>{t('confirm')}</button></div></Modal> }
export function Form({ children, onSubmit, error, saving, onCancel }: { children: ReactNode; onSubmit: (event: FormEvent) => void; error?: string; saving?: boolean; onCancel?: () => void }) { const { t } = useTranslation(); return <form onSubmit={onSubmit}><div className="form-grid">{children}</div>{error && <div className="form-error">{error}</div>}<div className="form-actions">{onCancel && <button type="button" className="btn secondary" onClick={onCancel}>{t('cancel')}</button>}<button className="btn primary" disabled={saving}>{saving ? t('saving') : t('save')}</button></div></form> }
