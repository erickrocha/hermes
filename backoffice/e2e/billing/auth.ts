import { expect, type Page } from '@playwright/test'

export async function signIn(page: Page, email: string, password: string) {
  await page.goto('/login')
  await page.getByRole('textbox', { name: 'Email' }).fill(email)
  await page.getByRole('textbox', { name: 'Password' }).fill(password)
  await page.getByRole('button', { name: 'Sign in' }).click()
  await expect(page.getByRole('heading', { name: /Hello|Welcome/ })).toBeVisible()
}

export async function openBilling(page: Page) {
  await page.goto('/billing')
  await expect(page).toHaveURL(/\/billing$/)
  await expect(page.getByRole('heading', { name: 'Billing', exact: true })).toBeVisible()
}

export function testValue(name: string) {
  const value = process.env[name]
  if (!value) throw new Error(`Missing Playwright fixture ${name}`)
  return value
}