import { api } from '../api'
import type { Page } from '../types'

/// C-026 EPIC-MT-06 (TRM-616): "a part this work order was waiting for has arrived".
export interface MaintenanceNotification {
  uuid: string
  workOrderUuid?: string | null
  partUuid?: string | null
  partName: string
  quantity: number
  read: boolean
  readAt?: string | null
  createdAt?: string | null
}

export const listNotifications = async (page: number, pageSize: number, unread: boolean): Promise<Page<MaintenanceNotification>> =>
  (await api.get<Page<MaintenanceNotification>>('/maintenance-notification', { params: { page, pageSize, ...(unread ? { unread: true } : {}) } })).data

export const unreadNotificationCount = async (): Promise<number> =>
  (await api.get<{ count: number }>('/maintenance-notification/unread-count')).data.count

export const markNotificationRead = async (uuid: string): Promise<MaintenanceNotification> =>
  (await api.put<MaintenanceNotification>(`/maintenance-notification/uuid/${uuid}/read`)).data

export const markAllNotificationsRead = async (): Promise<number> =>
  (await api.put<{ marked: number }>('/maintenance-notification/read-all')).data.marked
