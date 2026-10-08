#!/usr/bin/env bash
# Builds the hand-in PDF of the KOP (the Slovak technical documentation) from
# its Markdown pages under docs/project/kop/. Docusaurus leaves docs/project/
# out of both builds of the site (docusaurus.config.ts), so this PDF is the
# only thing made from them.
#
#   scripts/build-kop.sh [OUTPUT.pdf]        (default: build/kop/LosOS-KOP.pdf)
#
# Needs pandoc (3.x, for the Typst writer) and typst on PATH. The handbook
# workflow runs it as
#   nix shell --inputs-from . nixpkgs#pandoc nixpkgs#typst -c scripts/build-kop.sh
# so both come from the flake's own nixpkgs pin; the same line works locally.
#
# How the pages become one document:
#   1. the chapters are concatenated in the order below; each page's front
#      matter is dropped (its title is the page's own H1 anyway);
#   2. index.md is the front matter of the PDF: its sections become unnumbered
#      top-level sections (abstracts, the requirements map);
#   3. Docusaurus admonitions (:::info[Title] … :::) are rewritten to pandoc
#      fenced divs, which kop/filter.lua renders as a quoted block;
#   4. pandoc writes Typst using kop/template.typ (title page, declaration,
#      table of contents) and kop/metadata.yaml; typst compiles the PDF.
# Images are referenced relative to the pages (./img/…), so the Typst file is
# written next to a copy of img/.
set -euo pipefail

here=$(cd "$(dirname "$0")/.." && pwd)
src="$here/docs/project/kop"
out=${1:-"$here/build/kop/LosOS-KOP.pdf"}
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

chapters=(uvod funkcionality architektura kernel instalacia konfiguracia
  overenie cenova-ponuka dokumentacia zaver prilohy)

# Drop the YAML front matter at the top of a page.
strip_front_matter() {
  awk 'NR==1 && /^---$/ {fm=1; next} fm && /^---$/ {fm=0; next} !fm {print}' "$1"
}

# :::kind[Title]  ->  ::: {.admonition .kind title="Title"}   (closer stays :::)
admonitions() {
  sed -E -e 's/^:::([a-z]+)\[([^]]*)\][[:space:]]*$/::: {.admonition .\1 title="\2"}/' \
         -e 's/^:::([a-z]+)[[:space:]]*$/::: {.admonition .\1 title="\1"}/'
}

{
  # index.md: its sections become unnumbered top-level sections.
  strip_front_matter "$src/index.md" \
    | sed -E -e 's/^## /# /' -e 's/^# (.*)$/# \1 {.unnumbered}/' \
    | admonitions
  printf '\n\n'
  for c in "${chapters[@]}"; do
    strip_front_matter "$src/$c.md" | admonitions
    printf '\n\n'
  done
} > "$work/kop.md"

cp -r "$src/img" "$work/img"

pandoc "$work/kop.md" \
  --from markdown+pipe_tables+fenced_divs+implicit_figures \
  --to typst \
  --template "$here/kop/template.typ" \
  --metadata-file "$here/kop/metadata.yaml" \
  --lua-filter "$here/kop/filter.lua" \
  --resource-path "$work" \
  --output "$work/kop.typ"

mkdir -p "$(dirname "$out")"
typst compile --root "$work" "$work/kop.typ" "$out"
echo "wrote $out"
