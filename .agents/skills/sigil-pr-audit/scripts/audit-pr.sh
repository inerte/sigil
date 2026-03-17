#!/usr/bin/env bash
set -euo pipefail

base_ref="${1:-}"

if ! git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  echo "ERROR: run this script inside a git repository" >&2
  exit 1
fi

if [[ -z "${base_ref}" ]]; then
  if git rev-parse --verify origin/main >/dev/null 2>&1; then
    base_ref="origin/main"
  elif git rev-parse --verify main >/dev/null 2>&1; then
    base_ref="main"
  else
    echo "ERROR: no base ref supplied and neither origin/main nor main exists" >&2
    exit 1
  fi
fi

merge_base="$(git merge-base HEAD "${base_ref}")"
current_branch="$(git rev-parse --abbrev-ref HEAD)"

changed_files=()
while IFS= read -r line; do
  changed_files+=("${line}")
done < <(git diff --name-only "${merge_base}"...HEAD)

has_prefix() {
  local prefix="$1"
  shift
  local item
  for item in "$@"; do
    if [[ "${item}" == "${prefix}"* ]]; then
      return 0
    fi
  done
  return 1
}

classify_bucket() {
  local bucket="$1"
  case "${bucket}" in
    semantics)
      echo "language/compiler"
      ;;
    stdlib)
      echo "stdlib/examples/tests/docs"
      ;;
    ci)
      echo "ci/release/packaging"
      ;;
    project)
      echo "projects/website/tools"
      ;;
  esac
}

need_compiler_checks=false
need_language_checks=false
need_stdlib_checks=false
need_project_checks=false
need_site_checks=false
need_homebrew_checks=false

declare -a buckets=()

if ((${#changed_files[@]} > 0)); then
  if has_prefix "language/compiler/" "${changed_files[@]}"; then
    buckets+=("semantics")
    need_compiler_checks=true
  fi
  if has_prefix "language/stdlib/" "${changed_files[@]}" || has_prefix "language/examples/" "${changed_files[@]}" || has_prefix "language/tests/" "${changed_files[@]}" || has_prefix "language/test-fixtures/" "${changed_files[@]}" || has_prefix "language/docs/" "${changed_files[@]}" || has_prefix "language/spec/" "${changed_files[@]}"; then
    buckets+=("stdlib")
    need_language_checks=true
  fi
  if has_prefix ".github/workflows/" "${changed_files[@]}" || has_prefix "packaging/" "${changed_files[@]}"; then
    buckets+=("ci")
    need_homebrew_checks=true
  fi
  if has_prefix "projects/" "${changed_files[@]}" || has_prefix "website/" "${changed_files[@]}" || has_prefix "tools/" "${changed_files[@]}"; then
    buckets+=("project")
    need_project_checks=true
  fi
  if has_prefix "projects/ssg/" "${changed_files[@]}" || has_prefix "website/" "${changed_files[@]}" || [[ " ${changed_files[*]} " == *" .github/workflows/website.yml "* ]]; then
    need_site_checks=true
  fi
  if has_prefix "language/stdlib/" "${changed_files[@]}"; then
    need_stdlib_checks=true
  fi
fi

deduped_buckets=()
if ((${#buckets[@]} > 0)); then
  for bucket in "${buckets[@]}"; do
    already_seen=false
    for existing in "${deduped_buckets[@]}"; do
      if [[ "${existing}" == "${bucket}" ]]; then
        already_seen=true
        break
      fi
    done
    if [[ "${already_seen}" == false ]]; then
      deduped_buckets+=("${bucket}")
    fi
  done
fi

echo "== SIGIL PR AUDIT =="
echo "branch: ${current_branch}"
echo "base_ref: ${base_ref}"
echo "merge_base: ${merge_base}"
echo

echo "== DIFF STAT =="
git diff --stat "${merge_base}"...HEAD
echo

echo "== CHANGED FILES =="
if ((${#changed_files[@]} == 0)); then
  echo "(none)"
else
  printf '%s\n' "${changed_files[@]}"
fi
echo

echo "== CLASSIFICATION =="
if ((${#deduped_buckets[@]} == 0)); then
  echo "uncategorized"
else
  for bucket in "${deduped_buckets[@]}"; do
    echo "- $(classify_bucket "${bucket}")"
  done
fi
echo

echo "== HIGH-RISK FILES =="
high_risk_regex='^(\.github/workflows/|packaging/|tools/|language/compiler/|Cargo\.toml$|Cargo\.lock$|package\.json$|pnpm-lock\.yaml$)'
high_risk_found=false
if ((${#changed_files[@]} > 0)); then
  for file in "${changed_files[@]}"; do
    if [[ "${file}" =~ ${high_risk_regex} ]]; then
      echo "${file}"
      high_risk_found=true
    fi
  done
fi
if [[ "${high_risk_found}" == false ]]; then
  echo "(none)"
fi
echo

echo "== SUSPICIOUS ADDED LINES =="
suspicious_regex='(\bunsafe\b|Command::new|std::process|child_process|spawn\(|exec\(|execSync|curl |wget |fetch\(|axios|reqwest|TcpStream|UdpSocket|hyper|tokio::net|std::fs|fs::|File::create|OpenOptions|std::env|env::var|set_var|remove_var|github\.token|secrets\.|workflow_dispatch|pull_request_target)'
suspicious_output="$(git diff -U0 "${merge_base}"...HEAD | rg '^\+' -n | rg -v '^\+\+\+' | rg -n "${suspicious_regex}" || true)"
if [[ -n "${suspicious_output}" ]]; then
  printf '%s\n' "${suspicious_output}"
else
  echo "(none)"
fi
echo

echo "== CHECKS =="
echo "CHECK: git diff --name-only ${merge_base}...HEAD"
echo "CHECK: git diff --stat ${merge_base}...HEAD"
if [[ "${need_compiler_checks}" == true ]]; then
  echo "CHECK: cargo test --manifest-path language/compiler/Cargo.toml"
fi
if [[ "${need_language_checks}" == true ]]; then
  echo "CHECK: pnpm sigil:test:fixtures"
  echo "CHECK: pnpm sigil:test:language"
fi
if [[ "${need_stdlib_checks}" == true ]]; then
  echo "CHECK: pnpm sigil:test:stdlib"
fi
if [[ "${need_project_checks}" == true ]]; then
  echo "CHECK: pnpm sigil:test:algorithms"
  echo "CHECK: pnpm sigil:test:todo"
fi
if [[ "${need_site_checks}" == true ]]; then
  echo "CHECK: pnpm sigil:test:ssg"
fi
if [[ "${need_homebrew_checks}" == true ]]; then
  echo "CHECK: pnpm sigil:test:homebrew"
fi
echo

echo "== REVIEW QUESTIONS =="
echo "- What observable behavior changed?"
echo "- Which invariant is affected or at risk?"
echo "- Are any changed files unrelated to the stated goal?"
echo "- What proof is still missing before merge?"
