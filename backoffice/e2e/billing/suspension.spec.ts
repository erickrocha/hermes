import { expect, test } from '@playwright/test'
import { openBilling, signIn, testValue } from './auth'

test('SysAdmin suspends and restores a tenant from billing settings', async ({ page }) => {
  await signIn(page, testValue('BILLING_TEST_ADMIN'), testValue('BILLING_TEST_ADMIN_PASSWORD'))
  await openBilling(page)

  const tenantPicker = page.getByRole('textbox', { name: 'Tenant' })
  await tenantPicker.fill('tenant-B')
  await page.getByRole('option', { name: new RegExp(testValue('BILLING_TEST_TENANT_B')) }).click()

  await page.getByRole('button', { name: 'Suspend access' }).click()
  const suspension = page.getByRole('dialog')
  await expect(suspension).toContainText('All users of this tenant')
  await suspension.getByRole('button', { name: 'Confirm' }).click()
  const settings = page.getByRole('region', { name: 'Billing settings' })
  await expect(settings).toContainText('Tenant access suspended')

  await settings.getByRole('button', { name: 'Restore access' }).click()
  const restoration = page.getByRole('dialog')
  await expect(restoration).toContainText('will regain access')
  await restoration.getByRole('button', { name: 'Confirm' }).click()
  await expect(settings).toContainText('Tenant access active')
})