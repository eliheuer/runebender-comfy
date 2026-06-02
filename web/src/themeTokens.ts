// First Comfy-side consumer surface for the shared theme tokens now
// checked into runebender-core/themes/runebender.json. Keep these
// values byte-for-byte aligned until this file can be generated from
// the shared JSON artifact.

export const THEME_MARK_COLORS = [
  { name: "red", color: "#ff4a3d", ufoRgba: "1,0.29,0.24,1" },
  { name: "orange", color: "#ff980f", ufoRgba: "1,0.6,0.06,1" },
  { name: "yellow", color: "#ffdc32", ufoRgba: "1,0.86,0.2,1" },
  { name: "green", color: "#18b86f", ufoRgba: "0.09,0.72,0.44,1" },
  { name: "blue", color: "#456fff", ufoRgba: "0.27,0.44,1,1" },
  { name: "purple", color: "#8c6cff", ufoRgba: "0.55,0.42,1,1" },
  { name: "pink", color: "#e86ab8", ufoRgba: "0.91,0.42,0.72,1" },
] as const;

export const THEME_CHROME_COLORS = {
  appBackground: "#101010",
  controlBackground: "#303030",
  panelBackground: "#121212",
  gridCellHoverBackground: "#1d1d1d",
  buttonBackground: "#1d1d1d",
  panelOutline: "#404040",
  strokeWidth: "1.5px",
  panelRadius: "12px",
  buttonRadius: "8px",
  primaryText: "#909090",
  secondaryText: "#707070",
  mutedText: "#808080",
  subduedText: "#505050",
  accent: "#18b86f",
  glyphPreview: "#a0a0a0",
  warning: "#ffdc32",
  backgroundImageSelection: "#456fff",
  danger: "#ff4a3d",
  dangerText: "#ff4a3d",
  overlayText: "#f0f0f0",
  markSelectedRing: "#ffffff",
  markHoverRing: "#bbbbbb",
} as const;
