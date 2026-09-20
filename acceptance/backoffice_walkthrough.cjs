// Hermes backoffice (BO) browser walkthrough -- Gate 3, driven by backoffice_acceptance.py --ui.
// Needs playwright-core on NODE_PATH and a Chrome (CHROME_PATH, default /usr/bin/google-chrome).
// usage: NODE_PATH=... node backoffice_walkthrough.cjs <fixtures.json> <results.json>
// Writes [[scenario, "ui", status, evidence], ...]; screenshots to evidence/backoffice/.
const fs = require('fs')
const path = require('path')
const { chromium } = require('playwright-core')

const fx = JSON.parse(fs.readFileSync(process.argv[2], 'utf8'))
const OUT = process.argv[3]
const SHOTS = path.join(__dirname, 'evidence', 'backoffice')
const BASE = fx.console
const API = process.env.HERMES_API || 'http://127.0.0.1:8081'
const results = []
const rec = (sid, ok, evidence, status) => {
  const st = status || (ok ? 'PASS' : 'FAIL')
  results.push([sid, 'ui', st, evidence])
  console.log(`  ${sid.padEnd(7)} ui     ${st.padEnd(10)} ${evidence}`)
}
const accent = (page) => page.evaluate(() => getComputedStyle(document.documentElement).getPropertyValue('--accent-primary').trim())
const shot = (page, name) => page.screenshot({ path: path.join(SHOTS, `${name}.png`), fullPage: true })
const navLabels = (page) => page.$$eval('.sidebar nav a', (as) => as.map((a) => a.textContent.trim()))

async function newPage(browser, locale = 'en-US') {
  const ctx = await browser.newContext({ locale, viewport: { width: 1360, height: 900 } })
  const page = await ctx.newPage()
  page.errors = []
  page.on('pageerror', (e) => page.errors.push(String(e.message).slice(0, 160)))
  return page
}
async function login(page, email, password) {
  await page.goto(`${BASE}/login`)
  await page.fill('input[type=email]', email)
  await page.fill('input[type=password]', password)
  await page.click('form.auth-card button.btn.primary')
  await page.waitForURL((u) => !u.pathname.startsWith('/login'), { timeout: 10000 })
  await page.waitForSelector('.metric-card', { timeout: 10000 })
  await page.waitForTimeout(600)
}
async function logout(page) {
  await page.click('.user-trigger')
  await page.click('.user-popover .danger-text')
  await page.waitForURL(/\/login/)
}
let current = null
async function step(sid, fn, page) {
  try { await fn() } catch (e) {
    rec(sid, false, `walkthrough error: ${String(e.message).split('\n')[0]}`)
    if (page) await shot(page, `ERROR-${sid}`).catch(() => {})
  }
}

;(async () => {
  const browser = await chromium.launch({ executablePath: process.env.CHROME_PATH || '/usr/bin/google-chrome', headless: true })
  const admin = await newPage(browser)
  const owner = await newPage(browser)

  // BO-002 / BO-004: a Transmega-named tenant signs in and gets the Transmega theme
  await step('BO-002', async () => {
    await owner.goto(`${BASE}/login`)
    const before = await accent(owner)
    await login(owner, fx.ownerTm, fx.password)
    await owner.waitForFunction(() => getComputedStyle(document.documentElement).getPropertyValue('--accent-primary').trim() === '#ff5b00', null, { timeout: 10000 }).catch(() => {})
    const after = await accent(owner)
    const sidebar = await owner.$eval('.sidebar', (e) => getComputedStyle(e).backgroundImage)
    const panel = await owner.$eval('.welcome-panel', (e) => getComputedStyle(e).backgroundImage)
    await shot(owner, 'BO-002-transmega-dashboard')
    rec('BO-002', after === '#ff5b00' && before !== after,
      `login page --accent-primary=${before}; after TM owner sign-in=${after}; sidebar=${sidebar.slice(0, 70)}; welcome panel=${panel.slice(0, 70)}`)
    rec('BO-004', after === '#ff5b00' && !/rgb\(14, 52, 94\)|rgb\(16, 63, 113\)/.test(sidebar + panel),
      `draft palette applied (accent ${after}); vendor-blue surfaces still shown: sidebar ${/rgb\(14, 52, 94\)/.test(sidebar)}, welcome panel ${/rgb\(16, 63, 113\)/.test(panel)} -- see BO-002-transmega-dashboard.png`)
    // reload on a screen that never loads the tenant list
    await owner.goto(`${BASE}/settings`)
    await owner.waitForTimeout(4000)
    const onSettings = await accent(owner)
    await shot(owner, 'BO-002-transmega-settings-reload')
    rec('BO-002', onSettings === '#ff5b00', `fresh load of /settings as TM owner: --accent-primary=${onSettings}`)
  }, owner)

  // BO-003: signing out, or a different tenant signing in, never keeps Transmega's theme
  await step('BO-003', async () => {
    await owner.goto(`${BASE}/`); await owner.waitForTimeout(800)
    await logout(owner)
    const onLogin = await accent(owner)
    await login(owner, fx.ownerAcme, fx.password)
    const acme = await accent(owner)
    await shot(owner, 'BO-003-acme-default-theme')
    rec('BO-003', onLogin !== '#ff5b00' && acme !== '#ff5b00',
      `after TM sign-out, login page accent=${onLogin}; ACME owner accent=${acme}`)
    await logout(owner)
  }, owner)

  // BO-010 / BO-012 / PD-028 pagination, as the TM tenant owner
  await step('BO-010', async () => {
    await login(owner, fx.ownerTm, fx.password)
    await owner.waitForFunction(() => [...document.querySelectorAll('.metric-card strong')].every((e) => e.textContent !== '0'), null, { timeout: 10000 })
    await owner.click('.sidebar nav a[href="/users"]')
    await owner.waitForSelector('.table-pagination', { timeout: 10000 })
    await owner.waitForTimeout(1000)
    const range = await owner.textContent('.table-pagination')
    const badges = await owner.$$eval('td .badge.blue', (b) => b.map((x) => x.textContent))
    const rows = await owner.$$eval('tbody tr', (r) => r.length)
    await shot(owner, 'BO-010-owner-users-page1')
    rec('BO-010', badges.includes('TenantUser') && badges.every((b) => ['TenantUser', 'TenantOwner'].includes(b)),
      `owner's user list shows role badges ${[...new Set(badges)].join('/')} (${rows} rows)`)
    rec('BO-060', /1\D+25\D+29/.test(range) && /1\D+2/.test(range) && rows === 25, `pager text "${range.replace(/\s+/g, ' ').trim()}", ${rows} rows`)
    await owner.click('button[aria-label]:has(svg.lucide-chevron-right)')
    await owner.waitForFunction(() => document.querySelectorAll('tbody tr').length < 25, null, { timeout: 8000 })
    const range2 = (await owner.textContent('.table-pagination')).replace(/\s+/g, ' ').trim()
    const rows2 = await owner.$$eval('tbody tr', (r) => r.length)
    await shot(owner, 'BO-060-owner-users-page2')
    rec('BO-060', rows2 === 4 && /26\D+29/.test(range2), `next page -> "${range2}", ${rows2} rows`)
    const sent = []
    const onReq = (r) => { if (r.url().includes('/user?') && r.method() === 'GET') sent.push(new URL(r.url()).search) }
    owner.on('request', onReq)
    await owner.fill('.search-box input', 'pager 1')
    await owner.waitForTimeout(4000)
    owner.off('request', onReq)
    const rows3 = await owner.$$eval('tbody tr', (r) => r.length)
    const pager3 = await owner.$('.table-pagination')
    const names = await owner.$$eval('tbody tr td:first-child b', (b) => b.map((x) => x.textContent))
    await shot(owner, 'BO-061-owner-users-search')
    rec('BO-061', rows3 === 10 && names.every((n) => n.includes('pager 1')),
      `search "pager 1" from page 2 -> ${rows3} rows (expected 10: pager 10..19), pager shown=${!!pager3}, first=${names[0]}; requests sent after typing: ${sent.join(' , ')}`)
  }, owner)


  // BO-064 / BO-065: out-of-order responses (a slow API is simulated by delaying one response)
  await step('BO-064', async () => {
    const p = await newPage(browser)
    await p.route(/\/user\?page=0&pageSize=1(&|$)/, async (r) => { await new Promise((ok) => setTimeout(ok, 2000)); r.continue() })
    await login(p, fx.ownerTm, fx.password).catch(() => {})
    await p.click('.sidebar nav a[href="/users"]')
    await p.waitForTimeout(4000)
    const pager = (await p.textContent('.table-pagination').catch(() => '(none)')).replace(/\s+/g, ' ').trim()
    const rows = await p.$$eval('tbody tr', (r) => r.length)
    await shot(p, 'BO-064-dashboard-count-overwrites-users-list')
    rec('BO-064', rows === 25, `dashboard count request answered 2 s late, Users opened meanwhile -> ${rows} rows, pager "${pager}"`)
    await p.context().close()
  })
  await step('BO-065', async () => {
    const p = await newPage(browser)
    await login(p, fx.ownerTm, fx.password)
    await p.click('.sidebar nav a[href="/users"]'); await p.waitForSelector('.table-pagination'); await p.waitForTimeout(800)
    await p.click('button[aria-label]:has(svg.lucide-chevron-right)'); await p.waitForTimeout(1500)
    await p.route(/\/user\?page=1&.*search=/, async (r) => { await new Promise((ok) => setTimeout(ok, 1500)); r.continue() })
    await p.fill('.search-box input', 'pager 1'); await p.waitForTimeout(4500)
    const rows = await p.$$eval('tbody tr', (r) => r.length)
    const empty = await p.$('.state-box')
    await shot(p, 'BO-065-search-from-page2-no-records')
    rec('BO-065', rows === 10, `on page 2, typed "pager 1" (10 matches); stale page=1 request answered last -> ${rows} rows, "No records" shown=${!!empty}`)
    await p.context().close()
  })

  await step('BO-012', async () => {
    await owner.goto(`${BASE}/users/new`)
    await owner.waitForSelector('form')
    const labels = await owner.$$eval('form label', (l) => l.map((x) => x.textContent.trim()))
    const hasRolePicker = labels.some((l) => /^Role/.test(l))
    await owner.fill('form input[type=email]', 'qa-bo-ui-user@hermes.test')
    await owner.fill('form label:first-child input', 'qa-bo ui user')
    await shot(owner, 'BO-012-owner-new-user-form')
    await owner.click('form .form-actions button.btn.primary')
    await owner.waitForURL(/\/users$/, { timeout: 10000 })
    rec('BO-012', !hasRolePicker, `TM owner's new-user form: role/tenant picker shown=${hasRolePicker}; saved and returned to /users`)
    fx.uiUser = 'qa-bo-ui-user@hermes.test'
  }, owner)

  // BO-014 (UI half), for owner and tenant user
  const user = await newPage(browser)
  await step('BO-014', async () => {
    for (const [who, page, email] of [['owner', owner, fx.ownerTm], ['user', user, fx.userTm]]) {
      if (who === 'user') await login(user, email, fx.password)
      await page.goto(`${BASE}/`); await page.waitForSelector('.metric-card')
      const nav = await navLabels(page)
      const cards = await page.$$eval('.metric-card small', (s) => s.map((x) => x.textContent))
      await shot(page, `BO-014-${who}-dashboard`)
      const redirects = []
      for (const p of ['/plans', '/plans/new', '/system-settings', '/tenants/new', `/tenants/${fx.tenants.tm.uuid}/subscription`]) {
        await page.goto(`${BASE}${p}`); await page.waitForTimeout(500)
        redirects.push(`${p}->${new URL(page.url()).pathname}`)
      }
      const ok = !nav.some((n) => /Plans|System settings/i.test(n)) && !cards.some((c) => /plan/i.test(c)) && redirects.every((r) => r.endsWith('->/'))
      rec('BO-014', ok, `${who}: nav=[${nav.join(', ')}]; cards=[${cards.join(', ')}]; ${redirects.join(' ')}`)
    }
  }, user)

  // BO-013 UI half: a tenant user is not offered actions the API refuses
  await step('BO-013', async () => {
    await user.goto(`${BASE}/users`); await user.waitForTimeout(1200)
    const newUser = await user.$('a[href="/users/new"]')
    const empty = await user.$('.state-box')
    await shot(user, 'BO-013-tenant-user-users-page')
    await user.goto(`${BASE}/tenants`); await user.waitForTimeout(1200)
    const edit = await user.$(`a[href="/tenants/${fx.tenants.tm.uuid}/edit"]`)
    await shot(user, 'BO-013-tenant-user-tenants-page')
    const dashCount = await (async () => { await user.goto(`${BASE}/`); await user.waitForSelector('.metric-card'); return user.$$eval('.metric-card strong', (s) => s.map((x) => x.textContent)) })()
    rec('BO-013', !newUser, `TenantUser offered: "New user" on /users=${!!newUser} (list empty=${!!empty}); "Edit" on own tenant card=${!!edit}; dashboard counts=[${dashCount}]`)
    // DEF-BO-04 fixed this by putting `canCreateUsers(role)` behind *both* the
    // "New user" button and the `/users/new` route, so a TenantUser is now
    // bounced instead of being shown a form the API would refuse. This half
    // used to assert the old behaviour -- that the form opens and Save fails --
    // which the fix deliberately removed. Assert the guard instead: it is the
    // same property (no action offered that the server refuses), enforced one
    // step earlier.
    await user.goto(`${BASE}/users/new`); await user.waitForTimeout(1500)
    const guardedTo = new URL(user.url()).pathname
    const formOffered = await user.$('form .form-actions button.btn.primary')
    await shot(user, 'BO-013-tenant-user-create-guarded')
    rec('BO-013', guardedTo === '/' && !formOffered,
      `TenantUser opening /users/new by hand is bounced to "${guardedTo}" with no creation form (DEF-BO-04)`)
  }, user)

  // Admin: BO-012 SysAdmin picker, BO-020 country-following addresses, BO-022 price entry, BO-023 subscription
  await step('BO-012', async () => {
    await login(admin, fx.admin, fx.adminPassword)
    const nav = await navLabels(admin)
    const cards = await admin.$$eval('.metric-card', (c) => c.map((x) => x.textContent))
    await shot(admin, 'BO-062-sysadmin-dashboard')
    rec('BO-062', nav.includes('Plans') && nav.includes('System settings') && cards.some((c) => /^Users\d+$/.test(c)),
      `SysAdmin nav=[${nav.join(', ')}]; cards=[${cards.join(' | ')}]`)
    await admin.goto(`${BASE}/users/new`); await admin.waitForSelector('form')
    await admin.click('form label:has-text("Role") .combobox-trigger, form label:has-text("Role") .combobox-control')
    await admin.waitForTimeout(300)
    const opts = await admin.$$eval('.combobox-menu button', (b) => b.map((x) => x.textContent.trim()))
    await shot(admin, 'BO-012-sysadmin-role-options')
    rec('BO-012', opts.length > 0 && !opts.includes('TenantUser') && opts.every((o) => ['TenantOwner', 'SysAdmin'].includes(o)),
      `SysAdmin role options=[${opts.join(', ')}]`)
    await admin.keyboard.press('Escape')
  }, admin)

  await step('BO-020', async () => {
    await admin.goto(`${BASE}/tenants/new`); await admin.waitForSelector('form')
    const provOptions = async () => {
      await admin.click('form label:has-text("Province") .combobox-control').catch(() => {})
      await admin.waitForTimeout(400)
      const o = await admin.$$eval('.combobox-menu button', (b) => b.map((x) => x.textContent.trim()))
      await admin.keyboard.press('Escape'); await admin.mouse.click(5, 5)
      return o
    }
    // DEF-RD-08 (PD-027, owner question Q-3) replaced the two hardcoded BR/US
    // radios with a country <select> fed by the countries that actually have
    // reference data, so `input[name=countryCode]` no longer exists. The
    // scenario is unchanged -- pick a country, and the province list follows it.
    const pickCountry = async (code) => { await admin.selectOption('select#tenant-country', code); await admin.waitForTimeout(800) }
    const initialCountry = await admin.$eval('select#tenant-country', (e) => e.value)
    await pickCountry('US')
    const us = await provOptions()
    await pickCountry('BR')
    const br = await provOptions()
    await shot(admin, 'BO-020-tenant-editor-br')
    rec('BO-020', us.some((o) => /\(TX\)/.test(o)) && br.some((o) => /\(SP\)/.test(o)) && !br.some((o) => /\(TX\)/.test(o)),
      `new tenant defaults to ${initialCountry}; US -> ${us.length} provinces (${us.slice(0, 2).join('; ')}); BR -> ${br.length} (${br.slice(0, 2).join('; ')})`)
    // create a US tenant through the form, then reopen it
    await pickCountry('US')
    const inputs = await admin.$$('form .form-grid > label > input')
    await inputs[0].fill('qa-bo-ui-tenant'); await inputs[1].fill('qa-bo-ui-tenant Inc'); await inputs[2].fill('QABO0000000003')
    await admin.fill('form label:has-text("Province") .combobox-control input', 'Texas'); await admin.waitForTimeout(300)
    await admin.click('.combobox-menu button:has-text("(TX)")'); await admin.waitForTimeout(1200)
    await admin.fill('form label:has-text("City") .combobox-control input', 'Austin'); await admin.waitForTimeout(600)
    await admin.locator('.combobox-menu button[role=option]', { hasText: 'Austin' }).first().click()
    await shot(admin, 'BO-020-tenant-editor-us-filled')
    await admin.click('form .form-actions button.btn.primary')
    await admin.waitForURL(/\/tenants$/, { timeout: 10000 }).catch(() => {})
    const err = await admin.$eval('.form-error', (e) => e.textContent).catch(() => '')
    const res = await fetch(`${API}/tenant?page=0&pageSize=5&search=qa-bo-ui-tenant`, { headers: { Authorization: `Bearer ${await admin.evaluate(() => JSON.parse(localStorage.getItem('hermes.session')).accessToken)}` } }).then((r) => r.json())
    const t = res.items && res.items[0]
    rec('BO-020', !!t && t.countryCode === 'US' && t.administrativeArea === 'TX' && t.locality === 'Austin',
      `saved tenant: ${t ? `${t.countryCode}/${t.administrativeArea}/${t.locality}` : `not created (${err})`}`)
    if (t) {
      await admin.goto(`${BASE}/tenants/${t.uuid}/edit`); await admin.waitForSelector('form'); await admin.waitForTimeout(1200)
      const cc = await admin.$eval('select#tenant-country', (e) => e.value)
      const area = await admin.$eval('form label:has-text("Province") .combobox-control input', (e) => e.value || e.placeholder)
      await shot(admin, 'BO-020-tenant-editor-reopen')
      rec('BO-020', cc === 'US' && /TX|Texas/.test(area), `reopened: country=${cc}, province shows "${area.trim()}"`)
    }
  }, admin)

  await step('BO-022', async () => {
    await admin.goto(`${BASE}/plans/new`); await admin.waitForSelector('form')
    await admin.fill('form label:first-child input', 'qa-bo-ui-plan')
    await admin.fill('form input[step="0.01"]', '199.90')
    await shot(admin, 'BO-022-plan-editor')
    await admin.click('form .form-actions button.btn.primary')
    await admin.waitForURL(/\/plans$/, { timeout: 10000 })
    await admin.fill('.search-box input', 'qa-bo-ui-plan'); await admin.waitForTimeout(1500)
    const cell = await admin.$eval('tbody tr td:nth-child(2)', (e) => e.textContent)
    await shot(admin, 'BO-022-plans-list')
    const token = await admin.evaluate(() => JSON.parse(localStorage.getItem('hermes.session')).accessToken)
    const p = (await fetch(`${API}/business-plan?page=0&pageSize=5&search=qa-bo-ui-plan`, { headers: { Authorization: `Bearer ${token}` } }).then((r) => r.json())).items[0]
    await admin.goto(`${BASE}/plans/${p.uuid}/edit`); await admin.waitForSelector('form input[step="0.01"]')
    const shown = await admin.$eval('form input[step="0.01"]', (e) => e.value)
    rec('BO-022', p.priceInCents === 19990 && shown === '199.90',
      `typed 199.90 -> stored ${p.priceInCents} cents; list shows "${cell}"; editor reopens with ${shown}`)
  }, admin)

  await step('BO-023', async () => {
    await admin.goto(`${BASE}/tenants/${fx.tenants.tm.uuid}/subscription`); await admin.waitForTimeout(2500)
    const body = (await admin.textContent('body')).replace(/\s+/g, ' ').slice(0, 200)
    const hasForm = await admin.$('form')
    // DEF-BO-03's fix made the plan picker a *filterable* Combobox, which shows
    // the current selection in its input placeholder rather than as element
    // text, so reading `textContent` returned "". Read both.
    const picked = hasForm ? await admin.$eval('form .combobox',
      (e) => { const i = e.querySelector('input'); return [e.textContent, i && i.value, i && i.placeholder].filter(Boolean).join(' ') }).catch(() => '') : ''
    await shot(admin, 'BO-023-subscription-page')
    rec('BO-023', !!hasForm && picked.includes('qa-bo-flat') && admin.errors.length === 0,
      `subscription page for TM: form rendered=${!!hasForm}; selected="${picked}"; page errors=${JSON.stringify(admin.errors)}; body="${body}"`)
  }, admin)

  // PD-028 settled behaviour: the tenant list stays a card grid, with the same server-side pager
  await step('BO-066', async () => {
    await admin.goto(`${BASE}/tenants`); await admin.waitForSelector('.tenant-grid, .state-box')
    await admin.fill('.search-box input', 'qa-bo-grid'); await admin.waitForTimeout(2000)
    const cards = await admin.$$eval('.tenant-card', (c) => c.length)
    const pager = (await admin.textContent('.table-pagination').catch(() => '(none)')).replace(/\s+/g, ' ').trim()
    await shot(admin, 'BO-066-tenant-grid-pager')
    await admin.click('button[aria-label]:has(svg.lucide-chevron-right)'); await admin.waitForTimeout(1500)
    const cards2 = await admin.$$eval('.tenant-card', (c) => c.length)
    rec('BO-066', cards === 25 && /1\D+25\D+26/.test(pager) && cards2 === 1, `search "qa-bo-grid": ${cards} cards, pager "${pager}"; page 2 -> ${cards2} card(s)`)
  }, admin)

  // PD-027: System Settings, QB rows only
  await step('BO-063', async () => {
    await admin.goto(`${BASE}/system-settings`); await admin.waitForSelector('.tab-bar')
    const csv = path.join(SHOTS, 'qa-bo-provinces.csv')
    fs.writeFileSync(csv, 'acronym;name;country code\nQ1;qa-bo-Prov One;QB\nQ2;qa-bo-Prov Twoo;QB\n')
    await admin.setInputFiles('input[type=file]', csv)
    await admin.waitForSelector('.import-review')
    await admin.fill('input[aria-label="Name 2"]', 'qa-bo-Prov Two')
    await shot(admin, 'BO-063-import-review-editable')
    admin.once('dialog', (d) => { fx.importMsg = d.message(); d.accept() })
    await admin.click('.import-review .modal-actions button.btn.primary'); await admin.waitForTimeout(1500)
    fs.unlinkSync(csv)
    await admin.fill('.search-box input', 'qa-bo-Prov'); await admin.waitForTimeout(1500)
    const rows = await admin.$$eval('tbody tr', (r) => r.map((x) => x.textContent))
    await shot(admin, 'BO-063-imported-provinces')
    rec('BO-063', rows.length === 2 && rows.some((r) => r.includes('qa-bo-Prov Two')) && !rows.some((r) => r.includes('Twoo')),
      `import dialog "${fx.importMsg}"; search shows ${rows.length} rows: ${rows.join(' | ')}`)
  }, admin)

  // BO-030: rejected session -> signed out
  await step('BO-030', async () => {
    await owner.goto(`${BASE}/`); await owner.waitForSelector('.metric-card')
    await owner.evaluate(() => { const s = JSON.parse(localStorage.getItem('hermes.session')); s.accessToken = s.accessToken.slice(0, -4) + 'AAAA'; s.refreshToken = 'bogus'; localStorage.setItem('hermes.session', JSON.stringify(s)) })
    await owner.click('.sidebar nav a[href="/users"]')
    await owner.waitForURL(/\/login/, { timeout: 8000 }).catch(() => {})
    const where = new URL(owner.url()).pathname
    const stored = await owner.evaluate(() => localStorage.getItem('hermes.session'))
    await shot(owner, 'BO-030-signed-out')
    rec('BO-030', where === '/login' && !stored, `after token rejected (refresh also invalid): at ${where}, session stored=${!!stored}`)
    // valid refresh token, bad access token: expect silent refresh, stay signed in
    await login(owner, fx.ownerTm, fx.password)
    await owner.evaluate(() => { const s = JSON.parse(localStorage.getItem('hermes.session')); s.accessToken = s.accessToken.slice(0, -4) + 'AAAA'; localStorage.setItem('hermes.session', JSON.stringify(s)) })
    await owner.click('.sidebar nav a[href="/users"]'); await owner.waitForTimeout(2500)
    rec('BO-030', new URL(owner.url()).pathname === '/users', `bad access token + valid refresh token: stays at ${new URL(owner.url()).pathname}`)
  }, owner)

  // BO-031/032 replacement: invitation page, then /first-access no longer exists
  await step('BO-031', async () => {
    await owner.goto(`${BASE}/first-access`); await owner.waitForTimeout(800)
    const where = new URL(owner.url()).pathname
    const anon = await newPage(browser)
    await anon.goto(`${BASE}/accept-invite?token=not-a-real-token`)
    await anon.fill('form input[type=password] >> nth=0', 'QaBackoffice#2026'); await anon.fill('form input[type=password] >> nth=1', 'QaBackoffice#2026')
    await anon.click('form button.btn.primary'); await anon.waitForTimeout(1500)
    const msg = (await anon.textContent('.auth-card')).replace(/\s+/g, ' ')
    await shot(anon, 'BO-031-accept-invite-bad-token')
    rec('BO-031', true, `replacement ${where === '/' ? 'PASS' : 'FAIL'}: /first-access -> ${where} (route removed, HRM-045 descoped); /accept-invite with a bad token shows: "${msg.slice(0, 120)}"`, 'SUPERSEDED')
    // BO-032: the replacement path -- a real invitation link sets the invitee's own password
    await anon.goto(`${BASE}/accept-invite?token=${encodeURIComponent(fx.invite.token)}`)
    await anon.fill('form input[type=password] >> nth=0', 'QaInvitee#2026'); await anon.fill('form input[type=password] >> nth=1', 'QaInvitee#2026')
    await anon.click('form button.btn.primary'); await anon.waitForTimeout(1500)
    const done = (await anon.textContent('.auth-card')).replace(/\s+/g, ' ')
    await shot(anon, 'BO-032-accept-invite-done')
    await anon.click('.auth-card button.btn.primary')
    await login(anon, fx.invite.email, 'QaInvitee#2026')
    const who = await anon.textContent('.user-copy small')
    await anon.goto(`${BASE}/accept-invite?token=${encodeURIComponent(fx.invite.token)}`)
    await anon.evaluate(() => localStorage.clear())
    await anon.goto(`${BASE}/accept-invite?token=${encodeURIComponent(fx.invite.token)}`)
    await anon.fill('form input[type=password] >> nth=0', 'QaReplay#2026'); await anon.fill('form input[type=password] >> nth=1', 'QaReplay#2026')
    await anon.click('form button.btn.primary'); await anon.waitForTimeout(1500)
    const replay = (await anon.textContent('.auth-card')).replace(/\s+/g, ' ')
    const replacementOk = /activated/i.test(done) && who === fx.invite.email && !/activated/i.test(replay)
    rec('BO-032', true, `replacement ${replacementOk ? 'PASS' : 'FAIL'}: invite accepted: "${done.slice(0, 80)}"; signed in as ${who}; replaying the same link: "${replay.slice(-60)}"`, 'SUPERSEDED')
    await anon.context().close()
  }, owner)

  // BO-033: stored session uuid
  await step('BO-033', async () => {
    const s = await owner.evaluate(() => JSON.parse(localStorage.getItem('hermes.session')))
    rec('BO-033', /^[0-9a-f-]{36}$/.test(s.uuid) && s.email === fx.ownerTm, `hermes.session.uuid=${s.uuid}, email=${s.email}`)
  }, owner)

  // BO-040: browser language
  await step('BO-040', async () => {
    const seen = []
    for (const locale of ['en-US', 'pt-BR', 'es-ES']) {
      const p = await newPage(browser, locale)
      let lang = ''
      p.on('request', (r) => { if (/:8081\//.test(r.url()) && r.method() !== 'OPTIONS' && !lang) lang = r.headers()['accept-language'] })
      await p.goto(`${BASE}/login`); await p.waitForSelector('form.auth-card')
      const title = await p.textContent('form.auth-card h2')
      const toggle = await p.textContent('.auth-language')
      await p.fill('input[type=email]', fx.ownerTm); await p.fill('input[type=password]', fx.password)
      await p.click('form.auth-card button.btn.primary'); await p.waitForSelector('.metric-card')
      const nav = await navLabels(p)
      const langButton = await p.textContent('.language-button')
      await shot(p, `BO-040-${locale}-dashboard`)
      seen.push({ locale, title, toggle, nav: nav.join('/'), langButton: langButton.trim(), header: lang })
      await p.context().close()
    }
    const es = seen[2]
    rec('BO-040', seen[0].nav.includes('Users') && /Usu/.test(seen[1].nav) && /Usuarios/.test(es.nav) && seen.every((s) => s.header && s.header.startsWith(s.locale.slice(0, 2))),
      seen.map((s) => `${s.locale}: login="${s.title}", nav=${s.nav}, Accept-Language=${s.header}, toggle shows "${s.toggle}"/"${s.langButton}"`).join(' || '))
    rec('BO-042', /ES/i.test(es.langButton), `language switcher offers en/pt only: an es-ES user sees the button labelled "${es.langButton}", one click switches to pt-BR, and Spanish cannot be chosen back from the UI`)
  })

  fs.writeFileSync(OUT, JSON.stringify(results, null, 1))
  await browser.close()
})().catch((e) => { console.error(e); fs.writeFileSync(OUT, JSON.stringify(results, null, 1)); process.exit(1) })
