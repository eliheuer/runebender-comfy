<script setup lang="ts">
import { ref } from "vue";
import type {
  GlyphSidebarFilter,
  SidebarCategoryGroup,
  SidebarLanguageGroup,
  SidebarBuiltinFilter,
  SidebarSearchMode,
} from "../glyphSidebarData";

export type Category =
  | "All"
  | "Letter"
  | "Number"
  | "Punctuation"
  | "Symbol"
  | "Mark"
  | "Separator"
  | "Other";

const props = defineProps<{
  selected: GlyphSidebarFilter;
  searchQuery: string;
  searchMode: SidebarSearchMode;
  searchMatchCase: boolean;
  searchRegex: boolean;
  categoryGroups: SidebarCategoryGroup[];
  languageGroups: SidebarLanguageGroup[];
  filters: SidebarBuiltinFilter[];
  counts?: Record<string, number>;
  totalCount: number;
}>();

const emit = defineEmits<{
  (e: "select", filter: GlyphSidebarFilter): void;
  (e: "update:searchQuery", value: string): void;
  (e: "update:searchMode", value: SidebarSearchMode): void;
  (e: "update:searchMatchCase", value: boolean): void;
  (e: "update:searchRegex", value: boolean): void;
}>();

const expandedCategories = ref(new Set<Category>());
const expandedLanguages = ref(new Set<string>(["Arab"]));
const searchMenuOpen = ref(false);

const searchModes: Array<{ value: SidebarSearchMode; label: string }> = [
  { value: "all", label: "All" },
  { value: "name", label: "Name" },
  { value: "unicode", label: "Unicode" },
];

function filterKey(filter: GlyphSidebarFilter): string {
  if (filter.kind === "all") return "all";
  if (filter.kind === "category") {
    return filter.subcategory
      ? `category:${filter.category}:${filter.subcategory}`
      : `category:${filter.category}`;
  }
  return `${filter.kind}:${filter.id}`;
}

function isSelected(a: GlyphSidebarFilter, b: GlyphSidebarFilter): boolean {
  return filterKey(a) === filterKey(b);
}

function toggleCategory(category: Category) {
  const next = new Set(expandedCategories.value);
  if (next.has(category)) next.delete(category);
  else next.add(category);
  expandedCategories.value = next;
}

function toggleLanguage(id: string) {
  const next = new Set(expandedLanguages.value);
  if (next.has(id)) next.delete(id);
  else next.add(id);
  expandedLanguages.value = next;
}

function selectSearchMode(mode: SidebarSearchMode) {
  emit("update:searchMode", mode);
  searchMenuOpen.value = false;
}

function clearSearch() {
  emit("update:searchQuery", "");
}

function countFor(filter: GlyphSidebarFilter): string {
  return String(props.counts?.[filterKey(filter)] ?? "");
}

function badgeFor(filter: GlyphSidebarFilter, expected?: number): string {
  const count = props.counts?.[filterKey(filter)] ?? 0;
  return expected ? `${count}/${expected}` : count > 0 ? String(count) : "";
}
</script>

<template>
  <aside class="category-sidebar">
    <div class="search-wrap">
      <button
        type="button"
        class="search-mode"
        aria-label="Search options"
        @click="searchMenuOpen = !searchMenuOpen"
      >
        ⌕
        <svg class="search-chevron" viewBox="0 0 12 12" aria-hidden="true">
          <path d="M2.5 4 6 7.5 9.5 4" />
        </svg>
      </button>
      <input
        class="search-input"
        type="search"
        spellcheck="false"
        placeholder="Search"
        :value="searchQuery"
        @input="$emit('update:searchQuery', ($event.target as HTMLInputElement).value)"
      />
      <button
        v-if="searchQuery"
        type="button"
        class="clear-search"
        aria-label="Clear search"
        @click="clearSearch"
      >
        ×
      </button>
      <div v-if="searchMenuOpen" class="search-menu">
        <button
          v-for="mode in searchModes"
          :key="mode.value"
          type="button"
          class="search-menu-row"
          @click="selectSearchMode(mode.value)"
        >
          <span class="check">{{ searchMode === mode.value ? "✓" : "" }}</span>
          {{ mode.label }}
        </button>
        <div class="search-menu-separator"></div>
        <button
          type="button"
          class="search-menu-row"
          @click="$emit('update:searchMatchCase', !searchMatchCase)"
        >
          <span class="check">{{ searchMatchCase ? "✓" : "" }}</span>
          Match Case
        </button>
        <button
          type="button"
          class="search-menu-row"
          @click="$emit('update:searchRegex', !searchRegex)"
        >
          <span class="check">{{ searchRegex ? "✓" : "" }}</span>
          Regex
        </button>
      </div>
    </div>

    <div class="scroll">
      <button
        type="button"
        class="row top-row"
        :class="{ active: selected.kind === 'all' }"
        @click="$emit('select', { kind: 'all' })"
      >
        <span class="icon all-icon">▦</span>
        <span class="row-name">All</span>
        <span class="count">{{ totalCount }}</span>
      </button>

      <div class="section-title">Categories</div>
      <ul class="list">
        <li v-for="group in categoryGroups" :key="group.category">
          <div class="row-wrap">
            <button
              v-if="group.subfilters?.length"
              type="button"
              class="disclosure"
              :class="{ open: expandedCategories.has(group.category) }"
              @click="toggleCategory(group.category)"
            >
              <svg class="disclosure-icon" viewBox="0 0 12 12" aria-hidden="true">
                <path d="M4 2.5 7.5 6 4 9.5" />
              </svg>
            </button>
            <span v-else class="disclosure-spacer"></span>
            <button
              type="button"
              class="row"
              :class="{ active: isSelected(selected, { kind: 'category', category: group.category }) }"
              @click="$emit('select', { kind: 'category', category: group.category })"
            >
              <span class="icon">{{ group.icon }}</span>
              <span class="row-name">{{ group.category }}</span>
              <span class="count">{{ countFor({ kind: "category", category: group.category }) }}</span>
            </button>
          </div>
          <ul
            v-if="group.subfilters?.length && expandedCategories.has(group.category)"
            class="sublist"
          >
            <li v-for="sub in group.subfilters" :key="sub.id">
              <button
                type="button"
                class="row subrow"
                :class="{ active: isSelected(selected, { kind: 'category', category: group.category, subcategory: sub.id }) }"
                @click="$emit('select', { kind: 'category', category: group.category, subcategory: sub.id })"
              >
                <span class="row-name">{{ sub.label }}</span>
                <span class="badge">{{ badgeFor({ kind: "category", category: group.category, subcategory: sub.id }) }}</span>
              </button>
            </li>
          </ul>
        </li>
      </ul>

      <div class="section-title with-action">
        <span>Languages</span>
        <button type="button" class="section-action" aria-label="Manage languages">+</button>
      </div>
      <ul class="list">
        <li v-for="group in languageGroups" :key="group.id">
          <div class="row-wrap">
            <button
              type="button"
              class="disclosure"
              :class="{ open: expandedLanguages.has(group.id) }"
              @click="toggleLanguage(group.id)"
            >
              <svg class="disclosure-icon" viewBox="0 0 12 12" aria-hidden="true">
                <path d="M4 2.5 7.5 6 4 9.5" />
              </svg>
            </button>
            <button
              type="button"
              class="row"
              :class="{ active: isSelected(selected, { kind: 'languageGroup', id: group.id }) }"
              @click="$emit('select', { kind: 'languageGroup', id: group.id })"
            >
              <span class="icon">{{ group.icon }}</span>
              <span class="row-name">{{ group.label }}</span>
            </button>
          </div>
          <ul v-if="expandedLanguages.has(group.id)" class="sublist">
            <li v-for="filter in group.filters" :key="filter.id">
              <button
                type="button"
                class="row subrow"
                :class="{ active: isSelected(selected, { kind: 'language', id: filter.id }) }"
                @click="$emit('select', { kind: 'language', id: filter.id })"
              >
                <span class="row-name">{{ filter.label }}</span>
                <span class="badge">{{ badgeFor({ kind: "language", id: filter.id }, filter.expectedCount) }}</span>
              </button>
            </li>
          </ul>
        </li>
      </ul>

      <div class="section-title">Filters</div>
      <ul class="list">
        <li v-for="filter in filters" :key="filter.id">
          <button
            type="button"
            class="row"
            :class="{ active: isSelected(selected, filter.source === 'google-fonts-glyphsets' ? { kind: 'gfGlyphset', id: filter.id } : { kind: 'builtin', id: filter.id }) }"
            @click="$emit('select', filter.source === 'google-fonts-glyphsets' ? { kind: 'gfGlyphset', id: filter.id } : { kind: 'builtin', id: filter.id })"
          >
            <span class="icon">{{ filter.source === "runebender" ? "⚙" : "≡" }}</span>
            <span class="row-name">{{ filter.label }}</span>
            <span class="badge">{{ badgeFor(filter.source === "google-fonts-glyphsets" ? { kind: "gfGlyphset", id: filter.id } : { kind: "builtin", id: filter.id }, filter.expectedCount) }}</span>
          </button>
        </li>
      </ul>
    </div>
  </aside>
</template>

<style scoped>
.category-sidebar {
  width: 280px;
  flex-shrink: 0;
  background: var(--rb-panel-background, #1c1c1c);
  border: 1.5px solid var(--rb-panel-outline, #606060);
  border-radius: 8px;
  display: flex;
  flex-direction: column;
  overflow: visible;
}

.search-wrap {
  position: relative;
  display: flex;
  align-items: center;
  margin: 8px 8px 6px;
  height: 30px;
  border: 1.5px solid var(--rb-panel-outline, #606060);
  border-radius: 7px;
  background: #252525;
}

.search-wrap:focus-within {
  border-color: var(--rb-accent, #66ee88);
}

.search-mode,
.clear-search,
.disclosure,
.section-action {
  appearance: none;
  border: 0;
  background: transparent;
  color: var(--rb-primary-text, #909090);
  cursor: pointer;
}

.search-mode:focus,
.clear-search:focus,
.section-action:focus,
.disclosure:focus,
.row:focus,
.search-menu-row:focus {
  outline: none;
}

.search-mode:focus-visible,
.clear-search:focus-visible,
.section-action:focus-visible,
.disclosure:focus-visible {
  outline: 1.5px solid var(--rb-accent, #66ee88);
  outline-offset: 1.5px;
  border-radius: 4px;
}

.row:focus-visible,
.search-menu-row:focus-visible {
  outline: none;
  border-color: var(--rb-accent, #66ee88);
}

.search-mode {
  width: 42px;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 3px;
  padding: 0;
  font-size: 20px;
}

.search-chevron {
  width: 10px;
  height: 10px;
  display: block;
  overflow: visible;
}

.search-chevron path {
  fill: none;
  stroke: currentColor;
  stroke-width: 2;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.search-input {
  flex: 1;
  min-width: 0;
  height: 100%;
  border: 0;
  outline: 0;
  background: transparent;
  color: var(--rb-primary-text, #e8e8e8);
  font: 16px ui-sans-serif, system-ui, sans-serif;
}

.search-input::placeholder {
  color: var(--rb-muted-text, #707070);
}

.clear-search {
  width: 28px;
  height: 28px;
  margin-right: 2px;
  border-radius: 999px;
  font-size: 22px;
}

.search-menu {
  position: absolute;
  z-index: 5;
  top: 34px;
  left: 0;
  width: 146px;
  padding: 6px 0;
  background: #222;
  border: 1.5px solid var(--rb-panel-outline, #606060);
  border-radius: 7px;
  box-shadow: 0 12px 24px rgb(0 0 0 / 45%);
}

.search-menu-row {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  height: 28px;
  padding: 0 14px;
  border: 0;
  background: transparent;
  color: var(--rb-primary-text, #e8e8e8);
  cursor: pointer;
  font: 15px ui-sans-serif, system-ui, sans-serif;
}

.search-menu-separator {
  height: 1px;
  margin: 6px 14px;
  background: var(--rb-panel-outline, #606060);
}

.check {
  width: 14px;
}

.scroll {
  overflow-y: auto;
  min-height: 0;
  flex: 1;
  box-sizing: border-box;
  width: calc(100% + 7.5px);
  padding: 0 15.5px 8px 8px;
}

.scroll::-webkit-scrollbar {
  width: 6px;
}

.scroll::-webkit-scrollbar-thumb {
  border-width: 1.5px;
}

.scroll::-webkit-scrollbar-thumb:hover {
  border-width: 1.5px;
}

.section-title {
  margin: 12px 0 5px;
  color: var(--rb-muted-text, #808080);
  font: 12px ui-sans-serif, system-ui, sans-serif;
  font-weight: 700;
  letter-spacing: 0;
  text-transform: uppercase;
}

.with-action {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.section-action {
  width: 22px;
  height: 20px;
  font-size: 24px;
  line-height: 18px;
}

.list,
.sublist {
  list-style: none;
  margin: 0;
  padding: 0;
}

.row-wrap {
  display: flex;
  align-items: center;
}

.disclosure,
.disclosure-spacer {
  width: 24px;
  height: 24px;
  flex: 0 0 24px;
}

.disclosure {
  position: relative;
  z-index: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 0;
  transform: rotate(0deg);
}

.disclosure-icon {
  width: 12px;
  height: 12px;
  display: block;
  overflow: visible;
}

.disclosure-icon path {
  fill: none;
  stroke: currentColor;
  stroke-width: 2;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.disclosure.open {
  transform: rotate(90deg);
}

.row {
  appearance: none;
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  min-width: 0;
  height: 24px;
  padding: 0 8px;
  border: 1.5px solid transparent;
  border-radius: 6px;
  background: transparent;
  color: var(--rb-primary-text, #e8e8e8);
  cursor: pointer;
  font: 15px ui-sans-serif, system-ui, sans-serif;
  text-align: left;
}

.row-wrap > .row {
  margin-left: -24px;
  padding-left: 34px;
}

.row.active {
  background: transparent;
  border-color: var(--rb-accent, #66ee88);
  color: var(--rb-accent, #66ee88);
}

.top-row {
  margin-top: 2px;
}

.icon {
  flex: 0 0 22px;
  color: var(--rb-accent, #66ee88);
  font-weight: 700;
  text-align: center;
}

.all-icon {
  font-size: 18px;
}

.row-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.count,
.badge {
  margin-left: auto;
  flex: 0 0 auto;
  color: inherit;
}

.badge {
  min-width: 32px;
  height: 18px;
  padding: 0 6px;
  color: inherit;
  font-size: 13px;
  line-height: 18px;
  text-align: center;
}

.sublist {
  margin-left: 0;
}

.subrow {
  height: 22px;
  padding-left: 34px;
  font-size: 14px;
}
</style>
