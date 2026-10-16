import React, {type ComponentType, type ReactNode, type SVGProps} from 'react';
import Admonition from '@theme/Admonition';
import Layout from '@theme/Layout';
import Link from '@docusaurus/Link';
import Translate, {translate} from '@docusaurus/Translate';
import useDocusaurusContext from '@docusaurus/useDocusaurusContext';

import NightlyCycle from '@site/docs/img/nightly-cycle.svg';
import NightlyCycleSk from '@site/docs/img/nightly-cycle-sk.svg';
import NightlyCycleDe from '@site/docs/img/nightly-cycle-de.svg';
import SetupTypes from '@site/docs/img/setup-types.svg';
import SetupTypesSk from '@site/docs/img/setup-types-sk.svg';
import SetupTypesDe from '@site/docs/img/setup-types-de.svg';

import {
  Closing,
  Hero,
  Requirements,
  Screen,
  Section,
  Soon,
  SOURCE,
  Split,
  Tiles,
  styles,
  type Feature,
} from '@site/src/components/home';

/* LosOS for companies in the EU and the EEA: why a company would keep its
 * cloud on its own box, on the landing page's sections
 * (src/components/home). Public build only, like the landing page.
 *
 * Every product claim here is one the handbook makes and links to. The
 * legal ones name the law: the US CLOUD Act (2018), the Court of Justice's
 * Schrems I (2015) and Schrems II (2020) rulings, the EU-US Data Privacy
 * Framework (2023), GDPR articles 5, 28, 32 and chapter V, NIS2 article 21.
 * The page never says LosOS makes a company compliant. */

type Svg = ComponentType<SVGProps<SVGSVGElement> & {title?: string}>;

function useLocaleSvg(svgs: Record<string, Svg>): Svg {
  const {i18n} = useDocusaurusContext();
  return svgs[i18n.currentLocale] ?? svgs.en;
}

const IN_THE_BOX: Feature[] = [
  {
    title: <Translate id="losos.companies.box.cloud.title">LosOS cloud</Translate>,
    body: (
      <Translate id="losos.companies.box.cloud.body">
        Files, photos, calendar, contacts, notes, tasks, Deck, Forms, Tables
        and more, with desktop and phone apps. Every colleague gets an
        account. Built on Nextcloud.
      </Translate>
    ),
    to: '/manual/apps',
  },
  {
    title: <Translate id="losos.companies.box.git.title">LosOS Git</Translate>,
    body: (
      <Translate id="losos.companies.box.git.body">
        Repositories, issues and pull requests. Your source code stops living
        on someone else's servers. Built on Forgejo.
      </Translate>
    ),
    to: '/manual/apps',
  },
  {
    title: (
      <Translate id="losos.companies.box.disk.title">A disk that is useless when stolen</Translate>
    ),
    body: (
      <Translate id="losos.companies.box.disk.body">
        The data volume is encrypted and its key is sealed in the machine's
        own TPM chip. Pull the disk and plug it in elsewhere, and nobody can
        read it.
      </Translate>
    ),
    to: '/reference/tpm',
  },
  {
    title: <Translate id="losos.companies.box.forget.title">A system that forgets</Translate>,
    body: (
      <Translate id="losos.companies.box.forget.body">
        The system disk lives in memory and is built again on every restart.
        Whatever changed outside a short list of kept folders is gone the
        next morning.
      </Translate>
    ),
    to: '/reference/what-survives-a-reboot',
  },
  {
    title: (
      <Translate id="losos.companies.box.history.title">Every setting is a commit</Translate>
    ),
    body: (
      <Translate id="losos.companies.box.history.body">
        The box keeps its whole description in its own Git repository. Every
        change is a commit on the History pane, so you always know what
        changed and when.
      </Translate>
    ),
    to: '/manual/settings-and-apply',
  },
  {
    title: <Translate id="losos.companies.box.shell.title">Nothing to log in to</Translate>,
    body: (
      <Translate id="losos.companies.box.shell.body">
        No shell and no SSH. The admin pages answer only on your own network,
        and the terminal client signs every call it makes with OpenPGP.
      </Translate>
    ),
    to: '/reference/security-model',
  },
  {
    title: <Translate id="losos.companies.box.ai.title">AI that stays in the building</Translate>,
    body: (
      <Translate id="losos.companies.box.ai.body">
        Build with AI writes widgets with a language model that runs on the
        box itself. What you type never leaves it, and there is no bill per
        word.
      </Translate>
    ),
    to: '/in-depth/widget-builder',
  },
  {
    title: <Translate id="losos.companies.box.search.title">Search inside every file</Translate>,
    body: (
      <Translate id="losos.companies.box.search.body">
        The box indexes the text in your Word, Excel, PowerPoint, OpenDocument
        and PDF files. Find a contract by a sentence in it, and the index never
        leaves the box.
      </Translate>
    ),
    to: '/manual/admin-pages#search',
  },
  {
    title: <Translate id="losos.companies.box.backup.title">Backups you hold the key to</Translate>,
    body: (
      <Translate id="losos.companies.box.backup.body">
        Copies go to any S3-compatible bucket you choose, at an EU provider or
        on your own network, encrypted with your recovery code before they
        leave the box.
      </Translate>
    ),
    to: '/manual/backup-and-erase',
  },
];

const EUROPEAN: Feature[] = [
  {
    title: 'Nextcloud',
    body: (
      <Translate id="losos.companies.eu.nextcloud">
        The base of LosOS cloud. Developed by Nextcloud GmbH in Stuttgart,
        Germany.
      </Translate>
    ),
    to: 'https://nextcloud.com',
  },
  {
    title: 'Forgejo',
    body: (
      <Translate id="losos.companies.eu.forgejo">
        The base of LosOS Git. A community project under Codeberg e.V., a
        non-profit in Berlin.
      </Translate>
    ),
    to: 'https://forgejo.org',
  },
  {
    title: 'NixOS',
    body: (
      <Translate id="losos.companies.eu.nixos">
        The system every box builds itself from. Stewarded by the NixOS
        Foundation in the Netherlands.
      </Translate>
    ),
    to: 'https://nixos.org',
  },
];

function WhereDataLives(): ReactNode {
  const rows: [ReactNode, ReactNode][] = [
    [
      <Translate id="losos.companies.where.files.what">
        Files, photos, calendars and contacts in LosOS cloud
      </Translate>,
      <Translate id="losos.companies.where.files.where">
        On the box, on its encrypted disk
      </Translate>,
    ],
    [
      <Translate id="losos.companies.where.code.what">
        Code, issues and pull requests in LosOS Git
      </Translate>,
      <Translate id="losos.companies.where.code.where">On the box</Translate>,
    ],
    [
      <Translate id="losos.companies.where.ai.what">What you type to Build with AI</Translate>,
      <Translate id="losos.companies.where.ai.where">
        On the box, where the model runs
      </Translate>,
    ],
    [
      <Translate id="losos.companies.where.admin.what">The admin pages</Translate>,
      <Translate id="losos.companies.where.admin.where">Your own network only</Translate>,
    ],
    [
      <Translate id="losos.companies.where.backup.what">Backups, if you set them up</Translate>,
      <Translate id="losos.companies.where.backup.where">
        A bucket you pick, encrypted on the box before they leave
      </Translate>,
    ],
    [
      <Translate id="losos.companies.where.remote.what">
        Remote access, if you set it up
      </Translate>,
      <Translate id="losos.companies.where.remote.where">Your own edge server</Translate>,
    ],
    [
      <Translate id="losos.companies.where.updates.what">Updates</Translate>,
      <Translate id="losos.companies.where.updates.where">
        Public source code comes in from GitHub. Your data does not go out.
      </Translate>,
    ],
  ];
  return (
    <Section
      title={<Translate id="losos.companies.where.title">Where your data lives</Translate>}>
      <div className="markdown">
        <table>
          <thead>
            <tr>
              <th>
                <Translate id="losos.companies.where.head.what">What</Translate>
              </th>
              <th>
                <Translate id="losos.companies.where.head.where">Where it is</Translate>
              </th>
            </tr>
          </thead>
          <tbody>
            {rows.map(([what, where], i) => (
              <tr key={i}>
                <td>{what}</td>
                <td>{where}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </Section>
  );
}

function WhyNotUs(): ReactNode {
  return (
    <Split
      title={
        <Translate id="losos.companies.why.title">
          A server in Frankfurt does not make a US cloud European
        </Translate>
      }
      aside={
        <div className="markdown">
          <table>
            <thead>
              <tr>
                <th>
                  <Translate id="losos.companies.why.head.year">Year</Translate>
                </th>
                <th>
                  <Translate id="losos.companies.why.head.basis">
                    What EU-US transfers rested on
                  </Translate>
                </th>
              </tr>
            </thead>
            <tbody>
              <tr>
                <td>2015</td>
                <td>
                  <Translate id="losos.companies.why.2015">
                    Safe Harbor, struck down by the Court of Justice (Schrems I)
                  </Translate>
                </td>
              </tr>
              <tr>
                <td>2020</td>
                <td>
                  <Translate id="losos.companies.why.2020">
                    Privacy Shield, struck down by the Court of Justice (Schrems II)
                  </Translate>
                </td>
              </tr>
              <tr>
                <td>2023</td>
                <td>
                  <Translate id="losos.companies.why.2023">
                    The Data Privacy Framework, in force today
                  </Translate>
                </td>
              </tr>
              <tr>
                <td>LosOS</td>
                <td>
                  <Translate id="losos.companies.why.losos">
                    No transfer at all. Nothing on the box crosses a border.
                  </Translate>
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      }>
      <p className={styles.prose}>
        <Translate id="losos.companies.why.cloudAct">
          The US CLOUD Act of 2018 lets American authorities require a US
          provider to hand over data in its possession, custody or control.
          Where the server stands does not matter. Who owns the provider does.
        </Translate>
      </p>
      <p className={styles.prose}>
        <Translate id="losos.companies.why.dpf">
          Personal data that goes to the US today rests on the EU-US Data
          Privacy Framework. The EU Court of Justice struck down both
          frameworks before it. A company whose files sit with a US provider
          is betting that the third one holds.
        </Translate>
      </p>
      <p className={styles.prose}>
        <Translate id="losos.companies.why.losos.body">
          With LosOS the data stays in your building, and there is no bet to
          make.
        </Translate>
      </p>
    </Split>
  );
}

function EveryNight(): ReactNode {
  const Night = useLocaleSvg({en: NightlyCycle, sk: NightlyCycleSk, de: NightlyCycleDe});
  return (
    <Split
      title={
        <Translate id="losos.companies.night.title">
          Patch day is every night, and nobody has to remember it
        </Translate>
      }
      aside={
        <Night
          className={styles.diagram}
          role="img"
          title={translate({
            id: 'losos.companies.night.alt',
            message: 'A night on a box: restart at 00:07, update at 03:00, clean-up at 04:30.',
          })}
        />
      }>
      <p className={styles.prose}>
        <Translate id="losos.companies.night.body">
          At 00:07 every box restarts and builds its system disk again from its
          description. At 03:00 it takes the newest LosOS, keeps your settings
          and rebuilds on top. If that fails, the previous system keeps running
          and the box tries again the next night. At 04:30 it deletes versions
          older than 14 days, so the disk never fills itself up.
        </Translate>
      </p>
      <p className={styles.prose}>
        <Translate id="losos.companies.night.fork">
          Want your IT to read every update first? The update source is one
          setting, so your boxes can follow your own copy of LosOS.
        </Translate>
      </p>
      <Link className={styles.more} to="/manual/updates-and-reboots">
        <Translate id="losos.companies.night.link">Updates and reboots</Translate>
      </Link>
    </Split>
  );
}

function ForTheAuditor(): ReactNode {
  const rows: [string, ReactNode, ReactNode][] = [
    [
      'GDPR Art. 32(1)(a)',
      <Translate id="losos.companies.law.encryption.ask">
        Encryption of personal data
      </Translate>,
      <Translate id="losos.companies.law.encryption.does">
        The data volume is encrypted with its key in the TPM. The box serves
        HTTPS on your network with its own certificate, and its tunnel to an
        edge is encrypted end to end.
      </Translate>,
    ],
    [
      'GDPR Art. 28, Chapter V',
      <Translate id="losos.companies.law.transfer.ask">
        Processors and transfers to third countries
      </Translate>,
      <Translate id="losos.companies.law.transfer.does">
        Data on the box has no cloud provider processing it and never leaves
        the EEA, so there is no processing agreement or transfer assessment to
        write for it.
      </Translate>,
    ],
    [
      'GDPR Art. 5(1)(e)',
      <Translate id="losos.companies.law.storage.ask">Storage limitation</Translate>,
      <Translate id="losos.companies.law.storage.does">
        The system log keeps one month and then deletes itself. Container logs
        are capped at three files of 10 MiB each, and build logs go after 30
        days.
      </Translate>,
    ],
    [
      'NIS2 Art. 21(2)(c)',
      <Translate id="losos.companies.law.backup.ask">
        Backup management and disaster recovery
      </Translate>,
      <Translate id="losos.companies.law.backup.does">
        Encrypted backups to a bucket you choose. The recovery code brings
        everything back, on the same box or on a new one.
      </Translate>,
    ],
    [
      'NIS2 Art. 21(2)(d)',
      <Translate id="losos.companies.law.supply.ask">Supply chain security</Translate>,
      <Translate id="losos.companies.law.supply.does">
        All of LosOS is public source under the AGPL. Every box builds from
        pinned inputs in one lock file, so you can check what runs.
      </Translate>,
    ],
    [
      'NIS2 Art. 21(2)(i)',
      <Translate id="losos.companies.law.access.ask">Access control</Translate>,
      <Translate id="losos.companies.law.access.does">
        The admin pages answer only on your own network, even with remote
        access on. Ten failed sign-ins throttle an address, and an audit log
        records setting changes, password resets and resets.
      </Translate>,
    ],
  ];
  return (
    <Section
      title={
        <Translate id="losos.companies.law.title">
          For your data protection officer and your auditor
        </Translate>
      }>
      <div className="markdown">
        <table>
          <thead>
            <tr>
              <th>
                <Translate id="losos.companies.law.head.ask">What the law asks</Translate>
              </th>
              <th>
                <Translate id="losos.companies.law.head.does">What LosOS does</Translate>
              </th>
            </tr>
          </thead>
          <tbody>
            {rows.map(([ref, ask, does], i) => (
              <tr key={i}>
                <td>
                  <strong>{ref}</strong>
                  <br />
                  {ask}
                </td>
                <td>{does}</td>
              </tr>
            ))}
          </tbody>
        </table>
        <Admonition
          type="note"
          title={translate({id: 'losos.companies.law.note.title', message: 'No software makes you compliant'})}>
          <p>
            <Translate id="losos.companies.law.note.body">
              LosOS does not claim to. It gives you measures that already work,
              so your records describe something real.
            </Translate>
          </p>
        </Admonition>
      </div>
    </Section>
  );
}

function OwnEdge(): ReactNode {
  const Types = useLocaleSvg({en: SetupTypes, sk: SetupTypesSk, de: SetupTypesDe});
  return (
    <Split
      title={
        <Translate id="losos.companies.edge.title">Your boxes, your edge, your domain</Translate>
      }
      aside={
        <Types
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
        <Translate id="losos.companies.edge.body">
          Run an edge server of your own, in the office or on a server you rent
          from an EU host. Boxes on the same network find it by themselves
          within one 20-second scan.
        </Translate>
      </p>
      <ul className={styles.needsList}>
        <li>
          <Translate id="losos.companies.edge.domain">
            Every box answers under your company's domain, with no port opened
            on any box. The boxes dial out, and nothing dials in.
          </Translate>
        </li>
        <li>
          <Translate id="losos.companies.edge.keys">
            The tunnel keys, the cluster token and the list of allowed boxes
            live on your edge. None of your traffic touches the LosOS project.
          </Translate>
        </li>
        <li>
          <Translate id="losos.companies.edge.lan">
            The admin pages of every box stay on your own network.
          </Translate>
        </li>
      </ul>
      <p className={styles.prose}>
        <Translate id="losos.companies.edge.optin">
          Boxes can lend each other spare disk and CPU through your edge. It is
          off by default, each box's owner switches it on, and it runs only in
          the hours they choose and only while the box is idle.
        </Translate>
      </p>
      <Link className={styles.more} to="/types/company-edge">
        <Translate id="losos.companies.edge.link">A company with its own edge</Translate>
      </Link>
    </Split>
  );
}

export default function Companies(): ReactNode {
  return (
    <Layout
      title={translate({id: 'losos.companies.pageTitle', message: 'LosOS for companies'})}
      description={translate({
        id: 'losos.companies.description',
        message:
          "LosOS keeps a company's files, calendars and code on an encrypted box in its own office, under EU law, with no US cloud provider in between.",
      })}>
      <main className={styles.page}>
        <Hero
          eyebrow={
            <Translate id="losos.companies.eyebrow">For companies in the EU and the EEA</Translate>
          }
          title={<Translate id="losos.companies.title">LosOS for companies</Translate>}
          lead={
            <Translate id="losos.companies.lead">
              Your company's cloud, in your own office, under EU law.
            </Translate>
          }
          body={
            <Translate id="losos.companies.body">
              LosOS turns an office mini-PC into a private cloud for your
              team's files, calendars, contacts and code. The data sits on an
              encrypted disk in a building you control. No US cloud provider
              holds a copy, so no order served on one can hand it over.
            </Translate>
          }
          primary={{
            to: '/types/company-edge',
            label: <Translate id="losos.companies.cta.plan">Plan a company setup</Translate>,
          }}
          secondary={{
            to: '/start/install',
            label: <Translate id="losos.home.cta.install">Install LosOS</Translate>,
          }}
          fine={
            <Translate id="losos.home.fine">
              Free software under the AGPL. Runs on 64-bit Intel and AMD
              computers.
            </Translate>
          }
        />
        <Screen />
        <WhereDataLives />
        <WhyNotUs />
        <Section
          title={
            <Translate id="losos.companies.box.title">
              Everything your team rents today, on hardware you own
            </Translate>
          }>
          <Tiles items={IN_THE_BOX} />
        </Section>
        <EveryNight />
        <ForTheAuditor />
        <OwnEdge />
        <Section
          title={
            <Translate id="losos.companies.eu.title">
              Built on software from Germany and the Netherlands
            </Translate>
          }>
          <p className={styles.prose}>
            <Translate id="losos.companies.eu.languages">
              The admin pages and this handbook come in English, Slovak and
              German. LosOS cloud and LosOS Git follow each person's browser
              language and carry many more.
            </Translate>
          </p>
          <Tiles items={EUROPEAN} />
        </Section>
        <Section title={<Translate id="losos.companies.cost.title">No fee per seat</Translate>}>
          <p className={styles.prose}>
            <Translate id="losos.companies.cost.body">
              LosOS, Nextcloud and Forgejo are free software. Your fortieth
              hire costs a user account, not a subscription. You pay once for
              the hardware and then for the electricity, and an office mini-PC
              does the job.
            </Translate>
          </p>
        </Section>
        <Soon>
          <p className={styles.prose}>
            <Translate id="losos.companies.soon.intro">
              LosOS is pre-release software, with no support contract and no
              certification yet. These are the parts still being built.
            </Translate>
          </p>
        </Soon>
        <Requirements />
        <Closing
          title={
            <Translate id="losos.companies.closing.title">Set up LosOS for your company</Translate>
          }
          body={
            <Translate id="losos.companies.closing.body">
              The company setup page shows boxes for each team, one edge for the
              office, and how the install goes. Try a single box first, in a
              virtual machine if you like.
            </Translate>
          }
          primary={{
            to: '/types/company-edge',
            label: <Translate id="losos.companies.cta.plan">Plan a company setup</Translate>,
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
