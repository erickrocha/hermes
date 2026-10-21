import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import { i18n } from '../i18n'
import { ErrorBoundary } from './ErrorBoundary'
import { LanguagePicker } from './LanguagePicker'

describe('language picker and error boundary', () => {
  it('LanguagePicker switches the language', async () => {
    render(<LanguagePicker />)
    fireEvent.change(screen.getByRole('combobox'), { target: { value: 'en' } })
    expect(i18n.language).toMatch(/^en/)
    await i18n.changeLanguage('pt')
  })

  it('ErrorBoundary contains a render error and offers the way back', () => {
    vi.spyOn(console, 'error').mockImplementation(() => {})
    const Bomb = () => { throw new Error('boom') }
    render(<ErrorBoundary><Bomb /></ErrorBoundary>)
    expect(screen.getByRole('alert')).toBeInTheDocument()
    expect(screen.getByRole('button')).toBeInTheDocument()
  })

  it('ErrorBoundary renders children when nothing fails', () => {
    render(<ErrorBoundary><p>tudo certo</p></ErrorBoundary>)
    expect(screen.getByText('tudo certo')).toBeInTheDocument()
  })
})
