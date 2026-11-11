import { expect, test } from 'bun:test';
import { buildMainScreenshotPrBody, buildPrScreenshotComment, rawUrl } from './handbook-screens-comment.ts';

test('rawUrl pins sha and path', () => {
  expect(rawUrl('o', 'r', 'abc', 'handbook/docs/img/web/overview.png')).toBe(
    'https://raw.githubusercontent.com/o/r/abc/handbook/docs/img/web/overview.png',
  );
});

test('embeds before/after images when pushed', () => {
  const body = buildPrScreenshotComment({
    platform: 'web',
    changed: ['overview.png', 'apps.png'],
    skipped: false,
    pushed: true,
    owner: 'o',
    repo: 'r',
    baseSha: 'base',
    headSha: 'head',
  });
  expect(body).toContain('| Screen | Before | After |');
  expect(body).toContain('![overview.png before](https://raw.githubusercontent.com/o/r/base/handbook/docs/img/web/overview.png)');
  expect(body).toContain('![overview.png after](https://raw.githubusercontent.com/o/r/head/handbook/docs/img/web/overview.png)');
});

test('falls back without embeds when push refused', () => {
  const body = buildPrScreenshotComment({
    platform: 'tv',
    changed: ['overview.png'],
    skipped: false,
    pushed: false,
    owner: 'o',
    repo: 'r',
    baseSha: 'base',
    headSha: 'head',
  });
  expect(body).toContain('images are not embedded');
  expect(body).toContain('`overview.png`');
  expect(body).not.toContain('![');
});

test('records skipped platforms explicitly', () => {
  const body = buildPrScreenshotComment({
    platform: 'visionos',
    changed: [],
    skipped: true,
    skipReason: 'requires Apple visionOS tooling',
    pushed: false,
    owner: 'o',
    repo: 'r',
    baseSha: 'b',
    headSha: 'h',
  });
  expect(body).toContain('Skipped: requires Apple visionOS tooling');
});

test('main PR body embeds changed images', () => {
  const body = buildMainScreenshotPrBody({
    platform: 'web',
    changed: ['overview.png'],
    headSha: 'head',
    sourceSha: 'src',
    owner: 'o',
    repo: 'r',
  });
  expect(body).toContain('![web/overview.png](https://raw.githubusercontent.com/o/r/head/handbook/docs/img/web/overview.png)');
});
