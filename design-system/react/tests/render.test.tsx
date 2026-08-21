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

// Task 11: Display components
import {
  Card, StatusDot, Chip, ChipRow, Desc, HintCard, SysList, SysRow,
} from '../src/index';

expectMarkup(
  <Card title="Nextcloud" status={<StatusDot state="up" label="running" />}>
    <Desc>Files</Desc>
    <ChipRow><Chip>/nextcloud</Chip></ChipRow>
  </Card>,
  'class="card"', 'role="listitem"', 'class="card-head"', '<h2>Nextcloud</h2>',
  'class="status"', 'data-state="up"', 'class="desc"', 'class="chip-row"', 'class="chip"');
expectMarkup(<HintCard>Sign in first.</HintCard>, 'class="hint-card"');
expectMarkup(<SysList><SysRow label="Hostname"><Chip>losos</Chip></SysRow></SysList>,
  'class="syslist"', 'class="sysrow"', '<dt>Hostname</dt>', '<dd>');

console.log('render tests: OK');
