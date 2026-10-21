import { publicApi } from '../api'
import type { PublicPlan } from '../types'

export const loadPublicPlans = async (): Promise<PublicPlan[]> =>
  (await publicApi.get<PublicPlan[]>('/public/business-plan')).data
