import { Building2, CreditCard, Users, ArrowUpRight, ShieldCheck } from 'lucide-react'
import { useEffect } from 'react'
import { useDispatch, useSelector } from 'react-redux'
import { Link } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import type { AppDispatch, RootState } from '../store'
import { loadPlans, loadTenants, loadUsers } from '../store'
import { PageHeader } from '../components/UI'

export function DashboardPage() {
  const { t } = useTranslation(); const dispatch = useDispatch<AppDispatch>(); const { tenants, plans, users } = useSelector((s: RootState) => s.data); const session = useSelector((s: RootState) => s.auth.session)!
  useEffect(() => { dispatch(loadTenants()); dispatch(loadUsers()); if (session.role === 'SysAdmin') dispatch(loadPlans()) }, [dispatch, session.role])
  const cards = [{ label: t('activeTenants'), value: tenants.length, icon: Building2, color: 'blue', to: '/tenants' }, { label: t('activeUsers'), value: users.filter((u) => u.enabled).length, icon: Users, color: 'green', to: '/users' }, ...(session.role === 'SysAdmin' ? [{ label: t('availablePlans'), value: plans.length, icon: CreditCard, color: 'yellow', to: '/plans' }] : [])]
  return <><PageHeader title={t('welcome', { name: session.name?.split(' ')[0] || session.email })} subtitle={session.role === 'SysAdmin' ? t('sysSummary') : t('ownerSummary')} /><div className="metric-grid">{cards.map(({ label, value, icon: Icon, color, to }) => <Link to={to} className="metric-card" key={label}><span className={`metric-icon ${color}`}><Icon /></span><div><small>{label}</small><strong>{value}</strong></div><ArrowUpRight className="metric-arrow" /></Link>)}</div><section className="welcome-panel"><div><span className="eyebrow"><ShieldCheck size={14} /> {t('controlCenter')}</span><h2>{session.role === 'SysAdmin' ? t('sysPanelTitle') : t('ownerPanelTitle')}</h2><p>{session.role === 'SysAdmin' ? t('sysPanelText') : t('ownerPanelText')}</p></div><div className="pulse-visual"><span /><span /><span /><ShieldCheck /></div></section></>
}
