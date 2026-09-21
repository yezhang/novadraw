#!/usr/bin/env bash
set -euo pipefail

readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
readonly BOOK_DIR="${ROOT_DIR}/book"
readonly OUTPUT="${ROOT_DIR}/target/book/pdf/output.pdf"

for command in mdbook mdbook-mermaid mdbook-pdf; do
    if ! command -v "${command}" >/dev/null 2>&1; then
        printf '缺少 %s；请先执行: cargo install %s --locked\n' "${command}" "${command}" >&2
        exit 1
    fi
done

mdbook build "${BOOK_DIR}"

if [[ ! -f "${OUTPUT}" ]]; then
    printf 'PDF 构建未生成预期输出: %s\n' "${OUTPUT}" >&2
    exit 1
fi

printf 'PDF 已生成: %s\n' "${OUTPUT}"
