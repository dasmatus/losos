import React, {type ReactNode} from 'react';
import Toggle from '@theme-original/Navbar/MobileSidebar/Toggle';
import useIsBrowser from '@docusaurus/useIsBrowser';
import {translate} from '@docusaurus/Translate';
import IconMenu from '@theme/Icon/Menu';
import {NAV_ID, useDocsHome} from '@site/src/lib/no-js-nav';

/* The phone's menu button. Once React runs it is the stock button, which
 * opens Docusaurus's own sheet. In the server HTML, which is all a reader
 * without JavaScript gets, it is a link to #hb-nav: the docs sidebar opens
 * as a sheet on this page, or on the Welcome page when this page has none.
 * custom.css shows whichever of the two links fits. */
export default function ToggleWrapper(): ReactNode {
  const isBrowser = useIsBrowser();
  const home = useDocsHome();
  if (isBrowser) return <Toggle />;
  const label = translate({
    id: 'theme.docs.sidebar.toggleSidebarButtonAriaLabel',
    message: 'Toggle navigation bar',
    description: 'The ARIA label for hamburger menu button of mobile navigation',
  });
  return (
    <>
      <a
        href={`#${NAV_ID}`}
        aria-label={label}
        className="navbar__toggle clean-btn hb-nav-open hb-nav-open--here"
      >
        <IconMenu />
      </a>
      <a
        href={`${home}#${NAV_ID}`}
        aria-label={label}
        className="navbar__toggle clean-btn hb-nav-open hb-nav-open--away"
      >
        <IconMenu />
      </a>
    </>
  );
}
