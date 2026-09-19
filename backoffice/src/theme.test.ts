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

  // DEF-BO-01: the chrome the operator actually looks at -- sidebar, welcome
  // panel, sign-in panel, accent tints -- was painted with literal blues, so
  // a themed tenant got their accent on Hermes-blue furniture. Every variable
  // main.scss reads for that chrome must be in the theme, or the swap is
  // partial and reads as a rendering bug.
  const CHROME_VARIABLES = [
    '--sidebar-from', '--sidebar-to', '--sidebar-text', '--sidebar-link',
    '--brand-mark-to',
    '--panel-from', '--panel-to', '--panel-text',
    '--auth-panel-from', '--auth-panel-to', '--auth-panel-text', '--auth-panel-highlight',
    '--tint-accent', '--tint-accent-strong', '--tint-accent-deep', '--tint-muted',
  ] as const

  it('themes the console chrome, not just the buttons', () => {
    applyTheme(transmegaTheme)
    const root = document.documentElement.style
    CHROME_VARIABLES.forEach((key) => {
      expect(transmegaTheme.variables[key], `${key} missing from the theme`).toBeTruthy()
      expect(root.getPropertyValue(key).trim(), `${key} not applied`).toBe(transmegaTheme.variables[key])
    })
  })

  it('clears the chrome overrides too, so a tenant colour never survives a sign-out', () => {
    applyTheme(transmegaTheme)
    applyTheme(null)
    const root = document.documentElement.style
    CHROME_VARIABLES.forEach((key) => expect(root.getPropertyValue(key), `${key} left behind`).toBe(''))
  })

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
