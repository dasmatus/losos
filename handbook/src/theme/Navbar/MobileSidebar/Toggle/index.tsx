import React, {type ReactNode} from 'react';
import Toggle from '@theme-original/Navbar/MobileSidebar/Toggle';
import Link from '@docusaurus/Link';
import IconMenu from '@theme/Icon/Menu';
import {translate} from '@docusaurus/Translate';
import {useBasic} from '@site/src/lib/basic';

/* On a phone the chapters live in a sheet that a script slides in. Without
 * JavaScript the menu button is a link to the Contents page instead, which
 * lists the same chapters and every page in them, and the languages, which
 * the sheet would otherwise carry. */
export default function ToggleWrapper(): ReactNode {
  const basic = useBasic();
  if (!basic) return <Toggle />;
  return (
    <Link
      to="/contents"
      className="navbar__toggle clean-btn"
      aria-label={translate({
        id: 'losos.basic.menu',
        message: 'Contents',
        description: 'The phone menu button of the handbook without JavaScript',
      })}>
      <IconMenu />
    </Link>
  );
}
