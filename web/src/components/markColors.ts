// Mark color palette + utilities, shared by MarkColorPanel and
// GlyphCell. RGBA strings match runebender-xilem's
// `theme::mark::RGBA_STRINGS` byte for byte so the same .ufo files
// round-trip identically through both editors.

import { THEME_MARK_COLORS } from "../themeTokens";

export type MarkColor = {
  /** UFO `public.markColor` value: "r,g,b,a" with 0–1 floats. */
  rgba: string;
  /** Display name (tooltip / aria-label). */
  name: string;
};

export const MARK_COLORS: MarkColor[] = THEME_MARK_COLORS.map((color) => ({
  rgba: color.ufoRgba,
  name: color.name,
}));

/// Convert "r,g,b,a" (0–1 floats) to a CSS rgba(...) string.
export function rgbaToCss(s: string, alphaOverride?: number): string {
  const [r, g, b, a] = s.split(",").map(Number);
  if ([r, g, b, a].some((n) => !Number.isFinite(n))) return "transparent";
  const aa = alphaOverride ?? a;
  return `rgba(${Math.round(r * 255)}, ${Math.round(g * 255)}, ${Math.round(b * 255)}, ${aa})`;
}
