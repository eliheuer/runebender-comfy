"""Workspace helpers for FONT wires.

The graph should pass opaque workspace references between nodes. Each
reference identifies a workspace entry that keeps source and compiled
font forms side by side.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import xml.etree.ElementTree as ET


WORKSPACE_DIR = Path(__file__).resolve().parent.parent / "workspace"
FONTS_DIR = WORKSPACE_DIR / "fonts"
# Default source preference: UFO/designspace first, Glyphs as an
# alternate import/source format.
DEFAULT_SOURCE_EXTS = (".designspace", ".ufo", ".glyphs", ".glyphspackage")
SOURCE_EXTS_BY_KIND = {
    "ufo/designspace": (".designspace", ".ufo", ".glyphs", ".glyphspackage"),
    "glyphs": (".glyphs", ".designspace", ".ufo", ".glyphspackage"),
    "glyphspackage": (".glyphspackage", ".glyphs", ".designspace", ".ufo"),
}
COMPILED_EXTS = (".ttf", ".otf", ".woff", ".woff2")
TEXT_EXTS = (".glif", ".plist", ".designspace", ".fea", ".txt", ".md", ".glyphs", ".json", ".yaml", ".yml")
MANIFEST_NAME = "workspace.json"


@dataclass(frozen=True)
class WorkspaceSlot:
    name: str
    source_kind: str | None
    source_path: Path | None
    compiled_path: Path | None


@dataclass(frozen=True)
class WorkspaceWriteResult:
    workspace_path: Path
    source_path: Path | None = None


@dataclass(frozen=True)
class WorkspaceExportResult:
    destination: Path
    copied_paths: tuple[Path, ...]
    linked: bool = False
    origin_root: Path | None = None
    origin_source: Path | None = None


@dataclass(frozen=True)
class WorkspaceSourceInfo:
    linked: bool
    origin_root: Path | None = None
    origin_source: Path | None = None


def ensure_workspace() -> None:
    FONTS_DIR.mkdir(parents=True, exist_ok=True)


def _slot_dir(slot: str) -> Path:
    return FONTS_DIR / _clean_slot_name(slot)


def _slugify(text: str) -> str:
    slug = re.sub(r"[^A-Za-z0-9]+", "-", text).strip("-").lower()
    return slug or "font"


def make_slot_name(source_path: str) -> str:
    stem = Path(source_path).stem or "font"
    base = _slugify(stem)
    return _unique_slot_name(base)


def make_fork_name(slot: str) -> str:
    base = f"{_slugify(_clean_slot_name(slot))}-fork"
    return _unique_slot_name(base)


def _unique_slot_name(base: str) -> str:
    candidate = base
    i = 2
    while _slot_dir(candidate).exists():
        candidate = f"{base}-{i:03d}"
        i += 1
    return candidate


def _clean_slot_name(slot: str) -> str:
    name = Path(str(slot)).name.strip()
    if not name or name in {".", ".."}:
        raise ValueError("slot name is required")
    return name


def list_slots() -> list[str]:
    ensure_workspace()
    if not FONTS_DIR.exists():
        return []
    slots = []
    for entry in sorted(FONTS_DIR.iterdir()):
        if entry.is_dir() and slot_from_name(entry.name) is not None:
            slots.append(entry.name)
    return slots


def source_display_label(slot: str) -> str:
    source_info = source_info_for_slot(slot)
    if source_info.origin_source is not None:
        return source_info.origin_source.name
    return _clean_slot_name(slot)


def list_workspace_choices() -> list[dict[str, str]]:
    return [
        {
            "slot": slot,
            "label": source_display_label(slot),
            "origin_source": str(source_info_for_slot(slot).origin_source or ""),
        }
        for slot in list_slots()
    ]


def slot_from_name(slot: str) -> WorkspaceSlot | None:
    ensure_workspace()
    slot_dir = _slot_dir(slot)
    if not slot_dir.exists():
        return None
    manifest = _read_manifest(slot_dir)
    source_kind = manifest.get("source_kind") if manifest else None
    source_exts = SOURCE_EXTS_BY_KIND.get(source_kind or "", DEFAULT_SOURCE_EXTS)
    source = _find_entry(slot_dir, source_exts)
    compiled = None
    compiled_manifest = manifest.get("compiled_path") if manifest else None
    if compiled_manifest:
        candidate = slot_dir / compiled_manifest
        if candidate.exists():
            compiled = candidate
    if compiled is None:
        compiled = _find_entry(slot_dir, COMPILED_EXTS)
    if source is None and compiled is None:
        return None
    return WorkspaceSlot(
        name=slot,
        source_kind=source_kind,
        source_path=source,
        compiled_path=compiled,
    )


def source_info_for_slot(slot: str) -> WorkspaceSourceInfo:
    ensure_workspace()
    manifest = _read_manifest(_slot_dir(slot))
    if manifest.get("origin_mode") != "linked":
        return WorkspaceSourceInfo(linked=False)
    origin_root_raw = manifest.get("origin_root", "")
    origin_source_raw = manifest.get("origin_source", "")
    return WorkspaceSourceInfo(
        linked=True,
        origin_root=Path(origin_root_raw).expanduser().resolve() if origin_root_raw else None,
        origin_source=Path(origin_source_raw).expanduser().resolve() if origin_source_raw else None,
    )


def refresh_linked_slot_from_source_if_newer(slot: str) -> bool:
    """Refresh a linked workspace cache when its disk source changed.

    Linked workspaces are editable caches of real UFO/designspace
    sources. Saving from Runebender mirrors changes back to disk, but
    external editors can change the same source while ComfyUI is
    closed. This detects that case and rebuilds the cache from disk on
    the next editor open.
    """
    ensure_workspace()
    clean_slot = _clean_slot_name(slot)
    slot_dir = _slot_dir(clean_slot)
    manifest = _read_manifest(slot_dir)
    if manifest.get("origin_mode") != "linked":
        return False
    origin_source_raw = manifest.get("origin_source", "")
    if not origin_source_raw:
        return False
    origin_source = Path(origin_source_raw).expanduser().resolve()
    if not origin_source.exists():
        return False
    try:
        current_snapshot = _linked_source_snapshot_mtime_ns(origin_source)
    except OSError:
        return False
    try:
        cached_snapshot = int(manifest.get("origin_snapshot_mtime_ns", "0") or "0")
    except ValueError:
        cached_snapshot = 0
    if current_snapshot <= cached_snapshot:
        return False

    source_kind = manifest.get("origin_kind") or manifest.get("source_kind") or "ufo/designspace"
    create_slot_from_path(
        str(origin_source),
        slot_name=clean_slot,
        source_kind=source_kind,
        linked=True,
    )
    return True


def _linked_slot_for_source(src: Path) -> str | None:
    ensure_workspace()
    source = src.expanduser().resolve()
    if not FONTS_DIR.exists():
        return None
    for entry in sorted(FONTS_DIR.iterdir()):
        if not entry.is_dir():
            continue
        manifest = _read_manifest(entry)
        if manifest.get("origin_mode") != "linked":
            continue
        origin_source_raw = manifest.get("origin_source", "")
        if not origin_source_raw:
            continue
        try:
            origin_source = Path(origin_source_raw).expanduser().resolve()
        except OSError:
            continue
        if origin_source == source:
            return entry.name
    return None


def create_slot_from_path(
    source_path: str,
    slot_name: str | None = None,
    source_kind: str | None = None,
    linked: bool = False,
) -> str:
    ensure_workspace()
    src = Path(source_path).expanduser().resolve()
    if not src.exists():
        raise FileNotFoundError(f"Font not found: {source_path!r}")

    if source_kind is None or not str(source_kind).strip() or str(source_kind).strip().lower() == "auto":
        source_kind = _infer_source_kind(src) or "ufo/designspace"
    source_kind = _clean_source_kind(source_kind, src)
    if linked and source_kind != "ufo/designspace":
        raise ValueError("linked save-back is currently supported only for .designspace and .ufo sources")

    requested_slot = (slot_name or "").strip()
    reusable_slot = _linked_slot_for_source(src) if linked and not requested_slot else None
    slot = _clean_slot_name(requested_slot or reusable_slot or make_slot_name(src.name))
    dest = _slot_dir(slot)
    if dest.exists():
        shutil.rmtree(dest)
    dest.mkdir(parents=True, exist_ok=True)

    if src.is_dir():
        _copy_directory_payload(src, dest)
    else:
        shutil.copy2(src, dest / src.name)
        _copy_sibling_pair(src, dest)
        if src.suffix.lower() == ".designspace":
            _copy_designspace_sources(src, dest)

    if source_kind == "glyphs":
        converted = _convert_glyphs_source(src, dest)
        if converted:
            source_kind = "ufo/designspace"

    manifest = {
        "source_kind": source_kind,
    }
    if linked:
        origin_root = _origin_root_for_source(src)
        manifest.update({
            "origin_mode": "linked",
            "origin_kind": source_kind,
            "origin_root": str(origin_root),
            "origin_source": str(src),
            "origin_snapshot_mtime_ns": str(_linked_source_snapshot_mtime_ns(src)),
        })
    _write_manifest(dest, manifest)

    return slot


def fork_slot(slot: str, fork_name: str) -> str:
    ensure_workspace()
    src = resolve_slot(slot)
    fork_name = _clean_slot_name(fork_name)

    dest = _slot_dir(fork_name)
    if dest.exists():
        shutil.rmtree(dest)
    shutil.copytree(src, dest)
    return fork_name


def resolve_slot(slot: str) -> Path:
    ensure_workspace()
    path = _slot_dir(slot)
    if not path.exists():
        raise FileNotFoundError(f"Workspace slot not found: {slot!r}")
    return path


def source_path(slot: str) -> Path:
    slot_info = slot_from_name(slot)
    if slot_info is None or slot_info.source_path is None:
        raise FileNotFoundError(f"No editable source found in slot: {slot!r}")
    return slot_info.source_path


def compiled_path(slot: str) -> Path:
    slot_info = slot_from_name(slot)
    if slot_info is None:
        raise FileNotFoundError(f"Workspace slot not found: {slot!r}")
    if slot_info.compiled_path is not None:
        return slot_info.compiled_path
    if slot_info.source_path is not None and slot_info.source_path.suffix.lower() not in DEFAULT_SOURCE_EXTS:
        return slot_info.source_path
    raise FileNotFoundError(f"No compiled font found in slot: {slot!r}")


def compile_slot(slot: str, output_path: str | None = None, force: bool = False) -> Path:
    slot_dir = resolve_slot(slot).resolve()
    slot_info = slot_from_name(slot)
    if slot_info is None:
        raise FileNotFoundError(f"Workspace slot not found: {slot!r}")
    if slot_info.compiled_path is not None and not force:
        return slot_info.compiled_path

    source_kind = slot_info.source_kind or _infer_source_kind(slot_info.source_path) or "ufo/designspace"
    source = export_glyphspackage(
        slot_dir,
        slot_info,
        force=force or slot_info.compiled_path is None,
    )
    fontc_source = _fontc_source_from_package(source)

    fontc = shutil.which("fontc")
    if fontc is None:
        raise FileNotFoundError(
            "fontc executable not found on PATH; install Google Fonts fontc to compile glyphspackage sources"
        )

    target = Path(output_path).expanduser() if output_path else slot_dir / f"{slot}.ttf"
    if not target.is_absolute():
        target = (slot_dir / target).resolve()
    else:
        target = target.resolve()
    if slot_dir not in target.parents and target != slot_dir:
        raise ValueError("output_path must stay inside the workspace slot")
    target.parent.mkdir(parents=True, exist_ok=True)

    cmd = [
        fontc,
        str(fontc_source),
        "--flatten-components",
        "--decompose-transformed-components",
        "--output-file",
        str(target),
    ]
    try:
        subprocess.run(
            cmd,
            check=True,
            cwd=str(slot_dir),
            text=True,
            capture_output=True,
        )
    except subprocess.CalledProcessError as exc:
        detail = "\n".join(part.strip() for part in (exc.stderr, exc.stdout) if part and part.strip())
        message = f"fontc failed for workspace slot {slot!r}"
        if detail:
            message = f"{message}:\n{detail}"
        raise RuntimeError(message) from None
    manifest = _read_manifest(slot_dir)
    manifest.update({
        "source_kind": source_kind,
        "compiled_path": str(target.relative_to(slot_dir)),
        "compile_backend": "fontc",
        "package_path": str(source.relative_to(slot_dir)),
    })
    _write_manifest(slot_dir, manifest)
    return target


def _fontc_source_from_package(package_dir: Path) -> Path:
    sources_dir = package_dir / "sources"
    for suffix in (".designspace", ".ufo", ".glyphs"):
        for path in sorted(sources_dir.iterdir()):
            if path.suffix.lower() == suffix:
                return path
    raise FileNotFoundError(f"No fontc-compatible source found in {sources_dir}")


def export_glyphspackage(slot_dir: Path, slot_info: WorkspaceSlot, force: bool = False) -> Path:
    package_dir = _generated_glyphspackage_dir(slot_dir, slot_info)
    if package_dir.exists() and not force:
        return package_dir
    if package_dir.exists():
        shutil.rmtree(package_dir)
    package_dir.mkdir(parents=True, exist_ok=True)

    sources_dir = package_dir / "sources"
    sources_dir.mkdir(parents=True, exist_ok=True)

    source_entries = _collect_source_entries(slot_dir, slot_info)
    if not source_entries:
        raise FileNotFoundError(f"No source files found in slot: {slot_info.name!r}")

    copied_sources: list[str] = []
    source_base_dir = slot_dir
    if slot_info.source_kind == "glyphspackage" and slot_info.source_path is not None:
        source_base_dir = _glyphspackage_source_root(slot_info.source_path)
    for entry in source_entries:
        copied = _copy_source_entry(entry, sources_dir, base_dir=source_base_dir)
        copied_sources.append(str(copied.relative_to(package_dir)))

    config = _glyphspackage_config(slot_info, copied_sources)
    (sources_dir / "config.yaml").write_text(config, encoding="utf-8")
    _write_manifest(package_dir, {
        "source_kind": slot_info.source_kind or _infer_source_kind(slot_info.source_path) or "ufo/designspace",
        "source_slot": slot_info.name,
        "sources_root": "sources",
    })
    return package_dir


def _generated_glyphspackage_dir(slot_dir: Path, slot_info: WorkspaceSlot) -> Path:
    package_dir = slot_dir / f"{slot_info.name}.glyphspackage"
    source_path = slot_info.source_path.resolve() if slot_info.source_path else None
    if source_path is not None and (
        package_dir.resolve() == source_path
        or (
            package_dir.parent.resolve() == source_path.parent.resolve()
            and package_dir.name.lower() == source_path.name.lower()
        )
    ):
        return slot_dir / f"{slot_info.name}.fontc.glyphspackage"
    return package_dir


def export_slot_text_files(slot: str) -> list[dict[str, str]]:
    slot_dir = resolve_slot(slot)
    manifest = _read_manifest(slot_dir)
    generated_package = manifest.get("package_path")
    generated_package_path = (slot_dir / generated_package).resolve() if generated_package else None
    files: list[dict[str, str]] = []
    for path in sorted(slot_dir.rglob("*")):
        if not path.is_file():
            continue
        if generated_package_path is not None:
            try:
                path.resolve().relative_to(generated_package_path)
                continue
            except ValueError:
                pass
        if path.suffix.lower() not in TEXT_EXTS:
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            continue
        files.append({
            "path": str(path.relative_to(slot_dir)),
            "text": text,
        })
    return files


def write_workspace_text_file(rel_path: str, text: str) -> Path:
    return write_workspace_text_file_with_result(rel_path, text).workspace_path


def write_workspace_text_file_with_result(rel_path: str, text: str) -> WorkspaceWriteResult:
    ensure_workspace()
    cleaned = Path(rel_path)
    if cleaned.is_absolute():
        raise ValueError("workspace path must be relative")
    if ".." in cleaned.parts:
        raise ValueError("workspace path cannot contain parent traversal")
    target = _workspace_write_target(cleaned)
    root = WORKSPACE_DIR.resolve()
    fonts_root = FONTS_DIR.resolve()
    if (
        root not in target.parents
        and target != root
        and fonts_root not in target.parents
        and target != fonts_root
    ):
        raise ValueError("workspace path escapes workspace root")
    target.parent.mkdir(parents=True, exist_ok=True)
    text = _normalize_text_for_existing_file(target, text)
    target.write_text(text, encoding="utf-8")
    source_path = _mirror_linked_source_write(target, text)
    _invalidate_compiled_slot_for_path(target)
    return WorkspaceWriteResult(workspace_path=target, source_path=source_path)


def export_slot_to_directory(slot: str, destination: str, relink: bool = False) -> WorkspaceExportResult:
    ensure_workspace()
    slot_dir = resolve_slot(slot).resolve()
    slot_info = slot_from_name(slot)
    if slot_info is None:
        raise FileNotFoundError(f"Workspace slot not found: {slot!r}")

    dest = Path(destination).expanduser()
    if dest.exists() and not dest.is_dir():
        raise ValueError("destination must be a folder")
    dest = dest.resolve()
    dest.mkdir(parents=True, exist_ok=True)

    if dest == slot_dir or slot_dir in dest.parents:
        raise ValueError("destination must be outside the workspace slot")

    source_entries = _collect_source_entries(slot_dir, slot_info)
    if not source_entries:
        raise FileNotFoundError(f"No source files found in slot: {slot!r}")

    copied: list[Path] = []
    for entry in source_entries:
        copied.append(_copy_source_entry_overwriting(entry, dest))

    origin_source = _preferred_origin_source(copied)
    origin_root: Path | None = None
    if relink:
        source_kind = slot_info.source_kind or _infer_source_kind(origin_source) or "ufo/designspace"
        if source_kind != "ufo/designspace":
            raise ValueError("relink after Save As is currently supported only for .designspace and .ufo sources")
        if origin_source is None:
            raise FileNotFoundError("No relinkable designspace or UFO source was exported")
        origin_root = _origin_root_for_source(origin_source)
        manifest = _read_manifest(slot_dir)
        manifest.update({
            "source_kind": source_kind,
            "origin_mode": "linked",
            "origin_kind": source_kind,
            "origin_root": str(origin_root),
            "origin_source": str(origin_source),
        })
        _write_manifest(slot_dir, manifest)

    return WorkspaceExportResult(
        destination=dest,
        copied_paths=tuple(copied),
        linked=relink,
        origin_root=origin_root,
        origin_source=origin_source if relink else None,
    )


def invalidate_workspace_path(rel_path: str) -> None:
    ensure_workspace()
    cleaned = Path(rel_path)
    if cleaned.is_absolute():
        raise ValueError("workspace path must be relative")
    if ".." in cleaned.parts:
        raise ValueError("workspace path cannot contain parent traversal")
    target = _workspace_write_target(cleaned)
    root = WORKSPACE_DIR.resolve()
    fonts_root = FONTS_DIR.resolve()
    if (
        root not in target.parents
        and target != root
        and fonts_root not in target.parents
        and target != fonts_root
    ):
        raise ValueError("workspace path escapes workspace root")
    _invalidate_compiled_slot_for_path(target)


def _workspace_write_target(rel_path: Path) -> Path:
    parts = rel_path.parts
    if not parts:
        raise ValueError("workspace path is required")
    if parts[0] == "fonts":
        return (WORKSPACE_DIR / rel_path).resolve()
    if (FONTS_DIR / parts[0]).exists():
        return (FONTS_DIR / rel_path).resolve()
    return (WORKSPACE_DIR / rel_path).resolve()


def _invalidate_compiled_slot_for_path(target: Path) -> None:
    fonts_root = FONTS_DIR.resolve()
    try:
        rel = target.resolve().relative_to(fonts_root)
    except ValueError:
        return
    if not rel.parts:
        return

    slot_dir = fonts_root / rel.parts[0]
    manifest = _read_manifest(slot_dir)
    if not manifest:
        return

    compiled = manifest.pop("compiled_path", None)
    manifest.pop("compile_backend", None)
    package = manifest.pop("package_path", None)
    if compiled:
        compiled_path = slot_dir / compiled
        if compiled_path.exists():
            compiled_path.unlink()
    if package:
        package_path = slot_dir / package
        if package_path.exists() and package_path.is_dir():
            shutil.rmtree(package_path)
    _write_manifest(slot_dir, manifest)


def _origin_root_for_source(src: Path) -> Path:
    if src.suffix.lower() in {".designspace", ".ufo"}:
        return src.parent.resolve()
    return src.resolve().parent


def _linked_source_snapshot_mtime_ns(src: Path) -> int:
    src = src.expanduser().resolve()
    if not src.exists():
        return 0
    snapshot = _tree_mtime_ns(src)
    if src.suffix.lower() != ".designspace":
        return snapshot

    try:
        root = ET.parse(src).getroot()
    except ET.ParseError:
        return snapshot
    for source in root.findall(".//source"):
        filename = (
            source.get("filename")
            or source.get("path")
            or source.findtext("filename")
            or ""
        ).strip()
        if not filename:
            continue
        source_path = (src.parent / Path(filename)).resolve()
        if source_path.exists():
            snapshot = max(snapshot, _tree_mtime_ns(source_path))
    return snapshot


def _tree_mtime_ns(path: Path) -> int:
    if path.is_file():
        return path.stat().st_mtime_ns
    newest = path.stat().st_mtime_ns
    for entry in path.rglob("*"):
        try:
            newest = max(newest, entry.stat().st_mtime_ns)
        except OSError:
            continue
    return newest


def _mirror_linked_source_write(target: Path, text: str) -> Path | None:
    fonts_root = FONTS_DIR.resolve()
    try:
        rel = target.resolve().relative_to(fonts_root)
    except ValueError:
        return None
    if len(rel.parts) < 2:
        return None

    slot_dir = fonts_root / rel.parts[0]
    manifest = _read_manifest(slot_dir)
    if manifest.get("origin_mode") != "linked":
        return None
    if manifest.get("origin_kind") != "ufo/designspace":
        return None

    origin_root_raw = manifest.get("origin_root", "")
    if not origin_root_raw:
        return None
    origin_root = Path(origin_root_raw).expanduser().resolve()
    if not origin_root.exists() or not origin_root.is_dir():
        raise FileNotFoundError(f"Linked source root not found: {origin_root}")

    source_rel = Path(*rel.parts[1:])
    if source_rel.is_absolute() or ".." in source_rel.parts:
        raise ValueError("linked source path cannot escape origin root")
    source_target = (origin_root / source_rel).resolve()
    if origin_root not in source_target.parents and source_target != origin_root:
        raise ValueError("linked source path escapes origin root")
    if source_target.suffix.lower() not in TEXT_EXTS:
        raise ValueError(f"refusing to mirror unsupported linked source file: {source_target.name}")

    source_target.parent.mkdir(parents=True, exist_ok=True)
    text = _normalize_text_for_existing_file(source_target, text)
    source_target.write_text(text, encoding="utf-8")
    origin_source_raw = manifest.get("origin_source", "")
    if origin_source_raw:
        origin_source = Path(origin_source_raw).expanduser().resolve()
        if origin_source.exists():
            manifest["origin_snapshot_mtime_ns"] = str(_linked_source_snapshot_mtime_ns(origin_source))
            _write_manifest(slot_dir, manifest)
    return source_target


def _normalize_text_for_existing_file(path: Path, text: str) -> str:
    normalized = text.replace("\r\n", "\n").replace("\r", "\n")
    if not path.exists() or not path.is_file():
        return normalized
    try:
        current = path.read_bytes()
    except OSError:
        return normalized
    if not current:
        return normalized
    crlf_count = current.count(b"\r\n")
    lf_count = current.count(b"\n")
    lf_only_count = lf_count - crlf_count
    if crlf_count > 0 and crlf_count >= lf_only_count:
        return normalized.replace("\n", "\r\n")
    return normalized


def _find_entry(slot_dir: Path, exts: tuple[str, ...]) -> Path | None:
    for ext in exts:
        for entry in sorted(slot_dir.iterdir()):
            if entry.suffix.lower() == ext and (entry.is_file() or entry.is_dir()):
                return entry
    return None


def _clean_source_kind(source_kind: str, source_path: Path) -> str:
    kind = source_kind.strip().lower()
    if kind not in SOURCE_EXTS_BY_KIND:
        raise ValueError(f"unsupported source kind: {source_kind!r}")
    suffix = source_path.suffix.lower()
    if kind == "glyphs" and suffix != ".glyphs":
        raise ValueError("Glyphs source kind requires a .glyphs source")
    if kind == "glyphspackage" and suffix != ".glyphspackage":
        raise ValueError("glyphspackage source kind requires a .glyphspackage source")
    if kind == "ufo/designspace" and suffix not in {".ufo", ".designspace"}:
        raise ValueError("ufo/designspace source kind requires a .ufo or .designspace source")
    return kind


def _read_manifest(slot_dir: Path) -> dict[str, str]:
    manifest = slot_dir / MANIFEST_NAME
    if not manifest.exists():
        return {}
    try:
        data = json.loads(manifest.read_text(encoding="utf-8"))
    except Exception:
        return {}
    if not isinstance(data, dict):
        return {}
    return {str(k): str(v) for k, v in data.items()}


def _write_manifest(slot_dir: Path, data: dict[str, str]) -> None:
    manifest = slot_dir / MANIFEST_NAME
    manifest.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def _copy_directory_payload(src: Path, dest: Path) -> None:
    if src.suffix.lower() == ".ufo":
        shutil.copytree(src, dest / src.name)
        _copy_sibling_pair(src, dest)
        return
    if src.suffix.lower() == ".designspace":
        shutil.copy2(src, dest / src.name)
        _copy_sibling_pair(src, dest)
        return
    shutil.copytree(src, dest / src.name)


def _copy_sibling_pair(src: Path, dest: Path) -> None:
    stem = src.with_suffix("")
    for ext in DEFAULT_SOURCE_EXTS + COMPILED_EXTS:
        sibling = stem.with_suffix(ext)
        if sibling.is_dir():
            target = dest / sibling.name
            if target.exists():
                shutil.rmtree(target)
            shutil.copytree(sibling, target)
        elif sibling.exists():
            shutil.copy2(sibling, dest / sibling.name)


def _copy_designspace_sources(src: Path, dest: Path) -> None:
    for source_path in _designspace_source_paths(src):
        filename = source_path.relative_to(src.parent)
        target = dest / filename
        target.parent.mkdir(parents=True, exist_ok=True)
        if source_path.is_dir():
            if target.exists():
                shutil.rmtree(target)
            shutil.copytree(source_path, target)
        else:
            shutil.copy2(source_path, target)


def _designspace_source_paths(src: Path) -> list[Path]:
    try:
        root = ET.parse(src).getroot()
    except ET.ParseError:
        return []

    paths: list[Path] = []
    for source in root.findall(".//source"):
        filename = (
            source.get("filename")
            or source.get("path")
            or source.findtext("filename")
            or ""
        ).strip()
        if not filename:
            continue
        source_path = (src.parent / Path(filename)).resolve()
        if not source_path.exists():
            continue
        paths.append(source_path)
    return paths


def _infer_source_kind(source: Path | None) -> str | None:
    if source is None:
        return None
    suffix = source.suffix.lower()
    if suffix in {".ufo", ".designspace"}:
        return "ufo/designspace"
    if suffix == ".glyphs":
        return "glyphs"
    if suffix == ".glyphspackage":
        return "glyphspackage"
    return None


def locate_source_root(root: Path) -> Path | None:
    """Best-effort discovery of the actual font source inside an upload tree.

    The browser-side import flow uploads a selected folder into a
    temporary staging tree. A project root may contain the real source
    entry one level down, so we search for an importable root in the
    order type designers usually expect: designspace first, then
    Glyphs, glyphs packages, and finally standalone UFOs.
    """

    root = root.resolve()
    if root.is_file():
        return root if _infer_source_kind(root) is not None else None

    if _infer_source_kind(root) is not None:
        return root

    def candidates(suffixes: set[str], want_dir: bool) -> list[Path]:
        found: list[Path] = []
        for entry in sorted(root.rglob("*")):
            if want_dir and entry.is_dir() and entry.suffix.lower() in suffixes:
                found.append(entry)
            elif not want_dir and entry.is_file() and entry.suffix.lower() in suffixes:
                found.append(entry)
        return found

    for suffixes, want_dir in (
        ({".designspace"}, False),
        ({".glyphs"}, False),
        ({".glyphspackage"}, True),
        ({".ufo"}, True),
    ):
        found = candidates(suffixes, want_dir)
        if found:
            return found[0]
    return None


def _convert_glyphs_source(src: Path, dest: Path) -> bool:
    try:
        import glyphsLib  # type: ignore[import-not-found]
    except Exception:
        return False

    build_dir = Path(tempfile.mkdtemp(prefix="glyphs-import-", dir=str(dest)))
    try:
        result = glyphsLib.build_masters(str(src), str(build_dir))
        designspace_path = None
        if isinstance(result, tuple) and len(result) >= 2:
            designspace_path = Path(result[1]) if result[1] else None
        if designspace_path is None or not designspace_path.exists():
            return False

        for path in build_dir.rglob("*"):
            if not path.is_file():
                continue
            rel = path.relative_to(build_dir)
            target = dest / rel
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(path, target)
        return True
    except Exception:
        return False
    finally:
        shutil.rmtree(build_dir, ignore_errors=True)


def _copy_tree_contents(src: Path, dest: Path) -> None:
    for entry in src.iterdir():
        target = dest / entry.name
        if entry.is_dir():
            shutil.copytree(entry, target)
        else:
            shutil.copy2(entry, target)


def _collect_source_entries(slot_dir: Path, slot_info: WorkspaceSlot) -> list[Path]:
    entries: list[Path] = []
    seen: set[Path] = set()

    def add(path: Path) -> None:
        resolved = path.resolve()
        if resolved in seen:
            return
        seen.add(resolved)
        entries.append(path)

    if slot_info.source_kind == "glyphspackage" and slot_info.source_path is not None:
        package_sources = _glyphspackage_source_root(slot_info.source_path)
        if package_sources.exists():
            for path in sorted(package_sources.iterdir()):
                if path.name == MANIFEST_NAME:
                    continue
                if path.is_dir() and path.suffix.lower() in {".ufo", ".glyphspackage"}:
                    add(path)
                elif path.is_file() and path.suffix.lower() in {".designspace", ".glyphs", ".plist", ".fea", ".txt", ".md", ".json", ".yaml", ".yml"}:
                    add(path)
        return entries

    for path in sorted(slot_dir.iterdir()):
        if path.name.endswith(".glyphspackage"):
            continue
        if path.is_dir() and path.suffix.lower() == ".ufo":
            add(path)
        elif path.is_file() and path.suffix.lower() in {".designspace", ".glyphs", ".plist", ".fea", ".txt", ".md", ".json", ".yaml", ".yml"}:
            add(path)
            if path.suffix.lower() == ".designspace":
                for source_path in _designspace_source_paths(path):
                    add(source_path)

    if not entries and slot_info.source_path is not None:
        add(slot_info.source_path)
    return entries


def _glyphspackage_source_root(source_path: Path) -> Path:
    sources_dir = source_path / "sources"
    return sources_dir if sources_dir.is_dir() else source_path


def _copy_source_entry(src: Path, sources_dir: Path, base_dir: Path | None = None) -> Path:
    if base_dir is not None:
        try:
            rel = src.resolve().relative_to(base_dir.resolve())
        except ValueError:
            rel = Path(src.name)
    else:
        rel = Path(src.name)
    target = sources_dir / rel
    target.parent.mkdir(parents=True, exist_ok=True)
    if src.is_dir():
        shutil.copytree(src, target)
    else:
        shutil.copy2(src, target)
    return target


def _copy_source_entry_overwriting(src: Path, destination: Path) -> Path:
    target = destination / src.name
    if src.is_dir():
        if target.exists():
            if not target.is_dir():
                target.unlink()
            else:
                shutil.rmtree(target)
        shutil.copytree(src, target)
    else:
        target.parent.mkdir(parents=True, exist_ok=True)
        if target.exists() and target.is_dir():
            shutil.rmtree(target)
        shutil.copy2(src, target)
    return target


def _preferred_origin_source(paths: list[Path]) -> Path | None:
    for suffix in (".designspace", ".ufo"):
        for path in paths:
            if path.suffix.lower() == suffix:
                return path
    return None


def _glyphspackage_config(slot_info: WorkspaceSlot, copied_sources: list[str]) -> str:
    lines = [
        f"name: {slot_info.name}",
        f"source_kind: {slot_info.source_kind or 'ufo/designspace'}",
        "sources:",
    ]
    for rel in copied_sources:
        lines.append(f"  - {rel}")
    lines.append("")
    return "\n".join(lines)
