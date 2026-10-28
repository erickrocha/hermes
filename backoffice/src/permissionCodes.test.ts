import { readdirSync, readFileSync, statSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'

// Um botão que nomeia uma permissão que o servidor não conhece fica escondido para todo mundo, em silêncio.
// Este teste liga cada código usado no console à tabela de operações do backend.
const backend = join(__dirname, '../../backend/business/src/domain')
const known = new Set([...readFileSync(join(backend, 'operation_permission.rs'), 'utf8').matchAll(/"([a-z][a-z0-9.-]+\.[a-z0-9.-]+)"\)/g)].map((m) => m[1]))
for (const m of readFileSync(join(backend, 'permission.rs'), 'utf8').matchAll(/code: "([a-z0-9.-]+)"/g)) known.add(m[1])

const sources = (dir: string): string[] => readdirSync(dir).flatMap((name) => {
  const path = join(dir, name)
  return statSync(path).isDirectory() ? sources(path) : /\.(ts|tsx)$/.test(name) && !/\.test\./.test(name) ? [path] : []
})

describe('permission codes used by the console', () => {
  it('all exist in the backend', () => {
    const used = new Map<string, string>()
    for (const file of sources(__dirname)) {
      // C-035: `can`, `canPlatform`, `has`, `hasFull`, `onlyOwn` -- every helper that names a permission code.
      for (const m of readFileSync(file, 'utf8').matchAll(/\b(?:can|canPlatform|has|hasFull|onlyOwn)\(\s*[\w.!]+,\s*'([a-z0-9.-]+)'/g)) used.set(m[1], file)
      for (const m of readFileSync(file, 'utf8').matchAll(/USER_LIST_HOLDERS = \[([^\]]+)\]/g)) for (const code of m[1].matchAll(/'([a-z0-9.-]+)'/g)) used.set(code[1], file)
    }
    const unknown = [...used].filter(([code]) => !known.has(code))
    expect(unknown, `unknown codes: ${JSON.stringify(unknown)}`).toEqual([])
    expect(used.size).toBeGreaterThan(30)
  })
})
