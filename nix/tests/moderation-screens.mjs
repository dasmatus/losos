/* The roles and moderation screens, from the laptop of
 * nix/tests/moderation.nix.
 *
 *     node moderation-screens.mjs after <dark|light> <out dir>
 *     node moderation-screens.mjs before <dark|light> <out dir>
 *     node moderation-screens.mjs handbook <en-US|sk-SK|de-DE> <out dir>
 *
 * `after` walks what an owner sees: the Users pane with everyone's role,
 * Access on a person with the role picker, the table of what each role can
 * do and the box's own roles, then the Moderation pane: the queue, a spam
 * message held, a mute typed in and given, and the log. `before` is a box
 * without roles: the Users pane and Access as they were, at the same size
 * and in the same state, for a pull request's Before column. `handbook`
 * takes the handbook's pictures in light, in the given language, without
 * changing anything on the box. */

import { Shots } from './screens.mjs';

const [MODE, ARG, OUT] = Bun.argv.slice(2);
const BASE = 'https://mattbox.local';
const PASSWORD = 'Correct-Horse-Battery-9';

const handbook = MODE === 'handbook';
const shots = new Shots(OUT, {
  feature: 'moderation',
  test: 'losos-moderation-screens',
  base: BASE,
  scheme: handbook ? 'light' : ARG,
  locale: handbook ? ARG : 'en-US',
});

const row = (page, name) => page.locator(`[data-testid=users-person][data-person=${name}]`);

async function users(page) {
  await page.goto(shots.url('/settings/users'));
  await page.getByTestId('users-people').waitFor();
}

async function moderation(page) {
  await page.goto(shots.url('/settings/moderation'));
  await page.getByTestId('moderation-reports').waitFor();
}

/* The Access dialog on Eva's row, the same in both columns. */
async function access(page) {
  const button = row(page, 'eva').getByRole('button', { name: 'Access' });
  await shots.take(page, 'users', 'the Users pane, everyone with their access', {
    action: { kind: 'click', target: 'access' },
    targets: { people: page.getByTestId('users-people'), access: button },
  });
  await button.click();
  const dialog = page.getByRole('dialog');
  await dialog.waitFor();
  return dialog;
}

const page = await shots.open();
await shots.signIn(page, 'matus', PASSWORD);

if (handbook) {
  // The handbook's pictures, at the handbook's laptop width: the role table
  // on its own and the whole Moderation pane. The Users pane's picture is
  // the one the handbook already has.
  const suffix = { 'en-US': '', 'sk-SK': '-sk', 'de-DE': '-de' }[ARG];
  await users(page);
  const table = page.getByTestId('users-role-table');
  await table.scrollIntoViewIfNeeded();
  await shots.settle(page);
  await table.screenshot({ path: `${OUT}/settings-roles${suffix}.png` });
  await moderation(page);
  await shots.settle(page);
  await page.screenshot({ path: `${OUT}/settings-moderation${suffix}.png`, fullPage: true });
  await shots.finish();
} else {
  await users(page);

  if (MODE === 'before') {
    const dialog = await access(page);
    await shots.take(page, 'access', 'Access on Eva', { targets: { dialog } });
  } else {
    const dialog = await access(page);
    const select = dialog.getByTestId('users-role-select');
    await shots.take(page, 'access', 'Access on Eva: her role is one of the box\'s own', {
      action: { kind: 'click', target: 'role' },
      targets: { dialog, role: select },
    });
    await dialog.getByRole('button', { name: 'Cancel' }).click();
    await dialog.waitFor({ state: 'hidden' });

    const table = page.getByTestId('users-role-table');
    await table.scrollIntoViewIfNeeded();
    await shots.take(page, 'roles', 'what each role can do, and the roles of the box\'s own', {
      targets: { table, custom: page.getByTestId('users-roles') },
    });

    await moderation(page);
    const spam = page.locator('[data-testid=moderation-report][data-reason=spam][data-state=open]');
    const hold = spam.getByTestId('moderation-hide');
    await shots.take(page, 'queue', 'the Moderation pane: a report about a child first, then spam in mail', {
      action: { kind: 'click', target: 'hold' },
      targets: {
        child: page.locator('[data-testid=moderation-report][data-reason=child][data-state=open]'),
        spam,
        hold,
      },
    });
    await hold.click();
    const held = page.locator('[data-testid=moderation-report][data-reason=spam][data-state=hidden]');
    await held.waitFor();
    await shots.toastsGone(page);
    await shots.take(page, 'held', 'the message is held, and can be let go', {
      targets: { held, release: held.getByTestId('moderation-release') },
    });

    const tom = page.locator('[data-testid=moderation-person][data-person=tom]');
    const mute = tom.getByTestId('moderation-mute');
    await tom.scrollIntoViewIfNeeded();
    await shots.take(page, 'people', 'the people a moderator can hold', {
      action: { kind: 'click', target: 'mute' },
      targets: { people: page.getByTestId('moderation-people'), mute },
    });
    await mute.click();
    const box = page.getByRole('dialog', { name: 'Mute Tom' });
    await box.waitFor();
    await box.getByTestId('moderation-duration').selectOption('week');
    const reason = box.getByTestId('moderation-reason');
    await reason.click();
    let typed = '';
    for (const [i, word] of 'The same offer to everyone, twelve times'.split(' ').entries()) {
      const part = i === 0 ? word : ` ${word}`;
      await reason.pressSequentially(part);
      typed += part;
      await shots.take(page, `mute-${i + 1}`, `the mute dialog: “${typed}”`, {
        action: { kind: 'type', target: 'reason' },
        targets: { dialog: box, reason },
      });
    }
    const confirm = box.getByTestId('moderation-hold-confirm');
    await shots.take(page, 'mute-ready', 'a week, with the reason for the other moderators', {
      action: { kind: 'click', target: 'confirm' },
      targets: { dialog: box, confirm },
    });
    await confirm.click();
    await box.waitFor({ state: 'hidden' });
    await tom.getByTestId('moderation-muted').waitFor();
    await shots.toastsGone(page);
    await shots.take(page, 'muted', 'Tom is muted for a week', {
      targets: { tom, badge: tom.getByTestId('moderation-muted') },
    });

    const log = page.getByTestId('moderation-log');
    // The log is taller than the window: start at its title, under the top bar.
    await log.evaluate((el) => {
      el.style.scrollMarginTop = '80px';
      el.scrollIntoView({ block: 'start' });
    });
    await shots.take(page, 'log', 'what moderators did', { targets: { log } });
  }

  await shots.finish();
}
