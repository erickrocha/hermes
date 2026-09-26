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
  const [search, setSearch] = useState('')
  // DEF-BO-08: page and the debounced term have to move together. Held as two
  // independent states, typing a search fired one request with the *old* page
  // (the memo recomputed first) and a second with page 0 (the reset effect ran
  // after), and whichever reply landed last won -- so a search from page 3
  // intermittently rendered page 3 of the filtered set, or the unfiltered
  // list. One state object means one recomputation and one request.
  const [query, setQuery] = useState({ page: 0, search: '' })

  useEffect(() => {
    const timer = setTimeout(() => {
      setQuery((current) => (current.search === search ? current : { page: 0, search }))
    }, SEARCH_DEBOUNCE_MS)
    return () => clearTimeout(timer)
  }, [search])

  const request = useMemo<PageRequest>(
    () => ({ page: query.page, pageSize, search: query.search }),
    [query, pageSize],
  )

  useEffect(() => { dispatch(load(request) as never) }, [dispatch, load, request])

  return {
    search,
    setSearch,
    request,
    onPageChange: (next: PageRequest) => setQuery((current) => ({ ...current, page: next.page })),
    reload: () => dispatch(load(request) as never),
  }
}
