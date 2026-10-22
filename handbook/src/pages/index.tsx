import React, {type ReactNode} from 'react';
import Link from '@docusaurus/Link';
import Layout from '@theme/Layout';
import Translate, {translate} from '@docusaurus/Translate';

import SetupTypes from '@site/docs/img/setup-types.svg';

import {
  Closing,
  Hero,
  Requirements,
  Screen,
  Section,
  Soon,
  SOURCE,
  Split,
  Steps,
  Tiles,
  styles,
  type Feature,
  type Step,
  RELEASES,
} from '@site/src/components/home';

/* The public site's front page: what LosOS is, for someone who has never
 * seen a box, and the way into the handbook. Only the public build has it
 * (docusaurus.config.ts turns src/pages off for the box's own copy, whose
 * root stays the handbook's Welcome page, at /welcome here).
 *
 * Every claim on it is one the handbook makes in more words, and links to:
 * the features from "What LosOS is", the requirements from "Install". If a
 * fact changes there, change it here too. The sections are shared with
 * LosOS for companies (src/components/home). */

const FEATURES: Feature[] = [
  {
    title: <Translate id="losos.home.feature.cloud.title">LosOS Cloud</Translate>,
    body: (
      <Translate id="losos.home.feature.cloud.body">
        Your files, photos, calendars, contacts and boards, in the browser
        and in the Nextcloud desktop and phone apps.
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
      <Translate id="losos.home.feature.self.title">A fresh system every morning</Translate>
    ),
    body: (
      <Translate id="losos.home.feature.self.body">
        It keeps its state across restarts.
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
        Your data is on an encrypted volume. On a machine with a TPM chip, the
        key stays in the chip.
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
        There is no login prompt and no SSH. You use the box from a web
        browser, and one password, set when you claim the box, opens the
        admin pages, LosOS Cloud and LosOS Git.
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
        Your files, your apps and the admin pages keep working over the local
        network when the internet is down. The box also serves its own copy of
        this handbook, with search.
      </Translate>
    ),
    to: '/troubleshooting/only-the-lan-works',
  },
];

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

export default function Home(): ReactNode {
  return (
    <Layout
      title={translate({id: 'losos.home.pageTitle', message: 'A box for your files that looks after itself'})}
      description={translate({
        id: 'losos.home.description',
        message:
          'LosOS turns a small 64-bit PC into your own cloud for files, photos, calendar and code. It restarts every night and installs its own updates.',
      })}>
      <main className={styles.page}>
        <Hero
          eyebrow={
            <Translate id="losos.home.eyebrow">
              For a small computer at home or in the office
            </Translate>
          }
          title="LosOS"
          lead={
            <Translate id="losos.home.lead">
              A box for your files that looks after itself.
            </Translate>
          }
          body={
            <Translate id="losos.home.body">
              LosOS turns an old office mini-PC, or any small 64-bit PC, into
              your own cloud. You install it once from a USB stick and then use
              it from a web browser. It restarts every night and installs its
              own updates.
            </Translate>
          }
          primary={{
            to: '/start/install',
            label: <Translate id="losos.home.cta.install">Install LosOS</Translate>,
          }}
          secondary={{
            to: '/welcome',
            label: <Translate id="losos.home.cta.handbook">Read the handbook</Translate>,
          }}
          fine={
            <Translate id="losos.home.fine">
              Free software under the AGPL. Runs on 64-bit Intel and AMD
              computers.
            </Translate>
          }
        />
        <Screen />
        <Section title={<Translate id="losos.home.features.title">What you get</Translate>}>
          <Tiles items={FEATURES} />
        </Section>
        <Section
          title={
            <Translate id="losos.home.steps.title">Three steps to a running box</Translate>
          }>
          <Steps items={STEPS} />
        </Section>
        <Split
          title={<Translate id="losos.home.setups.title">From one box to a company</Translate>}
          aside={
            <SetupTypes
              className={styles.diagram}
              role="img"
              title={translate({
                id: 'losos.home.setups.alt',
                message:
                  'Three setup types side by side: a box on its own, a box with the LosOS edge, and a company with its own edge.',
              })}
            />
          }>
          <p className={styles.prose}>
            <Translate id="losos.home.setups.body">
              Every box starts on its own, on a home network. Later it can get
              a public address through an edge server, with no port opened at
              home. It can also lend its spare disk and CPU to other boxes,
              only in the hours you choose. A company can run several boxes
              behind its own edge. Each of these is a change of settings, not
              a reinstall.
            </Translate>
          </p>
          <p className={styles.links}>
            <Link className={styles.more} to="/types">
              <Translate id="losos.home.setups.link">Types of setup</Translate>
            </Link>
            <Link className={styles.more} to="/companies">
              <Translate id="losos.home.setups.companies">LosOS for companies</Translate>
            </Link>
          </p>
        </Split>
        <Soon />
        <Requirements />
        <Closing
          title={<Translate id="losos.home.closing.title">Get the installer</Translate>}
          body={
            <Translate id="losos.home.closing.body">
              Each release carries the installer ISO and its checksum. The
              handbook's Install page shows how to write it to a USB stick.
            </Translate>
          }
          primary={{
            href: RELEASES,
            label: (
              <Translate id="losos.home.closing.download">Download the latest release</Translate>
            ),
          }}
          secondary={{
            href: SOURCE,
            label: <Translate id="losos.home.closing.source">Source on GitHub</Translate>,
          }}
        />
      </main>
    </Layout>
  );
}
