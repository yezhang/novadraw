#!/usr/bin/env bash
set -euo pipefail

readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
readonly USER_BOOK_DIR="${ROOT_DIR}/book"
readonly INTERNALS_BOOK_DIR="${ROOT_DIR}/book/internals"
readonly CONTRIBUTOR_BOOK_DIR="${ROOT_DIR}/book/contributor"
readonly USER_OUTPUT="${ROOT_DIR}/target/book/user/pdf/output.pdf"
readonly INTERNALS_OUTPUT="${ROOT_DIR}/target/book/internals/pdf/output.pdf"
readonly CONTRIBUTOR_OUTPUT="${ROOT_DIR}/target/book/contributor/pdf/output.pdf"

for command in mdbook mdbook-mermaid mdbook-pdf; do
    if ! command -v "${command}" >/dev/null 2>&1; then
        printf '缺少 %s；请先执行: cargo install %s --locked\n' "${command}" "${command}" >&2
        exit 1
    fi
done

mdbook build "${USER_BOOK_DIR}"
mdbook build "${INTERNALS_BOOK_DIR}"
mdbook build "${CONTRIBUTOR_BOOK_DIR}"

if [[ ! -f "${USER_OUTPUT}" ]]; then
    printf '用户指南 PDF 构建未生成预期输出: %s\n' "${USER_OUTPUT}" >&2
    exit 1
fi

if [[ ! -f "${CONTRIBUTOR_OUTPUT}" ]]; then
    printf '贡献者指南 PDF 构建未生成预期输出: %s\n' "${CONTRIBUTOR_OUTPUT}" >&2
    exit 1
fi

if [[ ! -f "${INTERNALS_OUTPUT}" ]]; then
    printf '深入理解书 PDF 构建未生成预期输出: %s\n' "${INTERNALS_OUTPUT}" >&2
    exit 1
fi

printf '用户指南 PDF 已生成: %s\n' "${USER_OUTPUT}"
printf '深入理解书 PDF 已生成: %s\n' "${INTERNALS_OUTPUT}"
printf '贡献者指南 PDF 已生成: %s\n' "${CONTRIBUTOR_OUTPUT}"
