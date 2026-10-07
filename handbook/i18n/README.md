# Translations

`code.json` and the two `docusaurus-theme-classic/*.json` files hold the
interface strings for each locale; `docusaurus-plugin-content-docs/current.json`
the sidebar category names. All of them are filled in for Slovak and German.

The pages themselves are translated by copying a page from `../docs/` to
`<locale>/docusaurus-plugin-content-docs/current/<same path>` and translating
the copy; a page without a copy shows in English. Refresh the string files
with `npm run write-translations -- --locale <sk|de>` after adding a
category or a navbar item.
