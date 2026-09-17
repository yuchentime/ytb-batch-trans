import { expect } from '@playwright/test';
import { test } from './utils/fixtures';

test('it renders the environment check from the probe result', async ({ page }) => {
  await page.goto('/setup');

  await expect(page.getByRole('heading', { name: 'Environment check' })).toBeVisible();
  await expect(page.getByText('Everything needed for transcription is available.')).toBeVisible();
  await expect(page.getByText('NVIDIA Test GPU')).toBeVisible();
  await expect(page.getByText('C:\\Python314\\Scripts\\whisper.exe · 20250625')).toBeVisible();
});
