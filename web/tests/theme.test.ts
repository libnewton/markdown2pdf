import { describe, expect, it, vi } from 'vitest'

function channel(value: number): number {
  const normalized = value / 255
  return normalized <= 0.04045 ? normalized / 12.92 : ((normalized + 0.055) / 1.055) ** 2.4
}

function luminance(hex: string): number {
  const value = Number.parseInt(hex.slice(1), 16)
  return 0.2126 * channel(value >> 16) + 0.7152 * channel((value >> 8) & 255) + 0.0722 * channel(value & 255)
}

function contrast(a: string, b: string): number {
  const [light, dark] = [luminance(a), luminance(b)].sort((x, y) => y - x)
  return (light + 0.05) / (dark + 0.05)
}

describe('theme accessibility', () => {
  it.each([
    ['light body', '#1a222d', '#f4f6f8', 4.5],
    ['light surface', '#1a222d', '#ffffff', 4.5],
    ['light focus', '#245fc5', '#ffffff', 3],
    ['dark body', '#edf1f6', '#0d1218', 4.5],
    ['dark surface', '#edf1f6', '#141b24', 4.5],
    ['dark focus', '#82b1ff', '#141b24', 3],
  ])('%s meets its contrast gate', (_name, foreground, background, minimum) => {
    expect(contrast(foreground as string, background as string)).toBeGreaterThanOrEqual(minimum as number)
  })

  it('persists an explicit theme', async () => {
    vi.doMock('$app/environment', () => ({ browser: true }))
    const { settingsStore } = await import('$lib/stores/settingsStore.svelte')
    settingsStore.setTheme('dark')
    expect(settingsStore.theme).toBe('dark')
    expect(localStorage.getItem('md2pdf-theme')).toBe('dark')
  })
})
