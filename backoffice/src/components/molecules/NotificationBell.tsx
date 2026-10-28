import { useEffect } from 'react'
import { Bell } from 'lucide-react'
import { Link, useLocation } from 'react-router-dom'
import { useDispatch, useSelector } from 'react-redux'
import { useTranslation } from 'react-i18next'
import type { AppDispatch, RootState } from '../../store'
import { can } from '../../features/access'
import { fetchUnreadCount } from '../../features/maintenanceNotificationSlice'
import type { Session } from '../../types'

export const canSeeMaintenanceNotifications = (session: Session) => can(session, 'maintenance-notification.read')

/// EPIC-MT-06 (TRM-616): o sino do cabeçalho; o contador é relido a cada navegação.
export function NotificationBell({ session }: { session: Session }) {
  const { t } = useTranslation()
  const dispatch = useDispatch<AppDispatch>()
  const { pathname } = useLocation()
  // Os testes de página montam stores mínimos, sem esta fatia.
  const count = useSelector((state: RootState) => state.maintenanceNotifications?.unreadCount ?? 0)
  const visible = canSeeMaintenanceNotifications(session)
  useEffect(() => { if (visible) void dispatch(fetchUnreadCount()) }, [dispatch, visible, pathname])
  if (!visible) return null
  const label = t('maintenanceNotificationsBell', { count })
  return <Link className="notification-bell" to="/maintenance-notifications" aria-label={label} title={label}>
    <Bell size={19} />{count > 0 && <span className="notification-count" data-testid="notification-count">{count > 99 ? '99+' : count}</span>}
  </Link>
}
