import { describe, expect, it } from 'vitest';
import { availableLocales, resolveLocale } from '../../src/i18n';

describe('locale resolution', () => {
  it('resolves registered locale codes exactly', () => {
    expect(resolveLocale('zh-CN')).toBe('zh-CN');
    expect(resolveLocale('pt-BR')).toBe('pt-BR');
    expect(resolveLocale('en')).toBe('en');
  });

  it('maps language-only and legacy codes onto registered locales', () => {
    expect(resolveLocale('zh')).toBe('zh-CN');
    expect(resolveLocale('zh-Hans')).toBe('zh-CN');
    expect(resolveLocale('zh_Hant')).toBe('zh-CN');
    expect(resolveLocale('zh-TW')).toBe('zh-CN');
    expect(resolveLocale('zh-Hant')).toBe('zh-CN');
    expect(resolveLocale('pt')).toBe('pt-PT');
    expect(resolveLocale('nb-NO')).toBe('nb');
  });

  it('falls back to english for unknown or empty codes', () => {
    expect(resolveLocale('xx')).toBe('en');
    expect(resolveLocale('')).toBe('en');
    expect(resolveLocale(null)).toBe('en');
    expect(resolveLocale(undefined)).toBe('en');
  });

  it('registers Simplified Chinese instead of Traditional Chinese', () => {
    expect(Object.keys(availableLocales)).toContain('zh-CN');
    expect(Object.keys(availableLocales)).not.toContain('zh-TW');
  });
});
