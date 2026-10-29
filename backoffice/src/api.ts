import axios, { AxiosError, type InternalAxiosRequestConfig } from 'axios'

const baseURL = import.meta.env.VITE_API_URL || 'http://localhost:8080'
export const api = axios.create({ baseURL, headers: { 'Content-Type': 'application/json' } })
export const publicApi = axios.create({ baseURL, headers: { 'Content-Type': 'application/json' } })

publicApi.interceptors.request.use((config: InternalAxiosRequestConfig) => {
  config.headers['Accept-Language'] = localStorage.getItem('i18nextLng') || 'pt-BR'
  return config
})

export const TARGET_KEY = 'hermes.target'
/// The descendant tenant the console acts in, or `null` for the signed-in tenant. Browser storage can be empty or unreadable.
export const readTarget = (): { uuid: string; name: string } | null => {
  try {
    const raw = localStorage.getItem(TARGET_KEY)
    const parsed = raw ? JSON.parse(raw) : null
    return parsed && typeof parsed.uuid === 'string' ? { uuid: parsed.uuid, name: String(parsed.name ?? '') } : null
  } catch { return null }
}

const clearSession = () => { localStorage.removeItem('hermes.session'); window.dispatchEvent(new Event('hermes:logout')) }
api.interceptors.request.use((config: InternalAxiosRequestConfig) => {
  const raw = localStorage.getItem('hermes.session')
  if (raw) {
    const session = JSON.parse(raw)
    if (session.accessToken) config.headers.Authorization = `Bearer ${session.accessToken}`
  }
  // C-036 (`DS-1`): while the console acts inside a descendant tenant, every request names it.
  const target = readTarget()
  if (target) config.headers['X-Tenant-Uuid'] = target.uuid
  config.headers['Accept-Language'] = localStorage.getItem('i18nextLng') || 'pt-BR'
  return config
})

let refreshing: Promise<string> | null = null
api.interceptors.response.use((response) => response, async (error: AxiosError) => {
  const original = error.config as (InternalAxiosRequestConfig & { _retry?: boolean }) | undefined
  if (error.response?.status !== 401 || !original || original._retry || original.url?.includes('/login') || original.url?.includes('/refresh')) throw error
  const raw = localStorage.getItem('hermes.session')
  const session = raw ? JSON.parse(raw) : null
  if (!session?.refreshToken) { clearSession(); throw error }
  original._retry = true
  refreshing ??= axios.post(`${baseURL}/refresh`, { refreshToken: session.refreshToken }).then(({ data }) => {
    const next = normalizeSession(data)
    localStorage.setItem('hermes.session', JSON.stringify(next))
    return next.accessToken
  }).finally(() => { refreshing = null })
  try {
    const token = await refreshing
    original.headers.Authorization = `Bearer ${token}`
    return api(original)
  } catch (refreshError) { clearSession(); throw refreshError }
})

export const normalizeSession = (data: Record<string, unknown>) => ({
  accessToken: String(data.accessToken ?? data.access_token ?? ''), refreshToken: data.refreshToken ?? data.refresh_token,
  tokenType: String(data.tokenType ?? data.token_type ?? 'Bearer'), expireIn: Number(data.expireIn ?? data.expire_in ?? 0),
  email: String(data.email ?? ''), uuid: String(data.uuid ?? ''), name: String(data.name ?? ''), userId: Number(data.userId ?? data.user_id),
  // C-035: the session's reach and effective grants travel with the token; a stored session from before
  // the change has neither and is treated as a tenant session with no grants until it re-logs.
  scope: data.scope === 'Platform' ? 'Platform' : 'Tenant',
  profileKey: typeof data.profileKey === 'string' ? data.profileKey : undefined,
  profileName: typeof data.profileName === 'string' ? data.profileName : undefined,
  permissions: Array.isArray(data.permissions) ? data.permissions.map(String) : [],
  tenantId: data.tenantId ?? data.tenant_id ?? null,
  // HRMS-204/OBS-TP-05: the session tenant's public identifier. This whitelist
  // is the only thing that survives into the stored session, so a field missing
  // here is silently dropped on reload.
  tenantUuid: data.tenantUuid ?? data.tenant_uuid ?? null,
  // C-036: who may act in a descendant is known from the account's own grants, so it is set at sign-in.
  // A stored session keeps its own answer: inside a descendant the permissions are the delegated ones, which lack it.
  canTarget: typeof data.canTarget === 'boolean' ? data.canTarget : Array.isArray(data.permissions) && data.permissions.map(String).includes('tenant.child.data.read'),
})

export const apiMessage = (error: unknown, fallback: string) => {
  if (axios.isAxiosError(error)) return error.response?.data?.message || fallback
  return fallback
}

/// The server's stable `errorKey` (`DuplicateCustomerTaxId`…), for a page that maps a failure to a field.
export const apiErrorKey = (error: unknown): string | undefined => {
  if (axios.isAxiosError(error)) { const key = error.response?.data?.errorKey; return typeof key === 'string' ? key : undefined }
  return undefined
}
