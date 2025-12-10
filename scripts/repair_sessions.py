#!/usr/bin/env python3
"""
Repair Codex session JSONL files using their original .mixed.bak sources.

For each rollout-*.jsonl file under the given root (default: ~/.codex/sessions),
the script:
  - Finds the sibling rollout-*.mixed.bak file with the same timestamp prefix.
  - Selects only lines that belong to the jsonl file's cwd (using session_meta /
    turn_context cues, case/sep-insensitive).
  - Rewrites SessionMeta ids/cwds to match the target jsonl file.
  - Merges and de-duplicates lines, preserving source order, and overwrites the
    jsonl file.

This is a one-off patch helper; it does not run inside the app lifecycle.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Dict, Iterable, List, Optional, Tuple


def normalize_cwd(raw: str) -> str:
    s = raw.replace("\\", "/")
    for prefix in ("//?/", "\\\\?\\"):
        if s.lower().startswith(prefix.lower()):
            s = s[len(prefix) :]
    s = s.rstrip("/")
    return s.lower()


def extract_cwd_and_id_from_jsonl(path: Path) -> Tuple[Optional[str], Optional[str]]:
    """
    Return (cwd_str, conversation_id) from the first session_meta line.
    """
    try:
        with path.open("r", encoding="utf-8", errors="ignore") as fh:
            for line in fh:
                line = line.strip()
                if not line:
                    continue
                obj = json.loads(line)
                if obj.get("type") == "session_meta":
                    meta = obj.get("payload", {}).get("meta", {})
                    cwd = meta.get("cwd")
                    cid = meta.get("id")
                    return cwd, cid
    except FileNotFoundError:
        return None, None
    except Exception as exc:  # pragma: no cover - best effort tool
        print(f"[warn] Failed reading meta from {path}: {exc}", file=sys.stderr)
    return None, None


def ts_prefix_from_name(path: Path) -> Optional[str]:
    name = path.name
    if not name.startswith("rollout-"):
        return None
    core = name[len("rollout-") :]
    # Fixed 19-char timestamp: YYYY-MM-DDThh-mm-ss
    return core[:19] if len(core) >= 19 else None


def update_cwd_and_id(obj: Dict, new_cwd: str, new_cid: str) -> Dict:
    typ = obj.get("type")
    payload = obj.get("payload", {})
    if typ == "session_meta":
        meta = payload.get("meta", {})
        meta["cwd"] = new_cwd
        meta["id"] = new_cid
        payload["meta"] = meta
        obj["payload"] = payload
    elif typ == "turn_context":
        payload["cwd"] = new_cwd
        obj["payload"] = payload
    return obj


def collect_cwd_filtered_lines(
    src_path: Path, target_cwd_norm: str
) -> List[Dict]:
    lines: List[Dict] = []
    current_cwd: Optional[str] = None
    try:
        with src_path.open("r", encoding="utf-8", errors="ignore") as fh:
            for raw in fh:
                raw = raw.strip()
                if not raw:
                    continue
                try:
                    obj = json.loads(raw)
                except Exception:
                    continue

                payload = obj.get("payload", {})
                new_cwd = None
                if obj.get("type") == "session_meta":
                    new_cwd = payload.get("meta", {}).get("cwd")
                elif obj.get("type") == "turn_context":
                    new_cwd = payload.get("cwd")
                if new_cwd:
                    current_cwd = normalize_cwd(str(new_cwd))

                if current_cwd and current_cwd == target_cwd_norm:
                    lines.append(obj)
    except FileNotFoundError:
        pass
    return lines


def dedup_preserve_order(objs: Iterable[Dict]) -> List[Dict]:
    seen = set()
    out: List[Dict] = []
    for obj in objs:
        key = json.dumps(obj, sort_keys=True)
        if key in seen:
            continue
        seen.add(key)
        out.append(obj)
    return out


def process_jsonl(jsonl_path: Path, dry_run: bool) -> bool:
    target_cwd_raw, target_cid = extract_cwd_and_id_from_jsonl(jsonl_path)
    if not target_cwd_raw or not target_cid:
        return False
    target_cwd_norm = normalize_cwd(str(target_cwd_raw))

    ts_prefix = ts_prefix_from_name(jsonl_path)
    if not ts_prefix:
        return False

    day_dir = jsonl_path.parent
    candidates = list(day_dir.glob(f"rollout-{ts_prefix}-*.mixed.bak"))
    if not candidates:
        return False
    if len(candidates) > 1:
        # Choose the longest (most specific) filename to disambiguate.
        candidates.sort(key=lambda p: len(p.name), reverse=True)
    src_path = candidates[0]

    source_lines = collect_cwd_filtered_lines(src_path, target_cwd_norm)
    if not source_lines:
        return False

    # Rehydrate with target ids/cwd.
    source_lines = [
        update_cwd_and_id(obj, target_cwd_raw, target_cid) for obj in source_lines
    ]

    # Existing lines (keep anything already there).
    existing: List[Dict] = []
    with jsonl_path.open("r", encoding="utf-8", errors="ignore") as fh:
        for raw in fh:
            raw = raw.strip()
            if not raw:
                continue
            try:
                existing.append(json.loads(raw))
            except Exception:
                continue

    merged = dedup_preserve_order(source_lines + existing)

    if dry_run:
        return True

    with jsonl_path.open("w", encoding="utf-8") as fh:
        for obj in merged:
            fh.write(json.dumps(obj, ensure_ascii=False))
            fh.write("\n")
    return True


def iter_jsonl_files(root: Path) -> Iterable[Path]:
    for path in root.rglob("rollout-*.jsonl"):
        if path.name.endswith(".mixed.bak"):
            continue
        yield path


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--root",
        type=Path,
        default=Path.home() / ".codex" / "sessions",
        help="Root sessions directory (default: ~/.codex/sessions)",
    )
    parser.add_argument("--dry-run", action="store_true", help="Do not write files")
    args = parser.parse_args()

    root = args.root
    if not root.exists():
        print(f"[error] Root {root} does not exist", file=sys.stderr)
        return 1

    total = 0
    repaired = 0
    for jsonl in iter_jsonl_files(root):
        total += 1
        ok = process_jsonl(jsonl, args.dry_run)
        if ok:
            repaired += 1

    print(f"Scanned {total} jsonl files; repaired {repaired}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
