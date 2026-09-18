import axios, { AxiosError, type InternalAxiosRequestConfig } from 'axios'

const baseURL = import.meta.env.VITE_API_URL || 'http://localhost:8080'
export const api = axios.create({ baseURL, headers: { 'Content-Type': 'application/json' } })

const clearSession = () => { localStorage.removeItem('hermes.session'); window.dispatchEvent(new Event('hermes:logout')) }
api.interceptors.request.use((config: InternalAxiosRequestConfig) => {
  const raw = localStorage.getItem('hermes.session')
  if (raw) {
    const session = JSON.parse(raw)
    if (session.accessToken) config.headers.Authorization = `Bearer ${session.accessToken}`
  }
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
  role: data.role, tenantId: data.tenantId ?? data.tenant_id ?? null,
})

export const apiMessage = (error: unknown, fallback: string) => {
  if (axios.isAxiosError(error)) return error.response?.data?.message || fallback
  return fallback
}
