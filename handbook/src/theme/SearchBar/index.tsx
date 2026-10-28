import React, {type ReactNode} from 'react';
import SearchBar from '@theme-original/SearchBar';
import Link from '@docusaurus/Link';
import {translate} from '@docusaurus/Translate';
import {useBasic} from '@site/src/lib/basic';

/* The offline search needs its script and its index. Without JavaScript the
 * box is a link of the same shape to the Contents page, every page and
 * section on one page, which the browser's own Find searches. custom.css
 * draws .navbar__search-input as the box in the panel and as the glyph chip
 * on a phone, so the link takes both looks unchanged. */
export default function SearchBarWrapper(): ReactNode {
  const basic = useBasic();
  if (!basic) return <SearchBar />;
  const label = translate({
    id: 'losos.basic.search',
    message: 'Find a page',
    description: 'The search box of the handbook without JavaScript, a link to the Contents page',
  });
  return (
    <div className="navbar__search">
      <Link to="/contents" className="navbar__search-input hb-basic-search" aria-label={label}>
        {label}
      </Link>
    </div>
  );
}
