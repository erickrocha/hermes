import { createAsyncThunk, createSlice, isAnyOf } from '@reduxjs/toolkit'
import { apiMessage } from '../api'
import * as profileService from '../services/profileService'
import type { ProfileEditorData, UserProfileData } from '../services/profileService'
import type { AccessProfile } from '../types'

const failure = (cause: unknown) => apiMessage(cause, '')

export const fetchProfiles = createAsyncThunk<AccessProfile[], void, { rejectValue: string }>(
  'profiles/list', async (_, { rejectWithValue }) => {
    try { return await profileService.listProfiles() } catch (cause) { return rejectWithValue(failure(cause)) }
  },
)
export const fetchProfileEditor = createAsyncThunk<ProfileEditorData, string | undefined, { rejectValue: string }>(
  'profiles/editor', async (uuid: string | undefined, { rejectWithValue }) => {
    try { return await profileService.loadProfileEditor(uuid) } catch (cause) { return rejectWithValue(failure(cause)) }
  },
)
export const saveAccessProfile = createAsyncThunk<void, { uuid?: string; profile: AccessProfile }, { rejectValue: string }>(
  'profiles/save', async (request: { uuid?: string; profile: AccessProfile }, { rejectWithValue }) => {
    try { await profileService.saveProfile(request) } catch (cause) { return rejectWithValue(failure(cause)) }
  },
)
export const fetchUserProfile = createAsyncThunk<UserProfileData, string, { rejectValue: string }>(
  'profiles/user', async (userUuid: string, { rejectWithValue }) => {
    try { return await profileService.loadUserProfile(userUuid) } catch (cause) { return rejectWithValue(failure(cause)) }
  },
)
export const saveUserProfile = createAsyncThunk<void, { userUuid: string; profileUuid: string }, { rejectValue: string }>(
  'profiles/saveUser', async (request: { userUuid: string; profileUuid: string }, { rejectWithValue }) => {
    try { await profileService.updateUserProfile(request) } catch (cause) { return rejectWithValue(failure(cause)) }
  },
)

const initialState = { loading: false, saving: false, error: '' }
const profileSlice = createSlice({
  name: 'profiles', initialState, reducers: { clearProfileError: (state) => { state.error = '' } },
  extraReducers: (builder) => builder
    .addMatcher(isAnyOf(fetchProfiles.pending, fetchProfileEditor.pending, fetchUserProfile.pending), (state) => { state.loading = true; state.error = '' })
    .addMatcher(isAnyOf(fetchProfiles.fulfilled, fetchProfileEditor.fulfilled, fetchUserProfile.fulfilled), (state) => { state.loading = false })
    .addMatcher(isAnyOf(fetchProfiles.rejected, fetchProfileEditor.rejected, fetchUserProfile.rejected), (state, action) => { state.loading = false; state.error = action.payload || 'loadError' })
    .addMatcher(isAnyOf(saveAccessProfile.pending, saveUserProfile.pending), (state) => { state.saving = true; state.error = '' })
    .addMatcher(isAnyOf(saveAccessProfile.fulfilled, saveUserProfile.fulfilled), (state) => { state.saving = false })
    .addMatcher(isAnyOf(saveAccessProfile.rejected, saveUserProfile.rejected), (state, action) => { state.saving = false; state.error = action.payload || 'genericError' }),
})

export const profileReducer = profileSlice.reducer
export const { clearProfileError } = profileSlice.actions