import React, {type ReactNode} from 'react';
import SearchPage from '@theme-original/SearchPage';
import Layout from '@theme/Layout';
import Translate, {translate} from '@docusaurus/Translate';
import useIsBrowser from '@docusaurus/useIsBrowser';
import {usePluginData} from '@docusaurus/useGlobalData';
import type {PageList} from '@site/src/plugins/page-list';

/* The search page. The offline index is searched in the browser, so until
 * React runs (and for good without JavaScript) the page lists every page of
 * the handbook by chapter instead (src/plugins/page-list.ts), where the
 * browser's own Find does the searching. */
export default function SearchPageWrapper(): ReactNode {
  const isBrowser = useIsBrowser();
  const {groups} = usePluginData('hb-page-list') as PageList;
  if (isBrowser) return <SearchPage />;
  const title = translate({
    id: 'hb.search.allPages.title',
    message: 'Every page',
    description: 'The search page without JavaScript, which lists every page',
  });
  return (
    <Layout title={title}>
      <main className="container margin-vert--lg hb-pages">
        <h1>{title}</h1>
        <p className="hb-pages__lead">
          <Translate
            id="hb.search.allPages.lead"
            description="Under the title of the search page without JavaScript"
          >
            The search runs in the browser and needs JavaScript. Here is every
            page of the handbook instead; your browser's Find (Ctrl+F) searches
            the list.
          </Translate>
        </p>
        <div className="hb-pages__groups">
          {groups.map((group, index) => (
            <section key={group.label ?? index} className="hb-pages__group">
              {group.label !== null && (
                <h2>
                  {group.permalink !== null ? (
                    <a href={group.permalink}>{group.label}</a>
                  ) : (
                    group.label
                  )}
                </h2>
              )}
              <ul>
                {group.pages.map((page) => (
                  <li key={page.permalink} className={`hb-pages__depth-${Math.min(page.depth, 3)}`}>
                    <a href={page.permalink}>{page.title}</a>
                  </li>
                ))}
              </ul>
            </section>
          ))}
        </div>
      </main>
    </Layout>
  );
}
