import useDocusaurusContext from '@docusaurus/useDocusaurusContext';

/* Whether this page is the copy without JavaScript (src/plugins/basic.ts).
 * Its HTML is the server's render and nothing hydrates it, so a component
 * that needs a script to do its job renders a link or native HTML instead
 * when this is true. */
export function useBasic(): boolean {
  return useDocusaurusContext().siteConfig.customFields?.['basic'] === true;
}
