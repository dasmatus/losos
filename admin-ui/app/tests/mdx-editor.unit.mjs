/* LosOS Office's document model (src/mdx/), checked without a browser:
 * MDX is read into the editor's nodes and written back.
 *
 * What is checked: a document read and written untouched comes back byte
 * for byte; every block and component, written afresh, reads back as the
 * same document (the writer's output goes through losos-mdx again, so a
 * form it would refuse or misread fails here); text with Markdown's and
 * MDX's special characters stays text; a changed block is the only part
 * of the file that changes; and props that are not the component's keep
 * the element as raw source instead of losing them; and a new document
 * or presentation starts as the text the editor would write for it.
 *
 *   bun tests/mdx-editor.unit.mjs     (needs src/mdx/pkg: bun run mdx)
 */

import assert from './assert.mjs';

const pkg = new URL('../src/mdx/pkg/losos_mdx.js', import.meta.url).pathname;

// Vite turns `…_bg.wasm?url` into an address; here the module is started
// from bytes below, so the import only has to resolve.
Bun.plugin({
  name: 'mdx-wasm-url',
  setup(b) {
    b.onResolve({ filter: /\.wasm\?url$/ }, () => ({ path: 'wasm-url', namespace: 'stub' }));
    b.onLoad({ filter: /.*/, namespace: 'stub' }, () => ({ contents: 'export default ""', loader: 'js' }));
  },
});

// One copy of the package, shared with src/mdx/wasm.ts, so initSync below is
// the instance the model calls.
const wasm = await import(pkg);
wasm.initSync({ module: await Bun.file(pkg.replace(/\.js$/, '_bg.wasm')).bytes() });

const { parseMdx } = await import('../src/mdx/wasm.ts');
const { fromMdast } = await import('../src/mdx/from-mdast.ts');
const { toMdx, block } = await import('../src/mdx/to-mdx.ts');
const { schema } = await import('../src/mdx/schema.ts');
const { starterText } = await import('../src/cloud/office/paper/new.ts');

const read = (text) => {
  const { tree, problems } = parseMdx(text, 'mdx');
  return { doc: fromMdast(tree, text), problems };
};

/** The document with every block's remembered text forgotten, so the
 * writer writes all of it. */
const fresh = (doc) => schema.nodeFromJSON(doc.toJSON());

let failed = 0;
const check = (name, fn) => {
  try {
    fn();
    console.log(`ok   ${name}`);
  } catch (e) {
    failed++;
    console.log(`FAIL ${name}\n${e.stack ?? e}`);
  }
};

const EVERYTHING = `---
title: Trip
page:
  size: A4
---

# Trip to *Žiar* nad Hronom

Some **bold**, *em*, ~~gone~~, \`code\`, [a link](https://example.org "Ex"), and an ![img](cat.png).
A hard break follows\\
here, and <u>under</u>, H<sub>2</sub>O, x<sup>2</sup>, <mark>marked</mark>, <small>small</small>, <kbd>Ctrl</kbd>, <abbr title="LosOS">L</abbr>.

Ask <Mention login="eva" /> by <Date value="2026-08-14" format="long" />, see <Comment text="Which &quot;cabin&quot;?" author="eva" date="2026-10-28">the cabin</Comment>, page <PageNumber />, maths $a^2$ and <Equation tex="b" inline />, note[^n].

> A quote
>
> - with a list

- one
- two
  - nested
- [x] done
- [ ] todo

3. three
4. four

1. loose

2. list

\`\`\`rust
fn main() {}
\`\`\`

$$
x^2
$$

<Equation tex="e = mc^2" />

---

| a | b | c |
|:-|-:|:-:|
| 1 | **2** | x |

<Table caption="Budget">
<Row header><Cell>Item</Cell><Cell align="right">EUR</Cell></Row>
<Row><Cell colSpan="2">Cabin</Cell></Row>
</Table>

<TableOfContents title="Contents" />

<Callout type="tip" title="Hint">
Body

- a list
</Callout>

<Details summary="More" open>
Hidden
</Details>

<Columns>
<Column>
Left
</Column>
<Column>
Right
</Column>
</Columns>

<Tabs>
<Tab label="First">
A
</Tab>
<Tab label="Second">
B
</Tab>
</Tabs>

<Figure src="cat.png" alt="A cat" caption="Our cat" />

<Chart src="budget.sheet" range="A1:B5" type="pie" title="Where it goes" />

<PageBreak />

<Header>Trip plan</Header>

<Footer>Page <PageNumber /></Footer>

<Slide layout="title">
# Hello

<Notes>
Say hello.
</Notes>
</Slide>

Line with <Embed href="https://example.org" title="Example" /> inline.

[^n]: The note.
`;

check('a document read and written untouched is the same text', () => {
  const { doc, problems } = read(EVERYTHING);
  assert.deepEqual(problems, []);
  assert.equal(toMdx(doc), EVERYTHING);
});

check('nothing in the sample became raw source', () => {
  const { doc } = read(EVERYTHING);
  const raw = [];
  doc.descendants((n) => {
    if (n.type.name === 'raw' || n.type.name === 'raw_inline') raw.push(n.attrs.source);
  });
  assert.deepEqual(raw, []);
});

check('every block written afresh reads back as the same document', () => {
  const { doc } = read(EVERYTHING);
  const written = toMdx(fresh(doc));
  const again = read(written);
  assert.deepEqual(again.problems, [], written);
  assert.deepEqual(again.doc.toJSON(), doc.toJSON(), written);
  if (Bun.env.SHOW) console.log(written);
  // And it is stable: writing that again changes nothing.
  assert.equal(toMdx(fresh(again.doc)), written);
});

check('special characters stay text', () => {
  const text = 'a *b* _c_ snake_case `d` [e] <f> {g} ~h~ $i$ |j| & &amp; \\ # x';
  const p = schema.nodes.paragraph.create(null, schema.text(text));
  const lines = ['# heading?', '- not a list', '1. not a list', '+ nor', '> nor a quote', '=== nor'];
  const doc = schema.nodes.doc.create(null, [p, ...lines.map((l) => schema.nodes.paragraph.create(null, schema.text(l)))]);
  const back = read(toMdx(doc));
  assert.deepEqual(back.problems, []);
  assert.deepEqual(back.doc.toJSON(), doc.toJSON(), toMdx(doc));
});

check('marks with spaces at their edges still read back', () => {
  const s = schema.marks.strong.create();
  const doc = schema.nodes.doc.create(null, schema.nodes.paragraph.create(null, [schema.text('a'), schema.text(' bold ', [s]), schema.text('b')]));
  const written = toMdx(doc);
  assert.equal(written, 'a **bold** b\n');
});

check('a changed block is the only part of the file that changes', () => {
  const source = '# Title\n\nSome   *odd*   spacing.\n\n* star list\n* kept\n\nlast\n';
  const { doc } = read(source);
  // Replace the last paragraph, as typing in it would.
  const changed = doc.copy(doc.content.replaceChild(doc.childCount - 1, schema.nodes.paragraph.create(null, schema.text('last, edited'))));
  assert.equal(toMdx(changed), '# Title\n\nSome   *odd*   spacing.\n\n* star list\n* kept\n\nlast, edited\n');
});

check('a component with props it does not take stays as its source', () => {
  const source = '<Callout type="danger" title="x">\nBody\n</Callout>\n\n<Figure src="a.png" onClick="x" />\n';
  const { doc } = read(source);
  assert.equal(doc.child(0).type.name, 'raw');
  assert.equal(doc.child(1).type.name, 'raw');
  assert.equal(toMdx(fresh(doc)), source);
});

check('tables with merged cells or a caption are written as components', () => {
  const { doc } = read('| a | b |\n|-|-|\n| 1 | 2 |\n');
  const table = doc.child(0);
  assert.equal(block(table), '| a | b |\n| --- | --- |\n| 1 | 2 |');
  const captioned = table.type.create({ caption: 'Cap' }, table.content);
  assert.match(block(captioned), /^<Table caption="Cap">\n<Row header><Cell>a<\/Cell><Cell>b<\/Cell><\/Row>/);
});

check('an empty document is one empty paragraph', () => {
  const { doc } = read('');
  assert.equal(doc.childCount, 1);
  assert.equal(doc.child(0).type.name, 'paragraph');
});

check('a new document and presentation are what the editor writes', () => {
  const words = { name: 'x', title: 'Plan *1* <b>', subtitle: 'Matus, today', next: 'First_point', point: '#1 & more' };
  for (const kind of ['paper', 'slides']) {
    const text = starterText(kind, words);
    const { doc, problems } = read(text);
    assert.deepEqual(problems, []);
    doc.descendants((n) => assert.ok(!n.type.name.startsWith('raw'), `${kind}: ${n.type.name}`));
    assert.equal(toMdx(fresh(doc)), text, kind);
  }
  const { doc } = read(starterText('slides', words));
  assert.deepEqual(
    [doc.childCount, doc.child(0).type.name, doc.child(0).attrs.layout, doc.child(1).attrs.layout],
    [2, 'slide', 'title', 'content'],
  );
  assert.equal(doc.child(0).child(0).textContent, 'Plan *1* <b>');
});

if (failed > 0) {
  // Thrown: an uncaught error is what makes Bun exit non-zero.
  throw new Error(`${failed} failed`);
}
console.log('mdx editor model: all passed');
