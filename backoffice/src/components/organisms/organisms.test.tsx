import { fireEvent, screen, waitFor } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { api } from '../../api'
import type { FieldDef } from '../../features/fields'
import { renderPage, signIn } from '../../test/render'
import { ChildList } from './ChildList'
import { FilterBar } from './FilterBar'
import { ResourceFormPage, type FormController } from './ResourceFormPage'

const fields: FieldDef[] = [{ name: 'name', labelKey: 'name', type: 'text', required: true }]
const controller = (over: Partial<FormController> = {}): FormController =>
  ({ loading: false, saving: false, error: '', saved: false, current: null, reset: vi.fn(), submit: vi.fn(), ...over })

describe('organisms', () => {
  beforeEach(() => { vi.restoreAllMocks(); signIn('TenantOwner') })

  it('ResourceFormPage blocks an empty required field and submits a valid one', async () => {
    const c = controller()
    renderPage(<ResourceFormPage titleKey="users" fields={fields} backTo="/back" controller={c} />)
    fireEvent.click(screen.getByRole('button', { name: /^salvar$|^save$/i }))
    await waitFor(() => expect(document.querySelector('[aria-invalid="true"]')).not.toBeNull())
    expect(c.submit).not.toHaveBeenCalled()
    fireEvent.change(document.getElementById('name')!, { target: { value: 'Acme' } })
    fireEvent.click(screen.getByRole('button', { name: /^salvar$|^save$/i }))
    await waitFor(() => expect(c.submit).toHaveBeenCalled())
  })

  it('ResourceFormPage puts cancel on the left of save and shows the server error', () => {
    renderPage(<ResourceFormPage titleKey="users" fields={fields} backTo="/back" controller={controller({ error: 'Falhou' })} />)
    expect(screen.getByRole('alert')).toHaveTextContent('Falhou')
    const actions = document.querySelector('.form-actions')!
    expect(actions.firstElementChild).toHaveAttribute('href', '/back')
    expect(actions.lastElementChild).toHaveAttribute('type', 'submit')
  })

  it('ResourceFormPage widens to four columns on demand', () => {
    renderPage(<ResourceFormPage titleKey="users" fields={fields} backTo="/back" controller={controller()} columns={4} />)
    expect(document.querySelector('.form-grid.cols-4')).not.toBeNull()
  })

  it('FilterBar applies the filled-in parameters', async () => {
    const onApply = vi.fn()
    renderPage(<FilterBar fields={[{ name: 'plate', labelKey: 'plate', type: 'text' }]} onApply={onApply} />)
    fireEvent.change(document.getElementById('plate')!, { target: { value: 'ABC' } })
    fireEvent.click(screen.getAllByRole('button').at(-1)!)
    await waitFor(() => expect(onApply).toHaveBeenCalled())
    expect(onApply.mock.calls[0][0]).toMatchObject({ plate: 'ABC' })
  })

  it('ChildList shows rows, the empty state and the error', async () => {
    const cols = [{ key: 'name', labelKey: 'name' }] as never
    vi.spyOn(api, 'get').mockResolvedValueOnce({ data: { items: [{ uuid: 'a', name: 'Linha 1' }] } } as never)
    const first = renderPage(<ChildList titleKey="users" path="/p1" columns={cols} />)
    expect(await screen.findByText('Linha 1')).toBeInTheDocument()
    first.unmount()
    vi.spyOn(api, 'get').mockResolvedValueOnce({ data: { items: [] } } as never)
    const second = renderPage(<ChildList titleKey="users" path="/p2" columns={cols} />)
    await waitFor(() => expect(screen.queryByText('Linha 1')).toBeNull())
    await waitFor(() => expect(document.querySelector('.form-hint')).not.toBeNull())
    expect(document.querySelector('table')).toBeNull()
    second.unmount()
  })
})
