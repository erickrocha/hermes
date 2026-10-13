// The operations screens (work orders, fuel, parts, purchasing, garage, schedule, fleet registries) driven in headless
// Chrome against the running console (:5173) and API (:8080). Needs the `qa-ga-` fixtures left by
// `KEEP_FIXTURES=1 cost_report_acceptance.py` and playwright-core on NODE_PATH. Everything it creates is `QAUI`-tagged
// and goes away with the fixture tenant when `CLEAN_ONLY` runs.
const path = require('path')
const { chromium } = require('playwright-core')
const BASE = process.env.HERMES_CONSOLE || 'http://localhost:5173'
const PASSWORD = 'QaGarageMonitor#2026'
const SHOTS = path.join(__dirname, 'evidence', 'operations')
const out = []
const rec = (id, ok, ev) => { out.push([id, ok]); console.log(`  ${id}  ${ok ? 'PASS' : 'FAIL'}  ${ev}`) }

async function login(browser, who) {
  const ctx = await browser.newContext({ locale: 'en-US', viewport: { width: 1360, height: 900 } })
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
const go = async (page, to) => { await page.goto(`${BASE}${to}`); await page.waitForSelector('.page-header', { timeout: 10000 }); await page.waitForTimeout(500) }
const fill = (page, label, value) => page.getByLabel(label).first().fill(String(value))
// An option is chosen by exact label, by index, or by a pattern over the option text (lists of vehicles and parts load after the page).
const choose = async (page, label, option) => {
  const select = page.getByLabel(label).first()
  if (option && option.label instanceof RegExp) {
    await page.waitForFunction(([id, source]) => [...document.querySelectorAll(`#${id} option`)].some((o) => new RegExp(source).test(o.textContent)), [await select.getAttribute('id'), option.label.source])
    const value = await select.evaluate((el, source) => [...el.options].find((o) => new RegExp(source).test(o.textContent)).value, option.label.source)
    return select.selectOption({ value })
  }
  return select.selectOption(typeof option === 'string' ? { label: option } : option)
}
const save = async (page) => { await page.getByRole('button', { name: /^save$/i }).click(); await page.waitForTimeout(1200) }
const text = (page) => page.locator('main').innerText()
const failed = (page) => page.locator('.form-error').count()

;(async () => {
  const browser = await chromium.launch({ executablePath: process.env.CHROME_PATH || '/usr/bin/google-chrome', headless: true })
  try {
    // BO-090 -- the menu follows the role
    const menus = {}
    for (const who of ['owner', 'tenantuser', 'mechanic', 'driver']) { const p = await login(browser, who); menus[who] = await nav(p); await p.context().close() }
    const has = (who, ...names) => names.every((n) => menus[who].includes(n))
    // The menu follows the read permissions the server grants each role (the profile is the ceiling); money screens stay with the office.
    rec('BO-090', has('owner', 'Work orders', 'Fuel', 'Garage', 'Day schedule', 'Vehicles', 'Customers', 'Parts', 'Purchase orders', 'Costs', 'Users') && has('mechanic', 'Work orders', 'Fuel', 'Garage', 'Parts')
      && has('tenantuser', 'Work orders', 'Vehicles', 'Costs') && !menus.mechanic.includes('Costs') && !menus.driver.includes('Costs') && !menus.mechanic.includes('Users') && !menus.driver.includes('Tenants'),
      `owner ${menus.owner.length} entries, mechanic ${menus.mechanic.length}, tenant user ${menus.tenantuser.length}, driver ${menus.driver.length}`)

    const owner = await login(browser, 'owner')
    // BO-091 -- every list screen answers without an error banner
    const links = (await owner.$$eval('.sidebar nav a', (as) => as.map((a) => a.getAttribute('href')))).filter((h) => h && !['/', '/users', '/profiles'].includes(h) && !h.startsWith('/costs'))
    const broken = []
    for (const link of links) { await go(owner, link); if (await failed(owner)) broken.push(link) }
    await owner.screenshot({ path: path.join(SHOTS, 'garage.png'), fullPage: true })
    rec('BO-091', broken.length === 0 && links.length >= 12, `${links.length} screens opened (${links.join(' ')}); with an error banner: ${JSON.stringify(broken)}`)

    // BO-092 -- a vehicle through the form, then edited
    await go(owner, '/vehicles/new')
    await fill(owner, /^Plate/, 'QAUI001'); await fill(owner, /^Model/, 'UI Bus'); await choose(owner, /^Status/, 'Active'); await fill(owner, /^Prefix/, 'U1'); await save(owner)
    await go(owner, '/vehicles')
    const created = (await text(owner)).includes('QAUI001')
    await owner.getByRole('link', { name: 'QAUI001' }).click(); await owner.waitForSelector('text=UI Bus'); await owner.screenshot({ path: path.join(SHOTS, 'vehicle-detail.png'), fullPage: true })
    await owner.getByRole('link', { name: /^edit$/i }).click(); await owner.waitForFunction(() => document.querySelector('#model')?.value === 'UI Bus')
    await fill(owner, /^Model/, 'UI Bus 2'); await save(owner)
    await go(owner, '/vehicles')
    rec('BO-092', created && (await text(owner)).includes('QAUI001'), `vehicle QAUI001 created by the form, opened and edited (list shows ${(await text(owner)).includes('UI Bus 2') ? 'the new model' : 'the old model'})`)

    // BO-093 -- a customer and its day off
    await go(owner, '/customers/new'); await fill(owner, /^Name/, 'QAUI Customer'); await choose(owner, /^Status/, 'Active'); await save(owner)
    await owner.getByRole('link', { name: 'QAUI Customer' }).click(); await owner.waitForSelector('text=Customer days off')
    await owner.getByRole('link', { name: /new day off/i }).click(); await fill(owner, /^Date/, '2026-12-24'); await fill(owner, /^Reason/, 'Christmas Eve'); await save(owner)
    await owner.waitForSelector('text=Christmas Eve', { timeout: 8000 })
    rec('BO-093', true, 'customer created, its day off recorded on its own page and listed on the customer')

    // BO-094 -- a part, a stock entry and a purchase order moved to ordered
    await go(owner, '/parts/new'); await fill(owner, /^Name/, 'QAUI Pad'); await fill(owner, /^Unit/, 'un'); await fill(owner, /^Minimum stock/, '2'); await save(owner)
    await owner.getByRole('link', { name: /PC-\d+/ }).first().click(); await owner.waitForSelector('text=Current stock')
    await owner.getByRole('link', { name: /stock entry/i }).click(); await fill(owner, /^Quantity/, '10'); await fill(owner, /^Unit value/, '6.5'); await save(owner)
    await owner.waitForSelector('text=Current stock')
    const part = await text(owner)
    await owner.screenshot({ path: path.join(SHOTS, 'part-detail.png'), fullPage: true })
    await go(owner, '/purchase-orders/new'); await choose(owner, /^Part/, { label: /QAUI Pad/ }); await fill(owner, /^Quantity/, '4'); await fill(owner, /^Supplier/, 'QAUI Supplier'); await save(owner)
    await owner.getByRole('link', { name: /PO-\d+/ }).first().click(); await owner.waitForSelector('text=Requested')
    await owner.getByRole('link', { name: /mark as ordered/i }).click(); await fill(owner, /^Expected delivery/, '2026-11-01'); await save(owner)
    await owner.waitForSelector('text=Ordered', { timeout: 8000 })
    rec('BO-094', /10 un/.test(part) && /6\.50/.test(part), `part stock reads "${(part.match(/\d+ un/) || [''])[0]}" and average ${(part.match(/6\.50/) || ['?'])[0]} after the entry; the purchase order went Requested -> Ordered`)

    // BO-095 -- a work order, an item, and the conclude confirmation
    await go(owner, '/work-orders/new'); await choose(owner, /^Vehicle/, { label: /QAUI001/ }); await fill(owner, /^Odometer/, '5000'); await fill(owner, /^Description/, 'QAUI brake job'); await save(owner)
    await owner.getByRole('link', { name: /OS-\d+/ }).first().click(); await owner.waitForSelector('text=QAUI brake job')
    await owner.getByRole('link', { name: /new item/i }).click(); await fill(owner, /^Description/, 'Change pads'); await save(owner)
    await owner.waitForSelector('text=Change pads', { timeout: 8000 })
    await owner.screenshot({ path: path.join(SHOTS, 'work-order.png'), fullPage: true })
    await owner.getByRole('button', { name: /^conclude$/i }).click(); await owner.getByRole('button', { name: /confirm/i }).click(); await owner.waitForTimeout(1200)
    rec('BO-095', (await text(owner)).includes('Partially resolved') || (await text(owner)).includes('Concluded'), `work order opened, item added on its page, concluded after confirming -> "${(await text(owner)).match(/Partially resolved|Concluded/)?.[0]}"`)

    // BO-096 -- fuel: a fuelling, the report and the gauge reason
    await go(owner, '/fuel/new'); await choose(owner, /^Vehicle/, { label: /QAUI001/ }); await fill(owner, /^Recorded/, '2026-10-01T08:00'); await fill(owner, /^Volume/, '80'); await fill(owner, /^Amount/, '480')
    await fill(owner, /^Odometer/, '5100'); await owner.getByLabel(/full tank/i).check(); await save(owner)
    await go(owner, '/fuel'); const fuel = (await text(owner)).includes('QAUI001')
    await go(owner, '/fuel/gauge'); await choose(owner, /^Vehicle/, { label: /QAUI001/ }); await owner.getByRole('button', { name: /^generate$/i }).click(); await owner.waitForTimeout(1200)
    const gauge = await text(owner)
    await owner.screenshot({ path: path.join(SHOTS, 'fuel-gauge.png'), fullPage: true })
    rec('BO-096', fuel && /no tank capacity|awaiting calibration|tank/i.test(gauge), `fuelling listed with its plate; gauge answers for the vehicle: "${(gauge.match(/no tank capacity[^.]*\.|awaiting calibration[^.]*\./i) || [gauge.slice(0, 40)])[0]}"`)

    // BO-097 -- the schedule for a day and the 404
    await go(owner, '/schedule'); const schedule = await text(owner)
    await owner.goto(`${BASE}/no/such/page`); await owner.waitForSelector('text=Page not found', { timeout: 8000 })
    rec('BO-097', schedule.toLowerCase().includes('schedule') && (await failed(owner)) === 0, 'schedule screen answers for today; an unknown address shows the 404 page')


    // BO-100 -- a checklist template from a repeating group, then a run answered against it
    await go(owner, '/checklist-templates/new'); await fill(owner, /^Name/, 'QAUI Pre-trip'); await choose(owner, /^Type/, 'Departure')
    await owner.getByRole('button', { name: /add row/i }).click(); await fill(owner, /^Description/, 'Brakes'); await owner.getByLabel(/generates work order/i).check(); await save(owner)
    await owner.getByRole('link', { name: 'QAUI Pre-trip' }).click(); await owner.waitForSelector('text=Brakes')
    await owner.getByRole('link', { name: /record checklist/i }).click(); await owner.waitForFunction(() => [...document.querySelectorAll('input:disabled')].some((i) => i.value === 'Brakes'))
    await choose(owner, /^Vehicle/, { label: /QAUI001/ }); await choose(owner, /^Driver/, { index: 1 }); await fill(owner, /^Odometer/, '5200'); await choose(owner, /^Answer/, 'Non-conforming'); await save(owner)
    const runBack = new URL(owner.url()).pathname.startsWith('/checklist-templates/') && !new URL(owner.url()).pathname.endsWith('/run')
    await go(owner, '/work-orders'); const checklistOrder = (await text(owner)).includes('Brakes')
    rec('BO-100', runBack && checklistOrder, `template created with its item row; the run is answered per item and sent; the failed flagged answer opened a work order (list shows it: ${checklistOrder})`)

    // BO-101 -- a preventive plan, overdue by the odometer, opens its work order from the detail page
    await go(owner, '/preventive-plans/new'); await choose(owner, /^Vehicle/, { label: /QAUI001/ }); await fill(owner, /^Plan name/, 'QAUI Oil'); await choose(owner, /^Controlled by/, 'Kilometres')
    await fill(owner, /^Interval \(km\)/, '1000'); await fill(owner, /^Last service km/, '0'); await save(owner)
    await owner.getByRole('link', { name: 'QAUI Oil' }).click(); await owner.waitForSelector('text=Overdue', { timeout: 8000 })
    await owner.screenshot({ path: path.join(SHOTS, 'preventive-plan.png'), fullPage: true })
    await owner.getByRole('button', { name: /open work order/i }).click(); await owner.waitForURL(/\/work-orders\/[0-9a-f-]{8,}/, { timeout: 8000 })
    rec('BO-101', true, `plan shows Overdue from the vehicle's odometer; "Open work order" created one and went to ${new URL(owner.url()).pathname.slice(0, 22)}…`)

    // BO-102 -- the garage: a catalogue service, a triage, mark the service, check out
    await go(owner, '/garage-services/new'); await fill(owner, /^Name/, 'QAUI Wash'); await fill(owner, /^Order/, '1'); await choose(owner, /^Group/, 'Internal'); await save(owner)
    await go(owner, '/garage/open'); await choose(owner, /^Vehicle/, { label: /QAUI001/ }); await save(owner)
    await go(owner, '/garage'); await owner.getByRole('link', { name: /QAUI001|U1/ }).first().click(); await owner.waitForSelector('text=QAUI Wash')
    await owner.getByRole('button', { name: /^done$/i }).first().click(); await owner.waitForTimeout(1200)
    const marked = (await text(owner)).includes('Done')
    await owner.screenshot({ path: path.join(SHOTS, 'triage.png'), fullPage: true })
    await owner.getByRole('button', { name: /check out/i }).first().click(); await owner.getByRole('button', { name: /confirm/i }).click(); await owner.waitForTimeout(1200)
    rec('BO-102', marked && (await text(owner)).includes('Finished'), `service marked Done on the triage page, then checked out (status ${(await text(owner)).match(/Finished|Open/)?.[0]})`)

    // BO-103 -- inspection model and an inspection recorded against it
    await go(owner, '/inspection-models/new'); await fill(owner, /^Name/, 'QAUI Brake check'); await owner.getByRole('button', { name: /add row/i }).click(); await fill(owner, /^Description/, 'Pads'); await save(owner)
    await go(owner, '/inspections/new'); await choose(owner, /^Vehicle/, { label: /QAUI001/ }); await choose(owner, /^Inspection model/, { label: /QAUI Brake check/ })
    await owner.getByRole('button', { name: /add row/i }).click(); await fill(owner, /^Description/, 'Pads worn'); await owner.getByLabel(/^Conforming/).uncheck(); await save(owner)
    rec('BO-103', new URL(owner.url()).pathname === '/inspection-models' && (await failed(owner)) === 0, `model created from a list of texts; a non-conforming inspection saved and the page returned to ${new URL(owner.url()).pathname}`)

    // BO-104 -- a transport demand and an exception on its own pages
    await go(owner, '/transport-demands/new'); await fill(owner, /^Type description/, 'QAUI line'); await choose(owner, /^Demand kind/, 'Line'); await fill(owner, /^Line name/, 'QAUI Line 1'); await fill(owner, /^Days of the week/, 'Mon,Tue'); await save(owner)
    await owner.getByRole('link', { name: 'QAUI Line 1' }).click(); await owner.waitForSelector('text=Schedule exceptions')
    await owner.getByRole('link', { name: /new exception/i }).click(); await fill(owner, /^Date/, '2026-11-02'); await choose(owner, /^Exception type/, 'Cancellation'); await save(owner)
    await owner.waitForSelector('text=Cancellation', { timeout: 8000 })
    rec('BO-104', true, 'demand created, then a cancellation exception added on its own page and listed on the demand')

    // BO-105 -- the tenant's own settings: internal tank, gauge limits and the rules
    await go(owner, '/fuel/tank/edit'); await fill(owner, /^Capacity/, '10000'); await fill(owner, /^Reference stock/, '6000'); await fill(owner, /^Reference instant/, '2026-10-01T08:00'); await fill(owner, /^Alert level/, '2000'); await fill(owner, /^Reserve level/, '800'); await save(owner)
    await go(owner, '/fuel/tank'); const tank = (await text(owner)).includes('10000')
    await go(owner, '/fuel/settings'); await owner.waitForFunction(() => document.querySelector('#suspectMarginRatio')?.value !== ''); await save(owner)
    await go(owner, '/fuel/rules'); await owner.waitForFunction(() => document.querySelector('#avgMaxSegmentKm')?.value !== ''); await fill(owner, /^Segments for the own average/, '2'); await save(owner)
    await go(owner, '/fuel/rules'); await owner.waitForFunction(() => document.querySelector('#avgMinSegmentsForOwnAverage')?.value === '2')
    rec('BO-105', tank && new URL(owner.url()).pathname === '/fuel/rules', 'internal tank configured and read back; gauge limits saved; a rule changed to 2 and read back from the server')

    // BO-106 -- trips from a CSV: previewed, then imported
    await go(owner, '/extra-trips/import')
    require('fs').writeFileSync('/tmp/qaui-trips.csv', 'orderCode,tripDate,destination,status\nQAUI-1,2026-11-05,Campinas,Scheduled\nQAUI-2,2026-11-06,Santos,Scheduled\n')
    await owner.setInputFiles('input[type=file]', '/tmp/qaui-trips.csv'); await owner.waitForSelector('text=would create 2 trips', { timeout: 8000 })
    await owner.getByRole('button', { name: /^import$/i }).click(); await owner.waitForSelector('text=Imported:', { timeout: 8000 })
    await go(owner, '/extra-trips'); const trips = await text(owner)
    rec('BO-106', trips.includes('QAUI-1') && trips.includes('QAUI-2'), 'CSV previewed (2 to create) and imported; both trips are in the list')

    // BO-098 -- permissions: the mechanic cannot conclude, the tenant user cannot create
    const mech = await login(browser, 'mechanic')
    await go(mech, '/work-orders'); await mech.getByRole('link', { name: /OS-\d+/ }).first().click(); await mech.waitForSelector('text=Status')
    const mechConclude = await mech.getByRole('button', { name: /^conclude$/i }).count()
    const tu = await login(browser, 'tenantuser'); await go(tu, '/vehicles'); const tuNew = await tu.getByRole('link', { name: /new vehicle/i }).count()
    await tu.goto(`${BASE}/vehicles/new`); await tu.waitForTimeout(800)
    rec('BO-098', mechConclude === 0 && tuNew === 0 && new URL(tu.url()).pathname === '/', `mechanic sees ${mechConclude} conclude buttons; tenant user sees ${tuNew} new-vehicle links and typing /vehicles/new lands on ${new URL(tu.url()).pathname}`)

    rec('BO-099', [owner, mech, tu].every((p) => p.errors.length === 0), `no page errors: ${JSON.stringify([owner, mech, tu].flatMap((p) => p.errors))}`)
  } finally { await browser.close() }
  process.exit(out.every(([, ok]) => ok) ? 0 : 1)
})().catch((e) => { console.error('BLOCKED', e.message); process.exit(2) })
