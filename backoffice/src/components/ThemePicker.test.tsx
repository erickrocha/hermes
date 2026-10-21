import { fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'
import '../i18n'
import { applyTheme } from '../theme'
import { ThemePicker } from './ThemePicker'

describe('ThemePicker', () => {
  afterEach(() => { applyTheme(null); localStorage.clear() })

  it('starts on default and persists the chosen theme', () => {
    render(<ThemePicker />)
    const select = screen.getByRole('combobox') as HTMLSelectElement
    expect(select.value).toBe('default')
    fireEvent.change(select, { target: { value: 'transmega' } })
    expect(localStorage.getItem('hermes.theme')).toBe('transmega')
    expect(document.documentElement.style.getPropertyValue('--accent-primary').trim()).toBe('#ff5b00')
  })

  it('reads the stored theme on mount', () => {
    localStorage.setItem('hermes.theme', 'transmega')
    render(<ThemePicker />)
    expect((screen.getByRole('combobox') as HTMLSelectElement).value).toBe('transmega')
  })
})
