import { useEffect, useMemo, useState } from 'react'
import { useDispatch } from 'react-redux'
import type { AppDispatch } from './store'
import type { PageRequest } from './types'

const DEFAULT_PAGE_SIZE = 25
const SEARCH_DEBOUNCE_MS = 300

/// PD-028: o estado de paginação que as três listas compartilham.
///
/// Digitar na busca volta para a primeira página: manter o índice atual mostra
/// "página 4 de 1" e uma tabela vazia para uma busca que tem resultados.
export function usePagedList(load: (request: PageRequest) => unknown, pageSize = DEFAULT_PAGE_SIZE) {
  const dispatch = useDispatch<AppDispatch>()
  const [page, setPage] = useState(0)
  const [search, setSearch] = useState('')
  const [debouncedSearch, setDebouncedSearch] = useState('')

  useEffect(() => {
    const timer = setTimeout(() => setDebouncedSearch(search), SEARCH_DEBOUNCE_MS)
    return () => clearTimeout(timer)
  }, [search])

  useEffect(() => { setPage(0) }, [debouncedSearch])

  const request = useMemo<PageRequest>(
    () => ({ page, pageSize, search: debouncedSearch }),
    [page, pageSize, debouncedSearch],
  )

  useEffect(() => { dispatch(load(request) as never) }, [dispatch, load, request])

  return {
    search,
    setSearch,
    request,
    onPageChange: (next: PageRequest) => setPage(next.page),
    reload: () => dispatch(load(request) as never),
  }
}
