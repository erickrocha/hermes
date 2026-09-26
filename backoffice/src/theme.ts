// EPIC-BO-01-S02/S03 (HRMS-408, HRMS-409, PD-018): the console's identity is
// data driven by which tenant the session belongs to (AD-011's "swap the
// theme per tenant" promise), not a component rewrite. `main.scss` reads
// every brand colour from these CSS custom properties (EPIC-BO-01-S01), so
// overriding them here at runtime is the entire mechanism.
//
// `transmegaTheme` below was extracted from a read-only survey of
// `04-codebase/operacao-trm` (its `index.html` `theme-color` and
// `src/styles.css` `:root` block: `#ff5b00`/`#f97316` accent, `#f8f8f7`
// background, `#111111` text) and was **approved by Erick Rocha (Project
// Owner) on 2026-09-21** -- see `02-system_requirements/hermes/
// transmega-brand-signoff.md` for the record, which closes EPIC-XF-10-S09
// (HRMS-409) and decision-register.md D-12.
//
// The approval covers the brand colours as surveyed. `--accent-text`,
// `--accent-deep` and `--input-border` were added at the same time as
// *derived* values, not new brand colours: `--accent-primary` (#ff5b00) is
// too light to carry a white label or to be read as small text, so it stays
// the fill and the derived shades carry the text. See ThemeVariableKey below.
//
// One surveyed value did change: `--panel-to` was darkened from `#ea580c` to
// `#c2410c`, because the welcome panel's body text over the light end of its
// gradient sat at 3.11:1. It is recorded in the sign-off as a revision rather
// than presented as part of the original survey.

export type ThemeVariableKey =
  | '--bg-primary'
  | '--text-heading'
  | '--text-body'
  | '--accent-primary'
  | '--accent-hover'
  | '--accent-secondary'
  // EPIC-XF-10-S09: `--accent-primary` is the brand colour, and it is too
  // light to be legible as small text or to carry a white label -- white on
  // `#ff5b00` is 3.11:1 where WCAG AA wants 4.5:1, and that is the primary
  // button, the control this console is used through. Separating the *fill*
  // accent from a *text-safe* accent fixes legibility without altering the
  // brand: `--accent-text` is the existing `--accent-hover` value, so no new
  // colour is introduced for the tenant to approve.
  | '--accent-text'
  | '--accent-deep'
  | '--border-color'
  // Form controls are the one border that carries meaning rather than
  // decoration -- it is the only thing marking where a field is -- so it is
  // held to the 3:1 WCAG asks of a UI component, while `--border-color` stays
  // light for the card edges that are purely decorative.
  | '--input-border'
  // DEF-BO-01: the chrome around the content. Without these a themed tenant
  // got orange buttons inside a blue console, which reads as a rendering bug
  // rather than as their identity.
  | '--sidebar-from'
  | '--sidebar-to'
  | '--sidebar-text'
  | '--sidebar-link'
  | '--brand-mark-to'
  | '--panel-from'
  | '--panel-to'
  | '--panel-text'
  | '--auth-panel-from'
  | '--auth-panel-to'
  | '--auth-panel-text'
  | '--auth-panel-highlight'
  | '--tint-accent'
  | '--tint-accent-strong'
  | '--tint-accent-deep'
  | '--tint-muted'

export type ThemeVariables = Partial<Record<ThemeVariableKey, string>>

export interface Theme {
  variables: ThemeVariables
}

export const transmegaTheme: Theme = {
  variables: {
    '--bg-primary': '#f8f8f7',
    '--text-heading': '#111111',
    '--text-body': '#5f6368',
    '--accent-primary': '#ff5b00',
    '--accent-hover': '#c2410c',
    '--accent-secondary': '#f59e0b',
    '--accent-text': '#c2410c',
    '--accent-deep': '#9a3412',
    '--border-color': '#e0ded9',
    '--input-border': '#8a8178',
    '--sidebar-from': '#1c1917',
    '--sidebar-to': '#0c0a09',
    '--sidebar-text': '#f5f5f4',
    '--sidebar-link': '#d6d3d1',
    '--brand-mark-to': '#f97316',
    '--panel-from': '#7c2d12',
    '--panel-to': '#c2410c',
    '--panel-text': '#ffedd5',
    '--auth-panel-from': '#1c1917',
    '--auth-panel-to': '#7c2d12',
    '--auth-panel-text': '#ffedd5',
    '--auth-panel-highlight': '#fdba74',
    '--tint-accent': '#fff1e8',
    '--tint-accent-strong': '#ffe8d9',
    '--tint-accent-deep': '#ffddc7',
    '--tint-muted': '#faf7f4',
  },
}

const KNOWN_THEMES: Theme[] = [transmegaTheme]

// The variables with an alpha companion in main.scss (`rgba(var(--x-rgb), a)`).
const RGB_COMPANION: Partial<Record<ThemeVariableKey, string>> = {
  '--accent-primary': '--accent-primary-rgb',
  '--text-heading': '--text-heading-rgb',
  '--border-color': '--border-color-rgb',
  '--accent-secondary': '--accent-secondary-rgb',
}

const ALL_VARIABLE_KEYS = Array.from(new Set(KNOWN_THEMES.flatMap((theme) => Object.keys(theme.variables)))) as ThemeVariableKey[]

export const hexToRgbTriple = (hex: string) => {
  const value = hex.replace('#', '')
  const r = Number.parseInt(value.slice(0, 2), 16)
  const g = Number.parseInt(value.slice(2, 4), 16)
  const b = Number.parseInt(value.slice(4, 6), 16)
  return `${r},${g},${b}`
}

// There is exactly one tenant with a signed-off identity today (Transmega),
// so matching by name is honest about the current state. A `theme` field on
// the tenant record is the right shape once a *second* tenant needs its own
// identity -- adding it before one exists would be speculative (AD-011 only
// promises the mechanism, not a theme picker UI nobody can use yet).
export function themeForTenant(tenantName: string | null | undefined): Theme | null {
  return tenantName && /transmega/i.test(tenantName) ? transmegaTheme : null
}

// Applies `theme`'s variables to `:root`, clearing every variable any known
// theme could have set first -- so switching from one tenant's theme to
// another's (or back to the vendor's own default) never leaves a stale
// override behind.
export function applyTheme(theme: Theme | null) {
  const root = document.documentElement.style
  ALL_VARIABLE_KEYS.forEach((key) => {
    root.removeProperty(key)
    const companion = RGB_COMPANION[key]
    if (companion) root.removeProperty(companion)
  })
  if (!theme) return
  ;(Object.entries(theme.variables) as [ThemeVariableKey, string][]).forEach(([key, value]) => {
    root.setProperty(key, value)
    const companion = RGB_COMPANION[key]
    if (companion) root.setProperty(companion, hexToRgbTriple(value))
  })
}
