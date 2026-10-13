import { api } from '../api'
import type { Params } from './resourceService'
import type { Page } from '../types'

export type ReferenceRow = Record<string, string | number | null | undefined>

export const listReferenceRows = async ({ path, params }: { path: string; params: Params }): Promise<Page<ReferenceRow>> =>
  (await api.get<Page<ReferenceRow>>(path, { params })).data

export const saveReferenceRow = async ({ path, row }: { path: string; row: ReferenceRow }): Promise<void> => {
  await api.post(path, row)
}

export const importReferenceRows = async ({ path, rows }: { path: string; rows: ReferenceRow[] }): Promise<{ created: number; updated: number }> =>
  (await api.post<{ created: number; updated: number }>(path, rows)).data