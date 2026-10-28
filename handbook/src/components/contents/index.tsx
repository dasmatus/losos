import React, {type ReactNode} from 'react';
import Link from '@docusaurus/Link';
import Layout from '@theme/Layout';
import Heading from '@theme/Heading';
import Translate, {translate} from '@docusaurus/Translate';
import useDocusaurusContext from '@docusaurus/useDocusaurusContext';
import {useAlternatePageUtils} from '@docusaurus/theme-common/internal';
import type {ContentsEntry, ContentsSection} from '@site/src/plugins/basic';

/* The Contents page of the handbook without JavaScript (src/plugins/basic.ts
 * builds its data and its route): every chapter, every page and every
 * section, all open, on one page. It stands in for what needs a script on
 * the usual copy: the phone's sheet with the chapters, the language picker
 * in it, and the search, since the browser's own Find searches this page
 * for any page or section by its heading. */

type Props = {readonly contents: ContentsEntry[]};

function Sections({items}: {readonly items: ContentsSection[]}): ReactNode {
  if (items.length === 0) return null;
  return (
    <ul className="hb-contents__sections">
      {items.map((section) => (
        <li key={section.to}>
          <Link to={section.to}>{section.label}</Link>
          <Sections items={section.items} />
        </li>
      ))}
    </ul>
  );
}

function Entries({items}: {readonly items: ContentsEntry[]}): ReactNode {
  return (
    <ul className="hb-contents__pages">
      {items.map((entry) => (
        <li key={entry.to ?? entry.label}>
          {entry.to === undefined ? entry.label : <Link to={entry.to}>{entry.label}</Link>}
          <Sections items={entry.sections ?? []} />
          {entry.items !== undefined && entry.items.length > 0 && <Entries items={entry.items} />}
        </li>
      ))}
    </ul>
  );
}

function Languages(): ReactNode {
  const {
    i18n: {locales, currentLocale, localeConfigs},
  } = useDocusaurusContext();
  const {createUrl} = useAlternatePageUtils();
  return (
    <p className="hb-contents__languages">
      {locales.map((locale) => {
        const label = localeConfigs[locale]?.label ?? locale;
        return locale === currentLocale ? (
          <strong key={locale}>{label}</strong>
        ) : (
          // A full address into the other locale's build, which this
          // build's broken-link check cannot see: a plain <a>, as the theme's
          // own language picker does.
          <a key={locale} href={createUrl({locale, fullyQualified: false})} lang={locale}>
            {label}
          </a>
        );
      })}
    </p>
  );
}

export default function Contents({contents}: Props): ReactNode {
  const title = translate({
    id: 'losos.basic.contents.title',
    message: 'Contents',
    description: 'The title of the Contents page of the handbook without JavaScript',
  });
  return (
    <Layout title={title}>
      <main className="container margin-vert--lg hb-contents">
        <Heading as="h1">{title}</Heading>
        <p>
          <Translate
            id="losos.basic.contents.lead"
            description="The lead of the Contents page of the handbook without JavaScript">
            Every page of the handbook and every section in it. To find one,
            search this page with your browser's Find, Ctrl+F or ⌘F.
          </Translate>
        </p>
        <Languages />
        {contents.map((chapter) => (
          <section key={chapter.to ?? chapter.label} className="hb-contents__chapter">
            <Heading as="h2">
              {chapter.to === undefined ? chapter.label : <Link to={chapter.to}>{chapter.label}</Link>}
            </Heading>
            <Sections items={chapter.sections ?? []} />
            {chapter.items !== undefined && <Entries items={chapter.items} />}
          </section>
        ))}
      </main>
    </Layout>
  );
}
