import { api, normalizeSession, publicApi } from '../api'
import type { Session } from '../types'

export const login = async (credentials: { email: string; password: string }): Promise<Session> => {
  const body = new URLSearchParams(credentials)
  const { data } = await api.post('/login', body, { headers: { 'Content-Type': 'application/x-www-form-urlencoded' } })
  return normalizeSession(data) as Session
}

export const loadPermissions = async (): Promise<string[]> =>
  (await api.get<{ permissions: string[] }>('/access-profile/mine')).data.permissions

export const changePassword = async (payload: { currentPassword: string; newPassword: string }): Promise<void> => {
  await api.put('/user/change-password', payload)
}

export const acceptInvitation = async (payload: { token: string; newPassword: string }): Promise<void> => {
  await publicApi.post('/accept-invite', payload)
}