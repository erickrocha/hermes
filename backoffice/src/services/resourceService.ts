import { api } from '../api'
import type { Page } from '../types'

// Os serviços falam com o backend por axios; slices e componentes nunca chamam o axios direto.
export type Params = Record<string, string | number | boolean | undefined>
export type Method = 'post' | 'put' | 'delete'

export const listPage = async <T>(path: string, params: Params): Promise<Page<T>> => (await api.get<Page<T>>(path, { params })).data
export const read = async <T>(path: string, params?: Params): Promise<T> => (await api.get<T>(path, { params })).data
export const send = async <T>(method: Method, path: string, body?: unknown): Promise<T> =>
  method === 'delete' ? (await api.delete<T>(path)).data : (await api[method]<T>(path, body)).data

export async function downloadFile(path: string, params: Params): Promise<void> {
  const response = await api.get<Blob>(path, { params, responseType: 'blob' })
  const name = /filename="([^"]+)"/.exec(String(response.headers['content-disposition'] ?? ''))?.[1] ?? 'export.csv'
  const link = document.createElement('a')
  link.href = URL.createObjectURL(response.data)
  link.download = name
  link.click()
  URL.revokeObjectURL(link.href)
}
