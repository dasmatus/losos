import React, {type ReactNode} from 'react';
import Content from '@theme-original/DocSidebar/Desktop/Content';
import type ContentType from '@theme/DocSidebar/Desktop/Content';
import SearchBar from '@theme/SearchBar';
import NavbarLogo from '@theme/Navbar/Logo';
import IconClose from '@theme/Icon/Close';
import IconLanguage from '@theme/Icon/Language';
import IconExternalLink from '@theme/Icon/ExternalLink';
import useIsBrowser from '@docusaurus/useIsBrowser';
import useDocusaurusContext from '@docusaurus/useDocusaurusContext';
import {translate} from '@docusaurus/Translate';
import {useThemeConfig} from '@docusaurus/theme-common';
import {useAlternatePageUtils} from '@docusaurus/theme-common/internal';
import type {WrapperProps} from '@docusaurus/types';
import {NAV_ID} from '@site/src/lib/no-js-nav';

type Props = WrapperProps<typeof ContentType>;

/* The admin UI keeps its search box at the top of the sidebar panel, so the
 * handbook does the same on a desktop: the offline search bar (the
 * @easyops-cn/docusaurus-search-local theme's SearchBar) is rendered here
 * and the top-bar copy is hidden by custom.css at that width. On a phone the
 * panel is a sheet and the top-bar search stays, as the admin UI's does.
 *
 * Without JavaScript Docusaurus's phone sheet is empty, so this panel is the
 * sheet instead: the server HTML adds the anchor the menu button links to,
 * the sheet's head with its close link, a backdrop that closes it, and the
 * languages and outside links that the stock sheet lists on its main menu.
 * All of it is hidden on a desktop and gone once React runs. */
function Sheet({children}: {children: ReactNode}): ReactNode {
  const {
    i18n: {currentLocale, locales, localeConfigs},
  } = useDocusaurusContext();
  const alternate = useAlternatePageUtils();
  const links = useThemeConfig().navbar.items.filter(
    (item): item is {href: string; label: string} =>
      typeof item['href'] === 'string' && typeof item['label'] === 'string',
  );
  const close = translate({
    id: 'theme.docs.sidebar.closeSidebarButtonAriaLabel',
    message: 'Close navigation bar',
    description: 'The ARIA label for close button of mobile sidebar',
  });
  return (
    <>
      <span id={NAV_ID} className="hb-nav__anchor" />
      <a href="#_" className="hb-nav__backdrop" aria-label={close} tabIndex={-1} />
      <div className="hb-nav__head">
        <NavbarLogo />
        <a href="#_" className="hb-nav__close clean-btn" aria-label={close}>
          <IconClose color="var(--ifm-color-emphasis-600)" />
        </a>
      </div>
      {children}
      <ul className="hb-nav__more menu__list">
        <li className="menu__list-item hb-nav__languages">
          <span className="menu__link hb-nav__label">
            <IconLanguage width={20} height={20} />
            {translate({
              id: 'theme.navbar.mobileLanguageDropdown.label',
              message: 'Languages',
              description: 'The label for the mobile language switcher dropdown',
            })}
          </span>
          <ul className="menu__list">
            {locales.map((locale) => (
              <li key={locale} className="menu__list-item">
                <a
                  href={alternate.createUrl({locale, fullyQualified: false})}
                  lang={localeConfigs[locale]?.htmlLang}
                  aria-current={locale === currentLocale ? 'page' : undefined}
                  className={
                    locale === currentLocale ? 'menu__link menu__link--active' : 'menu__link'
                  }
                >
                  {localeConfigs[locale]?.label ?? locale}
                </a>
              </li>
            ))}
          </ul>
        </li>
        {links.map((link) => (
          <li key={link.href} className="menu__list-item">
            <a href={link.href} target="_blank" rel="noopener noreferrer" className="menu__link">
              {link.label}
              <IconExternalLink />
            </a>
          </li>
        ))}
      </ul>
    </>
  );
}

export default function ContentWrapper(props: Props): ReactNode {
  const isBrowser = useIsBrowser();
  const panel = (
    <>
      <div className="hb-sidebar-search">
        <SearchBar />
      </div>
      <Content {...props} />
    </>
  );
  return isBrowser ? panel : <Sheet>{panel}</Sheet>;
}
