// PD-027: o CSV é lido no navegador, mostrado editável e só então enviado.
// Por isso o parser vive aqui e não no servidor -- e por isso ele precisa ser
// tolerante: o arquivo vem de uma planilha de alguém, não de um exportador.

export interface ParsedCsv {
  headers: string[]
  rows: string[][]
}

/// Divide uma linha respeitando aspas: `São Paulo, SP` dentro de aspas é um
/// campo só. Aspas duplicadas (`""`) são uma aspa literal, como no Excel.
function splitLine(line: string, delimiter: string): string[] {
  const fields: string[] = []
  let current = ''
  let quoted = false

  for (let index = 0; index < line.length; index += 1) {
    const char = line[index]
    if (quoted) {
      if (char === '"') {
        if (line[index + 1] === '"') { current += '"'; index += 1 } else { quoted = false }
      } else current += char
    } else if (char === '"') quoted = true
    else if (char === delimiter) { fields.push(current); current = '' }
    else current += char
  }
  fields.push(current)
  return fields.map((field) => field.trim())
}

/// Planilhas brasileiras e europeias exportam com `;` porque a vírgula é o
/// separador decimal. Adivinhar pela primeira linha evita obrigar o operador a
/// saber qual delas o Excel dele usou.
function detectDelimiter(headerLine: string): string {
  const candidates = [',', ';', '\t']
  return candidates.reduce((best, candidate) =>
    splitLine(headerLine, candidate).length > splitLine(headerLine, best).length ? candidate : best,
  candidates[0])
}

export function parseCsv(text: string): ParsedCsv {
  // BOM do Excel: invisível, e sem remover vira parte do primeiro cabeçalho,
  // que então não casa com nenhuma coluna esperada.
  const clean = text.replace(/^﻿/, '')
  const lines = clean.split(/\r\n|\n|\r/).filter((line) => line.trim() !== '')
  if (!lines.length) return { headers: [], rows: [] }

  const delimiter = detectDelimiter(lines[0])
  const headers = splitLine(lines[0], delimiter)
  const rows = lines.slice(1).map((line) => {
    const fields = splitLine(line, delimiter)
    // Linha curta é preenchida em vez de rejeitada: a grade é editável, então
    // o operador vê a célula vazia e a completa.
    return headers.map((_, index) => fields[index] ?? '')
  })
  return { headers, rows }
}

/// Casa os cabeçalhos do arquivo com os campos esperados, ignorando caixa,
/// espaços, acentos e separadores. `Country Code`, `country_code` e
/// `CÓDIGO PAÍS` não deveriam ser três problemas diferentes.
export function matchColumns(headers: string[], fields: string[]): Record<string, number> {
  const normalize = (value: string) =>
    value.normalize('NFD').replace(/[̀-ͯ]/g, '').toLowerCase().replace(/[^a-z0-9]/g, '')
  const normalized = headers.map(normalize)
  const mapping: Record<string, number> = {}
  fields.forEach((field) => {
    const index = normalized.indexOf(normalize(field))
    if (index >= 0) mapping[field] = index
  })
  return mapping
}
