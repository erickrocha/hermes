import { AlertTriangle, ArrowUpRight, Bell, Building2, Bus, CreditCard, Package, ShieldCheck, Siren, Users, Wrench, type LucideIcon } from 'lucide-react'
import { useEffect } from 'react'
import { useDispatch, useSelector } from 'react-redux'
import { Link } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { canSeeMaintenanceNotifications } from '../components/molecules/NotificationBell'
import { PageHeader } from '../components/UI'
import { can, isPlatform } from '../features/access'
import { canSeeBilling, canSeeNonExecutions, canSeeUsers, canSeeWorkOrders } from '../features/routeAccess'
import { emptyTenantDashboard, type TenantDashboard } from '../services/dataService'
import type { AppDispatch, RootState } from '../store'
import { loadCounts, loadTenantDashboard } from '../store'
import type { Session } from '../types'

type Card = { label: string; value: number; icon: LucideIcon; color: string; to?: string }

export function DashboardPage() {
  const { t } = useTranslation()
  const dispatch = useDispatch<AppDispatch>()
  const counts = useSelector((s: RootState) => s.data.counts)
  // The access-surface tests stub the data reducer, so this slice may be absent.
  const dashboard = useSelector((s: RootState) => s.data.tenantDashboard) ?? emptyTenantDashboard()
  const session = useSelector((s: RootState) => s.auth.session)!
  const platform = isPlatform(session)
  // DEF-BO-07: platform counts live in `data.counts`, never in the list pages.
  // A tenant member never asks for those totals — `/tenant` is already scoped
  // to their own row, so "registered tenants" was always 1 and not useful.
  useEffect(() => {
    if (platform) dispatch(loadCounts(true))
    else dispatch(loadTenantDashboard())
  }, [dispatch, platform])

  const cards: Card[] = platform
    ? [
        { label: t('activeTenants'), value: counts?.tenants ?? 0, icon: Building2, color: 'blue', to: '/tenants' },
        { label: t('users'), value: counts?.users ?? 0, icon: Users, color: 'green', to: '/users' },
        { label: t('availablePlans'), value: counts?.plans ?? 0, icon: CreditCard, color: 'yellow', to: '/plans' },
      ]
    : tenantCards(t, session, dashboard)

  return <>
    <PageHeader title={t('welcome', { name: session.name?.split(' ')[0] || session.email })} subtitle={platform ? t('sysSummary') : t('ownerSummary')} />
    <div className="metric-grid">{cards.map((card) => <Metric key={card.label} card={card} />)}</div>
    <section className="welcome-panel"><div><span className="eyebrow"><ShieldCheck size={14} /> {t('controlCenter')}</span><h2>{platform ? t('sysPanelTitle') : t('ownerPanelTitle')}</h2><p>{platform ? t('sysPanelText') : t('ownerPanelText')}</p></div><div className="pulse-visual"><span /><span /><span /><ShieldCheck /></div></section>
  </>
}

function Metric({ card }: { card: Card }) {
  const Icon = card.icon
  const body = <><span className={`metric-icon ${card.color}`}><Icon /></span><div><small>{card.label}</small><strong>{card.value}</strong></div>{card.to && <ArrowUpRight className="metric-arrow" />}</>
  return card.to
    ? <Link to={card.to} className="metric-card">{body}</Link>
    : <div className="metric-card">{body}</div>
}

function tenantCards(t: (key: string) => string, session: Session, dashboard: TenantDashboard): Card[] {
  const vehicles = can(session, 'vehicle.read') ? '/vehicles' : undefined
  const workOrders = canSeeWorkOrders(session) ? '/work-orders' : undefined
  const cards: Card[] = [
    { label: t('dashActiveVehicles'), value: dashboard.activeVehicles ?? 0, icon: Bus, color: 'blue', to: vehicles },
    { label: t('dashInMaintenance'), value: dashboard.vehiclesInMaintenance ?? 0, icon: Wrench, color: 'yellow', to: vehicles },
    { label: t('dashOpenWorkOrders'), value: dashboard.openWorkOrders ?? 0, icon: Wrench, color: 'green', to: workOrders },
    { label: t('dashAwaitingParts'), value: dashboard.workOrdersAwaitingParts ?? 0, icon: Package, color: 'yellow', to: workOrders },
    { label: t('yardQueue'), value: dashboard.yardQueue ?? 0, icon: Siren, color: 'blue', to: can(session, 'garage.queue.read') ? '/garage' : undefined },
    { label: t('dashUnreadNotices'), value: dashboard.unreadMaintenanceNotices ?? 0, icon: Bell, color: 'yellow', to: canSeeMaintenanceNotifications(session) ? '/maintenance-notifications' : undefined },
    { label: t('dashUnjustified'), value: dashboard.unjustifiedNonExecutions ?? 0, icon: AlertTriangle, color: 'yellow', to: canSeeNonExecutions(session) ? '/non-executions' : undefined },
    { label: t('dashEnabledUsers'), value: dashboard.enabledUsers ?? 0, icon: Users, color: 'green', to: canSeeUsers(session) ? '/users' : undefined },
  ]
  if (dashboard.overdueCharges !== undefined && canSeeBilling(session)) {
    cards.push({ label: t('dashOverdueCharges'), value: dashboard.overdueCharges, icon: CreditCard, color: 'yellow', to: '/billing' })
  }
  return cards
}
