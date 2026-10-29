import { api } from '../api'
import type { CrossTenantAction, Page, PageRequest } from '../types'

/// C-036 (`HRMS-877`): what a tenant did inside its descendants, and what was done in it.
export const fetchCrossTenantActions = async ({ page, pageSize }: PageRequest): Promise<Page<CrossTenantAction>> =>
  (await api.get<Page<CrossTenantAction>>('/cross-tenant-action', { params: { page, pageSize } })).data
