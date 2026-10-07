import React from 'react';
import Content from '@theme-original/DocSidebar/Desktop/Content';
import type ContentType from '@theme/DocSidebar/Desktop/Content';
import SearchBar from '@theme/SearchBar';
import type {WrapperProps} from '@docusaurus/types';

type Props = WrapperProps<typeof ContentType>;

/* The admin UI keeps its search box at the top of the sidebar panel, so the
 * handbook does the same on a desktop: the offline search bar (the
 * @easyops-cn/docusaurus-search-local theme's SearchBar) is rendered here
 * and the top-bar copy is hidden by custom.css at that width. On a phone the
 * panel is a sheet and the top-bar search stays, as the admin UI's does. */
export default function ContentWrapper(props: Props): React.ReactElement {
  return (
    <>
      <div className="hb-sidebar-search">
        <SearchBar />
      </div>
      <Content {...props} />
    </>
  );
}
