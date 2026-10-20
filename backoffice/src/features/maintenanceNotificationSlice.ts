import { createAsyncThunk, createSlice, isAnyOf } from '@reduxjs/toolkit'
import { apiMessage } from '../api'
import * as service from '../services/maintenanceNotificationService'
import type { MaintenanceNotification } from '../services/maintenanceNotificationService'
import { emptyPage } from '../types'
import type { Page } from '../types'

const failure = (cause: unknown) => apiMessage(cause, '')
type Config = { rejectValue: string }

export const fetchNotifications = createAsyncThunk<Page<MaintenanceNotification>, { page: number; pageSize: number; unread: boolean }, Config>(
  'maintenanceNotifications/list', async ({ page, pageSize, unread }, { rejectWithValue }) => {
    try { return await service.listNotifications(page, pageSize, unread) } catch (cause) { return rejectWithValue(failure(cause)) }
  },
)
/// O contador do sino: falha em silêncio (o sino só some do número), nunca derruba a tela.
export const fetchUnreadCount = createAsyncThunk<number, void, Config>(
  'maintenanceNotifications/count', async (_, { rejectWithValue }) => {
    try { return await service.unreadNotificationCount() } catch (cause) { return rejectWithValue(failure(cause)) }
  },
)
export const markNotificationRead = createAsyncThunk<MaintenanceNotification, string, Config>(
  'maintenanceNotifications/read', async (uuid, { dispatch, rejectWithValue }) => {
    try {
      const saved = await service.markNotificationRead(uuid)
      void dispatch(fetchUnreadCount())
      return saved
    } catch (cause) { return rejectWithValue(failure(cause)) }
  },
)
export const markAllNotificationsRead = createAsyncThunk<number, void, Config>(
  'maintenanceNotifications/readAll', async (_, { dispatch, rejectWithValue }) => {
    try {
      const marked = await service.markAllNotificationsRead()
      void dispatch(fetchUnreadCount())
      return marked
    } catch (cause) { return rejectWithValue(failure(cause)) }
  },
)

const initialState = { page: emptyPage<MaintenanceNotification>(), unreadCount: 0, loading: false, saving: false, error: '' }
const maintenanceNotificationSlice = createSlice({
  name: 'maintenanceNotifications', initialState, reducers: {},
  extraReducers: (builder) => builder
    .addCase(fetchNotifications.pending, (state) => { state.loading = true; state.error = '' })
    .addCase(fetchNotifications.fulfilled, (state, action) => { state.loading = false; state.page = action.payload })
    .addCase(fetchNotifications.rejected, (state, action) => { state.loading = false; state.error = action.payload || 'loadError' })
    .addCase(fetchUnreadCount.fulfilled, (state, action) => { state.unreadCount = action.payload })
    .addCase(markNotificationRead.fulfilled, (state, action) => {
      const row = state.page.items.find((item) => item.uuid === action.payload.uuid)
      if (row) Object.assign(row, action.payload)
    })
    .addMatcher(isAnyOf(markNotificationRead.pending, markAllNotificationsRead.pending), (state) => { state.saving = true; state.error = '' })
    .addMatcher(isAnyOf(markNotificationRead.fulfilled, markAllNotificationsRead.fulfilled), (state) => { state.saving = false })
    .addMatcher(isAnyOf(markNotificationRead.rejected, markAllNotificationsRead.rejected), (state, action) => { state.saving = false; state.error = action.payload || 'genericError' }),
})

export const maintenanceNotificationReducer = maintenanceNotificationSlice.reducer
