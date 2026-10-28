import { expect, test } from '@playwright/test'
import { openBilling, signIn, testValue } from './auth'

test('Tenant owner (TenantOwner profile) sees only their history and no platform actions', async ({ page }) => {
  await signIn(page, testValue('BILLING_TEST_OWNER_B'), testValue('BILLING_TEST_PASSWORD'))
  await openBilling(page)

  const rows = page.getByRole('row')
  await expect(rows).toHaveCount(2)
  await expect(rows.nth(1)).toContainText(`${testValue('BILLING_TEST_TAG')}-billing-plan`)
  await expect(page.getByRole('button', { name: 'Confirm payment' })).toHaveCount(0)
  await expect(page.getByRole('spinbutton', { name: 'Grace days' })).toHaveCount(0)
  await expect(page.getByRole('combobox', { name: 'Tenant' })).toHaveCount(0)
})