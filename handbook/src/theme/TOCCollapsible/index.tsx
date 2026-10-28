import React, {type ReactNode} from 'react';
import TOCCollapsible from '@theme-original/TOCCollapsible';
import type TOCCollapsibleType from '@theme/TOCCollapsible';
import TOCItems from '@theme/TOCItems';
import Translate from '@docusaurus/Translate';
import type {WrapperProps} from '@docusaurus/types';
import {useBasic} from '@site/src/lib/basic';

type Props = WrapperProps<typeof TOCCollapsibleType>;

/* "On this page", above the article on a phone, folds open by a script.
 * Without JavaScript it is a <details>, which the browser folds by itself;
 * basic.css gives it the same box. */
export default function TOCCollapsibleWrapper(props: Props): ReactNode {
  const basic = useBasic();
  if (!basic) return <TOCCollapsible {...props} />;
  const {toc, className, minHeadingLevel, maxHeadingLevel} = props;
  return (
    <details className={`hb-basic-toc ${className ?? ''}`}>
      <summary>
        <Translate
          id="theme.TOCCollapsible.toggleButtonLabel"
          description="The label used by the button on the collapsible TOC component">
          On this page
        </Translate>
      </summary>
      <TOCItems toc={toc} minHeadingLevel={minHeadingLevel} maxHeadingLevel={maxHeadingLevel} />
    </details>
  );
}
