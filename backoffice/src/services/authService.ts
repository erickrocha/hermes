import { api, normalizeSession, publicApi } from '../api'
import type { EffectiveProfile, Session } from '../types'

export const login = async (credentials: { email: string; password: string }): Promise<Session> => {
  const body = new URLSearchParams(credentials)
  const { data } = await api.post('/login', body, { headers: { 'Content-Type': 'application/x-www-form-urlencoded' } })
  return normalizeSession(data) as Session
}

/// `GET /access-profile/mine`: the effective grants, refreshed after a profile or extras change took effect server-side.
export const loadPermissions = async (): Promise<EffectiveProfile> =>
  (await api.get<EffectiveProfile>('/access-profile/mine')).data

export const changePassword = async (payload: { currentPassword: string; newPassword: string }): Promise<void> => {
  await api.put('/user/change-password', payload)
}

export const acceptInvitation = async (payload: { token: string; newPassword: string }): Promise<void> => {
  await publicApi.post('/accept-invite', payload)
}