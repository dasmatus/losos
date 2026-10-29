import React, {type ReactNode} from 'react';
import SearchBar from '@theme-original/SearchBar';
import useBaseUrl from '@docusaurus/useBaseUrl';
import useIsBrowser from '@docusaurus/useIsBrowser';
import {translate} from '@docusaurus/Translate';

/* The offline search runs in the browser, so in the server HTML, all a
 * reader without JavaScript gets, its box is a link drawn the same way that
 * opens the search page, which then lists every page (src/theme/SearchPage).
 * Once React runs it is the stock search box. */
export default function SearchBarWrapper(): ReactNode {
  const isBrowser = useIsBrowser();
  const page = useBaseUrl('/search');
  if (isBrowser) return <SearchBar />;
  const label = translate({id: 'theme.SearchBar.label', message: 'Search'});
  return (
    <div className="navbar__search hb-search">
      <svg aria-hidden="true" focusable="false" viewBox="0 0 16 16" className="hb-search__icon">
        <path
          fill="currentColor"
          d="M6.02945 10.20327a4.17382 4.17382 0 1 1 4.17382-4.17382 4.15609 4.15609 0 0 1-4.17382 4.17382Zm9.69195 4.2199-4.8225-4.82338A5.88021 5.88021 0 0 0 12.058 6.02856 6.00467 6.00467 0 1 0 9.59979 10.8989l4.82338 4.82338a.89729.89729 0 0 0 1.29912 0 .89749.89749 0 0 0-.00087-1.29909Z"
        />
      </svg>
      <a href={page} aria-label={label} className="navbar__search-input hb-search__link">
        <span>{label}</span>
      </a>
    </div>
  );
}
