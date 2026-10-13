import { api } from '../api'
import type { AccessProfile, EffectiveProfile, Page, Permission } from '../types'

export interface ProfileEditorData {
  catalogue: Permission[]
  profile: AccessProfile | null
}

export interface UserProfileData {
  profiles: AccessProfile[]
  current: EffectiveProfile
}

export const listProfiles = async (): Promise<AccessProfile[]> =>
  (await api.get<Page<AccessProfile>>('/access-profile', { params: { page: 0, pageSize: 200 } })).data.items

export const loadProfileEditor = async (uuid?: string): Promise<ProfileEditorData> => {
  const [catalogue, profile] = await Promise.all([
    api.get<Permission[]>('/access-profile/catalogue'),
    uuid ? api.get<AccessProfile>(`/access-profile/uuid/${uuid}`) : Promise.resolve(null),
  ])
  return { catalogue: catalogue.data, profile: profile?.data ?? null }
}

export const saveProfile = async ({ uuid, profile }: { uuid?: string; profile: AccessProfile }): Promise<void> => {
  if (uuid) await api.put(`/access-profile/uuid/${uuid}`, profile)
  else await api.post('/access-profile', profile)
}

export const loadUserProfile = async (userUuid: string): Promise<UserProfileData> => {
  const [profiles, current] = await Promise.all([
    api.get<Page<AccessProfile>>('/access-profile', { params: { page: 0, pageSize: 200 } }),
    api.get<EffectiveProfile>(`/access-profile/user/uuid/${userUuid}`),
  ])
  return { profiles: profiles.data.items, current: current.data }
}

export const updateUserProfile = async ({ userUuid, profileUuid }: { userUuid: string; profileUuid: string }): Promise<void> => {
  await api.put(`/access-profile/user/uuid/${userUuid}`, { profileUuid: profileUuid || null })
}