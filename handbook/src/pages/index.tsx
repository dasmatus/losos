import React, {type ReactNode} from 'react';
import Link from '@docusaurus/Link';
import Layout from '@theme/Layout';
import ThemedImage from '@theme/ThemedImage';
import Translate, {translate} from '@docusaurus/Translate';
import useBaseUrl from '@docusaurus/useBaseUrl';

import overviewLight from '@site/docs/img/overview.png';
import overviewDark from '@site/docs/img/overview-dark.png';
import SetupTypes from '@site/docs/img/setup-types.svg';

import styles from './index.module.css';

/* The public site's front page: what LosOS is, for someone who has never
 * seen a box, and the way into the handbook. Only the public build has it
 * (docusaurus.config.ts turns src/pages off for the box's own copy, whose
 * root stays the handbook's Welcome page, at /welcome here).
 *
 * Every claim on it is one the handbook makes in more words, and links to:
 * the features from "What LosOS is", the requirements from "Install". If a
 * fact changes there, change it here too. */

const RELEASES = 'https://github.com/dasmatus/losos/releases/latest';
const SOURCE = 'https://github.com/dasmatus/losos';

type Feature = {title: ReactNode; body: ReactNode; to: string};

const FEATURES: Feature[] = [
  {
    title: <Translate id="losos.home.feature.cloud.title">LosOS cloud</Translate>,
    body: (
      <Translate id="losos.home.feature.cloud.body">
        Your files, photos, calendar, contacts, notes and tasks, with desktop
        and phone apps. Built on Nextcloud.
      </Translate>
    ),
    to: '/start/what-is-losos',
  },
  {
    title: <Translate id="losos.home.feature.git.title">LosOS Git</Translate>,
    body: (
      <Translate id="losos.home.feature.git.body">
        Your code repositories, on the same box and behind the same password.
        Built on Forgejo.
      </Translate>
    ),
    to: '/manual/apps',
  },
  {
    title: (
      <Translate id="losos.home.feature.self.title">It looks after itself</Translate>
    ),
    body: (
      <Translate id="losos.home.feature.self.body">
        It updates itself at night and builds its system disk from scratch on
        every boot. Whatever went wrong yesterday is gone by the morning.
      </Translate>
    ),
    to: '/manual/updates-and-reboots',
  },
  {
    title: (
      <Translate id="losos.home.feature.disk.title">An encrypted disk</Translate>
    ),
    body: (
      <Translate id="losos.home.feature.disk.body">
        The disk key stays in the machine's own TPM chip. A disk pulled out
        and plugged into another computer cannot be read.
      </Translate>
    ),
    to: '/types/install-variants',
  },
  {
    title: (
      <Translate id="losos.home.feature.password.title">One password, no shell</Translate>
    ),
    body: (
      <Translate id="losos.home.feature.password.body">
        No login prompt and no SSH. You use the box from a web browser, and
        the password of your cloud also opens its admin pages.
      </Translate>
    ),
    to: '/manual/sign-in-and-spare-key',
  },
  {
    title: (
      <Translate id="losos.home.feature.lan.title">Works without the internet</Translate>
    ),
    body: (
      <Translate id="losos.home.feature.lan.body">
        Everything answers on your local network. The box even carries this
        handbook, search included, for the day nothing else does.
      </Translate>
    ),
    to: '/troubleshooting/only-the-lan-works',
  },
];

type Step = {title: ReactNode; body: ReactNode; to: string; link: ReactNode};

const STEPS: Step[] = [
  {
    title: <Translate id="losos.home.step.install.title">Install it</Translate>,
    body: (
      <Translate id="losos.home.step.install.body">
        Write the installer to a USB stick and boot the machine from it. It
        asks one question, wipes the disks and installs in about half an hour.
      </Translate>
    ),
    to: '/start/install',
    link: <Translate id="losos.home.step.install.link">Install</Translate>,
  },
  {
    title: <Translate id="losos.home.step.first.title">Open it in a browser</Translate>,
    body: (
      <Translate id="losos.home.step.first.body">
        The box's screen shows its address. Open it on any computer on the
        same network and a three-step wizard sets your password.
      </Translate>
    ),
    to: '/start/first-run',
    link: <Translate id="losos.home.step.first.link">The first run</Translate>,
  },
  {
    title: <Translate id="losos.home.step.use.title">Use it</Translate>,
    body: (
      <Translate id="losos.home.step.use.body">
        Put your files in and connect your phone and your computer. From
        then on the box needs no screen and no keyboard.
      </Translate>
    ),
    to: '/manual/admin-pages',
    link: <Translate id="losos.home.step.use.link">The admin pages</Translate>,
  },
];

function Hero(): ReactNode {
  const plate = useBaseUrl('/img/plate.png');
  const plateTip = translate({
    id: 'losos.navbar.logoTip',
    message: "Losos is Slovak for salmon. The logo is a plate of it, from the owner's own photo.",
    description: 'The tooltip on the navbar logo, saying what the picture means',
  });
  return (
    <header className={styles.hero}>
      <div className={styles.heroText}>
        <p className={styles.eyebrow}>
          <Translate id="losos.home.eyebrow">
            Free software for a small computer at home or at work
          </Translate>
        </p>
        <h1 className={styles.heroTitle}>LosOS</h1>
        <p className={styles.heroLead}>
          <Translate id="losos.home.lead">
            A box for your files that looks after itself.
          </Translate>
        </p>
        <p className={styles.heroBody}>
          <Translate id="losos.home.body">
            LosOS turns a small computer, such as an old office mini-PC, into
            your own cloud. You install it once from a USB stick. After that
            you use it from a web browser, and it updates, repairs and
            restarts itself every night.
          </Translate>
        </p>
        <div className={styles.actions}>
          <Link className={styles.primary} to="/start/install">
            <Translate id="losos.home.cta.install">Install LosOS</Translate>
          </Link>
          <Link className={styles.secondary} to="/welcome">
            <Translate id="losos.home.cta.handbook">Read the handbook</Translate>
          </Link>
        </div>
        <p className={styles.fine}>
          <Translate id="losos.home.fine">
            AGPL licensed. Runs on 64-bit Intel and AMD machines.
          </Translate>
        </p>
      </div>
      <img
        className={styles.plate}
        src={plate}
        width={240}
        height={240}
        alt={plateTip}
        title={plateTip}
      />
    </header>
  );
}

function Screen(): ReactNode {
  return (
    <figure className={styles.screen}>
      <ThemedImage
        className={styles.screenImage}
        sources={{light: overviewLight, dark: overviewDark}}
        alt={translate({
          id: 'losos.home.screen.alt',
          message:
            'The admin pages of a box: the app tiles for LosOS cloud and LosOS Git, the storage card and a board of widgets.',
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

function Features(): ReactNode {
  return (
    <section className={styles.section}>
      <h2 className={styles.sectionTitle}>
        <Translate id="losos.home.features.title">What you get</Translate>
      </h2>
      <div className={styles.grid}>
        {FEATURES.map((f, i) => (
          <Link key={i} to={f.to} className={styles.tile}>
            <h3 className={styles.itemTitle}>{f.title}</h3>
            <p className={styles.itemBody}>{f.body}</p>
          </Link>
        ))}
      </div>
    </section>
  );
}

function Steps(): ReactNode {
  return (
    <section className={styles.section}>
      <h2 className={styles.sectionTitle}>
        <Translate id="losos.home.steps.title">Three steps to a running box</Translate>
      </h2>
      <ol className={styles.steps}>
        {STEPS.map((s, i) => (
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
    </section>
  );
}

function Setups(): ReactNode {
  return (
    <section className={styles.section}>
      <div className={styles.split}>
        <div>
          <h2 className={styles.sectionTitle}>
            <Translate id="losos.home.setups.title">From one box to a company</Translate>
          </h2>
          <p className={styles.prose}>
            <Translate id="losos.home.setups.body">
              Every box starts on its own, on a home network. Later it can get a
              public address through an edge server, without opening a port at
              home, and lend its spare disk and CPU to other boxes in the hours
              you choose. A company can run several boxes behind its own edge.
              You get there by changing settings, not by reinstalling.
            </Translate>
          </p>
          <Link className={styles.more} to="/types">
            <Translate id="losos.home.setups.link">Types of setup</Translate>
          </Link>
        </div>
        <SetupTypes
          className={styles.diagram}
          role="img"
          title={translate({
            id: 'losos.home.setups.alt',
            message:
              'Three setup types side by side: a box on its own, a box with the LosOS edge, and a company with its own edge.',
          })}
        />
      </div>
    </section>
  );
}

function Requirements(): ReactNode {
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
            To try it first, run it in a virtual machine: QEMU, virt-manager
            and VirtualBox all work.
          </Translate>
        </p>
      </div>
    </section>
  );
}

function Closing(): ReactNode {
  return (
    <section className={styles.closing}>
      <h2 className={styles.sectionTitle}>
        <Translate id="losos.home.closing.title">Get the installer</Translate>
      </h2>
      <p className={styles.prose}>
        <Translate id="losos.home.closing.body">
          Each release carries the installer ISO and its checksum. The
          handbook takes you from the download to a running box.
        </Translate>
      </p>
      <div className={styles.actions}>
        <Link className={styles.primary} href={RELEASES}>
          <Translate id="losos.home.closing.download">Download the latest release</Translate>
        </Link>
        <Link className={styles.secondary} href={SOURCE}>
          <Translate id="losos.home.closing.source">Source on GitHub</Translate>
        </Link>
      </div>
    </section>
  );
}

export default function Home(): ReactNode {
  return (
    <Layout
      title={translate({id: 'losos.home.pageTitle', message: 'A box for your files that looks after itself'})}
      description={translate({
        id: 'losos.home.description',
        message:
          'LosOS turns a small x86_64 computer into your own cloud for files, photos, calendar and code. It updates, repairs and restarts itself every night.',
      })}>
      <main className={styles.page}>
        <Hero />
        <Screen />
        <Features />
        <Steps />
        <Setups />
        <Requirements />
        <Closing />
      </main>
    </Layout>
  );
}
