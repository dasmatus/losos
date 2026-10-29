/* The profile screens, from the laptop of nix/tests/profile-screens.nix.
 *
 *     node profile-screens.mjs <after|before> <dark|light> <out dir> <pictures dir>
 *
 * `after` walks the whole thing: Your account with no picture, the crop
 * dialog for a picture and for a banner, the colour, pronouns and line typed
 * in, the finished profile, the picture in the top bar and in the Users
 * pane. `before` is a box without profiles: Your account and the Users pane
 * as they were, at the same size and in the same state, for a pull
 * request's Before column. */

import { Shots } from './screens.mjs';

const [MODE, SCHEME, OUT, PICTURES] = Bun.argv.slice(2);
const BASE = 'https://mattbox.local';
const PASSWORD = 'Correct-Horse-Battery-9';

const shots = new Shots(OUT, { feature: 'profiles', test: 'losos-profile-screens', base: BASE, scheme: SCHEME });

async function account(page) {
  await page.goto(shots.url('/settings/account'));
  await page.getByTestId('account-details').waitFor();
}

async function users(page) {
  await page.goto(shots.url('/settings/users'));
  await page.getByTestId('users-people').waitFor();
}

/* Open the crop dialog for `kind` with `picture`, move it, and use it. */
async function crop(page, kind, picture, slug, title) {
  const chooser = page.waitForEvent('filechooser');
  await page.getByTestId(`profile-${kind}-choose`).click();
  await (await chooser).setFiles(picture);
  const canvas = page.getByTestId('profile-crop-canvas');
  await canvas.waitFor();
  await page.locator('[data-testid=profile-crop-save]:not([disabled])').waitFor();
  const zoom = page.getByTestId('profile-crop-zoom');
  await zoom.fill('1.2');
  const b = await canvas.boundingBox();
  await page.mouse.move(b.x + b.width / 2, b.y + b.height / 2);
  await page.mouse.down();
  await page.mouse.move(b.x + b.width / 2 + 18, b.y + b.height / 2 + 10, { steps: 4 });
  await page.mouse.up();
  const save = page.getByTestId('profile-crop-save');
  await shots.take(page, slug, title, {
    action: { kind: 'click', target: 'use' },
    targets: { use: save, frame: canvas, zoom },
  });
  await save.click();
  await canvas.waitFor({ state: 'hidden' });
  await shots.toastsGone(page);
}

/* Type `text` a word at a time, one shot per word. */
async function typeWords(page, testid, text, slug, title) {
  const field = page.getByTestId(testid);
  await field.click();
  let typed = '';
  for (const [i, word] of text.split(' ').entries()) {
    const part = i === 0 ? word : ` ${word}`;
    await field.pressSequentially(part);
    typed += part;
    await shots.take(page, `${slug}-${i + 1}`, `${title}: “${typed}”`, {
      action: { kind: 'type', target: 'field' },
      targets: { field },
    });
  }
}

const page = await shots.open();
await shots.signIn(page, 'matus', PASSWORD);
await account(page);

if (MODE === 'before') {
  await shots.take(page, 'account', 'Your account', { targets: { details: page.getByTestId('account-details') } });
  await users(page);
  await shots.take(page, 'users', 'the Users pane', { targets: { people: page.getByTestId('users-people') } });
} else {
  const section = page.getByTestId('account-profile');
  await section.waitFor();
  await shots.take(page, 'account', 'Your account, with no picture yet', {
    action: { kind: 'click', target: 'choose' },
    targets: { profile: section, choose: page.getByTestId('profile-picture-choose') },
  });

  await crop(page, 'picture', `${PICTURES}/picture.png`, 'crop-picture', 'the crop dialog for the picture');
  await shots.take(page, 'picture-set', 'the picture on the profile card', {
    action: { kind: 'click', target: 'choose' },
    targets: { card: page.getByTestId('profile-card'), choose: page.getByTestId('profile-banner-choose') },
  });

  await crop(page, 'banner', `${PICTURES}/banner.png`, 'crop-banner', 'the crop dialog for the banner');

  const rose = page.locator('[data-testid=profile-accent][data-accent=rose]');
  await shots.take(page, 'banner-set', 'the banner behind the picture', {
    action: { kind: 'click', target: 'colour' },
    targets: { card: page.getByTestId('profile-card'), colour: rose },
  });
  await rose.click();
  await typeWords(page, 'profile-pronouns', 'he/him', 'pronouns', 'pronouns');
  await typeWords(page, 'profile-about', 'Keeps the family photos and the code in one place.', 'about', 'a line about him');
  const save = page.getByTestId('profile-save');
  await shots.take(page, 'ready', 'ready to save', {
    action: { kind: 'click', target: 'save' },
    targets: { save, card: page.getByTestId('profile-card') },
  });
  await save.click();
  await page.locator('[data-testid=profile-save][disabled]').waitFor();
  await shots.toastsGone(page);
  await page.reload();
  await page.getByTestId('account-profile').waitFor();
  await page.getByTestId('signed-in-avatar').locator('img').waitFor();
  await shots.take(page, 'profile', 'the finished profile, and the picture in the top bar', {
    targets: { card: page.getByTestId('profile-card'), avatar: page.getByTestId('signed-in-as') },
  });

  await users(page);
  await shots.take(page, 'users', "everyone's picture on the Users pane", {
    targets: { people: page.getByTestId('users-people'), avatar: page.getByTestId('users-avatar').first() },
  });
}

await shots.finish();
