import { createAsyncThunk, createSlice, isAnyOf } from '@reduxjs/toolkit'
import { apiMessage } from '../api'
import * as service from '../services/nonExecutionService'
import type { NonExecutionEvaluation, NonExecutionVehicleGroup } from '../services/nonExecutionService'
import { emptyPage } from '../types'
import type { Page } from '../types'

const failure = (cause: unknown) => apiMessage(cause, 'genericError')
type Config = { rejectValue: string }

export const fetchInbox = createAsyncThunk<Page<NonExecutionVehicleGroup>, { page: number; pageSize: number }, Config>(
  'nonExecutions/inbox', async ({ page, pageSize }, { rejectWithValue }) => {
    try { return await service.listInbox(page, pageSize) } catch (cause) { return rejectWithValue(apiMessage(cause, 'loadError')) }
  },
)
export const justifyOccurrence = createAsyncThunk<void, { uuid: string; justification: string }, Config>(
  'nonExecutions/justify', async ({ uuid, justification }, { rejectWithValue }) => {
    try { await service.justifyOccurrence(uuid, justification) } catch (cause) { return rejectWithValue(failure(cause)) }
  },
)
export const recordManualOccurrence = createAsyncThunk<void, { workOrderUuid: string; justification: string }, Config>(
  'nonExecutions/manual', async ({ workOrderUuid, justification }, { rejectWithValue }) => {
    try { await service.recordManual(workOrderUuid, justification) } catch (cause) { return rejectWithValue(failure(cause)) }
  },
)
export const evaluateNow = createAsyncThunk<NonExecutionEvaluation, void, Config>(
  'nonExecutions/evaluate', async (_, { dispatch, rejectWithValue }) => {
    try {
      const outcome = await service.evaluateNow()
      void dispatch(fetchInbox({ page: 0, pageSize: 25 }))
      return outcome
    } catch (cause) { return rejectWithValue(failure(cause)) }
  },
)

const initialState = {
  page: emptyPage<NonExecutionVehicleGroup>(), loading: false, saving: false, saved: false, evaluating: false,
  error: '', evaluation: null as NonExecutionEvaluation | null,
}
const nonExecutionSlice = createSlice({
  name: 'nonExecutions', initialState,
  reducers: { resetForm: (state) => { state.saving = false; state.saved = false; state.error = ''; state.evaluation = null } },
  extraReducers: (builder) => builder
    .addCase(fetchInbox.pending, (state) => { state.loading = true; state.error = '' })
    .addCase(fetchInbox.fulfilled, (state, action) => { state.loading = false; state.page = action.payload })
    .addCase(fetchInbox.rejected, (state, action) => { state.loading = false; state.error = action.payload || 'loadError' })
    .addCase(evaluateNow.pending, (state) => { state.evaluating = true; state.error = ''; state.evaluation = null })
    .addCase(evaluateNow.fulfilled, (state, action) => { state.evaluating = false; state.evaluation = action.payload })
    .addCase(evaluateNow.rejected, (state, action) => { state.evaluating = false; state.error = action.payload || 'genericError' })
    .addMatcher(isAnyOf(justifyOccurrence.pending, recordManualOccurrence.pending), (state) => { state.saving = true; state.saved = false; state.error = '' })
    .addMatcher(isAnyOf(justifyOccurrence.fulfilled, recordManualOccurrence.fulfilled), (state) => { state.saving = false; state.saved = true })
    .addMatcher(isAnyOf(justifyOccurrence.rejected, recordManualOccurrence.rejected), (state, action) => { state.saving = false; state.error = action.payload || 'genericError' }),
})

export const { resetForm } = nonExecutionSlice.actions
export const nonExecutionReducer = nonExecutionSlice.reducer
