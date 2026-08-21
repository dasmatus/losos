import { renderToStaticMarkup } from 'react-dom/server';
import assert from 'node:assert';
import type { ReactElement } from 'react';

export function expectMarkup(el: ReactElement, ...needles: string[]) {
  const html = renderToStaticMarkup(el);
  for (const n of needles) {
    assert.ok(html.includes(n), `expected ${JSON.stringify(n)} in:\n${html}`);
  }
  return html;
}

// Tasks 10-13 append component assertions below this line.
console.log('render tests: OK');
