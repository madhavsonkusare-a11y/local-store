"""Build a complete, pinned, validated catalog icon set with local monogram fallbacks."""
import argparse
from collections import Counter, defaultdict
from concurrent.futures import ThreadPoolExecutor
import hashlib
import html
import io
import json
from pathlib import Path
import re
import unicodedata
import urllib.parse
import urllib.request
import xml.etree.ElementTree as ET

from fontTools.pens.boundsPen import BoundsPen
from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.ttLib import TTFont

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "catalog/icons.json"
AUDIT = ROOT / "catalog/icon-audit.json"
SOURCE_LOCK = ROOT / "catalog/icon-sources.lock.json"
# Artwork from an app's own repository, for apps no icon source covers.
OVERRIDES = ROOT / "catalog/icon-overrides.json"
CATALOG = ROOT / "src/generated/catalog.json"
ASSET_DIR = ROOT / "src/assets/catalog"
FONT_PATH = ROOT / "docs/design/v2/assets/fonts/InstrumentSans-SemiBold.woff2"
MAX_BYTES = 512 * 1024
MAX_DIMENSION = 4096
ICON_PATH = r"assets/catalog/[a-z0-9-]+\.(svg|png)"
ALLOWED_HOSTS = ("api.github.com", "raw.githubusercontent.com")
ALLOWED_SVG_MIME = ("image/svg+xml", "text/plain", "application/octet-stream")
ALLOWED_PNG_MIME = ("image/png", "application/octet-stream")
ACTIVE_ELEMENTS = {"script", "foreignObject", "image", "iframe", "animate", "animateTransform", "set"}
GRAPHIC_ELEMENTS = {"path", "rect", "circle", "ellipse", "polygon", "polyline", "line", "text", "use"}
GENERATOR_VERSION = "monogram-v1"
UNREVIEWED_LICENSES = {None, "", "NONE", "NOASSERTION", "UNLICENSED"}


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        raise ValueError("Icon redirects are not allowed")


def fetch(url, limit, accepted_mime=None):
    parsed = urllib.parse.urlsplit(url)
    if parsed.scheme != "https" or parsed.hostname not in ALLOWED_HOSTS:
        raise ValueError("Unapproved icon host")
    request = urllib.request.Request(url, headers={"User-Agent": "Local-Store-Icons/2"})
    with urllib.request.build_opener(NoRedirect()).open(request, timeout=20) as response:
        content_type = response.headers.get_content_type().lower()
        if accepted_mime and content_type not in accepted_mime:
            raise ValueError(f"Unexpected MIME type: {content_type}")
        data = response.read(limit + 1)
    if len(data) > limit:
        raise ValueError("Icon exceeds size limit")
    return data


def fetch_json(url, limit):
    return json.loads(fetch(url, limit, ("application/json",)))


def numeric_dimension(value):
    if value is None:
        return None
    match = re.fullmatch(r"\s*([0-9]+(?:\.[0-9]+)?)(?:px)?\s*", value)
    return float(match.group(1)) if match else None


def validate_svg(data):
    if len(data) > MAX_BYTES or re.search(br"<!DOCTYPE|<!ENTITY", data, re.I):
        raise ValueError("Unsafe SVG declaration or size")
    root = ET.fromstring(data)
    if root.tag.split("}")[-1] != "svg":
        raise ValueError("Expected SVG root")
    visible = False
    for element in root.iter():
        tag = element.tag.split("}")[-1]
        if tag in ACTIVE_ELEMENTS:
            raise ValueError("SVG contains active or external content")
        visible = visible or tag in GRAPHIC_ELEMENTS
        for key, value in element.attrib.items():
            name = key.split("}")[-1].lower()
            if name.startswith("on") or (name in ("href", "src") and not value.startswith("#")):
                raise ValueError("SVG contains an event or external reference")
            if re.search(r"url\(\s*[\"']?(?!#)", value, re.I) or "javascript:" in value.lower():
                raise ValueError("SVG contains external styling")
        if tag == "style" and element.text:
            if "@import" in element.text.lower() or re.search(r"url\(\s*[\"']?(?!#)", element.text, re.I):
                raise ValueError("SVG style references external content")
    if not visible:
        raise ValueError("SVG has no visible graphic element")
    view_box = root.attrib.get("viewBox", "").replace(",", " ").split()
    if len(view_box) == 4:
        try:
            width, height = float(view_box[2]), float(view_box[3])
        except ValueError as error:
            raise ValueError("Invalid SVG viewBox") from error
    else:
        width, height = numeric_dimension(root.attrib.get("width")), numeric_dimension(root.attrib.get("height"))
    if width is None or height is None or not (0 < width <= MAX_DIMENSION and 0 < height <= MAX_DIMENSION):
        raise ValueError("Missing or invalid SVG dimensions")
    return {"width": width, "height": height, "mime": "image/svg+xml"}


def validate_icon(data, suffix):
    if suffix == ".svg":
        return validate_svg(data)
    if suffix != ".png" or len(data) > MAX_BYTES or not data.startswith(b"\x89PNG\r\n\x1a\n"):
        raise ValueError("Invalid PNG signature, format or size")
    from PIL import Image
    with Image.open(io.BytesIO(data)) as image:
        if image.format != "PNG" or not (0 < image.width <= 2048 and 0 < image.height <= 2048):
            raise ValueError("Invalid PNG dimensions")
        if getattr(image, "is_animated", False):
            raise ValueError("Animated icons are not allowed")
        dimensions = {"width": image.width, "height": image.height, "mime": "image/png"}
        image.verify()
    with Image.open(io.BytesIO(data)) as image:
        image.load()
    return dimensions


def validate_catalog_suitability(metadata):
    """Reject artwork that cannot read as a compact, uncropped app icon."""
    ratio = metadata["width"] / metadata["height"]
    if ratio < 0.2 or ratio > 5:
        raise ValueError("Icon aspect ratio is unsuitable for the catalog grid")


def source_tree(source):
    tree = fetch_json(
        f"https://api.github.com/repos/{source['repository']}/git/trees/{source['revision']}?recursive=1",
        12 * 1024 * 1024,
    )
    if tree.get("truncated"):
        raise ValueError(f"Incomplete icon inventory: {source['repository']}")
    return {item["path"] for item in tree["tree"] if item.get("type") == "blob"}


def homarr_candidates(project_id, paths, aliases):
    preferred = aliases.get(project_id)
    if preferred and "/" in preferred:
        return [(preferred, "reviewed-path")] if preferred in paths else []
    keys = [(preferred, "reviewed-alias"), (project_id, "exact-id"), (project_id.replace("-", ""), "normalized-id")]
    output = []
    for key, match in keys:
        if not key:
            continue
        for variant in (key + "-light", key):
            for suffix in ("svg", "png"):
                path = f"{suffix}/{variant}.{suffix}"
                if path in paths and path not in {row[0] for row in output}:
                    output.append((path, match))
    return output


def umbrel_candidates(project_id, paths, aliases):
    keys = [(project_id, "exact-id")]
    if aliases.get(project_id):
        keys.insert(0, (aliases[project_id], "reviewed-alias"))
    output = []
    for key, match in keys:
        for suffix in ("svg", "png"):
            path = f"{key}/icon.{suffix}"
            if path in paths and path not in {row[0] for row in output}:
                output.append((path, match))
    return output


def coolify_icons(source):
    lock = json.loads((ROOT / "catalog/sources.lock.json").read_text())
    listing = next(item for item in lock["sources"] if item["id"] == "coolify")
    if listing["revision"] != source["revision"] or listing["repository"] != source["repository"]:
        raise ValueError("Icon and catalog Coolify pins differ")
    data = (ROOT / "catalog/sources/coolify.json").read_bytes()
    if hashlib.sha256(data).hexdigest() != listing["sha256"]:
        raise ValueError("Coolify source checksum mismatch")
    snapshot = json.loads(data)
    prefix = f"https://raw.githubusercontent.com/{source['repository']}/{source['revision']}/"
    return {
        row["upstream_id"]: [prefix + "public/" + row["icon"][len(prefix):], row["icon"]]
        for row in snapshot["records"]
        if row.get("icon")
        and row["icon"].startswith(prefix)
        and re.fullmatch(r"svgs/[a-zA-Z0-9_-]+\.(svg|png)", row["icon"][len(prefix):])
    }


def glyph_library():
    font = TTFont(str(FONT_PATH))
    glyph_set = font.getGlyphSet()
    cmap = font.getBestCmap()
    metrics = font["hmtx"].metrics
    output = {}
    for character in "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789?":
        glyph_name = cmap.get(ord(character))
        if not glyph_name:
            continue
        path_pen = SVGPathPen(glyph_set)
        bounds_pen = BoundsPen(glyph_set)
        glyph_set[glyph_name].draw(path_pen)
        glyph_set[glyph_name].draw(bounds_pen)
        output[character] = {"path": path_pen.getCommands(), "bounds": bounds_pen.bounds, "advance": metrics[glyph_name][0]}
    font.close()
    return output


GLYPHS = glyph_library()
PALETTE = (
    ("#34302e", "#1b1817"), ("#342b36", "#1d171f"), ("#293638", "#151d1f"),
    ("#3a3027", "#201a15"), ("#293532", "#151c1a"), ("#2d3040", "#181a24"),
    ("#3b2a2c", "#211719"), ("#353229", "#1d1b16"),
)


def monogram_initials(name):
    plain = unicodedata.normalize("NFKD", name).encode("ascii", "ignore").decode("ascii")
    words = re.findall(r"[A-Za-z0-9]+", plain.upper())
    if not words:
        return "?"
    return words[0][0] + words[1][0] if len(words) > 1 else words[0][:2]


def monogram_svg(entry):
    initials = monogram_initials(entry["name"])
    glyphs = [GLYPHS.get(character, GLYPHS["?"]) for character in initials]
    positions, cursor = [], 0
    for glyph in glyphs:
        positions.append(cursor)
        cursor += glyph["advance"] - 35
    min_x = min(position + glyph["bounds"][0] for position, glyph in zip(positions, glyphs))
    max_x = max(position + glyph["bounds"][2] for position, glyph in zip(positions, glyphs))
    min_y = min(glyph["bounds"][1] for glyph in glyphs)
    max_y = max(glyph["bounds"][3] for glyph in glyphs)
    width, height = max_x - min_x, max_y - min_y
    scale = min((35 if len(glyphs) == 1 else 39) / width, 23 / height)
    origin_x = (64 - width * scale) / 2 - min_x * scale
    origin_y = (64 - height * scale) / 2 + max_y * scale
    paths = "".join(
        f'<path d="{glyph["path"]}" transform="translate({origin_x + position * scale:.3f} {origin_y:.3f}) scale({scale:.5f} {-scale:.5f})" fill="#FFF7EF"/>'
        for position, glyph in zip(positions, glyphs)
    )
    digest = hashlib.sha256(entry["id"].encode()).digest()
    top_color, bottom_color = PALETTE[digest[0] % len(PALETTE)]
    title = html.escape(f'{entry["name"]} fallback icon')
    identifier = html.escape(entry["id"])
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64" role="img" data-catalog-id="{identifier}">'
        f'<title>{title}</title><defs><linearGradient id="g" x1="8" y1="5" x2="56" y2="59" gradientUnits="userSpaceOnUse">'
        f'<stop stop-color="{top_color}"/><stop offset="1" stop-color="{bottom_color}"/></linearGradient>'
        '<linearGradient id="r" x1="32" y1="4" x2="32" y2="60" gradientUnits="userSpaceOnUse">'
        '<stop stop-color="#FFF7EF" stop-opacity=".18"/><stop offset="1" stop-color="#FFF7EF" stop-opacity=".04"/>'
        '</linearGradient></defs><rect x="2" y="2" width="60" height="60" rx="15" fill="url(#g)"/>'
        '<rect x="2.5" y="2.5" width="59" height="59" rx="14.5" fill="none" stroke="url(#r)"/>'
        f'{paths}<rect x="48" y="10" width="6" height="6" rx="2" fill="#F26419"/></svg>\n'
    ).encode()


def provenance_record(path, data, metadata, source_key, source, url, match, fallback_reason=None):
    record = {
        "bytes": len(data), "format": Path(path).suffix[1:], "height": metadata["height"],
        "license": source["license"], "match": match, "mime": metadata["mime"],
        "notice": source["notice"], "path": path, "repository": source["repository"],
        "revision": source["revision"], "sha256": hashlib.sha256(data).hexdigest(),
        "source": source_key, "url": url, "width": metadata["width"],
    }
    if fallback_reason:
        record["fallback_reason"] = fallback_reason
        record["generator"] = GENERATOR_VERSION
    return record


def audit_manifest(manifest, projects):
    expected = {entry["id"] for entry in projects}
    actual = set(manifest["icons"])
    source_counts = Counter(record["source"] for record in manifest["icons"].values())
    format_counts = Counter(record["format"] for record in manifest["icons"].values())
    hashes = defaultdict(list)
    suspicious = []
    reviewed_matches = []
    for project_id, record in manifest["icons"].items():
        if record["source"] != "local-store-monogram":
            hashes[record["sha256"]].append(project_id)
        ratio = record["width"] / record["height"]
        if ratio < 0.2 or ratio > 5 or record["bytes"] < 100:
            suspicious.append({"id": project_id, "reason": "extreme dimensions or unusually small file"})
        if record["match"].startswith("reviewed"):
            reviewed_matches.append({"id": project_id, "match": record["match"], "source": record["source"], "url": record["url"]})
    duplicate_groups = [{"sha256": digest, "ids": sorted(ids)} for digest, ids in hashes.items() if len(ids) > 1]
    duplicate_groups.sort(key=lambda row: row["ids"])
    return {
        "schema_version": 1, "catalog_entries": len(expected), "resolved_icons": len(actual & expected),
        "coverage_percent": round(100 * len(actual & expected) / len(expected), 2),
        "missing": sorted(expected - actual), "extra": sorted(actual - expected),
        "source_counts": dict(sorted(source_counts.items())), "format_counts": dict(sorted(format_counts.items())),
        "duplicate_upstream_groups": duplicate_groups, "suspicious": suspicious,
        "reviewed_alias_matches": sorted(reviewed_matches, key=lambda row: row["id"]),
        "umbrel_matches": sorted(project_id for project_id, record in manifest["icons"].items() if record["source"] == "umbrel-apps-gallery"),
    }


def load_overrides(sources):
    """Per-app artwork choices, each tied to a pinned source in the lock."""
    if not OVERRIDES.exists():
        return {}
    overrides = {key: value for key, value in json.loads(OVERRIDES.read_text(encoding="utf-8")).items()
                 if not key.startswith("_")}
    for project_id, override in overrides.items():
        if override.get("source") not in sources:
            raise ValueError(f"Icon override for {project_id} names an unlocked source")
        if sources[override["source"]].get("license") in UNREVIEWED_LICENSES:
            raise ValueError(f"Icon override for {project_id} has no reviewed artwork license")
        if not re.fullmatch(r"[A-Za-z0-9._/-]+\.(svg|png)", override.get("path", "")) or ".." in override["path"]:
            raise ValueError(f"Icon override for {project_id} names an unusable path")
    return overrides


def validate_manifest(manifest, projects, sources):
    expected = {entry["id"] for entry in projects}
    if manifest.get("schema_version") != 2 or set(manifest.get("icons", {})) != expected:
        raise ValueError("Icon manifest does not provide exact catalog coverage")
    for project_id, record in manifest["icons"].items():
        if record.get("license") in UNREVIEWED_LICENSES:
            raise ValueError(f"Icon has no reviewed artwork license: {project_id}")
        path = record["path"]
        if not re.fullmatch(ICON_PATH, path):
            raise ValueError(f"Invalid cached icon path: {project_id}")
        data = (ROOT / "src" / path).read_bytes()
        metadata = validate_icon(data, Path(path).suffix)
        if hashlib.sha256(data).hexdigest() != record["sha256"]:
            raise ValueError(f"Icon checksum mismatch: {path}")
        for key in ("source", "repository", "revision", "url", "license", "notice", "match", "mime", "bytes", "width", "height"):
            if key not in record:
                raise ValueError(f"Missing icon provenance field {key}: {project_id}")
        if metadata["mime"] != record["mime"] or len(data) != record["bytes"]:
            raise ValueError(f"Icon metadata mismatch: {project_id}")
        if record["source"] == "local-store-monogram":
            if record.get("generator") != GENERATOR_VERSION or record["license"] != "MIT":
                raise ValueError(f"Invalid monogram provenance: {project_id}")
        elif record["source"] not in sources:
            raise ValueError(f"Unknown icon source: {project_id}")
        elif any(record[key] != sources[record["source"]][key] for key in ("repository", "revision", "license", "notice")):
            raise ValueError(f"Icon source lock mismatch: {project_id}")
    # An override is a decision somebody made; a manifest that quietly used
    # something else instead would read as honouring it.
    for project_id, override in load_overrides(sources).items():
        record = manifest["icons"].get(project_id)
        if project_id in expected and (not record or record["source"] != override["source"]
                                       or not record["url"].endswith("/" + override["path"])):
            raise ValueError(f"Icon override for {project_id} is not the icon in use")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--refresh", action="store_true")
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    if args.refresh == args.check:
        parser.error("Choose exactly one of --refresh or --check")
    projects = json.loads(CATALOG.read_text(encoding="utf-8"))["entries"]
    sources = json.loads(SOURCE_LOCK.read_text())["sources"]
    if args.check:
        manifest = json.loads(MANIFEST.read_text())
        validate_manifest(manifest, projects, sources)
        audit = audit_manifest(manifest, projects)
        expected_audit = json.dumps(audit, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
        if AUDIT.read_text(encoding="utf-8") != expected_audit:
            raise ValueError("Icon audit is stale")
        print(f"Verified {len(manifest['icons'])} local icons offline; catalog coverage is 100%.")
        return

    previous = json.loads(MANIFEST.read_text()) if MANIFEST.exists() else {"icons": {}}
    homarr, umbrel, coolify = (sources[key] for key in ("homarr-dashboard-icons", "umbrel-apps-gallery", "coolify"))
    homarr_paths = {path for path in source_tree(homarr) if re.fullmatch(r"(svg|png)/[^/]+\.(svg|png)", path)}
    umbrel_paths = ({path for path in source_tree(umbrel) if re.fullmatch(r"[a-z0-9-]+/icon\.(svg|png)", path)}
                    if umbrel.get("license") not in UNREVIEWED_LICENSES else set())
    homarr_aliases = json.loads((ROOT / "catalog/icon-aliases.json").read_text())
    umbrel_aliases = json.loads((ROOT / "catalog/umbrel-icon-aliases.json").read_text())
    supplemental = coolify_icons(coolify)
    local_source = {"repository": "madhavsonkusare-a11y/local-store", "revision": GENERATOR_VERSION, "license": "MIT", "notice": "LICENSE"}
    ASSET_DIR.mkdir(exist_ok=True)

    overrides = load_overrides(sources)

    def cache(entry):
        candidates = []
        override = overrides.get(entry["id"])
        if override:
            source = sources[override["source"]]
            candidates.append((override["source"], source,
                               f"https://raw.githubusercontent.com/{source['repository']}/{source['revision']}/{override['path']}",
                               "project-repository"))
        for path, match in homarr_candidates(entry["id"], homarr_paths, homarr_aliases):
            candidates.append(("homarr-dashboard-icons", homarr, f"https://raw.githubusercontent.com/{homarr['repository']}/{homarr['revision']}/{path}", match))
        for source in entry["sources"]:
            if source["source"] == "coolify" and source["revision"] == coolify["revision"] and source["upstream_id"] in supplemental:
                candidates.extend(("coolify", coolify, url, "source-catalog") for url in supplemental[source["upstream_id"]])
        for path, match in umbrel_candidates(entry["id"], umbrel_paths, umbrel_aliases):
            candidates.append(("umbrel-apps-gallery", umbrel, f"https://raw.githubusercontent.com/{umbrel['repository']}/{umbrel['revision']}/{path}", match))
        old = previous.get("icons", {}).get(entry["id"])
        if old:
            matching_old = [candidate for candidate in candidates if candidate[2] == old.get("url")]
            candidates = matching_old + [candidate for candidate in candidates if candidate not in matching_old]
        errors = []
        for source_key, source, url, match in candidates:
            if source.get("license") in UNREVIEWED_LICENSES:
                continue
            suffix = Path(urllib.parse.urlsplit(url).path).suffix.lower()
            path = f"assets/catalog/{entry['id']}{suffix}"
            output = ROOT / "src" / path
            try:
                data = None
                if old and old.get("url") == url and output.exists():
                    cached = output.read_bytes()
                    if hashlib.sha256(cached).hexdigest() == old.get("sha256"):
                        data = cached
                if data is None:
                    data = fetch(url, MAX_BYTES, ALLOWED_SVG_MIME if suffix == ".svg" else ALLOWED_PNG_MIME)
                metadata = validate_icon(data, suffix)
                validate_catalog_suitability(metadata)
                output.write_bytes(data)
                return entry["id"], provenance_record(path, data, metadata, source_key, source, url, match), None
            except (ValueError, ET.ParseError, OSError, SyntaxError) as error:
                errors.append(f"{Path(urllib.parse.urlsplit(url).path).name}: {error}")
        reason = "; ".join(errors) if errors else "No matching pinned upstream artwork"
        data = monogram_svg(entry)
        path = f"assets/catalog/{entry['id']}.svg"
        (ROOT / "src" / path).write_bytes(data)
        metadata = validate_icon(data, ".svg")
        url = "https://github.com/madhavsonkusare-a11y/local-store/blob/main/scripts/v2_catalog_icons.py"
        return entry["id"], provenance_record(path, data, metadata, "local-store-monogram", local_source, url, "generated", reason), reason

    with ThreadPoolExecutor(max_workers=8) as pool:
        results = list(pool.map(cache, projects))
    icons = {project_id: record for project_id, record, _ in results}
    fallbacks = {project_id: reason for project_id, _, reason in results if reason}
    manifest = {"schema_version": 2, "sources": sources, "icons": dict(sorted(icons.items())), "fallbacks": dict(sorted(fallbacks.items()))}
    audit = audit_manifest(manifest, projects)
    MANIFEST.write_text(json.dumps(manifest, ensure_ascii=False, indent=2, sort_keys=True) + "\n", encoding="utf-8", newline="\n")
    AUDIT.write_text(json.dumps(audit, ensure_ascii=False, indent=2, sort_keys=True) + "\n", encoding="utf-8", newline="\n")
    print(f"Resolved {len(icons)} icons: {audit['source_counts']}; {len(audit['duplicate_upstream_groups'])} duplicate groups; {len(audit['suspicious'])} suspicious files.")


if __name__ == "__main__":
    main()
