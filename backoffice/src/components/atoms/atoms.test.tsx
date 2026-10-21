import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import '../../i18n'
import { Empty } from './Empty'
import { Loading } from './Loading'
import { Modal } from './Modal'
import { PageHeader } from './PageHeader'
import { SearchBox } from './SearchBox'

describe('atoms', () => {
  it('PageHeader shows title, subtitle and actions', () => {
    render(<PageHeader title="Frota" subtitle="Veículos" actions={<button>Novo</button>} />)
    expect(screen.getByRole('heading', { name: 'Frota' })).toBeInTheDocument()
    expect(screen.getByText('Veículos')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Novo' })).toBeInTheDocument()
  })

  it('Loading and Empty render their state boxes', () => {
    const { container } = render(<><Loading /><Empty /></>)
    expect(container.querySelectorAll('.state-box')).toHaveLength(2)
  })

  it('SearchBox reports what is typed', () => {
    const onChange = vi.fn()
    render(<SearchBox value="" onChange={onChange} />)
    fireEvent.change(screen.getByRole('textbox'), { target: { value: 'abc' } })
    expect(onChange).toHaveBeenCalledWith('abc')
  })

  it('Modal is a dialog that closes by the button and by the backdrop', () => {
    const onClose = vi.fn()
    const { container } = render(<Modal title="Título" onClose={onClose}><p>corpo</p></Modal>)
    expect(screen.getByRole('dialog', { name: 'Título' })).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button'))
    fireEvent.mouseDown(container.querySelector('.modal-backdrop')!)
    expect(onClose).toHaveBeenCalledTimes(2)
  })
})
