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
 * The two differ in these two environment variables and in what sits at the
 * root. The public site opens on a landing page (src/pages/index.tsx) that
 * says what LosOS is to someone who has never seen a box, and its Welcome
 * page moves to /welcome. On the box the root stays the Welcome page and
 * there is no landing page: the reader there already owns the box, and the
 * About pane's "Handbook" link should open the manual, not an advert for it.
 * Everything else, the search index included, is static: a box has no
 * internet to promise and the manual is most needed exactly when nothing
 * else answers.
 *
 * This runs in Node.js at build time — no browser APIs, no JSX here. */
const url = process.env.LOSOS_HANDBOOK_URL ?? 'https://losos.dasmat.us';
const baseUrl = process.env.LOSOS_HANDBOOK_BASE ?? '/';
// The box's copy is the only build under another base (flake/packages.nix).
const landing = baseUrl === '/';

const config: Config = {
  title: 'LosOS handbook',
  tagline: 'A box for your files that looks after itself',
  favicon: 'img/losos.png',

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
    // On the public site the landing page takes /, so the docs page that
    // claims it (docs/index.md, and any translation of it) moves aside.
    // Keyed on the slug rather than the file, so a translated copy follows.
    parseFrontMatter: async (params) => {
      const result = await params.defaultParseFrontMatter(params);
      if (landing && result.frontMatter.slug === '/') {
        result.frontMatter.slug = '/welcome';
      }
      return result;
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
          // The docs sit at the root, beside the landing page rather than
          // under it, so every address is short enough to read off a screen.
          routeBasePath: '/',
          // Nothing under docs/project/ is published by either build: the
          // site is the product's, not a school report. The first four globs
          // are Docusaurus's own defaults, which an explicit list replaces.
          exclude: [
            '**/_*.{js,jsx,ts,tsx,md,mdx}',
            '**/_*/**',
            '**/*.test.{js,jsx,ts,tsx}',
            '**/__tests__/**',
            'project/**',
          ],
          sidebarPath: './sidebars.ts',
          editUrl: 'https://github.com/dasmatus/losos/tree/main/handbook/',
          showLastUpdateTime: false,
        },
        blog: false,
        // src/pages holds only the landing page, which the box leaves out.
        pages: landing ? {} : false,
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
        // The box lives in the sidebar panel (src/theme/DocSidebar), at the
        // page's left edge, so its result list opens to the right.
        searchBarPosition: 'left',
        // No Ctrl+K: neither the hint in the box nor the key itself. The
        // admin UI's search box has no shortcut either.
        searchBarShortcut: false,
        searchBarShortcutHint: false,
        docsRouteBasePath: '/',
        indexBlog: false,
        language: ['en', 'de'],
        highlightSearchTermsOnTargetPage: true,
      },
    ],
  ],

  themeConfig: {
    image: 'img/losos.png',
    colorMode: {
      respectPrefersColorScheme: true,
    },
    docs: {
      sidebar: {
        // No hide button under the menu: the page is 72rem wide at most and
        // the panel is part of its shape, as the SPA's is.
        hideable: false,
        autoCollapseCategories: false,
      },
    },
    navbar: {
      // The admin UI puts its logo beside a chip naming the box; here the
      // chip names the handbook (custom.css draws .navbar__title as that chip).
      title: 'Handbook',
      logo: {
        alt: 'LosOS',
        src: 'img/losos.png',
      },
      // No page links in the bar: the SPA's TopBar carries none (its
      // navigation is the sidebar), so the chapters are reached from the
      // sidebar here too. Right-hand side, in the SPA's order: the language
      // picker, the three-way theme control, and a plain text link where the
      // SPA has "Sign out".
      items: [
        {type: 'localeDropdown', position: 'right'},
        {
          href: 'https://github.com/dasmatus/losos',
          label: 'GitHub',
          position: 'right',
        },
      ],
    },
    footer: {
      style: 'light',
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
