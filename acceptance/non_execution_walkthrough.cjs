// C-026 `EPIC-MT-04` (NE-UI-01): the mechanic opens "Non-executed maintenance", sees the order grouped under its vehicle, justifies it on its
// own page and the order leaves the inbox; the owner has "Evaluate now" and the manual-record page refuses what the tracker does not evidence.
// Headless Chrome against the test stack (`infra/test`: console :5174, API :8082). Needs the fixtures left by `NE_KEEP=1
// non_execution_acceptance.py` (owner qa-ga-owner@hermes.test, mechanic qa-ga-ne-mech@hermes.test, vehicle QAGANEK with an unresolved order
// that "did not appear"). Clean up afterwards with driver_app_acceptance.cleanup().
const { chromium } = require('playwright-core')
const CONSOLE = process.env.HERMES_CONSOLE || 'http://localhost:5174'
const API = process.env.HERMES_API || 'http://127.0.0.1:8082'
const PASSWORD = 'QaGarageMonitor#2026'
const rec = (id, ok, ev) => { console.log(`  ${id}  ${ok ? 'PASS' : 'FAIL'}  ${ev}`); if (!ok) process.exitCode = 1 }

async function api(method, path, token, body) {
  const res = await fetch(API + path, { method, headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' }, body: body ? JSON.stringify(body) : undefined })
  return { status: res.status, body: await res.json().catch(() => null) }
}
async function token(email) {
  const res = await fetch(`${API}/login`, { method: 'POST', headers: { 'Content-Type': 'application/x-www-form-urlencoded' }, body: new URLSearchParams({ email, password: PASSWORD }) })
  return (await res.json()).accessToken
}
async function signIn(browser, email) {
  const page = await (await browser.newContext({ locale: 'en-US', viewport: { width: 1360, height: 900 } })).newPage()
  page.errors = []
  page.on('pageerror', (e) => page.errors.push(String(e.message).slice(0, 160)))
  await page.goto(`${CONSOLE}/login`)
  await page.fill('input[type=email]', email)
  await page.fill('input[type=password]', PASSWORD)
  await page.click('form.auth-card button.btn.primary')
  await page.waitForSelector('.sidebar nav', { timeout: 10000 })
  return page
}
const inboxRow = async (owner, plate) => {
  const groups = (await api('GET', '/non-execution?page=0&pageSize=100', owner)).body.items
  return groups.find((g) => g.vehicle.plate === plate)
}

;(async () => {
  const owner = await token('qa-ga-owner@hermes.test')
  const before = await inboxRow(owner, 'QAGANEK')
  const orderUuid = before?.workOrders[0]?.workOrderUuid
  const browser = await chromium.launch({ executablePath: '/usr/bin/google-chrome', headless: true })

  // The mechanic: menu -> inbox grouped by vehicle -> Justify page -> back, and the order is gone.
  const mech = await signIn(browser, 'qa-ga-ne-mech@hermes.test')
  await mech.click('.sidebar nav a[href="/non-executions"]')
  await mech.waitForSelector('.non-execution-vehicle', { timeout: 10000 })
  const group = mech.locator('.non-execution-vehicle', { hasText: 'QAGANEK' })
  const text = await group.innerText()
  const crumbs = await mech.locator('.breadcrumbs').innerText()
  rec('NE-UI-01a', /QAGANEK/.test(text) && /1× unresolved/.test(text) && /Did not appear/.test(text) && (await mech.getByRole('button', { name: /evaluate now/i }).count()) === 0 && /Non-executed maintenance/.test(crumbs),
    `the mechanic's menu entry opens the inbox: vehicle group "${text.replace(/\n/g, ' | ').slice(0, 150)}"; no "Evaluate now" button for the mechanic`)

  await group.getByRole('link', { name: /^justify$/i }).click()
  await mech.waitForURL(/\/non-executions\/[0-9a-f-]+\/justify$/, { timeout: 10000 })
  await mech.waitForSelector('form.page-form-card', { timeout: 10000 })
  const dialogs = await mech.locator('.modal, [role=dialog]').count()
  const heading = await mech.locator('h1').innerText()
  const trail = await mech.locator('.breadcrumbs').innerText()
  const onPage = dialogs === 0 && /Justif/.test(heading) && /Non-executed maintenance/i.test(trail)
  await mech.getByRole('button', { name: /^save$/i }).click()
  const required = await mech.getByText(/required field/i).count()
  await mech.getByLabel(/justification/i).fill('Part is on its way; the vehicle left for a trip.')
  await mech.getByRole('button', { name: /^save$/i }).click()
  await mech.waitForURL(/\/non-executions$/, { timeout: 10000 })
  await mech.waitForSelector('.non-execution-vehicle', { timeout: 10000 })
  await mech.waitForFunction(() => !document.body.innerText.includes('QAGANEK'), null, { timeout: 10000 })
  const gone = (await mech.locator('.non-execution-vehicle', { hasText: 'QAGANEK' }).count()) === 0
  const status = (await api('GET', `/work-order/uuid/${orderUuid}`, owner)).body.status
  const history = (await api('GET', `/non-execution/work-order/uuid/${orderUuid}`, owner)).body.workOrder
  rec('NE-UI-01b', onPage && required >= 1 && gone && status === 'Open' && history.occurrences[0].justification?.startsWith('Part is on its way'),
    `Justify is its own page (no dialog, breadcrumb present), an empty text shows "required" (${required}), the saved justification returns to the inbox where QAGANEK is gone (${gone}); the work order is still ${status} and the history keeps the occurrence`)
  rec('NE-UI-01-errors-mechanic', mech.errors.length === 0, `page errors ${mech.errors.length}`)

  // The owner: "Evaluate now" and the manual page, which says no when the tracker has no evidence.
  const boss = await signIn(browser, 'qa-ga-owner@hermes.test')
  await boss.click('.sidebar nav a[href="/non-executions"]')
  await boss.getByRole('button', { name: /evaluate now/i }).click()
  await boss.getByRole('status').waitFor({ timeout: 10000 })
  const outcome = await boss.getByRole('status').innerText()
  await boss.getByRole('link', { name: /record manually/i }).click()
  await boss.waitForURL(/\/non-executions\/manual$/, { timeout: 10000 })
  await boss.waitForFunction(() => [...document.querySelectorAll('select option')].some((o) => /qa-ga-ne nek-ui/.test(o.textContent)), null, { timeout: 10000 })
  await boss.locator('select#workOrderUuid').selectOption({ label: await boss.locator('select#workOrderUuid option', { hasText: 'qa-ga-ne nek-ui' }).first().innerText() })
  await boss.getByLabel(/justification/i).fill('The driver said it left.')
  await boss.getByRole('button', { name: /^save$/i }).click()
  const refusal = await boss.getByRole('alert').innerText()
  rec('NE-UI-01c', /Evaluation done/.test(outcome) && /does not evidence|not evidence/i.test(refusal) && /\/non-executions\/manual$/.test(boss.url()) && boss.errors.length === 0,
    `the owner's "Evaluate now" reports "${outcome}"; recording by hand for an order the tracker never saw stays on its page and says "${refusal}"; page errors ${boss.errors.length}`)
  await browser.close()
})().catch((e) => { console.error('QA BLOCKED:', e.message); process.exit(2) })
