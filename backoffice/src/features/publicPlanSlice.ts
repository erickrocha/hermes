import { createAsyncThunk, createSlice } from '@reduxjs/toolkit'
import { apiMessage } from '../api'
import * as publicPlanService from '../services/publicPlanService'
import type { PublicPlan } from '../types'

export const fetchPublicPlans = createAsyncThunk<PublicPlan[], void, { rejectValue: string }>(
  'publicPlans/fetch', async (_, { rejectWithValue }) => {
    try { return await publicPlanService.loadPublicPlans() } catch (cause) { return rejectWithValue(apiMessage(cause, '')) }
  },
)

type PublicPlanState = { items: PublicPlan[]; loading: boolean; error: string }
const initialState: PublicPlanState = { items: [], loading: false, error: '' }

const slice = createSlice({
  name: 'publicPlans', initialState, reducers: {},
  extraReducers: (builder) => builder
    .addCase(fetchPublicPlans.pending, (state) => { state.loading = true; state.error = '' })
    .addCase(fetchPublicPlans.fulfilled, (state, action) => { state.loading = false; state.items = action.payload })
    .addCase(fetchPublicPlans.rejected, (state, action) => { state.loading = false; state.error = action.payload || 'error' }),
})

export const publicPlanReducer = slice.reducer
