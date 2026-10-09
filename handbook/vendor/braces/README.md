# braces, with a depth limit

A copy of [braces](https://github.com/micromatch/braces) 3.0.3 that the
handbook's `package.json` puts in place of every `braces` in the tree
(an `overrides` entry). micromatch, chokidar and fast-glob, which Docusaurus
and webpack need, all load this copy.

braces 3.0.3 is the newest release and is affected by
[GHSA-vfj7-8cjw-p6xm](https://github.com/advisories/GHSA-vfj7-8cjw-p6xm):
its AST walkers recurse once per level of brace nesting with no limit, so a
pattern of a few thousand nested `{` overflows the stack. This copy changes
two things and nothing else:

- `parse()` refuses nesting deeper than 256 levels (`MAX_DEPTH` in
  `lib/constants.js`, lowered per call with `options.maxDepth`) with a
  `SyntaxError`, the same way it already refuses input over 10,000
  characters.
- `compile()`, `expand()` and `stringify()` stop at 512 levels, for an AST
  passed to them directly.

braces' own test suite (764 tests at the 3.0.3 release) passes against this
copy unchanged. `npm run check:braces` checks that the override is in effect
and that deep nesting is refused.

Drop this directory, the `braces` line in `devDependencies` and the one in
`overrides` once braces publishes a fixed release, then recompute
`npmDepsHash` in `flake/packages.nix`.

## Licence

The MIT License (MIT)

Copyright (c) 2014-present, Jon Schlinkert.

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in
all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
THE SOFTWARE.
