import { useCallback, useEffect, useMemo, useRef, useState, type ChangeEvent, type FormEvent } from 'react'
import type { ColumnDef } from '@tanstack/react-table'
import { Pencil, Plus, RefreshCw, Upload, X } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { api, apiMessage } from '../api'
import { matchColumns, parseCsv } from '../csv'
import { DataTable } from '../components/DataTable'
import { Empty, Form, Loading, PageHeader, SearchBox } from '../components/UI'
import { emptyPage, type Page } from '../types'

// PD-027: a área de dados de referência. Listar, adicionar, editar e importar
// um CSV -- que é carregado, mostrado **editável**, e só grava quando o
// operador confirma. A importação é tudo ou nada no servidor, então o que a
// tela precisa garantir é que ele veja o que vai enviar antes de enviar.

interface FieldSpec {
  key: string
  label: string
  required?: boolean
}

interface ResourceSpec {
  key: string
  titleKey: string
  listPath: string
  savePath: string
  importPath: string
  fields: FieldSpec[]
}

const PROVINCES: ResourceSpec = {
  key: 'provinces',
  titleKey: 'provinces',
  listPath: '/province/page',
  savePath: '/province',
  importPath: '/province/import',
  fields: [
    { key: 'acronym', label: 'acronym', required: true },
    { key: 'name', label: 'name', required: true },
    { key: 'countryCode', label: 'country', required: true },
  ],
}

const CITIES: ResourceSpec = {
  key: 'cities',
  titleKey: 'cities',
  listPath: '/cities',
  savePath: '/city',
  importPath: '/city/import',
  fields: [
    { key: 'name', label: 'name', required: true },
    { key: 'provinceId', label: 'province', required: true },
  ],
}

type Row = Record<string, string | number | null | undefined>

const blankRow = (spec: ResourceSpec): Row =>
  Object.fromEntries(spec.fields.map((field) => [field.key, '']))

/// Números chegam do CSV como texto; o servidor espera número em `provinceId`.
const coerce = (spec: ResourceSpec, row: Row): Row => {
  const out: Row = { ...row }
  if (spec.key === 'cities' && out.provinceId !== undefined && out.provinceId !== '') {
    out.provinceId = Number(out.provinceId)
  }
  return out
}

function ResourcePanel({ spec }: { spec: ResourceSpec }) {
  const { t } = useTranslation()
  const [page, setPage] = useState<Page<Row>>(emptyPage<Row>())
  const [pageIndex, setPageIndex] = useState(0)
  const [search, setSearch] = useState('')
  const [debounced, setDebounced] = useState('')
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState('')
  const [editing, setEditing] = useState<Row | null>(null)
  const [importRows, setImportRows] = useState<Row[] | null>(null)
  const fileInput = useRef<HTMLInputElement>(null)

  useEffect(() => {
    const timer = setTimeout(() => setDebounced(search), 300)
    return () => clearTimeout(timer)
  }, [search])
  useEffect(() => { setPageIndex(0) }, [debounced])

  const load = useCallback(async () => {
    setLoading(true)
    try {
      const { data } = await api.get<Page<Row>>(spec.listPath, {
        params: { page: pageIndex, pageSize: 25, search: debounced || undefined },
      })
      setPage(data)
      setError('')
    } catch (cause) {
      setError(apiMessage(cause, t('genericError')))
    } finally {
      setLoading(false)
    }
  }, [spec.listPath, pageIndex, debounced, t])

  useEffect(() => { load() }, [load])

  const columns = useMemo<ColumnDef<Row, unknown>[]>(() => [
    ...spec.fields.map((field) => ({
      id: field.key,
      header: () => t(field.label),
      cell: ({ row }: { row: { original: Row } }) => String(row.original[field.key] ?? '—'),
    })),
    {
      id: 'actions',
      header: () => t('actions'),
      cell: ({ row }: { row: { original: Row } }) => (
        <div className="row-actions">
          <button onClick={() => setEditing(row.original)} aria-label={t('edit')}><Pencil size={16} /></button>
        </div>
      ),
    },
  ], [spec.fields, t])

  const save = async (event: FormEvent) => {
    event.preventDefault()
    if (!editing) return
    try {
      await api.post(spec.savePath, coerce(spec, editing))
      setEditing(null)
      load()
    } catch (cause) {
      setError(apiMessage(cause, t('genericError')))
    }
  }

  const pickFile = (event: ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0]
    if (!file) return
    const reader = new FileReader()
    reader.onload = () => {
      const { headers, rows } = parseCsv(String(reader.result ?? ''))
      const mapping = matchColumns(headers, spec.fields.map((field) => field.key))
      // Nenhuma coluna reconhecida quase sempre significa arquivo errado, não
      // arquivo vazio -- dizer isso é mais útil que mostrar uma grade em branco.
      if (!Object.keys(mapping).length) {
        setError(t('importNoColumns', { expected: spec.fields.map((f) => f.key).join(', ') }))
        return
      }
      setError('')
      setImportRows(rows.map((cells) =>
        Object.fromEntries(spec.fields.map((field) =>
          [field.key, mapping[field.key] !== undefined ? cells[mapping[field.key]] : ''])),
      ))
    }
    reader.readAsText(file)
    // Permite reescolher o mesmo arquivo depois de corrigi-lo no disco.
    event.target.value = ''
  }

  const confirmImport = async () => {
    if (!importRows) return
    try {
      const { data } = await api.post<{ created: number; updated: number }>(
        spec.importPath,
        importRows.map((row) => coerce(spec, row)),
      )
      setImportRows(null)
      setError('')
      setPageIndex(0)
      load()
      window.alert(t('importDone', { created: data.created, updated: data.updated }))
    } catch (cause) {
      // O servidor recusa o lote inteiro e diz qual linha: é o texto que
      // permite corrigir na grade, então ele aparece inteiro.
      setError(apiMessage(cause, t('genericError')))
    }
  }

  if (importRows) {
    return (
      <section className="import-review">
        <header className="import-head">
          <div>
            <h3>{t('importReview')}</h3>
            <p>{t('importReviewHint', { count: importRows.length })}</p>
          </div>
          <button className="btn tertiary" onClick={() => { setImportRows(null); setError('') }} aria-label={t('close')}><X size={16} /></button>
        </header>
        {error && <div className="form-error import-error">{error}</div>}
        <div className="table-card"><div className="table-scroll">
          <table>
            <thead><tr>{spec.fields.map((field) => <th key={field.key}>{t(field.label)}</th>)}</tr></thead>
            <tbody>
              {importRows.map((row, rowIndex) => (
                <tr key={rowIndex}>
                  {spec.fields.map((field) => (
                    <td key={field.key}>
                      <input
                        aria-label={`${t(field.label)} ${rowIndex + 1}`}
                        value={String(row[field.key] ?? '')}
                        onChange={(event) => setImportRows((rows) =>
                          rows!.map((current, index) =>
                            index === rowIndex ? { ...current, [field.key]: event.target.value } : current))}
                      />
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div></div>
        <div className="modal-actions">
          <button className="btn secondary" onClick={() => { setImportRows(null); setError('') }}>{t('cancel')}</button>
          <button className="btn primary" onClick={confirmImport} disabled={!importRows.length}>{t('save')}</button>
        </div>
      </section>
    )
  }

  return (
    <>
      <div className="toolbar">
        <SearchBox value={search} onChange={setSearch} />
        <div className="page-actions">
          <button className="btn secondary" onClick={load}><RefreshCw size={16} />{t('refresh')}</button>
          <button className="btn secondary" onClick={() => fileInput.current?.click()}><Upload size={16} />{t('importCsv')}</button>
          <button className="btn primary" onClick={() => setEditing(blankRow(spec))}><Plus size={17} />{t('add')}</button>
        </div>
        <input ref={fileInput} type="file" accept=".csv,text/csv" hidden onChange={pickFile} aria-label={t('importCsv')} />
      </div>
      {error && <div className="form-error">{error}</div>}
      {editing && (
        <section className="page-form-card">
          <Form onSubmit={save} error="" saving={false} onCancel={() => setEditing(null)}>
            {spec.fields.map((field) => (
              <label key={field.key}>
                {t(field.label)}
                <input
                  required={field.required}
                  value={String(editing[field.key] ?? '')}
                  onChange={(event) => setEditing({ ...editing, [field.key]: event.target.value })}
                />
              </label>
            ))}
          </Form>
        </section>
      )}
      {loading && !page.items.length ? <Loading />
        : !page.items.length ? <Empty />
        : <DataTable columns={columns} page={page} loading={loading} onPageChange={(next) => setPageIndex(next.page)} />}
    </>
  )
}

export function SystemSettingsPage() {
  const { t } = useTranslation()
  const [tab, setTab] = useState<ResourceSpec>(PROVINCES)
  return (
    <>
      <PageHeader title={t('systemSettings')} subtitle={t('systemSettingsHint')} />
      <nav className="tab-bar" role="tablist">
        {[PROVINCES, CITIES].map((spec) => (
          <button
            key={spec.key}
            role="tab"
            aria-selected={tab.key === spec.key}
            className={tab.key === spec.key ? 'tab active' : 'tab'}
            onClick={() => setTab(spec)}
          >
            {t(spec.titleKey)}
          </button>
        ))}
      </nav>
      <ResourcePanel key={tab.key} spec={tab} />
    </>
  )
}
