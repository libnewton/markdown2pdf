#!/bin/sh
set -eu

root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
output=$(mktemp -d "${TMPDIR:-/tmp}/md2pdf-tests.XXXXXX")
trap 'rm -rf "$output"' EXIT

for source in "$root"/tests/*.md; do
  name=$(basename "$source" .md)
  "$root/bin/md2pdf" "$source" "$output/$name.pdf"
  "$root/bin/md2pdf" "$source" "$output/$name.html"
done

python3 "$root/tests/check_html.py" "$output/html-edge.html"
