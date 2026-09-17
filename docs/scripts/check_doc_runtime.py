#!/usr/bin/env python3
"""Lightweight checks for Agent Runtime documentation libraries.

This script intentionally uses only Python's standard library and a small
frontmatter parser so it can run in most repositories without setup.

Checks:
- structural (index/manifest presence, domain index rule)
- per-file line budgets and stale-current phrases
- superseded docs must declare superseded_by
- manifest references must exist
- route token budget: required docs per manifest route must fit token_budget
- last_verified staleness: current docs verified too long ago are flagged
- orphan current docs: current docs unreachable from manifest or any index.md
- postmortem backlink: postmortems must promote a guardrail or be marked one-off
- worklog contract: `changes/*/worklog.md` Loop rows must reuse the review
  Result vocabulary in `评审结论`, tie a PASS loop to a real commit, and record
  a re-anchor outcome at each L004/L007/L010/... checkpoint
- review contract: `changes/*/reviews/Lxxx.md` must declare a Review Summary
  Result (PASS/PASS_WITH_WARNINGS/FAIL) and keep Final Verdict Gate results in
  pending/passed/failed, without mixing the two vocabularies
- complex-change review gap: a change dir with design.md and >= 3 worklog
  loops but no reviews/ directory is flagged as a possible skipped review
"""

from __future__ import annotations

import argparse
import re
import sys
from datetime import date, datetime
from pathlib import Path


DEFAULT_LIMITS = {
    "index.md": 120,
    "overview.md": 200,
    "flow.md": 250,
    "api-contract.md": 350,
    "frontend-behavior.md": 300,
    "backend-behavior.md": 300,
    "verification.md": 300,
}

CURRENT_WARN_LIMIT = 350
CURRENT_HARD_LIMIT = 600
RULE_LIMIT = 200

# Rough token estimate. Non-CJK text is ~4 characters per token, but CJK text
# (this codebase is heavily Chinese) is far denser: most tokenizers spend close
# to one token per CJK character. Treating everything as 4 chars/token would
# systematically UNDER-count Chinese docs and hide real budget overflows, so CJK
# characters are weighted separately.
CHARS_PER_TOKEN = 4
CJK_TOKENS_PER_CHAR = 1.0

# CJK-ish Unicode ranges: symbols/punctuation, kana, ideographs (+ extension A),
# compatibility ideographs, Hangul, and fullwidth forms.
_CJK_RANGES = (
    (0x3000, 0x303F),
    (0x3040, 0x30FF),
    (0x3400, 0x4DBF),
    (0x4E00, 0x9FFF),
    (0xF900, 0xFAFF),
    (0xAC00, 0xD7AF),
    (0xFF00, 0xFFEF),
)


def _is_cjk(ch: str) -> bool:
    code = ord(ch)
    return any(lo <= code <= hi for lo, hi in _CJK_RANGES)


def estimate_tokens(text: str) -> int:
    """CJK-weighted rough token estimate, biased slightly high so a budget guard
    fails loud rather than silently missing an overflow."""
    cjk = sum(1 for ch in text if _is_cjk(ch))
    other = len(text) - cjk
    return int(cjk * CJK_TOKENS_PER_CHAR + other / CHARS_PER_TOKEN)

DEFAULT_STALE_DAYS = 120

STALE_CURRENT_PHRASES = [
    "not yet scaffolded",
    "documentation-only",
    "TODO:",
    "[TODO",
]

# A postmortem is treated as "guardrail-closed" if it is referenced by a rule or
# verification doc, or if it explicitly declares itself one-off.
ONE_OFF_MARKERS = [
    "not generalized",
    "one-off",
    "one off",
    "一次性",
    "不推广",
    "不泛化",
]

# --- Worklog / review vocabulary contract -----------------------------
#
# SKILL.md's Non-Negotiables define two distinct vocabulary layers: a per-loop
# review Result (pending review/PASS/PASS_WITH_WARNINGS/FAIL) and milestone
# Gates (CODE_PASS/DOCS_PASS/READY_FOR_PR/PR_PASS, each pending/passed/failed).
# `评审结论` in worklog.md must reuse the Result vocabulary verbatim; seeing a
# Gate word there, or free text like "完成", means the contract drifted.
WORKLOG_RESULT_TOKENS = ("PENDING REVIEW", "PASS_WITH_WARNINGS", "PASS", "FAIL")
GATE_ONLY_TOKENS = ("READY_FOR_PR", "CODE_PASS", "DOCS_PASS", "PR_PASS")
COMMIT_NO_HASH_MARKERS = {"not committed", "暂未提交", "未提交", "n/a", "-", "--", "—", ""}
REANCHOR_TOKENS = ("on-track", "scope-drift", "intent-drift", "doc-drift")
LOOP_ID_RE = re.compile(r"L(\d+)", re.IGNORECASE)

REVIEW_RESULT_VALUES = {"PASS", "PASS_WITH_WARNINGS", "FAIL"}
GATE_RESULT_VALUES = {"pending", "passed", "failed"}

# --- Worklog / review structural template contract ---------------------
#
# These check exact template scaffolding (headings, table headers, naming),
# a layer below the value-vocabulary checks above: a file can have the right
# headings but still put the wrong word in a cell, or have the right words but
# be missing a whole required table.
WORKLOG_REQUIRED_HEADER = "| Loop | 本轮目标 | 主要改动 | 验证结果 | 评审结论 | 下一轮动作 | 风险/待评估点 | Commit | Re-anchor |"
WORKLOG_LEGACY_HEADERS = (
    "| Loop | 本轮目标 | 主要改动 | 验证结果 | 评审结论 | 下一轮动作 | 风险/待评估点 | Commit |",
    "| Loop | 本轮目标 | 主要改动 | 验证结果 | 风险/待评估点 | Commit |",
)
REVIEW_REQUIRED_TITLE = "# Review: "
REVIEW_SUMMARY_HEADER = "| Review | Target Loop | Result | Summary | Next Action |"
REVIEW_EVALUATION_HEADER = "| 维度 | 结果 | 说明 |"
REVIEW_ISSUES_HEADER = "| ID | 严重级别 | 状态 | 维度 | 问题 | 证据 | 建议 | Fixed In | Verified In |"
REVIEW_FINAL_VERDICT_HEADER = "| Gate | Result | Notes |"


def read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8", errors="replace")


def parse_frontmatter(text: str) -> dict[str, str]:
    if not text.startswith("---\n"):
        return {}
    end = text.find("\n---", 4)
    if end == -1:
        return {}
    raw = text[4:end]
    data: dict[str, str] = {}
    for line in raw.splitlines():
        if ":" not in line or line.lstrip().startswith("-"):
            continue
        key, value = line.split(":", 1)
        data[key.strip()] = value.strip().strip('"').strip("'")
    return data


def strip_frontmatter(text: str) -> str:
    if not text.startswith("---\n"):
        return text
    end = text.find("\n---", 4)
    if end == -1:
        return text
    return text[end + 4 :].lstrip("\n")


def rel(path: Path, root: Path) -> str:
    return path.relative_to(root).as_posix()


def collect_doc_refs(text: str) -> set[str]:
    """Collect docs/*.md path references from any text blob."""
    return set(re.findall(r"docs/[A-Za-z0-9_./-]+\.md", text))


def collect_index_refs(index_path: Path, text: str, repo_root: Path) -> set[str]:
    """Collect every .md reference from an index file and resolve it to a
    repo-relative path.

    Index files link to sibling docs by bare filename (`api-contract.md`) or
    relative path (`../asset-upload/index.md`), not only by absolute `docs/...`
    paths, so those references must be resolved against the index's directory to
    avoid false orphan reports.
    """
    resolved: set[str] = set()
    for token in re.findall(r"[A-Za-z0-9_][A-Za-z0-9_./-]*\.md", text):
        if token.startswith("docs/"):
            resolved.add(token)
            continue
        candidate = (index_path.parent / token).resolve()
        try:
            resolved.add(candidate.relative_to(repo_root).as_posix())
        except ValueError:
            continue
    return resolved


def parse_manifest_routes(manifest_text: str) -> list[dict]:
    """Parse manifest routes by indentation without a YAML dependency.

    Returns a list of {name, token_budget, required, optional, history} dicts.
    """
    routes: list[dict] = []
    in_routes = False
    current: dict | None = None
    section: str | None = None

    for raw_line in manifest_text.splitlines():
        if not raw_line.strip() or raw_line.lstrip().startswith("#"):
            continue
        indent = len(raw_line) - len(raw_line.lstrip(" "))
        stripped = raw_line.strip()

        if indent == 0:
            in_routes = stripped == "routes:"
            current = None
            section = None
            continue
        if not in_routes:
            continue

        if indent == 2 and stripped.endswith(":"):
            current = {
                "name": stripped[:-1],
                "token_budget": None,
                "required": [],
                "optional": [],
                "history_on_demand": [],
                "code_globs": [],
            }
            routes.append(current)
            section = None
            continue

        if current is None:
            continue

        if indent == 4:
            if stripped.startswith("token_budget:"):
                value = stripped.split(":", 1)[1].strip()
                try:
                    current["token_budget"] = int(value)
                except ValueError:
                    current["token_budget"] = None
                section = None
            elif stripped.rstrip(":") in ("required", "optional", "history_on_demand", "code_globs"):
                section = stripped.rstrip(":")
            else:
                section = None
            continue

        if indent >= 6 and stripped.startswith("- ") and section:
            current[section].append(stripped[2:].strip().strip('"').strip("'"))

    return routes


_SEPARATOR_CELL_RE = re.compile(r"^:?-{2,}:?$")


def _split_table_row(line: str) -> list[str]:
    inner = line.strip()
    if inner.startswith("|"):
        inner = inner[1:]
    if inner.endswith("|"):
        inner = inner[:-1]
    return [cell.strip() for cell in inner.split("|")]


def find_markdown_tables(text: str) -> list[tuple[list[str], list[list[str]]]]:
    """Find GitHub-style pipe tables (header row + `---` separator + data
    rows). Returns a list of (header_cells, data_rows) per table found."""
    lines = text.splitlines()
    tables: list[tuple[list[str], list[list[str]]]] = []
    i = 0
    n = len(lines)
    while i < n - 1:
        header_line = lines[i]
        sep_line = lines[i + 1]
        if header_line.strip().startswith("|") and sep_line.strip().startswith("|"):
            sep_cells = _split_table_row(sep_line)
            if sep_cells and all(_SEPARATOR_CELL_RE.match(c) for c in sep_cells):
                header = _split_table_row(header_line)
                rows: list[list[str]] = []
                j = i + 2
                while j < n and lines[j].strip().startswith("|"):
                    rows.append(_split_table_row(lines[j]))
                    j += 1
                tables.append((header, rows))
                i = j
                continue
        i += 1
    return tables


def find_col(header: list[str], *keywords: str) -> int | None:
    """Return the index of the first header cell containing any keyword
    (case-insensitive substring match), checked keyword-by-keyword so the
    first keyword acts as the preferred match."""
    lowered = [h.lower() for h in header]
    for keyword in keywords:
        needle = keyword.lower()
        for idx, cell in enumerate(lowered):
            if needle in cell:
                return idx
    return None


def check_worklog_contract(rel_path: str, text: str) -> list[str]:
    """Flag `worklog.md` Loop rows that drift from the vocabulary contract:
    `评审结论` must reuse the review Result vocabulary (not Gate words or free
    text), a loop claiming PASS must carry a real commit, and every re-anchor
    checkpoint (L004, L007, L010, ...) must record an outcome."""
    warnings: list[str] = []
    for header, rows in find_markdown_tables(text):
        loop_idx = find_col(header, "loop")
        if loop_idx is None:
            continue
        review_idx = find_col(header, "评审结论", "review result")
        commit_idx = find_col(header, "commit")
        reanchor_idx = find_col(header, "re-anchor", "reanchor")
        for row in rows:
            if loop_idx >= len(row):
                continue
            match = LOOP_ID_RE.search(row[loop_idx])
            if not match:
                continue
            loop_num = int(match.group(1))
            loop_label = f"L{loop_num:03d}"

            review_val = row[review_idx].strip().strip("`") if review_idx is not None and review_idx < len(row) else ""
            review_upper = review_val.upper()
            has_result_token = any(tok in review_upper for tok in WORKLOG_RESULT_TOKENS)
            has_gate_token = any(tok in review_upper for tok in GATE_ONLY_TOKENS)
            if review_val and not has_result_token:
                if has_gate_token:
                    warnings.append(
                        f"{rel_path} {loop_label}: 评审结论 {review_val!r} uses milestone-gate vocabulary "
                        "instead of the review Result vocabulary (pending review/PASS/PASS_WITH_WARNINGS/FAIL)"
                    )
                else:
                    warnings.append(
                        f"{rel_path} {loop_label}: 评审结论 {review_val!r} does not use the contract vocabulary "
                        "(pending review/PASS/PASS_WITH_WARNINGS/FAIL)"
                    )

            if "PASS" in review_upper and "PASS_WITH_WARNINGS" not in review_upper and "FAIL" not in review_upper:
                commit_val = (
                    row[commit_idx].strip().strip("`") if commit_idx is not None and commit_idx < len(row) else ""
                )
                if commit_val.lower() in COMMIT_NO_HASH_MARKERS:
                    warnings.append(
                        f"{rel_path} {loop_label}: 评审结论 claims PASS but Commit is {commit_val!r} "
                        "(expected a real commit hash)"
                    )

            if reanchor_idx is not None and loop_num >= 4 and loop_num % 3 == 1:
                reanchor_raw = row[reanchor_idx] if reanchor_idx < len(row) else ""
                reanchor_val = reanchor_raw.strip().lower()
                if not any(tok in reanchor_val for tok in REANCHOR_TOKENS):
                    warnings.append(
                        f"{rel_path} {loop_label}: re-anchor checkpoint but Re-anchor cell {reanchor_raw!r} is not "
                        "on-track/scope-drift/intent-drift/doc-drift"
                    )
    return warnings


def check_review_contract(rel_path: str, text: str) -> list[str]:
    """Flag `reviews/Lxxx.md` value drift within the two review vocabulary
    layers: the Review Summary `Result` column must be
    PASS/PASS_WITH_WARNINGS/FAIL, and the Final Verdict `Gate` table's Result
    column must be pending/passed/failed. Whether the tables exist at all is
    a structural concern owned by `check_change_records`; this function only
    checks the *values* once a table is present."""
    warnings: list[str] = []
    for header, rows in find_markdown_tables(text):
        lowered = [h.lower() for h in header]
        is_gate_table = any("gate" in h for h in lowered)
        is_review_summary = (not is_gate_table) and (
            any("target loop" in h for h in lowered) or (lowered and "review" in lowered[0])
        )
        if not (is_gate_table or is_review_summary):
            continue
        result_idx = find_col(header, "result")
        if result_idx is None:
            continue
        for row in rows:
            if result_idx >= len(row):
                continue
            val = row[result_idx].strip().strip("`")
            if not val or "/" in val:
                continue  # blank cell or an un-filled "A / B / C" template placeholder
            if is_gate_table:
                if val.lower() not in GATE_RESULT_VALUES:
                    warnings.append(f"{rel_path}: Final Verdict Gate Result {val!r} not in pending/passed/failed")
            elif val.upper() not in REVIEW_RESULT_VALUES:
                warnings.append(f"{rel_path}: Review Summary Result {val!r} not in PASS/PASS_WITH_WARNINGS/FAIL")
    return warnings


def check_complex_change_missing_review(root: Path) -> list[str]:
    """A change dir with a design.md and >= 3 worklog loops signals a
    non-trivial, multi-loop task. If it still has no reviews/ directory, the
    review may have been skipped via the 'explicit developer override' escape
    hatch rather than genuinely requested; surface it so that path does not
    silently become the default."""
    warnings: list[str] = []
    changes_dir = root / "changes"
    if not changes_dir.exists():
        return warnings
    for feature_dir in sorted(p for p in changes_dir.iterdir() if p.is_dir()):
        design_path = feature_dir / "design.md"
        worklog_path = feature_dir / "worklog.md"
        if not design_path.exists() or not worklog_path.exists():
            continue
        if (feature_dir / "reviews").exists():
            continue
        loop_nums: set[int] = set()
        for header, rows in find_markdown_tables(read_text(worklog_path)):
            loop_idx = find_col(header, "loop")
            if loop_idx is None:
                continue
            for row in rows:
                if loop_idx < len(row):
                    match = LOOP_ID_RE.search(row[loop_idx])
                    if match:
                        loop_nums.add(int(match.group(1)))
        if len(loop_nums) >= 3:
            warnings.append(
                f"{feature_dir.name}: has design.md and {len(loop_nums)} worklog loops but no reviews/ "
                "directory (possible skipped review)"
            )
    return warnings


def change_dirs(root: Path) -> list[Path]:
    changes = root / "changes"
    if not changes.exists():
        return []
    return sorted([p for p in changes.iterdir() if p.is_dir()])


def has_loop_like_marker(text: str) -> bool:
    return bool(re.search(r"^\|\s*L\d{3}\s*\|", text, flags=re.MULTILINE))


def has_review_like_marker(text: str) -> bool:
    return bool(re.search(r"^\|\s*R\d{3}\s*\|", text, flags=re.MULTILINE))


def check_change_records(root: Path, errors: list[str], warnings: list[str]) -> None:
    """Structural template-scaffolding checks for `changes/*/worklog.md` and
    `changes/*/reviews/Lxxx.md`: correct naming, required heading, and every
    required table present. This is independent of `check_worklog_contract` /
    `check_review_contract`, which check cell *values* once a table exists."""
    for change_dir in change_dirs(root):
        worklog = change_dir / "worklog.md"
        legacy_review = change_dir / "review.md"
        reviews_dir = change_dir / "reviews"

        if legacy_review.exists():
            warnings.append(f"{rel(legacy_review, root)} uses legacy review.md; migrate to reviews/Lxxx.md")

        if reviews_dir.exists():
            # The skill mandates `reviews/design-review.md` (pre-implementation gate)
            # and `reviews/index.md` (running issue ledger) alongside the per-loop
            # `Lxxx.md` files; only the per-loop files follow the Lxxx naming rule.
            allowed_non_loop_names = {"index.md", "design-review.md"}
            for review_file in sorted(reviews_dir.glob("*.md")):
                if review_file.name in allowed_non_loop_names:
                    continue
                if not re.fullmatch(r"L\d{3}\.md", review_file.name):
                    errors.append(f"{rel(review_file, root)} must use Lxxx.md naming inside reviews/")
                    continue
                review_body = strip_frontmatter(read_text(review_file))
                review_lines = review_body.splitlines()
                if not review_lines or not review_lines[0].startswith(REVIEW_REQUIRED_TITLE):
                    warnings.append(f"{rel(review_file, root)} must start with '# Review: <feature-name>'")
                if REVIEW_SUMMARY_HEADER not in review_body:
                    warnings.append(f"{rel(review_file, root)} is missing the review summary table from the template")
                if REVIEW_EVALUATION_HEADER not in review_body:
                    warnings.append(f"{rel(review_file, root)} is missing the evaluation table from the template")
                if REVIEW_ISSUES_HEADER not in review_body:
                    warnings.append(f"{rel(review_file, root)} is missing the issues table from the template")
                if REVIEW_FINAL_VERDICT_HEADER not in review_body:
                    warnings.append(f"{rel(review_file, root)} is missing the final verdict table from the template")
                if not has_review_like_marker(review_body):
                    warnings.append(f"{rel(review_file, root)} has no review summary rows yet")

        if worklog.exists():
            body = strip_frontmatter(read_text(worklog))
            lines = body.splitlines()
            if not lines or not lines[0].startswith("# Worklog: "):
                warnings.append(f"{rel(worklog, root)} does not use the standard '# Worklog: <feature-name>' heading")
                continue
            if WORKLOG_REQUIRED_HEADER not in body:
                matched_legacy = next((h for h in WORKLOG_LEGACY_HEADERS if h in body), None)
                if matched_legacy is not None:
                    warnings.append(
                        f"{rel(worklog, root)} uses legacy worklog table (no Re-anchor column); "
                        "migrate to the fixed 9-column table"
                    )
                else:
                    warnings.append(f"{rel(worklog, root)} is missing the fixed 9-column worklog header")
            if not has_loop_like_marker(body):
                warnings.append(f"{rel(worklog, root)} has no loop rows yet")

        # Historical plan directories may predate worklog enforcement; only require
        # worklog when review artifacts already exist in the directory.
        if not worklog.exists() and (legacy_review.exists() or reviews_dir.exists()):
            errors.append(f"{rel(change_dir, root)} has review records but no worklog.md")


def parse_iso_date(value: str) -> date | None:
    value = value.strip()
    for fmt in ("%Y-%m-%d", "%Y/%m/%d"):
        try:
            return datetime.strptime(value, fmt).date()
        except ValueError:
            continue
    return None


def check_docs(root: Path, stale_days: int) -> tuple[list[str], list[str]]:
    # Resolve up front so symlinked roots (e.g. macOS temp dirs) do not break
    # relative_to() when comparing against .resolve()d reference paths.
    root = root.resolve()
    errors: list[str] = []
    warnings: list[str] = []

    if not (root / "index.md").exists():
        errors.append("docs/index.md is missing")
    if not (root / "manifest.yaml").exists():
        warnings.append("docs/manifest.yaml is missing")

    repo_root = root.parent
    today = date.today()

    current = root / "current"
    domains = current / "domains"
    if domains.exists():
        for domain_dir in sorted(p for p in domains.iterdir() if p.is_dir()):
            md_files = list(domain_dir.glob("*.md"))
            if len(md_files) > 3 and not (domain_dir / "index.md").exists():
                errors.append(f"{rel(domain_dir, root)} has more than 3 docs but no index.md")

    # Aggregate reachability references from manifest + every index.md.
    reachable_refs: set[str] = set()

    manifest = root / "manifest.yaml"
    manifest_text = read_text(manifest) if manifest.exists() else ""
    if manifest_text:
        reachable_refs |= collect_doc_refs(manifest_text)

    for md in sorted(root.rglob("*.md")):
        text = read_text(md)
        meta = parse_frontmatter(text)
        path_rel = rel(md, root)
        lines = len(text.splitlines())
        norm_rel = path_rel.replace("\\", "/")

        if md.name == "index.md":
            reachable_refs |= collect_index_refs(md, text, repo_root)

        if re.search(r"changes/[^/]+/worklog\.md$", norm_rel):
            warnings.extend(check_worklog_contract(path_rel, text))
        elif re.search(r"changes/[^/]+/reviews/L[^/]+\.md$", norm_rel):
            warnings.extend(check_review_contract(path_rel, text))

        if "current/" in norm_rel:
            if not meta:
                warnings.append(f"{path_rel} has no frontmatter")
            elif meta.get("status") != "current":
                warnings.append(f"{path_rel} is under current/ but status is {meta.get('status')!r}")

            name_limit = DEFAULT_LIMITS.get(md.name)
            if name_limit and lines > name_limit:
                warnings.append(f"{path_rel} has {lines} lines, expected <= {name_limit}")
            elif ".rules.md" in md.name and lines > RULE_LIMIT:
                warnings.append(f"{path_rel} has {lines} lines, expected rules <= {RULE_LIMIT}")
            elif lines > CURRENT_HARD_LIMIT:
                errors.append(f"{path_rel} has {lines} lines and must not be required current context")
            elif lines > CURRENT_WARN_LIMIT:
                warnings.append(f"{path_rel} has {lines} lines; split or justify")

            lowered = text.lower()
            for phrase in STALE_CURRENT_PHRASES:
                if phrase.lower() in lowered:
                    warnings.append(f"{path_rel} contains stale/draft phrase: {phrase}")

            last_verified = meta.get("last_verified")
            if last_verified:
                verified_date = parse_iso_date(last_verified)
                if verified_date is None:
                    warnings.append(f"{path_rel} has unparseable last_verified: {last_verified!r}")
                else:
                    age = (today - verified_date).days
                    if age > stale_days:
                        warnings.append(
                            f"{path_rel} last_verified {last_verified} is {age} days old (> {stale_days}); re-verify or refresh"
                        )
            elif meta:
                warnings.append(f"{path_rel} is under current/ but has no last_verified")

        if meta.get("status") == "superseded" and "superseded_by" not in meta:
            errors.append(f"{path_rel} is superseded but lacks superseded_by")

    # Manifest references must exist.
    for ref in sorted(collect_doc_refs(manifest_text)):
        if not (repo_root / ref).exists():
            errors.append(f"manifest references missing file: {ref}")

    # Structural template-scaffolding checks for worklog.md / reviews/Lxxx.md.
    check_change_records(root, errors, warnings)

    # Manifest parse must not fail silently: a guard that quietly parses nothing
    # is worse than no guard.
    routes = parse_manifest_routes(manifest_text)
    if manifest_text and "routes:" in manifest_text and not routes:
        errors.append("manifest.yaml has 'routes:' but no routes could be parsed (check indentation/tabs)")
    if manifest_text and "\t" in manifest_text:
        warnings.append("manifest.yaml contains tab characters; indentation-based route parsing may be unreliable")

    # Route token budget: required docs must fit the declared budget.
    for route in routes:
        budget = route.get("token_budget")
        if not budget:
            continue
        total_tokens = 0
        for ref in route["required"]:
            ref_path = repo_root / ref
            if ref_path.exists():
                total_tokens += estimate_tokens(read_text(ref_path))
        if total_tokens > budget:
            warnings.append(
                f"route '{route['name']}' required docs ~{total_tokens} tokens exceed token_budget {budget}; trim or re-scope required set"
            )

    # Complex-change review gap: design.md + >=3 worklog loops but no reviews/.
    warnings.extend(check_complex_change_missing_review(root))

    # Orphan current docs: reachable from neither manifest nor any index.md.
    if domains.exists():
        for md in sorted(domains.rglob("*.md")):
            if md.name == "index.md":
                continue
            ref = rel(md, repo_root)
            if ref not in reachable_refs:
                warnings.append(f"orphan current doc (not routed by manifest or any index.md): {ref}")

    # Postmortem guardrail commitment: each postmortem must name a concrete
    # guardrail target in its own body (a rule, verification doc, manifest route,
    # or script) or be explicitly marked one-off. Merely being listed in the
    # index is NOT a guardrail; it is only cataloging.
    postmortems = root / "postmortems"
    if postmortems.exists():
        for md in sorted(postmortems.glob("*.md")):
            if md.name == "index.md":
                continue
            text = read_text(md)
            lowered = text.lower()
            is_one_off = any(marker in lowered for marker in ONE_OFF_MARKERS)
            has_guardrail_target = bool(
                re.search(
                    r"docs/current/rules/|verification\.md|manifest\.yaml|\.rules\.md|\brules\b|\.py\b",
                    text,
                )
            )
            if not has_guardrail_target and not is_one_off:
                warnings.append(
                    f"postmortem {rel(md, root)} names no concrete guardrail target (rule/verification/manifest/script) and is not marked one-off"
                )

    return errors, warnings


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("docs_root", help="Path to docs directory")
    parser.add_argument(
        "--stale-days",
        type=int,
        default=DEFAULT_STALE_DAYS,
        help=f"Flag current docs whose last_verified is older than this many days (default {DEFAULT_STALE_DAYS})",
    )
    args = parser.parse_args()

    root = Path(args.docs_root).resolve()
    if not root.exists() or not root.is_dir():
        print(f"ERROR: docs root does not exist: {root}", file=sys.stderr)
        return 2

    errors, warnings = check_docs(root, args.stale_days)

    for warning in warnings:
        print(f"WARN: {warning}")
    for error in errors:
        print(f"ERROR: {error}")

    if errors:
        print(f"doc runtime check failed: {len(errors)} error(s), {len(warnings)} warning(s)")
        return 1

    print(f"doc runtime check passed: {len(warnings)} warning(s)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
