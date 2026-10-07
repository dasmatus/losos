// Pandoc Typst template for the KOP hand-in PDF. Dollar-delimited variables are
// filled by pandoc (metadata.yaml and the Markdown body); everything else is
// plain Typst. Fonts are the ones Typst embeds (Libertinus Serif, DejaVu Sans
// Mono), so the build needs no font packages and renders the same in CI and
// on a laptop.
#let conf(
  title: none,
  subtitle: none,
  doctype: none,
  author: none,
  school: none,
  school-address: none,
  field: none,
  consultant: none,
  school-year: none,
  place: none,
  year: none,
  lang: "sk",
  doc,
) = {
  set document(title: "$title$", author: "$author$")
  set page(
    paper: "a4",
    margin: (top: 2.5cm, bottom: 2.5cm, left: 3cm, right: 2.5cm),
    numbering: "1",
    number-align: center,
  )
  set text(font: "Libertinus Serif", size: 11pt, lang: lang, hyphenate: true)
  set par(justify: true, leading: 0.65em)
  show raw: set text(font: "DejaVu Sans Mono", size: 9pt)
  show raw.where(block: true): it => block(
    width: 100%, fill: luma(245), inset: 8pt, radius: 3pt, it)
  show link: set text(fill: rgb("#0e6e7d"))
  set heading(numbering: "1.1")
  show heading.where(level: 1): it => {
    pagebreak(weak: true)
    v(1em)
    set text(size: 20pt, weight: "bold")
    block(below: 1.2em, it)
  }
  show heading.where(level: 2): it => {
    set text(size: 14pt, weight: "bold")
    block(above: 1.4em, below: 0.8em, it)
  }
  show heading.where(level: 3): it => {
    set text(size: 12pt, weight: "bold")
    block(above: 1.2em, below: 0.6em, it)
  }
  set table(stroke: 0.4pt + luma(170), inset: 5pt)
  // pandoc wraps every table in a figure; a figure is unbreakable unless told
  // otherwise, and a long table would leave most of a page empty before it.
  show figure.where(kind: table): set block(breakable: true)
  show table.cell.where(y: 0): set text(weight: "bold")
  show table: set text(size: 9.5pt)
  show figure.caption: set text(size: 9.5pt, style: "italic")
  set figure(gap: 0.6em)

  // Front matter: title page, declaration, table of contents. Set rules
  // apply to the rest of this block, so the page numbering is switched off
  // here and on again just before the body.
  set page(numbering: none)
  {
    set align(center)
    text(size: 13pt, weight: "bold")[#school]
    linebreak()
    text(size: 11pt)[#school-address]
    v(5cm)
    text(size: 14pt, tracking: 0.08em)[#upper(doctype)]
    v(1.5cm)
    text(size: 30pt, weight: "bold")[#title]
    v(0.6cm)
    text(size: 15pt)[#subtitle]
    v(1fr)
    grid(
      columns: (auto, auto),
      column-gutter: 1.5em,
      row-gutter: 0.6em,
      align: (right, left),
      [Autor:], [#author],
      [Konzultant:], [#if consultant == "" [#h(6cm)] else [#consultant]],
      [Študijný odbor:], [#if field == "" [#h(6cm)] else [#field]],
      [Školský rok:], [#school-year],
    )
    v(1.5cm)
    text(size: 12pt)[#place #year]
  }
  pagebreak()
  {
    v(1fr)
    text(size: 20pt, weight: "bold")[Čestné vyhlásenie]
    v(1em)
    [Vyhlasujem, že som komplexnú odbornú prácu vypracoval samostatne pod
    vedením konzultanta, s použitím uvedených zdrojov. Pri vývoji softvéru
    som používal nástroje asistované umelou inteligenciou; návrh systému,
    rozhodnutia o architektúre, recenzia a overenie každej zmeny sú mojou
    prácou a celá história vývoja je verejne dostupná v repozitári projektu.]
    v(2cm)
    grid(columns: (1fr, 1fr), [V #place, dňa ......................], align(right)[......................................... \ #author])
    v(1fr)
  }
  pagebreak()
  {
    show outline.entry.where(level: 1): it => {
      v(0.8em, weak: true)
      strong(it)
    }
    outline(title: [Obsah], depth: 2, indent: 1.2em)
  }
  pagebreak()
  set page(numbering: "1")
  counter(page).update(1)
  doc
}

#show: doc => conf(
  title: [$title$],
  subtitle: [$subtitle$],
  doctype: [$doctype$],
  author: [$author$],
  school: [$school$],
  school-address: [$school-address$],
  field: "$field$",
  consultant: "$consultant$",
  school-year: [$school-year$],
  place: [$place$],
  year: [$year$],
  lang: "$lang$",
  doc,
)

$body$
