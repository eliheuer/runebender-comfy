// First Comfy-side consumer surface for the shared theme tokens now
// checked into runebender-core/themes/runebender.json. Keep these
// values byte-for-byte aligned until this file can be generated from
// the shared JSON artifact.

export const THEME_MARK_COLORS = [
  { name: "red", color: "#ff4040", ufoRgba: "1,0.3,0.3,1" },
  { name: "orange", color: "#ff9933", ufoRgba: "1,0.6,0.2,1" },
  { name: "yellow", color: "#ffdd33", ufoRgba: "1,0.9,0.2,1" },
  { name: "green", color: "#22bb77", ufoRgba: "0.3,0.7,0.3,1" },
  { name: "blue", color: "#4488ff", ufoRgba: "0.1,0.3,0.8,1" },
  { name: "purple", color: "#9955dd", ufoRgba: "0.6,0.3,0.9,1" },
  { name: "pink", color: "#dd55aa", ufoRgba: "0.9,0.3,0.7,1" },
] as const;

export const THEME_CHROME_COLORS = {
  appBackground: "#101010",
  controlBackground: "#303030",
  panelBackground: "#121212",
  panelOutline: "#404040",
  primaryText: "#909090",
  secondaryText: "#707070",
  mutedText: "#808080",
  subduedText: "#505050",
  accent: "#66ee88",
  glyphPreview: "#a0a0a0",
  warning: "#ffdd33",
  backgroundImageSelection: "#4488ff",
  danger: "#ff3333",
  dangerText: "#ff7777",
  overlayText: "#f0f0f0",
  markSelectedRing: "#ffffff",
  markHoverRing: "#bbbbbb",
} as const;
