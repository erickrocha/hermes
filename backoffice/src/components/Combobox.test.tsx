import { fireEvent, render, screen } from '@testing-library/react'
import { useState } from 'react'
import { describe, expect, it, vi } from 'vitest'
import '../i18n'
import { Combobox } from './Combobox'

const options = [{ value: 'SP', label: 'São Paulo' }, { value: 'RJ', label: 'Rio de Janeiro' }]

describe('Combobox', () => {
  it('filters and selects a single option', () => {
    function Example() { const [value, setValue] = useState<string | null>(null); return <Combobox filterable ariaLabel="Province" options={options} value={value} onChange={setValue} /> }
    render(<Example />)
    const input = screen.getByRole('textbox')
    fireEvent.focus(input)
    fireEvent.change(input, { target: { value: 'rio' } })
    expect(screen.queryByText('São Paulo')).not.toBeInTheDocument()
    fireEvent.click(screen.getByRole('option', { name: 'Rio de Janeiro' }))
    expect(input).toHaveAttribute('placeholder', 'Rio de Janeiro')
  })

  it('supports multiple selection and removal', () => {
    function Example() { const [value, setValue] = useState<string[]>([]); return <Combobox multiple ariaLabel="Provinces" options={options} value={value} onChange={setValue} /> }
    render(<Example />)
    fireEvent.click(screen.getByRole('button', { name: /selecione|select/i }))
    fireEvent.click(screen.getByRole('option', { name: 'São Paulo' }))
    fireEvent.click(screen.getByRole('option', { name: 'Rio de Janeiro' }))
    expect(screen.getByText(/2 selecionado|2 selected/i)).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: /remover são paulo|remove são paulo/i }))
    expect(screen.queryByRole('button', { name: /remover são paulo|remove são paulo/i })).not.toBeInTheDocument()
  })

  it('selects using the keyboard', () => {
    const onChange = vi.fn()
    render(<Combobox ariaLabel="Province" options={options} value={null} onChange={onChange} />)
    const control = screen.getByRole('combobox')
    fireEvent.keyDown(control, { key: 'ArrowDown' })
    fireEvent.keyDown(control, { key: 'Enter' })
    expect(onChange).toHaveBeenCalledWith('SP')
  })
})
