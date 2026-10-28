import React, {type ReactNode} from 'react';
import clsx from 'clsx';
import Link from '@docusaurus/Link';
import ThemedImage from '@theme/ThemedImage';
import Translate, {translate} from '@docusaurus/Translate';
import useBaseUrl from '@docusaurus/useBaseUrl';
import useDocusaurusContext from '@docusaurus/useDocusaurusContext';

import overviewLight from '@site/docs/img/overview.png';
import overviewDark from '@site/docs/img/overview-dark.png';
import overviewLightSk from '@site/docs/img/overview-sk.png';
import overviewDarkSk from '@site/docs/img/overview-dark-sk.png';
import overviewLightDe from '@site/docs/img/overview-de.png';
import overviewDarkDe from '@site/docs/img/overview-dark-de.png';

import {logoTip, useHalloween} from '@site/src/lib/logo';
import styles from './home.module.css';

/* The sections the public site's own pages are built from: the landing
 * page (src/pages/index.tsx) and LosOS for companies
 * (src/pages/companies.tsx). Each page brings its own words; the shapes,
 * the screenshot, the "Coming soon" cards and the requirements are shared,
 * so the two pages cannot drift apart. */

export {styles};

export const SOURCE = 'https://github.com/dasmatus/losos';

/** Every release, newest first. Not /releases/latest: GitHub serves that
 * only for a release that is not marked pre-release. */
export const RELEASES = 'https://github.com/dasmatus/losos/releases';

export type Action = {label: ReactNode; to?: string; href?: string};

export function Actions({
  primary,
  secondary,
}: {
  primary: Action;
  secondary?: Action;
}): ReactNode {
  return (
    <div className={styles.actions}>
      <Link className={styles.primary} to={primary.to} href={primary.href}>
        {primary.label}
      </Link>
      {secondary && (
        <Link className={styles.secondary} to={secondary.to} href={secondary.href}>
          {secondary.label}
        </Link>
      )}
    </div>
  );
}

export function Hero({
  eyebrow,
  title,
  lead,
  body,
  primary,
  secondary,
  fine,
}: {
  eyebrow: ReactNode;
  title: ReactNode;
  lead: ReactNode;
  body: ReactNode;
  primary: Action;
  secondary?: Action;
  fine: ReactNode;
}): ReactNode {
  const halloween = useHalloween();
  const salmon = useBaseUrl('/img/salmon.png');
  const plate = useBaseUrl('/img/plate.png');
  const logo = halloween ? plate : salmon;
  const tip = logoTip(halloween);
  /* The salmon stays a small mark beside the title, so the first screen of
   * a phone shows the admin pages (<Screen />, right below) and not a fish. */
  return (
    <header className={styles.hero}>
      <p className={styles.eyebrow}>{eyebrow}</p>
      <h1 className={styles.heroTitle}>
        <img className={styles.mark} src={logo} width={64} height={64} alt="" title={tip} />
        {title}
      </h1>
      <p className={styles.heroLead}>{lead}</p>
      <p className={styles.heroBody}>{body}</p>
      <Actions primary={primary} secondary={secondary} />
      <p className={styles.fine}>{fine}</p>
    </header>
  );
}

// The admin pages in the language the handbook is read in.
const OVERVIEW: Record<string, {light: string; dark: string}> = {
  en: {light: overviewLight, dark: overviewDark},
  sk: {light: overviewLightSk, dark: overviewDarkSk},
  de: {light: overviewLightDe, dark: overviewDarkDe},
};

export function Screen(): ReactNode {
  const {i18n} = useDocusaurusContext();
  return (
    <figure className={styles.screen}>
      <ThemedImage
        className={styles.screenImage}
        sources={OVERVIEW[i18n.currentLocale] ?? OVERVIEW.en}
        alt={translate({
          id: 'losos.home.screen.alt',
          message:
            'The admin pages of a box: the app tiles for LosOS Cloud and LosOS Git, the storage card and a board of widgets.',
        })}
        width={1600}
        height={1125}
      />
      <figcaption className={styles.caption}>
        <Translate id="losos.home.screen.caption">
          The admin pages, where you set up and look after the box.
        </Translate>
      </figcaption>
    </figure>
  );
}

export function Section({
  title,
  children,
}: {
  title: ReactNode;
  children: ReactNode;
}): ReactNode {
  return (
    <section className={styles.section}>
      <h2 className={styles.sectionTitle}>{title}</h2>
      {children}
    </section>
  );
}

/* A section with its words on the left and a picture on the right, which
 * stacks below the words on a narrow screen. */
export function Split({
  title,
  children,
  aside,
}: {
  title: ReactNode;
  children: ReactNode;
  aside: ReactNode;
}): ReactNode {
  return (
    <section className={styles.section}>
      <div className={styles.split}>
        <div>
          <h2 className={styles.sectionTitle}>{title}</h2>
          {children}
        </div>
        {aside}
      </div>
    </section>
  );
}

export type Feature = {title: ReactNode; body: ReactNode; to: string};

/* One linked card per feature, three to a row. A `to` that starts with
 * https:// leaves the site. */
export function Tiles({items}: {items: Feature[]}): ReactNode {
  return (
    <div className={styles.grid}>
      {items.map((f, i) => (
        <Link key={i} to={f.to} className={styles.tile}>
          <h3 className={styles.itemTitle}>{f.title}</h3>
          <p className={styles.itemBody}>{f.body}</p>
        </Link>
      ))}
    </div>
  );
}

export type Step = {title: ReactNode; body: ReactNode; to: string; link: ReactNode};

export function Steps({items}: {items: Step[]}): ReactNode {
  return (
    <ol className={styles.steps}>
      {items.map((s, i) => (
        <li key={i} className={styles.step}>
          <span className={styles.stepNumber} aria-hidden="true">
            {i + 1}
          </span>
          <h3 className={styles.itemTitle}>{s.title}</h3>
          <p className={styles.itemBody}>{s.body}</p>
          <Link className={styles.more} to={s.to}>
            {s.link}
          </Link>
        </li>
      ))}
    </ol>
  );
}

/* Planned, each with the page that says how far along it is, and its parts
 * in order with the ones already shipped crossed off. The list is the
 * project page's "What is done and what is planned" plus what the code
 * shows unfinished: nix/keys/official-edge-root.pub is empty, so no box sees an
 * official edge to trade through yet, and nix/keys/secure-boot-db.pem holds no
 * certificate. Longhorn's chart is pinned in modules/options.nix and wired into
 * the edge's rke2 deployment; the remaining pooling gaps are listed in
 * nix/demo/edge-lan/CHECKLIST.md. Nothing goes here that is not built or
 * started. Once every part of a card is done, the card says so; move it up to
 * the features then. */
type Part = {done: boolean; text: ReactNode};
type Planned = Feature & {parts: Part[]};

const SOON: Planned[] = [
  {
    title: <Translate id="losos.home.soon.market.title">The market</Translate>,
    body: (
      <Translate id="losos.home.soon.market.body">
        Get paid for the disk and CPU your box lends, and buy room from other
        boxes.
      </Translate>
    ),
    parts: [
      {
        done: true,
        text: (
          <Translate id="losos.home.soon.market.card">
            Card payments through Stripe, tested in its test mode
          </Translate>
        ),
      },
      {
        done: true,
        text: (
          <Translate id="losos.home.soon.market.coins">
            Payments in USDC, Bitcoin, Monero and Zcash
          </Translate>
        ),
      },
      {
        done: true,
        text: (
          <Translate id="losos.home.soon.market.pane">
            The Market pane on every box
          </Translate>
        ),
      },
      {
        done: false,
        text: (
          <Translate id="losos.home.soon.market.edge">
            The official LosOS edge it trades through
          </Translate>
        ),
      },
    ],
    to: '/types/official-edge',
  },
  {
    title: (
      <Translate id="losos.home.soon.pool.title">One storage pool across boxes</Translate>
    ),
    body: (
      <Translate id="losos.home.soon.pool.body">
        Boxes behind one edge lend their unused disk to one replicated pool.
      </Translate>
    ),
    parts: [
      {
        done: true,
        text: (
          <Translate id="losos.home.soon.pool.share">
            The encrypted share on each box
          </Translate>
        ),
      },
      {
        done: false,
        text: (
          <Translate id="losos.home.soon.pool.service">
            The pool's storage service on the edge
          </Translate>
        ),
      },
      {
        done: true,
        text: (
          <Translate id="losos.home.soon.pool.switch">
            The sharing switch, which opens with the market
          </Translate>
        ),
      },
    ],
    to: '/types/storage-modes',
  },
  {
    title: (
      <Translate id="losos.home.soon.secureboot.title">A signed installer</Translate>
    ),
    body: (
      <Translate id="losos.home.soon.secureboot.body">
        Firmware that trusts the LosOS certificate boots the installer with
        Secure Boot on.
      </Translate>
    ),
    parts: [
      {
        done: true,
        text: (
          <Translate id="losos.home.soon.secureboot.ready">
            An installer ready to be signed
          </Translate>
        ),
      },
      {
        done: false,
        text: (
          <Translate id="losos.home.soon.secureboot.key">
            The LosOS signing key
          </Translate>
        ),
      },
      {
        done: false,
        text: (
          <Translate id="losos.home.soon.secureboot.releases">
            A signature on every release
          </Translate>
        ),
      },
    ],
    to: '/start/secure-boot',
  },
  {
    title: (
      <Translate id="losos.home.soon.company.title">Ready setups for companies</Translate>
    ),
    body: (
      <Translate id="losos.home.soon.company.body">
        Boxes for each team, one edge for the office and the install done for
        you.
      </Translate>
    ),
    parts: [
      {
        done: true,
        text: (
          <Translate id="losos.home.soon.company.setup">
            The company setup, with the office's own edge
          </Translate>
        ),
      },
      {
        done: true,
        text: (
          <Translate id="losos.home.soon.company.image">
            An edge image attached to every release
          </Translate>
        ),
      },
      {
        done: false,
        text: (
          <Translate id="losos.home.soon.company.package">
            Selling it as a package
          </Translate>
        ),
      },
    ],
    to: '/types/company-edge',
  },
];

/* `children`, when a page gives any, sit between the heading and the
 * cards. */
export function Soon({children}: {children?: ReactNode}): ReactNode {
  return (
    <section className={styles.section}>
      <h2 className={styles.sectionTitle}>
        <Translate id="losos.home.soon.title">Coming soon</Translate>
      </h2>
      {children}
      <div className={styles.soonGrid}>
        {SOON.map((f, i) => {
          const shipped = f.parts.every((p) => p.done);
          return (
            <Link key={i} to={f.to} className={clsx(styles.tile, styles.soon)}>
              <span className={clsx(styles.soonChip, shipped && styles.soonChipDone)}>
                {shipped ? (
                  <Translate id="losos.home.soon.chip.done">Done</Translate>
                ) : (
                  <Translate id="losos.home.soon.chip">Soon</Translate>
                )}
              </span>
              <h3 className={styles.itemTitle}>
                {shipped ? <s>{f.title}</s> : f.title}
              </h3>
              <p className={styles.itemBody}>{f.body}</p>
              <ul className={styles.parts}>
                {f.parts.map((p, j) => (
                  <li key={j} className={clsx(styles.part, p.done && styles.partDone)}>
                    {p.done ? (
                      <>
                        <span className={styles.srOnly}>
                          <Translate id="losos.home.soon.done">Done:</Translate>{' '}
                        </span>
                        <s>{p.text}</s>
                      </>
                    ) : (
                      p.text
                    )}
                  </li>
                ))}
              </ul>
            </Link>
          );
        })}
      </div>
    </section>
  );
}

export function Requirements(): ReactNode {
  return (
    <section className={styles.section}>
      <div className={styles.needs}>
        <h2 className={styles.sectionTitle}>
          <Translate id="losos.home.needs.title">What you need</Translate>
        </h2>
        <ul className={styles.needsList}>
          <li>
            <Translate id="losos.home.needs.cpu">
              A machine with a 64-bit Intel or AMD processor (x86_64) and UEFI
              or BIOS firmware.
            </Translate>
          </li>
          <li>
            <Translate id="losos.home.needs.memory">
              At least 4 GiB of memory and one disk of 40 GiB or more.
            </Translate>
          </li>
          <li>
            <Translate id="losos.home.needs.tpm">
              Ideally a TPM 2.0 chip, which most office mini-PCs have. Without
              one the box still installs and unlocks with a key file.
            </Translate>
          </li>
          <li>
            <Translate id="losos.home.needs.install">
              For the install only: a USB stick of 2 GiB, a wired network with
              internet access, a screen and a keyboard.
            </Translate>
          </li>
        </ul>
        <p className={styles.warning}>
          <Translate id="losos.home.needs.arm">
            LosOS runs on x86_64 only. It does not install on ARM machines,
            and in particular not on Apple Silicon (M-series) Macs.
          </Translate>
        </p>
        <p className={styles.prose}>
          <Translate id="losos.home.needs.vm">
            You can try it in a virtual machine first. QEMU, virt-manager and
            VirtualBox all work.
          </Translate>
        </p>
      </div>
    </section>
  );
}

/* The closing card: a heading, a sentence and the two buttons that end
 * each page. */
export function Closing({
  title,
  body,
  primary,
  secondary,
}: {
  title: ReactNode;
  body: ReactNode;
  primary: Action;
  secondary?: Action;
}): ReactNode {
  return (
    <section className={styles.closing}>
      <h2 className={styles.sectionTitle}>{title}</h2>
      <p className={styles.prose}>{body}</p>
      <Actions primary={primary} secondary={secondary} />
    </section>
  );
}
