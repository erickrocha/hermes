// C-031 `CA-116` (`SR-CA-03-10`): the owner makes a checklist template the default in the console, in headless Chrome against the
// test stack (`infra/test`: console :5174, API :8082). Needs the `qa-ga-` fixtures (`driver_app_acceptance.py`-style owner
// `qa-ga-owner@hermes.test`) and playwright-core on NODE_PATH. It creates one `qa-ga-ui-` template through the API and leaves it.
const { chromium } = require('playwright-core')
const CONSOLE = process.env.HERMES_CONSOLE || 'http://localhost:5174'
const API = process.env.HERMES_API || 'http://127.0.0.1:8082'
const PASSWORD = 'QaGarageMonitor#2026'
const rec = (id, ok, ev) => { console.log(`  ${id}  ${ok ? 'PASS' : 'FAIL'}  ${ev}`); if (!ok) process.exitCode = 1 }

async function api(method, path, token, body) {
  const res = await fetch(API + path, { method, headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' }, body: body ? JSON.stringify(body) : undefined })
  return { status: res.status, body: await res.json().catch(() => null) }
}

;(async () => {
  const login = await fetch(`${API}/login`, { method: 'POST', headers: { 'Content-Type': 'application/x-www-form-urlencoded' }, body: new URLSearchParams({ email: 'qa-ga-owner@hermes.test', password: PASSWORD }) })
  const token = (await login.json()).accessToken
  const created = await api('POST', '/checklist-template', token, { name: 'qa-ga-ui-default', checklistType: 'Departure', active: true, items: [{ description: 'ui item', generatesWorkOrder: false }] })
  const uuid = created.body.uuid
  const before = (await api('GET', '/checklist-template?page=0&pageSize=50', token)).body.items.filter((t) => t.checklistType === 'Departure' && t.isDefault).map((t) => t.name)

  const browser = await chromium.launch({ executablePath: '/usr/bin/google-chrome', headless: true })
  const page = await (await browser.newContext({ locale: 'en-US', viewport: { width: 1360, height: 900 } })).newPage()
  const errors = []
  page.on('pageerror', (e) => errors.push(String(e.message).slice(0, 160)))
  await page.goto(`${CONSOLE}/login`)
  await page.fill('input[type=email]', 'qa-ga-owner@hermes.test')
  await page.fill('input[type=password]', PASSWORD)
  await page.click('form.auth-card button.btn.primary')
  await page.waitForSelector('.sidebar nav', { timeout: 10000 })
  await page.goto(`${CONSOLE}/checklist-templates/${uuid}`)
  await page.waitForSelector('.page-header', { timeout: 10000 })
  const button = page.getByRole('button', { name: /set as default/i })
  await button.waitFor({ timeout: 10000 })
  const shownBefore = await page.locator('text=Default template').first().textContent().catch(() => '')
  await button.click()
  await page.waitForFunction(() => !/set as default/i.test(document.body.innerText), null, { timeout: 10000 })
  const detail = await page.locator('main, .page, body').first().innerText()
  const after = (await api('GET', '/checklist-template?page=0&pageSize=50', token)).body.items.filter((t) => t.checklistType === 'Departure' && t.isDefault).map((t) => t.name)
  rec('CA-116', created.status === 201 && !before.includes('qa-ga-ui-default') && after.length === 1 && after[0] === 'qa-ga-ui-default' && /Default template\s*Yes/i.test(detail.replace(/\n+/g, ' ')) && errors.length === 0,
    `before the click the defaults were ${JSON.stringify(before)}; the button is gone after it and the page says Default template: Yes; the API now has exactly ${JSON.stringify(after)}; page errors ${errors.length}`)
  await browser.close()
})().catch((e) => { console.error('QA BLOCKED:', e.message); process.exit(2) })
