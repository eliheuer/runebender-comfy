// Dev-only convenience: auto-load a UFO sitting at
// `web/assets/test-fonts/<MyFont>.ufo/` so reloading the page
// doesn't mean re-dragging the font in every time.
//
// Imported only behind `if (import.meta.env.DEV)` in Runebender.vue
// so prod builds tree-shake the whole module — neither the JS nor
// the bundled .glif/.plist URLs end up in the production bundle.

const FILES = import.meta.glob(
  "../assets/test-fonts/**/*.{glif,plist}",
  { eager: true, query: "?url", import: "default" },
) as Record<string, string>;

/**
 * Fetch every file in `web/assets/test-fonts/` and return them as
 * `File` objects with `webkitRelativePath` set, in the same shape
 * `loadGlifFiles` already accepts from drag-drop + directory picker.
 *
 * Returns an empty array if the test-fonts directory is empty.
 */
export async function readDevTestFontFiles(): Promise<File[]> {
  const entries = Object.entries(FILES);
  if (entries.length === 0) return [];

  return Promise.all(
    entries.map(async ([sourcePath, url]) => {
      const res = await fetch(url);
      const blob = await res.blob();
      // The source path looks like
      //   "../assets/test-fonts/VirtuaGrotesk-Regular.ufo/glyphs/A_.glif"
      // We want the part starting at the .ufo segment so the existing
      // `/glyphs/` filter and `*.ufo/` label-extraction work as if a
      // real folder had been dropped.
      const ufoMatch = sourcePath.match(/([^/]+\.ufo\/.*)$/);
      const rel = ufoMatch ? ufoMatch[1] : sourcePath.split("/").pop()!;
      const fileName = rel.split("/").pop()!;
      const file = new File([blob], fileName);
      try {
        Object.defineProperty(file, "webkitRelativePath", {
          value: rel,
          configurable: true,
        });
      } catch {
        // Some browsers refuse to override the prop; the
        // "any .glif fallback" filter still catches files.
      }
      return file;
    }),
  );
}
