import useBaseUrl from '@docusaurus/useBaseUrl';
import {useAllDocsData} from '@docusaurus/plugin-content-docs/client';

/* Without JavaScript the phone menu is the docs sidebar itself, opened as a
 * sheet when the address ends in #hb-nav (custom.css, `:target`). A page
 * with no sidebar (the landing page, search) sends the menu button to the
 * Welcome page's sheet instead. */
export const NAV_ID = 'hb-nav';

export function useDocsHome(): string {
  const fallback = useBaseUrl('/');
  const version = useAllDocsData()['default']?.versions[0];
  const main = version?.docs.find((doc) => doc.id === version.mainDocId);
  return main?.path ?? fallback;
}
