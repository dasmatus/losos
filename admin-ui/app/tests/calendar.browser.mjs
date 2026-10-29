/* LosOS Calendar (/calendar, a screen of the admin SPA inside its top bar
 * and sidebar) under the admin page's policy, with LosOS Cloud's
 * /cloud/api/calendar stood in by page.route the way
 * backend/src/cloud/calendar.rs answers. The clock is held at a Thursday
 * morning in Bratislava, so the week is always the same one. Walked once
 * each: the week with its events and the calendars column, the month and
 * the list, an event that is a video call (whose Join opens LosOS Chat), a
 * new event with a guest and a reminder, a subscription to a webcal://
 * address, an .ics import, a published calendar's feed address, the box
 * without LosOS Cloud (404), and the phone layout. Every page fails on a
 * CSP violation or an uncaught error. nix/tests/calendar.nix runs the same
 * page against a real LosOS Cloud and a real feed. */

import assert from './assert.mjs';
import { launch, runner, serve, SHELL_ME, shellRoutes, stash } from './harness.mjs';

const { origin, close: closeServer } = await serve({ csp: true });
const browser = await launch();
const { check, finish } = runner();

const json = (route, status, body) =>
  route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(body) });

const NOW = new Date('2026-10-22T09:30:00+02:00');
const ME = { ...SHELL_ME, shared: ['contacts', 'calendar', 'chat', 'mail'] };
const EVA = { email: 'eva@mattbox.home.arpa', name: 'Eva', matrix: '@eva:mattbox.local', status: 'ACCEPTED' };

function calendars() {
  return [
    { uri: 'personal', name: 'Personal', color: '#2563eb', count: 3, readOnly: false },
    {
      uri: 'webcal-taxes',
      name: 'Tax deadlines',
      color: '#dc2626',
      count: 1,
      readOnly: true,
      feed: { url: 'webcal://feeds.example/tax.ics', refresh: 86400, fetched: NOW.getTime() - 3600_000, ok: NOW.getTime() - 3600_000, error: '' },
    },
  ];
}

function event(id, calendar, summary, start, end, more = {}) {
  return {
    id,
    calendar,
    uid: `${id}@test`,
    summary,
    start,
    end,
    zone: 'Europe/Bratislava',
    allDay: start.length === 10,
    repeats: false,
    attendees: [],
    alarms: [],
    readOnly: calendar.startsWith('webcal-'),
    etag: `"${id}-1"`,
    ...more,
  };
}

function events() {
  return [
    event('e1', 'personal', 'Planning with Eva', '2026-10-22T11:00:00', '2026-10-22T12:00:00', {
      call: 'video',
      organizer: { email: 'matus@mattbox.home.arpa', name: 'Matus' },
      attendees: [EVA],
      alarms: [10],
      location: 'LosOS Chat',
    }),
    event('e2', 'personal', 'Bread from Jana', '2026-10-23T08:00:00', '2026-10-23T08:30:00'),
    event('e3', 'personal', 'Swimming', '2026-10-20T18:00:00', '2026-10-20T19:00:00', {
      repeats: true,
      repeat: { freq: 'weekly', days: ['TU'] },
      occurrence: '2026-10-20T18:00:00',
    }),
    event('t1', 'webcal-taxes', 'Income tax return', '2026-10-31', '2026-11-01'),
  ];
}

async function open(path = '/calendar', { cloud = true, viewport = { width: 1280, height: 860 } } = {}) {
  const context = await browser.newContext({ viewport, timezoneId: 'Europe/Bratislava', locale: 'en-US' });
  await context.clock.setFixedTime(NOW);
  const page = await context.newPage();
  const errors = [];
  const calls = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.addInitScript(() => {
    window.__violations = [];
    document.addEventListener('securitypolicyviolation', (e) => window.__violations.push(e.violatedDirective));
  });
  await shellRoutes(page, ME);
  await page.route('**/api/mail', (route) =>
    json(route, 200, { address: 'matus@mattbox.home.arpa', name: 'Matus', mailboxes: [], people: [] }),
  );
  let cals = calendars();
  let evs = events();
  await page.route('**/cloud/api/calendar/**', (route) => {
    const request = route.request();
    const url = new URL(request.url());
    const path = url.pathname.replace('/cloud/api/calendar', '');
    const method = request.method();
    if (!cloud) return json(route, 404, { error: 'not found' });
    calls.push({ method, path, search: url.search, body: request.postData() });
    if (path === '/calendars' && method === 'GET') return json(route, 200, { calendars: cals });
    if (path === '/events' && method === 'GET') {
      const from = url.searchParams.get('from');
      const to = url.searchParams.get('to');
      return json(route, 200, { events: evs.filter((e) => e.start.slice(0, 10) >= from && e.start.slice(0, 10) < to) });
    }
    if (path === '/events' && method === 'POST') {
      const f = JSON.parse(request.postData());
      const made = event(`n${evs.length}`, f.calendar, f.summary, f.start, f.end, {
        attendees: f.attendees,
        alarms: f.alarms,
        call: f.call || undefined,
        organizer: f.organizer,
        location: f.location || undefined,
        description: f.description || undefined,
      });
      evs = [...evs, made];
      return json(route, 200, made);
    }
    const one = path.match(/^\/events\/(\w+)$/);
    if (one && method === 'GET') return json(route, 200, evs.find((e) => e.id === one[1]));
    if (path === '/subscriptions' && method === 'POST') {
      const s = JSON.parse(request.postData());
      const made = {
        uri: 'webcal-timetable',
        name: s.name || 'Timetable (EduPage)',
        color: s.color,
        count: 12,
        readOnly: true,
        feed: { url: s.url, refresh: s.refresh, fetched: NOW.getTime(), ok: NOW.getTime(), error: '' },
      };
      cals = [...cals, made];
      evs = [...evs, event('s1', 'webcal-timetable', 'Mathematics', '2026-10-22T08:00:00', '2026-10-22T08:45:00')];
      return json(route, 200, { calendar: made, report: { added: 12, updated: 0, removed: 0, skipped: 0, leftOut: 0 } });
    }
    if (path === '/import' && method === 'POST') {
      cals = cals.map((c) => (c.uri === 'personal' ? { ...c, count: c.count + 2 } : c));
      return json(route, 200, { calendar: cals[0], report: { added: 2, updated: 0, removed: 0, skipped: 0, leftOut: 0 } });
    }
    if (path === '/calendars/personal/publish' && method === 'POST') {
      cals = cals.map((c) => (c.uri === 'personal' ? { ...c, published: '/cloud/feed/0123abcd.ics' } : c));
      return json(route, 200, cals[0]);
    }
    return json(route, 404, { error: `no fake for ${method} ${path}` });
  });
  await page.goto(origin + path, { waitUntil: 'networkidle' });
  /* Nothing is refused at load: Sonner's injected copy of its stylesheet is
   * dropped at build (vite.sonner-quiet.ts) and the bundle carries the file.
   * Nothing after that may add a violation either. */
  const atLoad = await page.evaluate(() => window.__violations);
  assert.deepEqual(atLoad, [], `CSP violations at load: ${JSON.stringify(atLoad)}`);
  const done = async () => {
    const violations = (await page.evaluate(() => window.__violations)).slice(atLoad.length);
    assert.deepEqual(violations, [], `CSP violations on the calendar page: ${JSON.stringify(violations)}`);
    assert.deepEqual(errors, [], 'uncaught errors on the calendar page');
    await context.close();
  };
  return { page, calls, done };
}

await check('the week shows the events of every calendar, with Calendar current in the sidebar', async () => {
  const { page, done } = await open();
  await page.getByRole('heading', { level: 1, name: 'Calendar' }).waitFor();
  const nav = page.getByRole('link', { name: 'Calendar', exact: true });
  assert.equal(await nav.getAttribute('aria-current'), 'page', 'Calendar is not current in the sidebar');
  await page.getByRole('button', { name: /Planning with Eva, video call/ }).waitFor();
  await page.getByRole('button', { name: /Bread from Jana/ }).waitFor();
  await page.getByRole('button', { name: /Swimming/ }).waitFor();
  const column = page.getByRole('region', { name: 'Calendars' });
  await column.getByText('Personal').waitFor();
  await column.getByText('Tax deadlines').waitFor();
  // The week is Monday 19 to Sunday 25 October.
  await page.getByText(/Oct(ober)? 19.*25, 2026/).first().waitFor();
  await done();
});

await check('the month and the list show the subscription too', async () => {
  const { page, done } = await open();
  await page.getByRole('button', { name: 'Month', exact: true }).click();
  await page.getByRole('button', { name: /Income tax return/ }).first().waitFor();
  await page.getByRole('button', { name: 'List', exact: true }).click();
  await page.getByRole('button', { name: /Income tax return/ }).first().waitFor();
  await page.getByRole('button', { name: /Bread from Jana/ }).first().waitFor();
  // Hiding a calendar takes its events away.
  await page.getByRole('button', { name: 'Hide Tax deadlines' }).click();
  await page.getByRole('button', { name: /Income tax return/ }).waitFor({ state: 'detached' });
  await page.getByRole('button', { name: 'Show Tax deadlines' }).click();
  await page.getByRole('button', { name: /Income tax return/ }).first().waitFor();
  await done();
});

await check('a call in the calendar joins in LosOS Chat, and its guest can be chatted with', async () => {
  const { page, done } = await open();
  await page.getByRole('button', { name: /Planning with Eva/ }).click();
  const dialog = page.getByRole('dialog');
  await dialog.getByRole('heading', { name: 'Planning with Eva' }).waitFor();
  await dialog.getByText('Eva').first().waitFor();
  await dialog.getByRole('button', { name: 'Chat with Eva' }).waitFor();
  await dialog.getByRole('button', { name: 'Join the call' }).click();
  await page.waitForURL(/\/chat\/\?with=%40eva%3Amattbox\.local&call=video|\/chat\/\?with=@eva:mattbox\.local&call=video/);
  await done();
});

await check('a new event with a guest, a reminder and a video call is saved', async () => {
  const { page, calls, done } = await open();
  await page.getByRole('button', { name: 'New event', exact: true }).click();
  const dialog = page.getByRole('dialog');
  await dialog.locator('#calendar-edit-summary').fill('Review the box');
  await dialog.locator('#calendar-edit-start').fill('2026-10-23');
  await dialog.getByLabel('Start time').fill('14:00');
  const guests = dialog.locator('#calendar-edit-guests');
  await guests.fill('eva@mattbox.home.arpa');
  await dialog.locator('#calendar-edit-alarm').selectOption('15');
  await dialog.locator('label[for="calendar-edit-call"]').click();
  await dialog.getByRole('button', { name: 'Save', exact: true }).click();
  await dialog.waitFor({ state: 'detached' });
  const post = calls.find((c) => c.method === 'POST' && c.path === '/events');
  assert.ok(post, 'no event was saved');
  const body = JSON.parse(post.body);
  assert.equal(body.summary, 'Review the box');
  assert.equal(body.start, '2026-10-23T14:00');
  assert.equal(body.end, '2026-10-23T15:00');
  assert.equal(body.zone, 'Europe/Bratislava');
  assert.deepEqual(body.alarms, [15]);
  assert.equal(body.call, 'video');
  assert.deepEqual(body.attendees.map((a) => a.email), ['eva@mattbox.home.arpa']);
  assert.equal(body.organizer?.email, 'matus@mattbox.home.arpa');
  assert.equal(body.organizer?.name, 'Matus');
  await page.getByRole('button', { name: /Review the box/ }).waitFor();
  await done();
});

await check('a webcal:// address is subscribed to and listed under Subscriptions', async () => {
  const { page, calls, done } = await open();
  await page.getByRole('button', { name: 'Subscribe', exact: true }).first().click();
  const dialog = page.getByRole('dialog');
  await dialog.locator('#calendar-subscribe-url').fill('webcal://school.example/timetable.ics');
  await dialog.locator('#calendar-subscribe-refresh').selectOption('21600');
  await dialog.getByRole('button', { name: 'Subscribe', exact: true }).click();
  await dialog.waitFor({ state: 'detached' });
  const post = JSON.parse(calls.find((c) => c.path === '/subscriptions').body);
  assert.equal(post.url, 'webcal://school.example/timetable.ics');
  assert.equal(post.refresh, 21600);
  await page.getByRole('region', { name: 'Calendars' }).getByText('Timetable (EduPage)').waitFor();
  await page.getByRole('button', { name: /Mathematics/ }).waitFor();
  await done();
});

await check('an .ics file is imported into a calendar', async () => {
  const { page, calls, done } = await open();
  await page.getByRole('button', { name: 'Import', exact: true }).click();
  const dialog = page.getByRole('dialog');
  await dialog.locator('#calendar-import-into').selectOption('personal');
  await dialog.locator('input[type="file"]').setInputFiles(await stash('holidays.ics', 'BEGIN:VCALENDAR\r\nVERSION:2.0\r\nEND:VCALENDAR\r\n'));
  await dialog.getByText(/2 new/).waitFor();
  assert.ok(calls.some((c) => c.path === '/import' && c.search.includes('calendar=personal')), JSON.stringify(calls));
  await done();
});

await check('a published calendar shows its webcal:// address', async () => {
  const { page, done } = await open();
  await page.getByRole('button', { name: 'Settings of Personal' }).click();
  const dialog = page.getByRole('dialog');
  await dialog.locator('[data-slot=switch]').click();
  const address = dialog.getByRole('textbox', { name: 'Subscription address' });
  await address.waitFor();
  assert.match(await address.inputValue(), /^webcal:\/\/127\.0\.0\.1:\d+\/cloud\/feed\/0123abcd\.ics$/);
  await done();
});

await check('without LosOS Cloud the page says so instead of failing', async () => {
  const { page, done } = await open('/calendar', { cloud: false });
  await page.getByRole('heading', { name: 'Calendar' }).waitFor();
  await page.getByText('There is no calendar on this box. Calendar runs wherever LosOS Cloud does.').waitFor();
  await done();
});

await check('on a phone the view comes first and the calendars are one tap away', async () => {
  const { page, done } = await open('/calendar', { viewport: { width: 390, height: 844 } });
  await page.getByRole('button', { name: /Planning with Eva/ }).waitFor();
  const width = await page.evaluate(() => document.documentElement.scrollWidth);
  assert.ok(width <= 390, `the page scrolls sideways: ${width}px`);
  await page.getByRole('button', { name: 'Calendars', exact: true }).click();
  await page.getByRole('region', { name: 'Calendars' }).getByText('Tax deadlines').waitFor();
  await done();
});

await closeServer();
await browser.close();
finish();
