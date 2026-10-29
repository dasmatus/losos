import React, {type ReactNode} from 'react';
import clsx from 'clsx';
import TOCCollapsible from '@theme-original/TOCCollapsible';
import type TOCCollapsibleType from '@theme/TOCCollapsible';
import TOCItems from '@theme/TOCItems';
import Translate from '@docusaurus/Translate';
import useIsBrowser from '@docusaurus/useIsBrowser';
import type {WrapperProps} from '@docusaurus/types';

type Props = WrapperProps<typeof TOCCollapsibleType>;

/* The phone's "On this page" fold. The stock one is a button that renders
 * its list only once clicked, so the server HTML, all a reader without
 * JavaScript gets, holds a button that does nothing and no list. Until
 * React runs it is a native <details> holding the whole list instead, and
 * custom.css draws both the same way, the opening animated in CSS. */
export default function TOCCollapsibleWrapper(props: Props): ReactNode {
  const isBrowser = useIsBrowser();
  if (isBrowser) return <TOCCollapsible {...props} />;
  const {toc, className, minHeadingLevel, maxHeadingLevel} = props;
  return (
    <details className={clsx('hb-toc', className)}>
      <summary className="hb-toc__summary">
        <Translate
          id="theme.TOCCollapsible.toggleButtonLabel"
          description="The label used by the button on the collapsible TOC component"
        >
          On this page
        </Translate>
      </summary>
      <div className="hb-toc__content">
        <TOCItems toc={toc} minHeadingLevel={minHeadingLevel} maxHeadingLevel={maxHeadingLevel} />
      </div>
    </details>
  );
}
