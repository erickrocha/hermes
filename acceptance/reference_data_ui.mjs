// Hermes reference-data (RD) console scenarios -- RD-031, RD-052, RD-053, RD-054.
// Run by `reference_data_acceptance.py --ui` while its fixtures exist (it passes the
// owner credentials in QA_OWNER_EMAIL / QA_OWNER_PASSWORD). Prints one JSON line per
// scenario. Playwright is not a hermes dependency: point PLAYWRIGHT_MODULE at an
// installed copy (see README). Writes only QR provinces named qa-rd-ui-*; the Python
// suite removes every QR row on exit.
import { createRequire } from 'node:module'
import { writeFileSync, mkdtempSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

const require = createRequire(import.meta.url)
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright')
const CONSOLE = process.env.HERMES_CONSOLE || 'http://localhost:5180'
const API = process.env.HERMES_API || 'http://127.0.0.1:8081'
const ADMIN = [process.env.HERMES_ADMIN_EMAIL || 'admin@hermes.dev', process.env.HERMES_ADMIN_PASSWORD || 'LocalDevOnly123!']
const OWNER = [process.env.QA_OWNER_EMAIL, process.env.QA_OWNER_PASSWORD]
const SHOTS = process.env.QA_SHOTS || mkdtempSync(join(tmpdir(), 'hermes-rd-ui-'))
const out = (id, pass, evidence) => console.log(JSON.stringify({ id, status: pass ? 'PASS' : 'FAIL', evidence }))

async function apiToken() {
  const r = await fetch(`${API}/login`, { method: 'POST', headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
    body: new URLSearchParams({ email: ADMIN[0], password: ADMIN[1] }) })
  return (await r.json()).accessToken
}
async function qrProvinces(token) {
  const r = await fetch(`${API}/province/page?pageSize=200&search=qa-rd-ui`, { headers: { Authorization: `Bearer ${token}` } })
  return Object.fromEntries((await r.json()).items.filter((p) => p.countryCode === 'QR').map((p) => [p.acronym, p.name]))
}
async function login(browser, [email, password]) {
  const context = await browser.newContext({ locale: 'pt-BR' })
  const page = await context.newPage()
  await page.goto(`${CONSOLE}/login`)
  await page.locator('input[type=email]').fill(email)
  await page.locator('input[autocomplete=current-password]').fill(password)
  await page.locator('button.btn.primary.full').click()
  await page.waitForURL((url) => !url.pathname.startsWith('/login'), { timeout: 15000 })
  await page.locator('a[href="/tenants"]').first().waitFor({ timeout: 15000 })  // shell rendered
  return page
}
function csv(name, text) {
  const file = join(SHOTS, name)
  writeFileSync(file, text)
  return file
}

const browser = await chromium.launch()
const token = await apiToken()
try {
  // RD-052 System Settings is a SysAdmin-only area
  {
    const admin = await login(browser, ADMIN)
    const adminLink = await admin.locator('a[href="/system-settings"]').count()
    await admin.goto(`${CONSOLE}/system-settings`)
    await admin.locator('table tbody tr').first().waitFor({ timeout: 15000 })
    const tabs = await admin.getByRole('tab').allInnerTexts()
    const rows = await admin.locator('table tbody tr').count()
    await admin.screenshot({ path: join(SHOTS, 'rd-052-admin.png') })
    let ownerLink = -1, ownerUrl = '', ownerTabs = -1
    if (OWNER[0]) {
      const owner = await login(browser, OWNER)
      ownerLink = await owner.locator('a[href="/system-settings"]').count()
      await owner.goto(`${CONSOLE}/system-settings`)
      await owner.waitForTimeout(1500)
      ownerUrl = new URL(owner.url()).pathname
      ownerTabs = await owner.getByRole('tab').count()
      await owner.context().close()
    }
    out('RD-052', adminLink === 1 && tabs.length === 2 && rows > 0 && ownerLink === 0 && ownerUrl !== '/system-settings' && ownerTabs === 0,
      `SysAdmin: nav link ${adminLink}, tabs ${JSON.stringify(tabs)}, ${rows} rows on page 1; TenantOwner: nav link ${ownerLink}, ` +
      `direct URL landed on ${ownerUrl}, tabs ${ownerTabs}`)

    // RD-053 upload CSV -> editable review grid -> edit a cell -> Save -> stored as edited
    const dialogs = []
    admin.on('dialog', async (d) => { dialogs.push(d.message()); await d.accept() })
    await admin.locator('input[type=file]').setInputFiles(csv('rd-053.csv',
      '﻿Acronym;Name;Country Code\nU1;qa-rd-ui-one;QR\nu2;qa-rd-ui-tyop;qr\n'))
    const review = admin.locator('section.import-review')
    await review.waitFor({ timeout: 10000 })
    const loaded = await review.locator('tbody tr').count()
    const cell = review.locator('tbody tr').nth(1).locator('input').nth(1)
    const before = await cell.inputValue()
    await cell.fill('qa-rd-ui-two')
    await admin.screenshot({ path: join(SHOTS, 'rd-053-review.png') })
    await review.locator('button.btn.primary').click()
    await review.waitFor({ state: 'detached', timeout: 10000 })
    await admin.waitForTimeout(800)
    const stored = await qrProvinces(token)
    out('RD-053', loaded === 2 && before === 'qa-rd-ui-tyop' && stored.U1 === 'qa-rd-ui-one' && stored.U2 === 'qa-rd-ui-two' && !('qa-rd-ui-tyop' in stored),
      `';'-delimited CSV with BOM and 'Country Code' header -> review grid ${loaded} rows; row 2 name '${before}' edited to ` +
      `'qa-rd-ui-two' before Save; confirmation "${dialogs.join(' / ')}"; stored ${JSON.stringify(stored)}`)

    // RD-054 a rejected batch: error names the row, grid stays for correction, nothing written; fix and save
    await admin.locator('input[type=file]').setInputFiles(csv('rd-054.csv',
      'acronym,name,countryCode\nU3,qa-rd-ui-three,QR\nU4,,QR\n'))
    await review.waitFor({ timeout: 10000 })
    await review.locator('button.btn.primary').click()
    const error = review.locator('.import-error')
    await error.waitFor({ timeout: 10000 })
    const message = await error.innerText()
    await admin.screenshot({ path: join(SHOTS, 'rd-054-rejected.png') })
    const afterReject = await qrProvinces(token)
    const gridKept = await review.locator('tbody tr').count()
    await review.locator('tbody tr').nth(1).locator('input').nth(1).fill('qa-rd-ui-four')
    await review.locator('button.btn.primary').click()
    await review.waitFor({ state: 'detached', timeout: 10000 })
    await admin.waitForTimeout(800)
    const afterFix = await qrProvinces(token)
    // The row-number assertion used to be English-only (`/row 2/`), but this
    // browser runs in pt-BR (RD-055 pins the three translations), so a correctly
    // localised message -- "linha 2: o nome é obrigatório" -- read as a failure.
    // Accept the row word in any of the three console languages.
    const rowN = (n) => new RegExp(`(row|linha|fila) ${n}`)
    out('RD-054', rowN(2).test(message) && !new RegExp('(row|linha|fila) 1:').test(message)
      && !('U3' in afterReject) && gridKept === 2
      && afterFix.U3 === 'qa-rd-ui-three' && afterFix.U4 === 'qa-rd-ui-four',
      `Save -> error shown in the review grid: "${message}" (console language pt-BR); U3 written after rejection=${'U3' in afterReject}; ` +
      `grid kept ${gridKept} rows; after fixing row 2 in the grid and saving again -> U3/U4 stored=${Boolean(afterFix.U3 && afterFix.U4)}`)

    // RD-031 the D-9 path: tenant editor offers Brazilian provinces, then that province's cities
    const feeds = []
    admin.on('response', (r) => { if (/\/(province\?|cities\/by-province)/.test(r.url())) feeds.push(r) })
    await admin.goto(`${CONSOLE}/tenants/new`)
    await admin.waitForTimeout(1500)
    // DEF-RD-08 (PD-027) replaced the hardcoded BR/US radios with a country
    // <select> fed by whichever countries actually have reference data, so
    // `input[name=countryCode]` no longer exists. The scenario is unchanged --
    // a pt-BR browser lands on BR, and the province/city feeds follow it.
    const countrySelect = admin.locator('select#tenant-country')
    await countrySelect.waitFor({ timeout: 15000 })
    const countries = await countrySelect.locator('option').evaluateAll((els) => els.map((e) => e.value))
    const checked = await countrySelect.inputValue()
    // `getByRole('combobox')` used to reach the province picker first. Since
    // DEF-RD-08 the country field is a native <select>, which also carries the
    // combobox role, so it now shadows the province picker at index 0. Address
    // the console's own Combobox component instead of the ARIA role.
    const province = admin.locator('.combobox-control').nth(0).locator('input')
    await province.click()
    await province.fill('Paulo')
    const provOptions = await admin.getByRole('option').allInnerTexts()
    await admin.getByRole('option', { name: 'São Paulo (SP)' }).click()
    await admin.waitForTimeout(1200)
    const city = admin.locator('.combobox-control').nth(1).locator('input')
    await city.click()
    const cityOptions = await admin.getByRole('option').allInnerTexts()
    await admin.screenshot({ path: join(SHOTS, 'rd-031-tenant-editor.png') })
    const feedLog = await Promise.all(feeds.map(async (r) => `${new URL(r.url()).pathname}${new URL(r.url()).search} ${r.status()}`))
    out('RD-031', checked === 'BR' && provOptions.includes('São Paulo (SP)') && cityOptions.includes('Campinas') && cityOptions.includes('São Paulo'),
      `pt-BR browser -> country ${checked} preselected (choices ${JSON.stringify(countries)}); province search 'Paulo' -> ` +
      `${JSON.stringify(provOptions)}; after SP, ${cityOptions.length} city options incl. Campinas/São Paulo; feeds ${JSON.stringify(feedLog)}`)
    await admin.context().close()
  }
} catch (e) {
  console.error(e)
  process.exitCode = 1
} finally {
  await browser.close()
  console.error(`screenshots: ${SHOTS}`)
}
