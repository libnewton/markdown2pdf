export type CardExportPresetId =
  | 'redbook-portrait'
  | 'instagram-feed'
  | 'x-square'
  | 'story'

export type CardExportPreset = {
  id: CardExportPresetId
  aspectRatio: string
  fullResWidth: number
  thumbWidth: number
  pageWidthMm: number
  pageHeightMm: number
  label: string
}

export const CARD_EXPORT_PRESETS: Record<CardExportPresetId, CardExportPreset> = {
  'redbook-portrait': {
    id: 'redbook-portrait',
    aspectRatio: '3 / 4',
    fullResWidth: 1242,
    thumbWidth: 400,
    pageWidthMm: 105,
    pageHeightMm: 140,
    label: 'Redbook 3:4',
  },
  'instagram-feed': {
    id: 'instagram-feed',
    aspectRatio: '4 / 5',
    fullResWidth: 1080,
    thumbWidth: 360,
    pageWidthMm: 108,
    pageHeightMm: 135,
    label: 'Instagram 4:5',
  },
  'x-square': {
    id: 'x-square',
    aspectRatio: '1 / 1',
    fullResWidth: 1080,
    thumbWidth: 340,
    pageWidthMm: 108,
    pageHeightMm: 108,
    label: 'X Square 1:1',
  },
  story: {
    id: 'story',
    aspectRatio: '9 / 16',
    fullResWidth: 1080,
    thumbWidth: 260,
    pageWidthMm: 108,
    pageHeightMm: 192,
    label: 'Story 9:16',
  },
}

export const DEFAULT_CARD_EXPORT_PRESET: CardExportPresetId = 'instagram-feed'

export function getCardExportPreset(presetId: CardExportPresetId): CardExportPreset {
  return CARD_EXPORT_PRESETS[presetId] ?? CARD_EXPORT_PRESETS[DEFAULT_CARD_EXPORT_PRESET]
}

export function isCardExportPresetId(value: string | null): value is CardExportPresetId {
  return value !== null && value in CARD_EXPORT_PRESETS
}

export function normalizeCardExportPresetId(value: string | null): CardExportPresetId | null {
  if (value === 'xiaohongshu-portrait') return 'redbook-portrait'
  return isCardExportPresetId(value) ? value : null
}
