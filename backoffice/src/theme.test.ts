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

// EPIC-XF-10-S09 (HRMS-409): the approved palette is legible, and stays that
// way. `--accent-primary` is the brand colour and is deliberately too light to
// carry text -- white on #ff5b00 is 3.11:1 where AA wants 4.5:1 -- so the
// console reads its foregrounds from the derived shades instead. Without this
// test that split survives only as a convention, and the next palette edit
// quietly undoes it; acceptance/brand_contrast_audit.py catches the same thing
// but only when somebody remembers to run it.
describe('palette legibility (EPIC-XF-10-S09)', () => {
  const channel = (c: number) => {
    const s = c / 255
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4
  }

  const luminance = (hex: string) => {
    const [r, g, b] = hexToRgbTriple(hex).split(',').map(Number)
    return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
  }

  const contrast = (a: string, b: string) => {
    const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x)
    return (hi + 0.05) / (lo + 0.05)
  }

  const colour = (key: keyof typeof transmegaTheme.variables) => {
    const value = transmegaTheme.variables[key]
    expect(value, `${key} missing from the theme`).toBeTruthy()
    return value as string
  }

  it('reproduces a known contrast ratio, so the maths itself is trustworthy', () => {
    expect(contrast('#ffffff', '#000000')).toBeCloseTo(21, 5)
    expect(contrast('#ffffff', '#ffffff')).toBeCloseTo(1, 5)
    // The failure that prompted the split.
    expect(contrast('#ffffff', '#ff5b00')).toBeLessThan(4.5)
  })

  it('carries a white label on the primary button at AA, across the whole gradient', () => {
    expect(contrast('#ffffff', colour('--accent-text'))).toBeGreaterThanOrEqual(4.5)
    expect(contrast('#ffffff', colour('--accent-deep'))).toBeGreaterThanOrEqual(4.5)
  })

  it('reads as body text on white and on the accent tints at AA', () => {
    const text = colour('--accent-text')
    expect(contrast(text, '#ffffff')).toBeGreaterThanOrEqual(4.5)
    expect(contrast(text, colour('--tint-accent'))).toBeGreaterThanOrEqual(4.5)
    // The metric icon is a glyph, so it is held to the 3:1 asked of a UI component.
    expect(contrast(text, colour('--tint-accent-strong'))).toBeGreaterThanOrEqual(3)
  })

  it('marks a form field at the 3:1 a UI component needs', () => {
    expect(contrast(colour('--input-border'), '#ffffff')).toBeGreaterThanOrEqual(3)
  })

  it('keeps the welcome and sign-in panel text legible over both ends of their gradients', () => {
    const panel = colour('--panel-text')
    expect(contrast(panel, colour('--panel-from'))).toBeGreaterThanOrEqual(4.5)
    expect(contrast(panel, colour('--panel-to'))).toBeGreaterThanOrEqual(4.5)
    const auth = colour('--auth-panel-text')
    expect(contrast(auth, colour('--auth-panel-from'))).toBeGreaterThanOrEqual(4.5)
    expect(contrast(auth, colour('--auth-panel-to'))).toBeGreaterThanOrEqual(4.5)
  })

  it('leaves the brand colour itself untouched, so the fix did not rebrand the console', () => {
    expect(colour('--accent-primary')).toBe('#ff5b00')
  })

  it('applies the derived shades at runtime, not just declares them', () => {
    applyTheme(transmegaTheme)
    const root = document.documentElement.style
    const derived = ['--accent-text', '--accent-deep', '--input-border'] as const
    derived.forEach((key) => {
      expect(root.getPropertyValue(key).trim(), `${key} not applied`).toBe(colour(key))
    })
    applyTheme(null)
    derived.forEach((key) => {
      expect(root.getPropertyValue(key), `${key} left behind`).toBe('')
    })
  })
})
