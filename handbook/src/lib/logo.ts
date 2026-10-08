import {useEffect, useState} from 'react';
import {translate} from '@docusaurus/Translate';

/* Which logo the site shows today, as on the admin pages
 * (admin-ui/app/src/lib/logo.ts): a live coho salmon, and on 31 October, by
 * the reader's own calendar, the earlier plate of salmon, a dead fish for
 * the one day that suits it. Docusaurus renders the pages ahead of time, so
 * the first paint is always the salmon and the hook reads the date after
 * hydration. Reading it during render would make the server's HTML and the
 * browser's first render disagree. */

export function isHalloween(now: Date = new Date()): boolean {
  return now.getMonth() === 9 && now.getDate() === 31;
}

export function useHalloween(): boolean {
  const [halloween, setHalloween] = useState(false);
  useEffect(() => setHalloween(isHalloween()), []);
  return halloween;
}

/** The sentence that says what the picture is, for the tooltip and alt text. */
export function logoTip(halloween: boolean): string {
  return halloween
    ? translate({
        id: 'losos.navbar.logoTipHalloween',
        message: 'Losos is Slovak for salmon. For Halloween, the logo is a plate of it.',
        description: 'The tooltip on the logo on 31 October, when it is a plate of salmon',
      })
    : translate({
        id: 'losos.navbar.logoTip',
        message: 'Losos is Slovak for salmon. The logo is a live coho salmon, photographed underwater.',
        description: 'The tooltip on the navbar logo, saying what the picture means',
      });
}
