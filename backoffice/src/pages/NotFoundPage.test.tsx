import { screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { renderPage } from '../test/render'
import { NotFoundPage } from './NotFoundPage'

describe('NotFoundPage', () => {
  it('says 404 and links back to the dashboard', () => {
    renderPage(<NotFoundPage />)
    expect(screen.getByText('404')).toBeInTheDocument()
    expect(screen.getByRole('link')).toHaveAttribute('href', '/home')
  })
})
