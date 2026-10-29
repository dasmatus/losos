/* losos-mdx's browser package, loaded the way a page loads it, against the
 * documents the crate's own suite uses: the same HTML, the same refusals,
 * and the limits holding inside WebAssembly's 1 MiB stack. Run by
 * nix/tests/mdx-wasm.nix on Bun, with the package's directory and the
 * admin UI's assertions (admin-ui/app/tests/assert.mjs) as its arguments. */

const [dir, asserts] = Bun.argv.slice(2);
const { default: assert } = await import(asserts);
const { default: init, render, tree } = await import(`${dir}/losos_mdx.js`);
await init({ module_or_path: await Bun.file(`${dir}/losos_mdx_bg.wasm`).bytes() });

const vault = {
  note: (t) => (t === 'trip.note' ? 'notes/trip.note' : undefined),
  file: (t) => `/cloud/files/${t}`,
  href: (p, s) => `/notes/${p}${s ? `#${s}` : ''}`,
};
const r = (src, flavour = 'mdx', resolver) => JSON.parse(render(src, flavour, resolver));

// A document with components draws as the native build draws it.
const doc = r('---\ntitle: Trip\n---\n# Trip\n\n<Callout type="tip" title="Hint">\nPack [the plan](trip.note#Day) and ![x](cat.png).\n</Callout>\n', 'mdx', vault);
assert.equal(
  doc.html,
  '<h1 id="trip">Trip</h1><aside data-callout="tip" class="callout callout-tip"><p class="callout-title">Hint</p>'
    + '<p>Pack <a href="/notes/notes/trip.note#Day" rel="noopener noreferrer">the plan</a> and '
    + '<img src="/cloud/files/cat.png" alt="x">.</p></aside>',
);
assert.deepEqual(doc.properties, { title: 'Trip' });
assert.deepEqual(doc.problems, []);

// Nothing runs and nothing gets through.
for (const attack of [
  '{alert(1)}',
  'import x from "y"',
  '<Callout onClick="alert(1)">x</Callout>',
  '<Callout title={alert(1)}>x</Callout>',
  '<script>alert(1)</script>',
  '<Figure src="javascript:alert(1)" />',
  '[x](javascript:alert(1))',
]) {
  const out = r(attack);
  assert.ok(out.problems.length > 0, `${attack} was refused silently`);
  assert.doesNotMatch(out.html, /<script|onclick|javascript:/i, attack);
}
const md = r('<img src=x onerror=alert(1)><div style="position:fixed" class="fixed inset-0">x</div>', 'md');
assert.doesNotMatch(md.html, /onerror|style=|fixed|inset/, md.html);

// Errors are thrown with their place, and the limits hold in a wasm stack.
assert.throws(() => render('<Callout>\nnever closed', 'mdx'), /line \d+/);
assert.throws(() => render('>'.repeat(20000) + ' x', 'md'), /nested more than 100/);
assert.throws(() => render('<Callout>'.repeat(5000) + 'x', 'mdx'), /nested more than 100|closing tag/);
assert.throws(() => render('a'.repeat(1024 * 1024 + 1), 'md'), /bytes/);
assert.ok(r('>'.repeat(90) + ' x', 'md').html.includes('x'));

// The tree the editor builds from.
const t = JSON.parse(tree('# A\n\n<Callout type="tip">\nb\n</Callout>', 'mdx'));
assert.equal(t.tree.children[1].type, 'mdxJsxFlowElement');
assert.equal(t.tree.children[1].name, 'Callout');

console.log('losos-mdx wasm: ok');
