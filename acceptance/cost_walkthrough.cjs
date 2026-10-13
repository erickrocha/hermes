// C-030 cost screens, driven in headless Chrome against the running console (:5173) and API (:8080).
// Needs the fixtures left by `KEEP_FIXTURES=1 cost_report_acceptance.py` and playwright-core on NODE_PATH.
const path = require('path')
const { chromium } = require('playwright-core')
const BASE = process.env.HERMES_CONSOLE || 'http://localhost:5173'
const PASSWORD = 'QaGarageMonitor#2026'
const SHOTS = path.join(__dirname, 'evidence', 'costs')
const out = []
const rec = (id, ok, ev) => { out.push([id, ok]); console.log(`  ${id}  ${ok ? 'PASS' : 'FAIL'}  ${ev}`) }

async function login(browser, who) {
  const ctx = await browser.newContext({ locale: 'en-US', viewport: { width: 1360, height: 900 }, acceptDownloads: true })
  const page = await ctx.newPage()
  page.errors = []
  page.on('pageerror', (e) => page.errors.push(String(e.message).slice(0, 160)))
  await page.goto(`${BASE}/login`)
  await page.fill('input[type=email]', `qa-ga-${who}@hermes.test`)
  await page.fill('input[type=password]', PASSWORD)
  await page.click('form.auth-card button.btn.primary')
  await page.waitForURL((u) => !u.pathname.startsWith('/login'), { timeout: 10000 })
  await page.waitForSelector('.sidebar nav', { timeout: 10000 })
  await page.waitForTimeout(800)
  return page
}
const nav = (page) => page.$$eval('.sidebar nav a', (as) => as.map((a) => a.textContent.trim()))
const setDates = async (page, from, to) => { const d = page.locator('input[type=date]'); await d.nth(0).fill(from); await d.nth(1).fill(to) }
const pick = async (page, text) => { const box = page.locator('input[role=combobox], .combobox input, form input[type=text]').first(); await box.click(); await page.getByRole('option', { name: text }).first().click() }

;(async () => {
  const browser = await chromium.launch({ executablePath: process.env.CHROME_PATH || '/usr/bin/google-chrome', headless: true })
  try {
    // BO-080 -- who is offered the screen
    const seen = {}
    for (const who of ['tenantuser', 'driver', 'mechanic']) { const p = await login(browser, who); seen[who] = (await nav(p)).includes('Costs'); await p.context().close() }
    rec('BO-080', seen.tenantuser && !seen.driver && !seen.mechanic, `menu offers Costs: ${JSON.stringify(seen)}`)

    const page = await login(browser, 'tenantuser')
    await page.click('.sidebar nav a:has-text("Costs")')
    await page.waitForSelector('text=Vehicle cost')
    await pick(page, /QAGAC1/)
    await setDates(page, '2026-09-01', '2026-09-30')
    await page.getByRole('button', { name: /^generate$/i }).click()
    await page.waitForSelector('text=Total cost', { timeout: 10000 })
    const text = await page.locator('main').innerText()
    await page.screenshot({ path: path.join(SHOTS, 'vehicle-cost.png'), fullPage: true })
    const has = (s) => text.includes(s)
    rec('BO-081', has('745.00') && has('125.00') && has('450.00') && has('100.00') && has('70.00') && has('2,000.00') && has('1,255.00') && has('To be classified') && has('Brakes'),
      `screen shows 745.00 total, the four sources, revenue 2,000.00, result 1,255.00, "To be classified" and Brakes`)

    // the CSV download keeps the API's file name
    const [dl] = await Promise.all([page.waitForEvent('download'), page.getByRole('button', { name: /download csv/i }).click()])
    const file = dl.suggestedFilename(); const body = require('fs').readFileSync(await dl.path(), 'utf8')
    rec('BO-083', file === 'vehicle-expenses_QAGAC1_2026-09-01_2026-09-30.csv' && body.startsWith('﻿') && body.includes('Subtotal,,,450.00'),
      `downloaded ${file}, ${body.length} bytes, BOM ${body.startsWith('﻿')}`)

    // October: the service performed today, the 300.00 invoice
    await setDates(page, '2026-10-01', '2026-10-31')
    await page.getByRole('button', { name: /^generate$/i }).click()
    await page.waitForSelector('text=Services performed')
    const oct = await page.locator('main').innerText()
    await page.screenshot({ path: path.join(SHOTS, 'vehicle-cost-october.png'), fullPage: true });
    rec('BO-084', oct.includes('QA Wash') && oct.includes('300.00') && oct.toLowerCase().includes('no priced trip'), `October: QA Wash 1, external 300.00, no result for a vehicle with no priced trip in the period`)

    // supplier closing
    await page.getByRole('tab', { name: /supplier closing/i }).click()
    await pick(page, /Oficina Sul/i)
    await setDates(page, '2026-09-01', '2026-09-30')
    await page.getByRole('button', { name: /^generate$/i }).click()
    await page.waitForSelector('text=NF-3', { timeout: 10000 })
    const sup = await page.locator('main').innerText()
    await page.screenshot({ path: path.join(SHOTS, 'supplier-closing.png'), fullPage: true })
    const [dl2] = await Promise.all([page.waitForEvent('download'), page.getByRole('button', { name: /download csv/i }).click()])
    const body2 = require('fs').readFileSync(await dl2.path(), 'utf8')
    rec('BO-082', sup.includes('NF-3') && sup.includes('450.00') && !sup.includes('NF-2') && dl2.suggestedFilename().startsWith('supplier-closing_Oficina') && body2.includes('NF-3'),
      `September lists NF-3 450.00 and not the October invoice; downloaded ${dl2.suggestedFilename()}`)

    // a driver reaching the route by hand is sent home
    const d = await login(browser, 'driver')
    await d.goto(`${BASE}/costs`); await d.waitForTimeout(800)
    rec('BO-080b', new URL(d.url()).pathname === '/', `a driver typing /costs lands on ${new URL(d.url()).pathname}`)
    rec('BO-085', page.errors.length === 0 && d.errors.length === 0, `no page errors: ${JSON.stringify([...page.errors, ...d.errors])}`)
  } finally { await browser.close() }
  process.exit(out.every(([, ok]) => ok) ? 0 : 1)
})().catch((e) => { console.error('BLOCKED', e.message); process.exit(2) })
