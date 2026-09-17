import { expect } from '@playwright/test';
import { test } from './utils/fixtures';

test('it runs an added video through the transcribe stages', async ({ page }) => {
  await page.goto('/');
  await page.fill('input[type="text"]', 'https://example.com/video');
  await page.getByRole('button', { name: 'Add', exact: true }).click();

  await expect(page.getByText('Test Video')).toBeVisible();
  await expect(page.getByText('Translating block 1/1')).toBeVisible();
  await expect(page.getByText('Tokens: 12 in / 6 out')).toBeVisible();
});
