import { afterEach, describe, expect, it } from 'vitest'
import { applyTheme, hexToRgbTriple, themeForTenant, transmegaTheme } from './theme'

describe('themeForTenant (EPIC-BO-01-S02/S03)', () => {
  it('matches Transmega by name, case-insensitively', () => {
    expect(themeForTenant('Transmega')).toBe(transmegaTheme)
    expect(themeForTenant('TRANSMEGA TRANSPORTES')).toBe(transmegaTheme)
  })

  it('gives no theme override to any other tenant or a SysAdmin session (no tenant)', () => {
    expect(themeForTenant('Acme Logistics')).toBeNull()
    expect(themeForTenant(undefined)).toBeNull()
    expect(themeForTenant(null)).toBeNull()
  })
})

describe('hexToRgbTriple', () => {
  it('decodes a hex colour into the comma triple rgba() needs', () => {
    expect(hexToRgbTriple('#ff5b00')).toBe('255,91,0')
  })
})

describe('applyTheme (EPIC-BO-01-S01/S02)', () => {
  afterEach(() => applyTheme(null))

  it('overrides the CSS custom properties main.scss reads its brand colours from', () => {
    applyTheme(transmegaTheme)
    const root = document.documentElement.style
    expect(root.getPropertyValue('--accent-primary').trim()).toBe('#ff5b00')
    expect(root.getPropertyValue('--accent-primary-rgb').trim()).toBe('255,91,0')
  })

  it('clears every override when no theme applies, instead of leaving a stale tenant colour behind', () => {
    applyTheme(transmegaTheme)
    applyTheme(null)
    const root = document.documentElement.style
    expect(root.getPropertyValue('--accent-primary')).toBe('')
    expect(root.getPropertyValue('--accent-primary-rgb')).toBe('')
  })
})
