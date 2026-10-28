import { api } from '../api'
import type { AccessProfile, EffectiveProfile, Grant, Page, Permission } from '../types'

export interface ProfileEditorData {
  catalogue: Permission[]
  profile: AccessProfile | null
}

/** What a user form needs to hand out access: the profiles on offer and the catalogue for the extras. */
export interface AccessOptions {
  profiles: AccessProfile[]
  catalogue: Permission[]
}

export interface UserProfileData extends AccessOptions {
  current: EffectiveProfile
}

export interface AssignProfileRequest { userUuid: string; profileUuid: string; extraPermissions: Grant[] }

/// C-035: the server lists system profiles first, then the caller's own tenant's.
export const listProfiles = async (): Promise<AccessProfile[]> =>
  (await api.get<Page<AccessProfile>>('/access-profile', { params: { page: 0, pageSize: 200 } })).data.items

export const loadCatalogue = async (): Promise<Permission[]> => (await api.get<Permission[]>('/access-profile/catalogue')).data

export const loadProfileEditor = async (uuid?: string): Promise<ProfileEditorData> => {
  const [catalogue, profile] = await Promise.all([
    loadCatalogue(),
    uuid ? api.get<AccessProfile>(`/access-profile/uuid/${uuid}`) : Promise.resolve(null),
  ])
  return { catalogue, profile: profile?.data ?? null }
}

export const saveProfile = async ({ uuid, profile }: { uuid?: string; profile: AccessProfile }): Promise<void> => {
  if (uuid) await api.put(`/access-profile/uuid/${uuid}`, profile)
  else await api.post('/access-profile', profile)
}

export const loadAccessOptions = async (): Promise<AccessOptions> => {
  const [profiles, catalogue] = await Promise.all([listProfiles(), loadCatalogue()])
  return { profiles, catalogue }
}

export const loadUserProfile = async (userUuid: string): Promise<UserProfileData> => {
  const [options, current] = await Promise.all([loadAccessOptions(), api.get<EffectiveProfile>(`/access-profile/user/uuid/${userUuid}`)])
  return { ...options, current: current.data }
}

/// `PUT /access-profile/user/uuid/{uuid}` with `AssignProfileJson`: the profile is required and the extras replace the previous ones.
export const updateUserProfile = async ({ userUuid, profileUuid, extraPermissions }: AssignProfileRequest): Promise<void> => {
  await api.put(`/access-profile/user/uuid/${userUuid}`, { profileUuid, extraPermissions })
}
