import type { Category } from "./components/CategorySidebar.vue";
import { GF_GLYPHSETS } from "./gfSidebarData.generated";

export type GlyphSidebarFilter =
  | { kind: "all" }
  | { kind: "category"; category: Category; subcategory?: string }
  | { kind: "languageGroup"; id: string }
  | { kind: "language"; id: string }
  | { kind: "gfGlyphset"; id: string }
  | { kind: "builtin"; id: string };

export type SidebarSearchMode = "all" | "name" | "unicode";
export type GlyphSortMode = "name" | "unicode";

export type SidebarCategorySubfilter = {
  id: string;
  label: string;
};

export type SidebarCategoryGroup = {
  category: Category;
  icon: string;
  subfilters?: SidebarCategorySubfilter[];
};

export type SidebarLanguageGroup = {
  id: string;
  label: string;
  icon: string;
  filters: SidebarCharacterFilter[];
};

export type SidebarCharacterFilter = {
  id: string;
  label: string;
  source: "google-fonts-lang" | "google-fonts-glyphsets" | "unicode-range";
  glyphNames?: readonly string[];
  chars?: string;
  ranges?: Array<[number, number]>;
  expectedCount?: number;
};

export type SidebarBuiltinFilter = {
  id: string;
  label: string;
  source: "runebender" | "google-fonts-glyphsets";
  glyphNames?: readonly string[];
  chars?: string;
  ranges?: Array<[number, number]>;
  expectedCount?: number;
};

export const CATEGORY_GROUPS: SidebarCategoryGroup[] = [
  {
    category: "Letter",
    icon: "A",
    subfilters: [
      { id: "uppercase", label: "Uppercase" },
      { id: "lowercase", label: "Lowercase" },
      { id: "ligature", label: "Ligature" },
    ],
  },
  {
    category: "Number",
    icon: "9",
    subfilters: [
      { id: "decimal", label: "Decimal" },
      { id: "fraction", label: "Fraction" },
      { id: "superior-inferior", label: "Superior/Inferior" },
    ],
  },
  {
    category: "Separator",
    icon: "··",
    subfilters: [
      { id: "space", label: "Space" },
      { id: "line", label: "Line" },
    ],
  },
  {
    category: "Punctuation",
    icon: "?",
    subfilters: [
      { id: "quote", label: "Quote" },
      { id: "dash", label: "Dash" },
      { id: "paren", label: "Parenthesis" },
    ],
  },
  {
    category: "Symbol",
    icon: "¶",
    subfilters: [
      { id: "currency", label: "Currency" },
      { id: "math", label: "Math" },
      { id: "arrow", label: "Arrow" },
    ],
  },
  {
    category: "Mark",
    icon: "◌",
    subfilters: [
      { id: "nonspacing", label: "Nonspacing" },
      { id: "spacing", label: "Spacing" },
    ],
  },
  { category: "Other", icon: "•" },
];

const SCRIPT_META: Record<string, { id: string; label: string; icon: string }> = {
  Arabic: { id: "Arab", label: "Arabic", icon: "ض" },
  Cyrillic: { id: "Cyrl", label: "Cyrillic", icon: "Я" },
  Greek: { id: "Grek", label: "Greek", icon: "Ω" },
  Latin: { id: "Latn", label: "Latin", icon: "G" },
  Phonetics: { id: "Phon", label: "Phonetics", icon: "ə" },
  TransLatin: { id: "Tran", label: "TransLatin", icon: "Ǧ" },
};

function glyphsetFiltersForScript(script: string): SidebarCharacterFilter[] {
  return GF_GLYPHSETS.filter((glyphset) => glyphset.script === script).map((glyphset) => ({
    id: glyphset.id,
    label: glyphset.label,
    source: "google-fonts-glyphsets",
    glyphNames: glyphset.glyphNames,
    expectedCount: glyphset.expectedCount,
  }));
}

export const SIDEBAR_LANGUAGE_GROUPS: SidebarLanguageGroup[] = [
  {
    ...SCRIPT_META.Arabic,
    filters: glyphsetFiltersForScript("Arabic"),
  },
  {
    id: "Hans",
    label: "Chinese",
    icon: "字",
    filters: [
      { id: "Hans", label: "Chinese Han", source: "unicode-range", ranges: [[0x4e00, 0x9fff]] },
    ],
  },
  {
    id: "Cyrl",
    label: "Cyrillic",
    icon: "Я",
    filters: glyphsetFiltersForScript("Cyrillic"),
  },
  {
    id: "Deva",
    label: "Devanagari",
    icon: "दे",
    filters: [
      { id: "Deva", label: "Devanagari", source: "unicode-range", ranges: [[0x0900, 0x097f]] },
    ],
  },
  {
    id: "Grek",
    label: "Greek",
    icon: "Ω",
    filters: glyphsetFiltersForScript("Greek"),
  },
  {
    id: "Jpan",
    label: "Japanese",
    icon: "あ",
    filters: [
      { id: "Jpan", label: "Kana + Han", source: "unicode-range", ranges: [[0x3040, 0x30ff], [0x4e00, 0x9fff]] },
    ],
  },
  {
    id: "Kore",
    label: "Korean",
    icon: "한",
    filters: [
      { id: "Kore", label: "Hangul", source: "unicode-range", ranges: [[0x1100, 0x11ff], [0xac00, 0xd7af]] },
    ],
  },
  {
    id: "Latn",
    label: "Latin",
    icon: "G",
    filters: glyphsetFiltersForScript("Latin"),
  },
  {
    id: "Thai",
    label: "Thai",
    icon: "ก",
    filters: [
      { id: "Thai", label: "Thai", source: "unicode-range", ranges: [[0x0e00, 0x0e7f]] },
    ],
  },
  ...Object.values(SCRIPT_META)
    .filter((meta) => !["Arab", "Cyrl", "Grek", "Latn"].includes(meta.id))
    .map((meta) => ({
      ...meta,
      filters: glyphsetFiltersForScript(meta.label),
    }))
    .filter((group) => group.filters.length > 0),
];

export const SIDEBAR_FILTERS: SidebarBuiltinFilter[] = [
  { id: "exporting", label: "Exporting glyphs", source: "runebender" },
  { id: "incompatible", label: "Incompatible masters", source: "runebender" },
  ...GF_GLYPHSETS.filter((glyphset) =>
    ["GF_Latin_Core", "GF_Latin_Plus", "GF_Arabic_Core", "GF_Cyrillic_Core", "GF_Greek_Core"].includes(
      glyphset.id,
    ),
  ).map((glyphset) => ({
    id: glyphset.id,
    label: glyphset.label,
    source: "google-fonts-glyphsets" as const,
    glyphNames: glyphset.glyphNames,
    expectedCount: glyphset.expectedCount,
  })),
];
