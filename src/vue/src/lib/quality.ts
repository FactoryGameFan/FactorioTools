import { Quality } from "./FactorioToolsApi"
import { QUALITY_LEVELS } from "./plannerDefaults"

export const QUALITY_ORDER: Quality[] = [
  Quality.Normal,
  Quality.Uncommon,
  Quality.Rare,
  Quality.Epic,
  Quality.Legendary,
]

// The integer levels come from the C# Quality enum via plannerDefaults.verified.json.
// Factorio skips a hidden level 4, which is why Legendary is 5 and not 4.
const levels: Record<Quality, number> = {
  [Quality.Normal]: QUALITY_LEVELS.Normal,
  [Quality.Uncommon]: QUALITY_LEVELS.Uncommon,
  [Quality.Rare]: QUALITY_LEVELS.Rare,
  [Quality.Epic]: QUALITY_LEVELS.Epic,
  [Quality.Legendary]: QUALITY_LEVELS.Legendary,
}

const labels: Record<Quality, string> = {
  [Quality.Normal]: "Normal",
  [Quality.Uncommon]: "Uncommon",
  [Quality.Rare]: "Rare",
  [Quality.Epic]: "Epic",
  [Quality.Legendary]: "Legendary",
}

// Official Factorio quality colors.
const colors: Record<Quality, string> = {
  [Quality.Normal]: "#bcbcbc",
  [Quality.Uncommon]: "#3eec57",
  [Quality.Rare]: "#2495ff",
  [Quality.Epic]: "#c400ff",
  [Quality.Legendary]: "#ff9500",
}

// Pip layouts for the in-game quality glyphs, in a 24x24 viewBox.
// Normal=1, Uncommon=2, Rare=3 (triangle), Epic=4 (square), Legendary=6 (flower).
const pips: Record<Quality, ReadonlyArray<readonly [number, number]>> = {
  [Quality.Normal]: [[12, 12]],
  [Quality.Uncommon]: [
    [12, 6.5],
    [12, 17.5],
  ],
  [Quality.Rare]: [
    [7.5, 7],
    [7.5, 17],
    [17, 17],
  ],
  [Quality.Epic]: [
    [7, 7],
    [17, 7],
    [7, 17],
    [17, 17],
  ],
  // 2x2 square of pips with a fifth pip in the center.
  [Quality.Legendary]: [
    [7, 7],
    [17, 7],
    [7, 17],
    [17, 17],
    [12, 12],
  ],
}

// Pip radius per quality, shrinking as the cluster gets denser so the glyph stays balanced.
const pipRadii: Record<Quality, number> = {
  [Quality.Normal]: 5.5,
  [Quality.Uncommon]: 5,
  [Quality.Rare]: 4.6,
  [Quality.Epic]: 4.6,
  [Quality.Legendary]: 4.4,
}

export function qualityLevel(quality: Quality): number {
  return levels[quality]
}

export function qualityLabel(quality: Quality): string {
  return labels[quality]
}

export function qualityColor(quality: Quality): string {
  return colors[quality]
}

export function qualityPips(quality: Quality): ReadonlyArray<readonly [number, number]> {
  return pips[quality]
}

export function qualityPipRadius(quality: Quality): number {
  return pipRadii[quality]
}
