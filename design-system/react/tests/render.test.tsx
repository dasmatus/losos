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

// Task 10: Chrome & shell components
import {
  Topbar, Indbar, Container, Grid, Layout, Sidebar, SideItem,
  Content, ContentHeader, Section,
} from '../src/index';

expectMarkup(<Topbar brand="losos"><a href="/settings">Settings</a></Topbar>,
  'class="topbar"', 'class="brand"', 'class="topnav"', '>losos<');
expectMarkup(<Indbar />, 'class="indbar"', 'role="progressbar"');
expectMarkup(<Container>x</Container>, 'class="container"');
expectMarkup(<Grid>x</Grid>, 'class="grid"', 'role="list"');
expectMarkup(<Layout>x</Layout>, 'class="layout"');
expectMarkup(<Sidebar><SideItem active>General</SideItem><SideItem danger>Danger</SideItem></Sidebar>,
  'class="sidebar"', 'class="side-list"', 'side-item is-active', 'side-item side-item--danger');
expectMarkup(<Content><ContentHeader title="Sharing" /></Content>,
  'class="content"', 'class="content-header"', '<h1>Sharing</h1>');
expectMarkup(<Section active>panel</Section>, 'section is-active');

console.log('render tests: OK');
