import {themes as prismThemes} from 'prism-react-renderer';
import type {Config} from '@docusaurus/types';
import type * as Preset from '@docusaurus/preset-classic';

/* The LosOS handbook: the owner's manual for a box, written so it can be read
 * from the local network alone.
 *
 * The same source builds twice:
 *
 *   - the public site at https://losos.dasmat.us (GitHub Pages, deployed by
 *     .github/workflows/handbook.yml), with baseUrl "/";
 *   - the copy every box serves itself at http://<box>/handbook/, built by
 *     flake/packages.nix (losos-handbook) and served by nginx from
 *     modules/containers.nix, with baseUrl "/handbook/".
 *
 * The two differ only in these two environment variables. Everything else,
 * the search index included, is static: a box has no internet to promise and
 * the manual is most needed exactly when nothing else answers.
 *
 * This runs in Node.js at build time — no browser APIs, no JSX here. */
const url = process.env.LOSOS_HANDBOOK_URL ?? 'https://losos.dasmat.us';
const baseUrl = process.env.LOSOS_HANDBOOK_BASE ?? '/';

const config: Config = {
  title: 'LosOS handbook',
  tagline: 'A box for your files that looks after itself',
  favicon: 'img/losos.svg',

  future: {
    v4: true,
    // The v4 flags would turn the Rspack-based "faster" bundler on, which
    // needs @docusaurus/faster and its native binaries. The default webpack
    // build is slower by a minute and has no native dependency, which is
    // what lets the same source build inside the Nix sandbox on the box.
    faster: false,
  },

  url,
  baseUrl,

  organizationName: 'dasmatus',
  projectName: 'losos',

  // A link that goes nowhere is a bug in a manual, not a warning.
  onBrokenLinks: 'throw',
  onBrokenAnchors: 'throw',
  markdown: {
    hooks: {
      onBrokenMarkdownLinks: 'throw',
    },
  },

  // English is written; Slovak and German are wired so pages can follow one
  // at a time (see docs/reference/for-developers.md, "Translating"). A locale
  // without a translated page shows the English page under its own chrome.
  i18n: {
    defaultLocale: 'en',
    locales: ['en', 'sk', 'de'],
    localeConfigs: {
      en: {label: 'English', htmlLang: 'en'},
      sk: {label: 'Slovenčina', htmlLang: 'sk'},
      de: {label: 'Deutsch', htmlLang: 'de'},
    },
  },

  presets: [
    [
      'classic',
      {
        docs: {
          // The handbook IS the site: no blog, no separate landing page, so
          // every address is short enough to read off a screen.
          routeBasePath: '/',
          sidebarPath: './sidebars.ts',
          editUrl: 'https://github.com/dasmatus/losos/tree/main/handbook/',
          showLastUpdateTime: false,
        },
        blog: false,
        theme: {
          customCss: './src/css/custom.css',
        },
      } satisfies Preset.Options,
    ],
  ],

  themes: [
    [
      // Offline full-text search: a lunr index built into the bundle, so the
      // search box works on a box that cannot reach the internet. Algolia
      // would not.
      '@easyops-cn/docusaurus-search-local',
      {
        hashed: true,
        docsRouteBasePath: '/',
        indexBlog: false,
        language: ['en', 'de'],
        highlightSearchTermsOnTargetPage: true,
      },
    ],
  ],

  themeConfig: {
    image: 'img/losos.svg',
    colorMode: {
      respectPrefersColorScheme: true,
    },
    docs: {
      sidebar: {
        hideable: true,
        autoCollapseCategories: false,
      },
    },
    navbar: {
      title: 'LosOS handbook',
      logo: {
        alt: 'LosOS',
        src: 'img/losos.svg',
      },
      items: [
        {type: 'docSidebar', sidebarId: 'handbook', position: 'left', label: 'Handbook'},
        {to: '/types', position: 'left', label: 'Types'},
        {to: '/troubleshooting', position: 'left', label: 'When something goes wrong'},
        {type: 'localeDropdown', position: 'right'},
        {
          href: 'https://github.com/dasmatus/losos',
          label: 'GitHub',
          position: 'right',
        },
      ],
    },
    footer: {
      style: 'dark',
      links: [
        {
          title: 'This handbook',
          items: [
            {label: 'Getting started', to: '/start/what-is-losos'},
            {label: 'Types of setup', to: '/types'},
            {label: 'When something goes wrong', to: '/troubleshooting'},
          ],
        },
        {
          title: 'For developers',
          items: [
            {label: 'Wiki', href: 'https://github.com/dasmatus/losos/wiki'},
            {label: 'Source', href: 'https://github.com/dasmatus/losos'},
            {label: 'Releases', href: 'https://github.com/dasmatus/losos/releases'},
          ],
        },
      ],
      copyright: `LosOS. Every box serves this handbook at /handbook/, with or without the internet.`,
    },
    prism: {
      theme: prismThemes.github,
      darkTheme: prismThemes.dracula,
      additionalLanguages: ['bash', 'nix', 'powershell', 'json'],
    },
  } satisfies Preset.ThemeConfig,
};

export default config;
