#!/usr/bin/env python3
"""Periodic doc reconciliation: gather drift signals between `current/` docs and
the real codebase, then emit a report. REPORT-ONLY: this tool never edits,
moves, or deletes any doc. Destructive actions (update/supersede/archive/delete)
are decided by a human on top of this report.

It intentionally does the mechanical, high-confidence signal gathering and leaves
semantic judgement (is a claim stale, or just terse?) to the agent's bounded
per-domain verification pass.

Signals:
1. code-newer-than-doc  : per manifest route, code under `code_globs` has commits
                          newer than the oldest `last_verified` of its required
                          current docs. (soft signal; depends on code_globs)
2. dangling-reference   : a current doc references a code entity that no longer
                          exists (file path, `pet_` table name, or *Service/
                          *ServiceImpl/*Controller class). (high confidence)
3. unsynced-change      : a `changes/*/design.md` marked implemented still has
                          unchecked `Current Doc Updates`, or is still `draft`
                          while its worklog already reached PASS/READY_FOR_PR.

Usage:
    python3 reconcile_docs.py <docs-root> [--out report.md] [--stale-days N]
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from datetime import date, datetime
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

from check_doc_runtime import parse_frontmatter, parse_manifest_routes, parse_iso_date  # noqa: E402


def read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8", errors="replace")


def git_last_commit_date(repo_root: Path, globs: list[str]) -> date | None:
    if not globs:
        return None
    pathspecs = [f":(glob){g}" for g in globs]
    try:
        out = subprocess.run(
            ["git", "-C", str(repo_root), "log", "-1", "--format=%cd", "--date=short", "--"] + pathspecs,
            capture_output=True,
            text=True,
            timeout=30,
        )
    except (OSError, subprocess.SubprocessError):
        return None
    line = out.stdout.strip()
    if not line:
        return None
    try:
        return datetime.strptime(line, "%Y-%m-%d").date()
    except ValueError:
        return None


def git_grep_exists(repo_root: Path, needle: str) -> bool:
    """True if the literal token appears anywhere in tracked files."""
    try:
        out = subprocess.run(
            ["git", "-C", str(repo_root), "grep", "-lF", "-e", needle],
            capture_output=True,
            text=True,
            timeout=30,
        )
    except (OSError, subprocess.SubprocessError):
        return True  # cannot check -> do not cry wolf
    return bool(out.stdout.strip())


# High-confidence code-entity extractors. Deliberately excludes API path
# matching, which is unreliable for split framework annotations.
RE_TABLE = re.compile(r"\bpet_[a-z][a-z0-9_]+\b")
RE_CLASS = re.compile(r"\b([A-Z][A-Za-z0-9]+(?:ServiceImpl|Service|Controller|Mapper|DO|VO))\b")
RE_FILEPATH = re.compile(r"\b((?:backend|owner-miniapp|store-miniapp|admin-vben)/[A-Za-z0-9_./\-]+\.(?:java|ts|vue|xml))\b")

GENERIC_CLASS_STOP = {"Service", "Controller", "Mapper"}  # too generic alone


def extract_entities(text: str) -> dict[str, set[str]]:
    tables = set(RE_TABLE.findall(text))
    classes = {c for c in RE_CLASS.findall(text) if c not in GENERIC_CLASS_STOP}
    files = set(RE_FILEPATH.findall(text))
    return {"tables": tables, "classes": classes, "files": files}


def check_dangling_refs(repo_root: Path, docs: list[Path]) -> list[str]:
    findings: list[str] = []
    # Cache existence lookups across docs to keep git grep calls down.
    exists_cache: dict[str, bool] = {}

    def exists(token: str) -> bool:
        if token not in exists_cache:
            exists_cache[token] = git_grep_exists(repo_root, token)
        return exists_cache[token]

    for doc in docs:
        text = read_text(doc)
        ents = extract_entities(text)
        rel_doc = doc.relative_to(repo_root).as_posix()
        for table in sorted(ents["tables"]):
            if not exists(table):
                findings.append(f"[dangling-reference] {rel_doc} -> table `{table}` not found in code")
        for cls in sorted(ents["classes"]):
            if not exists(cls):
                findings.append(f"[dangling-reference] {rel_doc} -> class `{cls}` not found in code")
        for fpath in sorted(ents["files"]):
            if not (repo_root / fpath).exists():
                findings.append(f"[dangling-reference] {rel_doc} -> file `{fpath}` does not exist")
    return findings


def oldest_last_verified(repo_root: Path, refs: list[str]) -> date | None:
    dates: list[date] = []
    for ref in refs:
        path = repo_root / ref
        if not path.exists():
            continue
        meta = parse_frontmatter(read_text(path))
        lv = meta.get("last_verified")
        if lv:
            d = parse_iso_date(lv)
            if d:
                dates.append(d)
    return min(dates) if dates else None


def check_code_newer(repo_root: Path, routes: list[dict]) -> tuple[list[str], list[str]]:
    findings: list[str] = []
    notes: list[str] = []
    for route in routes:
        globs = route.get("code_globs") or []
        if not globs:
            if route["required"]:
                notes.append(f"[no-code-globs] route '{route['name']}' has no code_globs; time-drift detection skipped")
            continue
        code_date = git_last_commit_date(repo_root, globs)
        doc_date = oldest_last_verified(repo_root, route["required"])
        if code_date and doc_date and code_date > doc_date:
            findings.append(
                f"[code-newer-than-doc] route '{route['name']}': code last changed {code_date} but oldest doc last_verified is {doc_date}; re-verify this domain"
            )
    return findings, notes


CHECKBOX_UNCHECKED = re.compile(r"^\s*-\s*\[\s*\]\s+", re.MULTILINE)


def check_unsynced_changes(root: Path) -> list[str]:
    findings: list[str] = []
    changes = root / "changes"
    if not changes.is_dir():
        return findings
    for design in sorted(changes.glob("*/design.md")):
        text = read_text(design)
        meta = parse_frontmatter(text)
        status = meta.get("status", "")
        rel_design = design.relative_to(root).as_posix()
        section = ""
        if "## Current Doc Updates" in text:
            section = text.split("## Current Doc Updates", 1)[1]
        if status == "implemented" and CHECKBOX_UNCHECKED.search(section):
            findings.append(f"[unsynced-change] {rel_design} is implemented but has unchecked Current Doc Updates")
        if status == "draft":
            worklog = design.parent / "worklog.md"
            if worklog.exists():
                wl = read_text(worklog)
                if re.search(r"\bPASS\b|READY_FOR_PR|PR_PASS", wl):
                    findings.append(f"[unclosed-design] {rel_design} still draft but its worklog already reached PASS/READY_FOR_PR")
    return findings


def build_report(root: Path, stale_days: int) -> str:
    repo_root = root.parent
    manifest = root / "manifest.yaml"
    routes = parse_manifest_routes(read_text(manifest)) if manifest.exists() else []

    current = root / "current"
    current_docs = sorted(current.rglob("*.md")) if current.exists() else []

    dangling = check_dangling_refs(repo_root, current_docs)
    code_newer, notes = check_code_newer(repo_root, routes)
    unsynced = check_unsynced_changes(root)

    lines: list[str] = []
    lines.append(f"# Doc Reconciliation Report ({date.today().isoformat()})")
    lines.append("")
    lines.append("REPORT-ONLY. No docs were modified. Use this to drive a per-domain verification pass; destructive actions require human approval.")
    lines.append("")
    lines.append("## Signal Summary")
    lines.append("")
    lines.append("| Signal | Count | Confidence |")
    lines.append("| --- | --- | --- |")
    lines.append(f"| dangling-reference | {len(dangling)} | high |")
    lines.append(f"| code-newer-than-doc | {len(code_newer)} | soft (depends on code_globs) |")
    lines.append(f"| unsynced/unclosed change | {len(unsynced)} | high |")
    lines.append(f"| notes | {len(notes)} | info |")
    lines.append("")

    def block(title: str, items: list[str]) -> None:
        lines.append(f"## {title}")
        lines.append("")
        if not items:
            lines.append("- none")
        else:
            for it in items:
                lines.append(f"- {it}")
        lines.append("")

    block("Dangling References (high confidence)", dangling)
    block("Code Newer Than Doc (verify these domains)", code_newer)
    block("Unsynced / Unclosed Changes", unsynced)
    block("Notes", notes)

    lines.append("## Next Step")
    lines.append("")
    lines.append("For each flagged domain, load only that domain's current docs plus the backing code, confirm whether each claim is stale or merely terse, then record proposed actions (update / supersede / archive / delete) in a reconciliation report under `docs/changes/YYYY-MM-DD-doc-reconciliation/` before making any change.")
    lines.append("")
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("docs_root", help="Path to docs directory")
    parser.add_argument("--out", help="Write the report to this file instead of stdout")
    parser.add_argument("--stale-days", type=int, default=120)
    args = parser.parse_args()

    root = Path(args.docs_root).resolve()
    if not root.is_dir():
        print(f"ERROR: docs root does not exist: {root}", file=sys.stderr)
        return 2

    report = build_report(root, args.stale_days)
    if args.out:
        Path(args.out).write_text(report, encoding="utf-8")
        print(f"reconciliation report written to {args.out}")
    else:
        print(report)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
