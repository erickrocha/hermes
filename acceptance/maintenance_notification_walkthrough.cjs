// C-026 `EPIC-MT-06` (TRM-616): the mechanic sees the bell with the unread counter after a part is received and marks the notice as read,
// in headless Chrome against the test stack (`infra/test`: console :5174, API :8082). Needs the fixtures left by
// `MN_KEEP=1 maintenance_notification_acceptance.py` (owner qa-ga-owner@hermes.test, mechanic qa-ga-mn-mech@hermes.test, two unread
// notices); it receives one more purchase order through the API. Clean up afterwards with driver_app_acceptance.cleanup().
const { chromium } = require('playwright-core')
const CONSOLE = process.env.HERMES_CONSOLE || 'http://localhost:5174'
const API = process.env.HERMES_API || 'http://127.0.0.1:8082'
const PASSWORD = 'QaGarageMonitor#2026'
const rec = (id, ok, ev) => { console.log(`  ${id}  ${ok ? 'PASS' : 'FAIL'}  ${ev}`); if (!ok) process.exitCode = 1 }

async function api(method, path, token, body) {
  const res = await fetch(API + path, { method, headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' }, body: body ? JSON.stringify(body) : undefined })
  return { status: res.status, body: await res.json().catch(() => null) }
}
async function login(email) {
  const res = await fetch(`${API}/login`, { method: 'POST', headers: { 'Content-Type': 'application/x-www-form-urlencoded' }, body: new URLSearchParams({ email, password: PASSWORD }) })
  return (await res.json()).accessToken
}

;(async () => {
  const owner = await login('qa-ga-owner@hermes.test')
  const startCount = (await api('GET', '/maintenance-notification/unread-count', owner)).body.count

  const browser = await chromium.launch({ executablePath: '/usr/bin/google-chrome', headless: true })
  const page = await (await browser.newContext({ locale: 'en-US', viewport: { width: 1360, height: 900 } })).newPage()
  const errors = []
  page.on('pageerror', (e) => errors.push(String(e.message).slice(0, 160)))
  await page.goto(`${CONSOLE}/login`)
  await page.fill('input[type=email]', 'qa-ga-mn-mech@hermes.test')
  await page.fill('input[type=password]', PASSWORD)
  await page.click('form.auth-card button.btn.primary')
  await page.waitForSelector('.sidebar nav', { timeout: 10000 })
  const counter = () => page.locator('[data-testid=notification-count]')
  await counter().waitFor({ timeout: 10000 })
  const shown = Number(await counter().textContent())

  // A part is received while the mechanic has the console open: the counter is re-read on the next navigation.
  const works = (await api('GET', '/work-order?page=0&pageSize=5', owner)).body.items
  const part = (await api('POST', '/part', owner, { name: 'qa-ga-mn-walk', unit: 'un', minimumStock: 0 })).body
  await api('POST', `/work-order/uuid/${works[0].uuid}/item`, owner, { description: 'walk', status: 'Pending' })
  const items = (await api('GET', `/work-order/uuid/${works[0].uuid}`, owner)).body.items
  const po = (await api('POST', '/purchase-order', owner, { partUuid: part.uuid, quantity: 7, workOrderUuid: works[0].uuid, workOrderItemUuid: items[items.length - 1].uuid })).body
  await api('PUT', `/purchase-order/uuid/${po.uuid}/mark-ordered`, owner, {})
  const received = await api('PUT', `/purchase-order/uuid/${po.uuid}/mark-purchased`, owner)
  await page.click('.sidebar nav a[href="/work-orders"]')
  await page.waitForFunction((n) => Number(document.querySelector('[data-testid=notification-count]')?.textContent) === n, shown + 1, { timeout: 10000 })
  rec('MN-UI-01', startCount === shown && received.status === 200, `the bell showed ${shown} unread (API said ${startCount}); after a part was received it shows ${await counter().textContent()}`)

  await page.click('a.notification-bell')
  await page.waitForSelector('.notification-item.unread', { timeout: 10000 })
  const unreadRows = await page.locator('.notification-item.unread').count()
  const first = await page.locator('.notification-item.unread').first().innerText()
  const link = await page.locator('.notification-item.unread a').first().getAttribute('href')
  rec('MN-UI-02', unreadRows === shown + 1 && /qa-ga-mn-walk/.test(first) && /^\/work-orders\//.test(link ?? ''), `${unreadRows} unread rows, newest first: "${first.replace(/\n/g, ' | ')}", linking to ${link}`)

  await page.locator('.notification-item.unread').first().getByRole('button', { name: /mark as read/i }).click()
  await page.waitForFunction((n) => Number(document.querySelector('[data-testid=notification-count]')?.textContent) === n, shown, { timeout: 10000 })
  const afterOne = await page.locator('.notification-item.unread').count()
  rec('MN-UI-03', afterOne === shown && (await api('GET', '/maintenance-notification/unread-count', owner)).body.count === shown, `after "Mark as read" the row turns read (${afterOne} unread rows) and the bell shows ${await counter().textContent()}`)

  await page.getByRole('button', { name: /mark all as read/i }).click()
  await page.waitForFunction(() => !document.querySelector('[data-testid=notification-count]'), null, { timeout: 10000 })
  rec('MN-UI-04', (await page.locator('.notification-item.unread').count()) === 0 && (await api('GET', '/maintenance-notification/unread-count', owner)).body.count === 0 && errors.length === 0,
    `"Mark all as read" clears the bell and every row; the API counter is 0; page errors ${errors.length}`)
  await browser.close()
})().catch((e) => { console.error('QA BLOCKED:', e.message); process.exit(2) })
