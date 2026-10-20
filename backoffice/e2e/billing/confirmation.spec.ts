import { expect, test } from '@playwright/test'
import { openBilling, signIn, testValue } from './auth'

test('SysAdmin confirms a pending charge from billing history', async ({ page }) => {
  await signIn(page, testValue('BILLING_TEST_ADMIN'), testValue('BILLING_TEST_ADMIN_PASSWORD'))
  await openBilling(page)

  const row = page.getByRole('row', { name: new RegExp(testValue('BILLING_TEST_TENANT_B')) })
  await expect(row).toContainText(/Pending|Overdue/)
  await row.getByRole('button', { name: 'Confirm payment' }).click()

  const dialog = page.getByRole('dialog')
  await expect(dialog).toContainText('R$19.99')
  await dialog.getByRole('button', { name: 'Confirm' }).click()
  await expect(row).toContainText('Paid')
  await expect(row.getByRole('button', { name: 'Confirm payment' })).toHaveCount(0)
})