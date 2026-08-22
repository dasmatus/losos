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

// Task 12: Feedback components
import { Banner, ErrorBanner, ProgressCard, Spinner } from '../src/index';

expectMarkup(<Banner kind="building" title="Rebuilding…" message="step 3/7" />,
  'class="banner"', 'data-kind="building"', 'class="banner-row"',
  'class="banner-msg"', 'class="indbar"');
expectMarkup(<ErrorBanner onClose={() => {}}>Request failed</ErrorBanner>,
  'class="error-banner"', 'class="banner-close"');
expectMarkup(<ProgressCard kind="failed" title="Rebuild failed" log="exit 1" />,
  'class="progress-card"', 'data-kind="failed"', 'class="progress-head"',
  'progress-icon spinner', 'class="progress-log"');
expectMarkup(<Spinner />, 'progress-icon spinner');

// Task 13: Form, group & overlay components
import {
  Button, Group, Row, GroupHint, Switch, Input, Select, AuthOverlay, AuthCard,
} from '../src/index';

expectMarkup(<Button>Apply</Button>, 'class="btn-primary"');
expectMarkup(<Button variant="danger">Reset…</Button>, 'class="btn-danger"');
expectMarkup(<Button variant="outline" href="/nextcloud">Open</Button>,
  'class="btn-outline"', '<a ');
expectMarkup(<Button block>Unlock</Button>, 'btn-primary btn-block');
expectMarkup(
  <Group danger><Row><Switch checked label="Share my storage" /></Row><GroupHint>hint</GroupHint></Group>,
  'group group--danger', 'class="row"', 'class="switch-row"', 'class="row-label"',
  'class="switch"', 'checked', 'class="switch-track"', 'class="group-hint"');
expectMarkup(<Row><Input type="text" defaultValue="losos" /><Select><option>local</option></Select></Row>,
  '<input type="text"', '<select');
expectMarkup(
  <AuthOverlay><AuthCard title="Unlock" hint="Paste the admin token." error="Wrong token."><Input type="password" /></AuthCard></AuthOverlay>,
  'class="auth-overlay"', 'class="auth-card"', '<h2>Unlock</h2>',
  'class="auth-hint"', 'class="auth-error"');

console.log('render tests: OK');
