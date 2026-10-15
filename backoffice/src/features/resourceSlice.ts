import { createAsyncThunk, createSlice } from '@reduxjs/toolkit'
import { apiMessage } from '../api'
import { listPage, read, send, type Method, type Params } from '../services/resourceService'
import { emptyPage, type Page } from '../types'

export interface ListRequest { page: number; pageSize: number; extra?: Params }
export interface ResourceState<T> { list: Page<T>; current: T | null; loading: boolean; saving: boolean; error: string; saved: boolean }
export interface SaveRequest { uuid?: string; body: unknown; suffix?: string; method?: Method }
export interface ActionRequest { method: Method; path: string; body?: unknown }

const failure = (cause: unknown) => apiMessage(cause, '')

/// Uma slice por feature: o slice chama o serviço e guarda o resultado; o componente só despacha e lê.
export function createResourceSlice<T>(name: string, path: string) {
  const fetchList = createAsyncThunk<Page<T>, ListRequest, { rejectValue: string }>(`${name}/fetchList`, async (request, { rejectWithValue }) => {
    try { return await listPage<T>(path, { page: request.page, pageSize: request.pageSize, ...request.extra }) } catch (cause) { return rejectWithValue(failure(cause)) }
  })
  const fetchOne = createAsyncThunk<T, string, { rejectValue: string }>(`${name}/fetchOne`, async (uuid, { rejectWithValue }) => {
    try { return await read<T>(`${path}/uuid/${uuid}`) } catch (cause) { return rejectWithValue(failure(cause)) }
  })
  const save = createAsyncThunk<T, SaveRequest, { rejectValue: string }>(`${name}/save`, async ({ uuid, body, suffix = '', method }, { rejectWithValue }) => {
    try { return await send<T>(method ?? (uuid ? 'put' : 'post'), `${path}${uuid ? `/uuid/${uuid}` : ''}${suffix}`, body) } catch (cause) { return rejectWithValue(failure(cause)) }
  })
  const remove = createAsyncThunk<void, string, { rejectValue: string }>(`${name}/remove`, async (uuid, { rejectWithValue }) => {
    try { await send('delete', `${path}/uuid/${uuid}`) } catch (cause) { return rejectWithValue(failure(cause)) }
  })
  const act = createAsyncThunk<unknown, ActionRequest, { rejectValue: string }>(`${name}/act`, async ({ method, path: target, body }, { rejectWithValue }) => {
    try { return await send(method, target, body) } catch (cause) { return rejectWithValue(failure(cause)) }
  })

  const initialState: ResourceState<T> = { list: emptyPage<T>(), current: null, loading: false, saving: false, error: '', saved: false }
  const slice = createSlice({
    name, initialState,
    reducers: { reset: (state) => { state.error = ''; state.saved = false; state.current = null } },
    extraReducers: (builder) => {
      builder
        .addCase(fetchList.pending, (s) => { s.loading = true; s.error = '' })
        .addCase(fetchList.fulfilled, (s, a) => { s.loading = false; s.list = a.payload as never })
        .addCase(fetchList.rejected, (s, a) => { s.loading = false; s.error = a.payload || 'loadError' })
        .addCase(fetchOne.pending, (s) => { s.loading = true; s.error = ''; s.current = null })
        .addCase(fetchOne.fulfilled, (s, a) => { s.loading = false; s.current = a.payload as never })
        .addCase(fetchOne.rejected, (s, a) => { s.loading = false; s.error = a.payload || 'loadError' })
      for (const thunk of [save, remove, act]) {
        builder
          .addCase(thunk.pending, (s) => { s.saving = true; s.error = ''; s.saved = false })
          .addCase(thunk.fulfilled, (s) => { s.saving = false; s.saved = true })
          .addCase(thunk.rejected, (s, a) => { s.saving = false; s.error = (a.payload as string | undefined) || 'genericError' })
      }
    },
  })
  return { name, slice, reducer: slice.reducer, reset: slice.actions.reset, fetchList, fetchOne, save, remove, act, path }
}
