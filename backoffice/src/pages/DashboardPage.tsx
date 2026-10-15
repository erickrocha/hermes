import { Building2, CreditCard, Users, ArrowUpRight, ShieldCheck } from 'lucide-react'
import { useEffect } from 'react'
import { useDispatch, useSelector } from 'react-redux'
import { Link } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import type { AppDispatch, RootState } from '../store'
import { loadCounts } from '../store'
import { PageHeader } from '../components/UI'

export function DashboardPage() {
  const { t } = useTranslation(); const dispatch = useDispatch<AppDispatch>(); const counts = useSelector((s: RootState) => s.data.counts); const session = useSelector((s: RootState) => s.auth.session)!
  // PD-028: os cartões são contagens da tabela inteira, então pedem a página
  // mínima e leem `totalItems` -- contar as linhas de uma página daria "25".
  // DEF-BO-07: essa contagem vai para um estado próprio (`data.counts`); antes
  // ela sobrescrevia a mesma página que as telas de lista usam, e a Tenants
  // passava a exibir "1-1 de 29 - Página 1 de 29".
  useEffect(() => { dispatch(loadCounts(session.role === 'SysAdmin')) }, [dispatch, session.role])
  const cards = [{ label: t('activeTenants'), value: counts.tenants, icon: Building2, color: 'blue', to: '/tenants' }, { label: t('users'), value: counts.users, icon: Users, color: 'green', to: '/users' }, ...(session.role === 'SysAdmin' ? [{ label: t('availablePlans'), value: counts.plans, icon: CreditCard, color: 'yellow', to: '/plans' }] : [])]
  return <><PageHeader title={t('welcome', { name: session.name?.split(' ')[0] || session.email })} subtitle={session.role === 'SysAdmin' ? t('sysSummary') : t('ownerSummary')} /><div className="metric-grid">{cards.map(({ label, value, icon: Icon, color, to }) => <Link to={to} className="metric-card" key={label}><span className={`metric-icon ${color}`}><Icon /></span><div><small>{label}</small><strong>{value}</strong></div><ArrowUpRight className="metric-arrow" /></Link>)}</div><section className="welcome-panel"><div><span className="eyebrow"><ShieldCheck size={14} /> {t('controlCenter')}</span><h2>{session.role === 'SysAdmin' ? t('sysPanelTitle') : t('ownerPanelTitle')}</h2><p>{session.role === 'SysAdmin' ? t('sysPanelText') : t('ownerPanelText')}</p></div><div className="pulse-visual"><span /><span /><span /><ShieldCheck /></div></section></>
}
