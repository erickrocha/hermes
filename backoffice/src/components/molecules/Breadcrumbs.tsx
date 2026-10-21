import { Link, matchPath, useLocation } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { sectionLabels } from '../../features/navigation'

export function Breadcrumbs() {
  const { t } = useTranslation(); const { pathname } = useLocation()
  const routes = [
    { pattern: '/tenants/new', parent: '/tenants', parentLabel: 'tenants', label: 'newTenant' },
    { pattern: '/tenants/:uuid/edit', parent: '/tenants', parentLabel: 'tenants', label: 'editTenant' },
    { pattern: '/tenants/:uuid/manage', parent: '/tenants', parentLabel: 'tenants', label: 'manageTenant' },
    { pattern: '/tenants/:uuid/subscription', parent: '/tenants', parentLabel: 'tenants', label: 'subscription' },
    { pattern: '/plans/new', parent: '/plans', parentLabel: 'plans', label: 'newPlan' },
    { pattern: '/plans/:uuid/edit', parent: '/plans', parentLabel: 'plans', label: 'editPlan' },
    { pattern: '/users/new', parent: '/users', parentLabel: 'users', label: 'newUser' },
    { pattern: '/users/:uuid/edit', parent: '/users', parentLabel: 'users', label: 'editUser' },
    { pattern: '/non-executions/manual', parent: '/non-executions', parentLabel: 'nonExecutions', label: 'recordManualNonExecution' },
    { pattern: '/non-executions/:uuid/justify', parent: '/non-executions', parentLabel: 'nonExecutions', label: 'justifyNonExecution' },
    { pattern: '/garage/devices', parent: '/garage', parentLabel: 'garage', label: 'garageDevices' },
  ].find((route) => matchPath(route.pattern, pathname))
  const lists: Record<string, string> = { '/tenants': 'tenants', '/plans': 'plans', '/users': 'users', '/settings': 'settings', '/costs': 'costs', '/billing': 'billing', '/maintenance-notifications': 'maintenanceNotifications', ...sectionLabels }
  const current = routes?.label || lists[pathname]
  // As telas de recurso seguem /<seção>[/<uuid>][/<ação>]: a raiz vem do mapa de seções e cada ação do restante do caminho.
  const [root, ...rest] = pathname.split('/').filter(Boolean)
  const rootPath = `/${root}`
  const trail = !routes && !current && root && sectionLabels[rootPath] ? rest.map((segment, index) => ({ segment, last: index === rest.length - 1 })) : []
  const crumb = (segment: string) => (/^[0-9a-f-]{8,}$/i.test(segment) ? t('detail') : t(`crumb_${segment}`, { defaultValue: segment }))
  return <nav className="breadcrumbs" aria-label={t('breadcrumbs')}><ol><li>{pathname === '/home' ? <span aria-current="page">{t('dashboard')}</span> : <Link to="/home">{t('dashboard')}</Link>}</li>{routes?.parent && <li><Link to={routes.parent}>{t(routes.parentLabel)}</Link></li>}{current && pathname !== '/home' && <li><span aria-current="page">{t(current)}</span></li>}
    {trail.length > 0 && <li><Link to={rootPath}>{t(sectionLabels[rootPath])}</Link></li>}
    {trail.map(({ segment, last }, index) => <li key={`${segment}-${index}`}>{last ? <span aria-current="page">{crumb(segment)}</span> : <Link to={`${rootPath}/${rest.slice(0, index + 1).join('/')}`}>{crumb(segment)}</Link>}</li>)}</ol></nav>
}
