// Mark color palette + utilities, shared by MarkColorPanel and
// GlyphCell. RGBA strings match runebender-xilem's
// `theme::mark::RGBA_STRINGS` byte for byte so the same .ufo files
// round-trip identically through both editors.

export type MarkColor = {
  /** UFO `public.markColor` value: "r,g,b,a" with 0–1 floats. */
  rgba: string;
  /** Display name (tooltip / aria-label). */
  name: string;
};

export const MARK_COLORS: MarkColor[] = [
  { rgba: "1,0.3,0.3,1", name: "red" },
  { rgba: "1,0.6,0.2,1", name: "orange" },
  { rgba: "1,0.9,0.2,1", name: "yellow" },
  { rgba: "0.3,0.7,0.3,1", name: "green" },
  { rgba: "0.1,0.3,0.8,1", name: "blue" },
  { rgba: "0.6,0.3,0.9,1", name: "purple" },
  { rgba: "0.9,0.3,0.7,1", name: "pink" },
];

/// Convert "r,g,b,a" (0–1 floats) to a CSS rgba(...) string.
export function rgbaToCss(s: string, alphaOverride?: number): string {
  const [r, g, b, a] = s.split(",").map(Number);
  if ([r, g, b, a].some((n) => !Number.isFinite(n))) return "transparent";
  const aa = alphaOverride ?? a;
  return `rgba(${Math.round(r * 255)}, ${Math.round(g * 255)}, ${Math.round(b * 255)}, ${aa})`;
}
