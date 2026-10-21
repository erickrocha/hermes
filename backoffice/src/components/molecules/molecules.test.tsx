import { fireEvent, render, screen } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import { describe, expect, it, vi } from 'vitest'
import '../../i18n'
import { Breadcrumbs } from './Breadcrumbs'
import { Confirm } from './Confirm'
import { DetailCard } from './DetailCard'
import { Form } from './Form'
import { PanelSection } from './PanelSection'
import { StateView } from './StateView'

describe('molecules', () => {
  it('Confirm cancels on the left and confirms on the right', () => {
    const onCancel = vi.fn(); const onConfirm = vi.fn()
    render(<Confirm title="Excluir" text="Tem certeza?" onCancel={onCancel} onConfirm={onConfirm} />)
    const [cancel, confirm] = screen.getAllByRole('button').slice(-2)
    fireEvent.click(cancel); fireEvent.click(confirm)
    expect(onCancel).toHaveBeenCalledOnce(); expect(onConfirm).toHaveBeenCalledOnce()
    expect(screen.getByText('Tem certeza?')).toBeInTheDocument()
  })

  it('Form submits, shows the error and offers cancel before save', () => {
    const onSubmit = vi.fn((e) => e.preventDefault()); const onCancel = vi.fn()
    render(<Form onSubmit={onSubmit} onCancel={onCancel} error="falhou"><input aria-label="x" /></Form>)
    expect(screen.getByText('falhou')).toBeInTheDocument()
    const buttons = screen.getAllByRole('button')
    expect(buttons).toHaveLength(2)
    fireEvent.click(buttons[0]); expect(onCancel).toHaveBeenCalled()
    fireEvent.click(buttons[1]); expect(onSubmit).toHaveBeenCalled()
  })

  it('DetailCard hides rows without value', () => {
    render(<DetailCard rows={[{ label: 'Placa', value: 'ABC' }, { label: 'Vazio', value: '' }, { label: 'Oculto', value: 'x', hide: true }]} />)
    expect(screen.getByText('Placa')).toBeInTheDocument()
    expect(screen.queryByText('Vazio')).toBeNull(); expect(screen.queryByText('Oculto')).toBeNull()
  })

  it('StateView covers loading, error, empty and success', () => {
    const { rerender } = render(<StateView loading>ok</StateView>)
    expect(screen.queryByText('ok')).toBeNull()
    rerender(<StateView loading={false} error="Quebrou">ok</StateView>)
    expect(screen.getByRole('alert')).toHaveTextContent('Quebrou')
    rerender(<StateView loading={false} empty>ok</StateView>)
    expect(screen.queryByText('ok')).toBeNull()
    rerender(<StateView loading={false}>ok</StateView>)
    expect(screen.getByText('ok')).toBeInTheDocument()
  })

  it('PanelSection shows title, actions and the state of the read', () => {
    const { rerender } = render(<PanelSection titleKey="users" loading={false} actions={<button>Add</button>}><p>conteúdo</p></PanelSection>)
    expect(screen.getByText('conteúdo')).toBeInTheDocument(); expect(screen.getByRole('button', { name: 'Add' })).toBeInTheDocument()
    rerender(<PanelSection titleKey="users" loading={false} error="falha"><p>conteúdo</p></PanelSection>)
    expect(screen.getByRole('status')).toHaveTextContent('falha')
    rerender(<PanelSection titleKey="users" loading={false} empty><p>conteúdo</p></PanelSection>)
    expect(screen.queryByText('conteúdo')).toBeNull()
  })

  it('Breadcrumbs link back to the parent of an edit screen', () => {
    render(<MemoryRouter initialEntries={['/tenants/abc/manage']}><Breadcrumbs /></MemoryRouter>)
    expect(screen.getAllByRole('link')).toHaveLength(2)
    expect(document.querySelector('[aria-current="page"]')).not.toBeNull()
  })
})
