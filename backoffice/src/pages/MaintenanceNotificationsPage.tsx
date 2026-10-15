import { useCallback, useEffect, useState } from 'react'
import { BellRing, CheckCheck, Check } from 'lucide-react'
import { Link } from 'react-router-dom'
import { useDispatch, useSelector } from 'react-redux'
import { useTranslation } from 'react-i18next'
import { PageHeader } from '../components/UI'
import { Pagination } from '../components/DataTable'
import { StateView } from '../components/molecules/StateView'
import type { AppDispatch, RootState } from '../store'
import { fetchNotifications, markAllNotificationsRead, markNotificationRead } from '../features/maintenanceNotificationSlice'

const PAGE_SIZE = 25

/// EPIC-MT-06 (TRM-616): a caixa de avisos "a peça que você aguardava chegou".
export function MaintenanceNotificationsPage() {
  const { t } = useTranslation()
  const dispatch = useDispatch<AppDispatch>()
  const { page, loading, saving, error } = useSelector((state: RootState) => state.maintenanceNotifications)
  const [unreadOnly, setUnreadOnly] = useState(false)
  const [index, setIndex] = useState(0)

  const load = useCallback(() => dispatch(fetchNotifications({ page: index, pageSize: PAGE_SIZE, unread: unreadOnly })), [dispatch, index, unreadOnly])
  useEffect(() => { void load() }, [load])

  const markRead = async (uuid: string) => { if (markNotificationRead.fulfilled.match(await dispatch(markNotificationRead(uuid))) && unreadOnly) void load() }
  const markAll = async () => { if (markAllNotificationsRead.fulfilled.match(await dispatch(markAllNotificationsRead()))) void load() }
  const unreadOnPage = page.items.some((item) => !item.read)

  return <>
    <PageHeader title={t('maintenanceNotifications')} subtitle={t('maintenanceNotificationsText')} actions={<>
      <button type="button" className={`btn ${unreadOnly ? 'primary' : 'secondary'}`} aria-pressed={unreadOnly} onClick={() => { setIndex(0); setUnreadOnly(!unreadOnly) }}>{t('onlyUnread')}</button>
      <button type="button" className="btn secondary" disabled={saving || !unreadOnPage} onClick={() => void markAll()}><CheckCheck size={16} />{t('markAllRead')}</button>
    </>} />
    <StateView loading={loading && !page.items.length} error={error} empty={!loading && !error && !page.items.length}>
      <ul className="notification-list">{page.items.map((item) => <li key={item.uuid} className={`notification-item ${item.read ? 'read' : 'unread'}`}>
        <span className="notification-icon"><BellRing size={18} /></span>
        <div className="notification-body">
          <b>{t('partArrived', { part: item.partName })}</b>
          <small>{t('partArrivedDetail', { quantity: item.quantity })}{item.createdAt ? ` · ${new Date(item.createdAt).toLocaleString()}` : ''}</small>
          {item.workOrderUuid && <Link to={`/work-orders/${item.workOrderUuid}`}>{t('openWorkOrder')}</Link>}
        </div>
        {item.read
          ? <span className="badge gray">{t('noticeRead')}</span>
          : <button type="button" className="btn tertiary" disabled={saving} onClick={() => void markRead(item.uuid)}><Check size={15} />{t('markRead')}</button>}
      </li>)}</ul>
      <Pagination page={page} onPageChange={(request) => setIndex(request.page)} />
    </StateView>
  </>
}
