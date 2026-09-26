import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { describe, expect, it, vi, beforeEach } from 'vitest'
import '../i18n'
import { api } from '../api'
import { SystemSettingsPage } from './SystemSettingsPage'
import { emptyPage } from '../types'

// PD-027: o valor da tela é que o CSV é revisável **antes** de gravar. Um
// import que envia o arquivo cru passaria despercebido em revisão de código,
// então é exatamente isso que estes casos prendem.
describe('SystemSettingsPage CSV import', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    vi.spyOn(api, 'get').mockResolvedValue({ data: emptyPage() } as never)
  })

  const loadCsv = async (text: string) => {
    render(<SystemSettingsPage />)
    const input = await screen.findByLabelText(/import csv|importar csv/i)
    const file = new File([text], 'provinces.csv', { type: 'text/csv' })
    // jsdom's FileReader needs the text available synchronously enough for
    // the onload handler; File.text() is stubbed by the polyfill it uses.
    fireEvent.change(input, { target: { files: [file] } })
  }

  it('shows the parsed rows as editable inputs instead of importing them straight away', async () => {
    const post = vi.spyOn(api, 'post')
    await loadCsv('acronym,name,countryCode\nSP,Sao Paulo,BR\nRJ,Rio de Janeiro,BR')

    const firstAcronym = await screen.findByLabelText(/^(acronym|sigla) 1$/i)
    expect(firstAcronym).toHaveValue('SP')
    expect(post).not.toHaveBeenCalled()
  })

  it('sends the edited values, not the values that were in the file', async () => {
    const post = vi.spyOn(api, 'post').mockResolvedValue({ data: { created: 1, updated: 0 } } as never)
    vi.spyOn(window, 'alert').mockImplementation(() => {})
    await loadCsv('acronym,name,countryCode\nsp,Sao Paulo,BR')

    const acronym = await screen.findByLabelText(/^(acronym|sigla) 1$/i)
    fireEvent.change(acronym, { target: { value: 'SP' } })
    fireEvent.click(screen.getByRole('button', { name: /^save$|^salvar$|^guardar$/i }))

    await waitFor(() => expect(post).toHaveBeenCalled())
    const [path, body] = post.mock.calls[0]
    expect(path).toBe('/province/import')
    expect(body).toEqual([{ acronym: 'SP', name: 'Sao Paulo', countryCode: 'BR' }])
  })

  it('rejects a file whose columns it does not recognise, rather than showing a blank grid', async () => {
    await loadCsv('totally,unrelated\n1,2')
    expect(await screen.findByText(/no recognised columns|nenhuma coluna reconhecida/i)).toBeInTheDocument()
  })

  it('lets the operator back out without sending anything', async () => {
    const post = vi.spyOn(api, 'post')
    await loadCsv('acronym,name,countryCode\nSP,Sao Paulo,BR')

    await screen.findByLabelText(/^(acronym|sigla) 1$/i)
    fireEvent.click(screen.getByRole('button', { name: /^cancel$|^cancelar$/i }))

    await waitFor(() => expect(screen.queryByLabelText(/^(acronym|sigla) 1$/i)).not.toBeInTheDocument())
    expect(post).not.toHaveBeenCalled()
  })
})
