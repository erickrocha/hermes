import { useCallback, useEffect } from 'react'
import { useDispatch, useSelector } from 'react-redux'
import type { AppDispatch, RootState } from '../store'
import { loadPanel } from './panelSlice'
import type { Params } from '../services/resourceService'
import type { createResourceSlice, ResourceState } from './resourceSlice'
import type { FormController } from '../components/organisms/ResourceFormPage'

export type Resource<T = unknown> = ReturnType<typeof createResourceSlice<T>>

export function useResourceState<T>(resource: Resource<T>) {
  return useSelector((state: RootState) => (state as unknown as Record<string, ResourceState<T>>)[resource.name])
}

/// Liga o formulário genérico ao slice: carrega o registro na edição, grava no `submit`, limpa ao sair.
export function useResourceForm<T>(resource: Resource<T>, uuid?: string, suffix?: string, method?: 'post' | 'put'): FormController {
  const dispatch = useDispatch<AppDispatch>()
  const state = useResourceState(resource)
  const load = useCallback(() => { if (uuid && !suffix) dispatch(resource.fetchOne(uuid)) }, [dispatch, resource, uuid, suffix])
  const reset = useCallback(() => { dispatch(resource.reset()) }, [dispatch, resource])
  return {
    // Só a edição espera um registro; um formulário novo não deve ficar preso numa lista que ainda carrega.
    loading: Boolean(uuid && !suffix) && state.loading, saving: state.saving, error: state.error, saved: state.saved, current: suffix ? null : (state.current as Record<string, unknown> | null),
    load: uuid && !suffix ? load : undefined, reset,
    submit: (body) => dispatch(resource.save({ uuid, body, suffix, method })),
  }
}

const idle = { data: undefined, loading: false, error: '' }

/// Uma leitura avulsa por chave: carrega ao montar (ou quando o caminho muda) e expõe `reload`.
export function usePanel<T>(path: string | null, params?: Params, key = path ?? ''): { data: T | undefined; loading: boolean; error: string; reload: () => void } {
  const dispatch = useDispatch<AppDispatch>()
  const state = useSelector((s: RootState) => s.panel[key]) ?? idle
  const serialized = JSON.stringify(params ?? {})
  const reload = useCallback(() => { if (path) dispatch(loadPanel({ key, path, params: JSON.parse(serialized) })) }, [dispatch, path, key, serialized])
  useEffect(() => { reload() }, [reload])
  return { data: state.data as T | undefined, loading: state.loading, error: state.error, reload }
}

/// Dispara uma ação (PUT/POST sem corpo de formulário) pelo slice e diz se deu certo.
export function useAction<T>(resource: Resource<T>) {
  const dispatch = useDispatch<AppDispatch>()
  return useCallback(async (method: 'post' | 'put' | 'delete', path: string, body?: unknown) =>
    resource.act.fulfilled.match(await dispatch(resource.act({ method, path, body }))), [dispatch, resource])
}

/// Cadastro de um registro único do tenant (sem uuid na URL): lê o caminho, grava com PUT. Antes do primeiro cadastro o formulário nasce vazio.
export function useSingletonForm<T>(resource: Resource<T>, path: string): FormController {
  const dispatch = useDispatch<AppDispatch>()
  const state = useResourceState(resource)
  const panel = usePanel<Record<string, unknown>>(path)
  const reset = useCallback(() => { dispatch(resource.reset()) }, [dispatch, resource])
  return { loading: panel.loading, saving: state.saving, error: state.error, saved: state.saved, current: panel.data ?? null, reset, submit: (body) => dispatch(resource.act({ method: 'put', path, body })) }
}

/// Um formulário cujo envio é uma ação em caminho montado a partir dos valores (restaurar, unificar, registrar recibo…).
export function useActionForm<T>(resource: Resource<T>, build: (body: Record<string, unknown>) => { method: 'post' | 'put' | 'delete'; path: string; body?: unknown }): FormController {
  const dispatch = useDispatch<AppDispatch>()
  const state = useResourceState(resource)
  const reset = useCallback(() => { dispatch(resource.reset()) }, [dispatch, resource])
  return { loading: false, saving: state.saving, error: state.error, saved: state.saved, current: null, reset, submit: (body) => dispatch(resource.act(build(body))) }
}
