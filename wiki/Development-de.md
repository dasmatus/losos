[English](Development) · [Slovenčina](Development-sk) · **Deutsch**

# Entwicklung

## Shell

```sh
devenv shell        # or: direnv allow
devenv test         # pin checks, lint, rust tests, flake eval (no builds)
```

`nix develop` liefert nur die Toolchain, für alle ohne devenv.

| Skript                         | Was es tut |
| ------------------------------ | ---- |
| `fmt` · `lint` · `test-rust`   | Formatiert, lintet und testet beide Rust-Crates |
| `check-flake` · `check-eval`   | Wertet den Flake aus; erzwingt den vollständigen Modul-Merge |
| `check-pins`                   | Schlägt fehl, wenn `flake.lock` und `devenv.lock` verschiedene nixpkgs pinnen |
| `build-pkgs` · `build-iso`     | Baut die Pakete; baut das Installer-ISO |
| `build-images` · `build-media` | OCI-Images; Demo-QCOW2 und Closure-ISO (groß, optional) |
| `vm-tests`                     | Alle NixOS-VM-Tests (braucht `/dev/kvm`) |

`devenv test` baut weder Pakete noch das Installer-ISO und führt keine
VM-Tests aus. Dafür gibt es eigene, optionale Skripte.

## Rust

```sh
cargo test   --manifest-path backend/Cargo.toml
cargo clippy --manifest-path backend/Cargo.toml --all-targets -- -D warnings
```

Dasselbe gilt für `backend-registrar/`. Beide Crates müssen clippy und
rustfmt ohne Beanstandung bestehen. Die Nix-Pakete setzen `doCheck = false`,
`nix build` führt also keine Tests aus.

Bei Pushes auf `main` führt ein CI-Job `cargo fmt` aus und committet das
Ergebnis. Clippy-Befunde werden nicht automatisch behoben.

Ein weiterer Job schreibt danach die History so um, dass kein Commit Claude
als Mitautor nennt oder auf eine claude.ai-Session verlinkt, und pusht die
verschobenen Branches und Tags per Force-Push. Hat sich `main` unter dir
geändert, übernimmt `git pull --rebase` die umgeschriebenen Commits sauber.
Ein einfaches `git pull` würde die beiden Historien mergen.

## Admin UI

In `admin-ui/app/`:

```sh
npm run dev           # set LOSOS_API_ORIGIN to a real box
npm run typecheck
npm run test:browser
```

`nix build .#losos-admin-ui` führt `npm ci` und danach
`tsc --noEmit && vite build` aus, die Typprüfung ist also Teil des Pakets.
`npm run test:browser` ist der Playwright-Check, den
`.#checks.x86_64-linux.losos-admin-ui` kapselt.

`tests/advanced.browser.mjs` rendert jede Option, die die Box deklariert.
Außerhalb von Nix liest es `tests/fixtures/options.json`, eine Kopie des
Dokuments, das `flake/options-doc.nix` erzeugt; aktualisiere es nach
Änderungen an `modules/options.nix` mit

```sh
nix build --no-link --print-out-paths .#checks.x86_64-linux.losos-options-doc
cp "$(nix build --no-link --print-out-paths .#checks.x86_64-linux.losos-options-doc)" \
   admin-ui/app/tests/fixtures/options.json
```

`nix flake check` und `tests/admin-ui.nix` verwenden ein frisch erzeugtes
Dokument, eine veraltete Fixture betrifft also nur einen Lauf auf einem
Entwicklerrechner.

## VM-Tests

CI kann sie nicht ausführen (kein KVM auf gehosteten Runnern). Führe
sie lokal aus, bevor du Änderungen an den Modulen, am Daemon oder am
Installer mergst:

```sh
devenv shell vm-tests
nix build .#checks.x86_64-linux.losos-admin-daemon
nix build .#checks.x86_64-linux.losos-install
```

Baue außerdem die System-Closure lokal:

```sh
nix build .#nixosConfigurations.install.config.system.build.toplevel
```

Ein Flake-Check ist keine VM. `losos-invariants` (`tests/invariants.nix`)
wertet die veröffentlichte `install`-Konfiguration aus und prüft die
Optionswerte, die die Appliance nicht durch einen abdriftenden Default
verlieren darf: Garbage Collection, die Obergrenze des Bootmenüs, den
Entsperrmodus, das `#install`-Fragment. `nix flake check --no-build` führt
ihn aus, der Eval-Job in CI schlägt also ohne jeden Build-Aufwand daran fehl.

## Pull Requests

Die Vorlage in `.github/pull_request_template.md` gibt die Beschreibung vor:
Before-/After-Text, eine Screenshot-Tabelle, How, Tested, Hinweise für den
Reviewer. Behalte jeden Abschnitt bei.

Screenshots sind Pflicht für jede Änderung, die ein Mensch sehen kann: die
Admin UI und der Assistent, der Installer und das tty1-Banner, die Themes
von Nextcloud und Forgejo, die Wiki- und Doku-Seiten. Mach pro
Bildschirm ein Before und ein After bei gleicher Fenstergröße und im
gleichen Zustand, sodass der einzige Unterschied die Änderung ist, und
zieh sie in die Tabelle. Eine Änderung ohne sichtbare Wirkung schreibt
unter diese Überschrift "No visible change." und warum.

Führe unter Tested auf, was gelaufen ist und was nicht. Die VM-Tests
brauchen KVM, sag es also, wenn sie nicht gelaufen sind.

## Lock-Dateien und Hashes

- `flake.lock` pinnt das nixpkgs, das die Appliance baut; `devenv.lock`
  dasjenige, das sie lintet und testet. Aktualisiere beide gemeinsam.
- Nach Änderungen an `Cargo.lock` oder `admin-ui/app/package-lock.json`
  aktualisiere `cargoHash` bzw. `npmDepsHash` in `flake/packages.nix`.
  Der Hash der Admin UI steht zusätzlich in `tests/admin-ui.nix`.

## Wiki

Dieses Wiki wird aus `wiki/` im Repository erzeugt. Bearbeite die
Seiten dort. Der Workflow `wiki` veröffentlicht sie bei einem Push auf
`main` und überschreibt alles, was im Wiki-Editor von GitHub bearbeitet
wurde.

## Fallstricke, die lautlos zubeißen

Jeder davon wurde einmal teuer bezahlt. `CLAUDE.md` enthält die einzeilige
Regel, hier steht die Begründung dahinter.

- **Drei Härtungseinstellungen werden bewusst *nicht* angewendet**, und alle
  drei stehen auf jeder Checkliste, von der du abschreiben wollen wirst.
  `rp_filter` ist `2` (lose), nicht `1`. Der strikte Modus verwirft die
  Multicast-Antworten, die eine Box ohne SSH erreichbar machen, und Calico
  funktioniert darunter auch nicht. `user.max_user_namespaces` bleibt
  ungleich null, denn null stoppt beide Kubelets und containerd. `/tmp` ist
  nicht `noexec`, denn Nix-Builds laufen dort, und der nächtliche,
  unbeaufsichtigte Rebuild ist der einzige Weg der Box, sich selbst zu
  reparieren. `tests/hardening.nix` prüft, dass alle drei fehlen, sodass das
  "Reparieren" einer davon einen Test rot färbt, statt eine Box zu bricken.
- **`fileSystems."/tmp"` mountet nichts.** NixOS maskiert `tmp.mount`,
  solange `boot.tmp.useTmpfs` nicht gesetzt ist. Die Deklaration erzeugt
  weder einen Mount noch einen Fehler, und `/tmp` erbt die Optionen des
  Root-Dateisystems. Das herauszufinden hat eine VM-Test-Runde gekostet.
- **Die Preload-Datei des Allocators ist `/etc/ld-nix.so.preload`**, gelesen
  vom gepatchten Loader von NixOS. Das glibc-übliche `/etc/ld.so.preload`
  existiert hier nie. Eine Prüfung auf den Standardpfad besteht, wenn der
  Allocator aus ist, und schlägt fehl, wenn er an ist.
- **`losos-ctl grow` führt drei Befehle aus, und nur ihre Reihenfolge
  zählt.** `resize2fs` fragt das LUKS-*Mapping* nach seiner Größe. Vor
  `cryptsetup resize` ausgeführt, liest es die Größe vor der Vergrößerung,
  gibt "Nothing to do!" aus und endet mit 0. Und `lvextend -l 77` ohne das
  `+` ist eine absolute Anzahl Extents, die ein größeres Volume
  *verkleinert* und ein darauf gemountetes ext4 zerstört.
  - `lososd` braucht lvm2, cryptsetup und e2fsprogs im `path` seiner Unit,
    sonst schlägt der erste Schritt mit "No such file or directory" fehl.
    Dasselbe gilt für `curl`, das `GET /api/apps/search` aufruft. `path`
    **ersetzt** PATH, kein Binary landet also zufällig darin, und ein
    fehlendes erreicht den Besitzer als Funktion, die nie funktioniert und
    nie sagt, warum.
  - `nixos-rebuild` steht aus demselben Grund auf dieser Liste. systemd-run
    löst einen nackten Befehl gegen den PATH des *Aufrufers* auf, ohne ihn
    schlug also jedes Apply fehl, bevor die Rebuild-Unit überhaupt
    existierte.
  - lososd darf kein `ProcSubset=pid` bekommen. Es versteckt
    `/proc/devices`, und vgs endet ohne diese Datei mit 4.
  - `tests/invariants.nix` pinnt die beiden letzten Punkte.
- **Das LUKS-Keyfile ist Hex-Text und muss frei von NUL bleiben.** disko
  übergibt `passwordFile` an `luksFormat` als `<(echo -n "$(cat FILE)")`,
  eine Befehlssubstitution, die NUL-Bytes und einen abschließenden
  Zeilenumbruch verwirft. Die initrd (`/crypto_keyfile.bin`) und
  `systemd-cryptenroll --unlock-key-file` lesen die Datei roh. Der Installer
  schrieb früher 4096 Zufallsbytes, sodass das Volume bei fast jeder
  Installation mit einem Schlüssel formatiert und mit einem anderen entsperrt
  wurde. Nichts stieß auf diesen Fehler, bis `tests/tpm.nix` den Chip
  registrierte. `ensure_keyfile` schreibt jetzt 2048 Zufallsbytes als 4096
  Hex-Zeichen. "Härte" es nicht zurück auf rohe Bytes, und füge
  keinen Zeilenumbruch ein.
- **Die LUKS-Schlüsseldatei darf nicht unter `/root` oder `/home` liegen.**
  `lososd` läuft mit `ProtectHome=true`, absichtlich, damit ein
  kompromittierter Request-Handler keine der beiden Datendomänen lesen kann.
  Das versteckt auch `/root`. `cryptsetup resize` schlägt dann mit "Failed
  to open key file" fehl, *nachdem* `lvextend` das Volume bereits vergrößert
  hat. `/etc/keys/persist-keyfile` ist der Pfad, auf den sich alles einigt.
  `disko.nix` formatiert damit, der Installer hält die Datei auf beiden
  Entsperrpfaden innerhalb von `/persist`, `daemon.nix` übergibt sie als
  `LOSOS_LUKS_KEYFILE`, und `tests/resize.nix` verwendet sie statt einer
  eigenen Fixture, sodass die vier nicht unbemerkt auseinanderlaufen können.
- **`lososd` wird mitten im Rebuild neu gestartet.** `nixos-rebuild switch`
  startet die geänderte `lososd.service` neu und beendet den Watcher-Thread,
  der den Rebuild verfolgt hat. `Daemon.startSupervisor` hängt sich beim
  Start des Daemons wieder an: Steht in `state.json` `Building`, startet er
  einen neuen Watcher für `losos-rebuild-<job>`. Ohne das zeigt der Rebuild
  für immer `building`. Entferne das Wiederanhängen nicht.
- **Die Admin-Routen verweigern Loopback absichtlich** (`lanOnly` in
  `containers.nix`). Der Tunnel-Verkehr des Master-Proxys erreicht den
  Front-vhost *von 127.0.0.1* (`local_addr` von rathole), also darf die
  LAN-only-Sperre auf `/`, `/assets/`, `/setup/` und `/api/` Loopback nicht
  erlauben. Ein `curl localhost/` auf der Box oder in einem VM-Test bekommt
  absichtlich 403. Eine "Reparatur" mit `allow 127.0.0.1` stellt die gesamte
  Admin-Oberfläche ins Internet, sobald `losos.proxy.enable` an ist. Teste
  direkt gegen :8082 von lososd oder per curl mit einer LAN-Quelladresse.
- **Das Passwort des Besitzers entsperrt die Admin-Seiten, und Nextcloud
  entscheidet.** `POST /api/sign-in` (`backend/src/signin.rs`) schickt das
  Passwort über Loopback an Nextcloud (`$LOSOS_NEXTCLOUD_LOGIN_URL`, je Modus
  in `daemon.nix` gesetzt) in einem Basic-Header und antwortet bei 200 mit
  dem Admin-Token. lososd speichert absichtlich keinen Hash des Passworts,
  denn eine zweite Kopie würde abweichen, sobald der Besitzer es in
  Nextcloud ändert. Der Preis ist, dass die Route 503 antwortet, solange
  Nextcloud nicht läuft, und genau dafür ist der ausgedruckte
  Ersatzschlüssel da. Entferne also den Schlüsselweg nicht aus dem
  Entsperrdialog, und mach aus der Anmeldeprüfung kein `curl -u`, das
  das Passwort in einen argv legen würde. Ein neues Passwort braucht 12+
  Zeichen, Groß- und Kleinbuchstaben, eine Ziffer und ein Sonderzeichen
  (`setup::validate_password`). Eine Anmeldung prüft nur die Form des
  Kandidaten (`signin::validate_candidate`). Alles Strengere würde einen
  Besitzer aussperren, dessen Passwort unter älteren Regeln gesetzt wurde.
- **Nextcloud vertraut der eigenen IP-Adresse der Box über einen
  nginx-Header.** Die Location `/nextcloud` setzt
  `X-Losos-Server-Addr $server_addr`, und `losos.config.php` im Pod hängt
  diesen Wert pro Request an `trusted_domains` an, nur IP-Literale. So
  erreicht man eine libvirt-VM, die keinen mDNS-Namen bekommt, unter
  `http://192.168.122.x/nextcloud` ohne "Untrusted domain". Ersetze das
  nicht durch eine Wildcard `192.168.*`. Nextclouds `*` entspricht
  `[-.a-zA-Z0-9]*`, also wäre auch `192.168.attacker.example`
  vertrauenswürdig. `localhost` und `127.0.0.1` brauchen nichts, denn
  Nextcloud vertraut ihnen immer.
- **Die App-Kacheln der Admin UI verlinken über `index.php`.** Nextcloud
  beantwortet `/nextcloud/index.php/apps/<id>/` in jeder Konfiguration. Die
  kurze Form `/nextcloud/apps/<id>/` funktioniert nur, wenn der Apache im Pod
  hübsche URLs umschreibt, und ohne das war es ein Apache-404 direkt von der
  Admin-Startseite. `admin-ui/app/src/lib/apps.ts` verwendet deshalb die
  lange Form. "Räume" die Kacheln nicht auf die kurze zurück.
- **Handgeschriebene Widgets laufen in `/widget-frame/`, nie in der
  Admin-Seite.** `<iframe sandbox="allow-scripts" src="/widget-frame/">`
  zeichnet HTML und Skript des Besitzers (`backend/src/look.rs`,
  `GET/POST /api/look*`). Der Frame liegt in
  `admin-ui/app/src/widgets/hand-frame.tsx`, seine Seite in
  `admin-ui/app/public/widget-frame/`. Zwei Dinge tragen die Last. Es gibt
  kein `allow-same-origin`, der Frame ist also ein opaker Origin ohne Token
  und ohne API. Und der Zweig `~^/widget-frame/` in nginx' Header-Map in
  `containers.nix` ist der einzige Pfad, der unter einer freizügigen CSP
  ausgeliefert wird. Ein `<iframe srcdoc>` oder ein `blob:`-Dokument würde
  die strenge Richtlinie der Admin-Seite erben und das Inline-Skript genauso
  ablehnen, also "vereinfache" den Frame nicht dazu.
  `tests/front-vhost.nix` prüft den Zweig und dass die strenge Richtlinie
  weiterhin alles andere abdeckt.
- **lososd erzeugt das Admin-Token, nicht NixOS.** lososd schreibt
  `losos.admin.tokenFile` (Standard `/var/secrets/losos-admin-token`,
  persistent über `/var`) beim ersten Start, falls es fehlt, als zufälligen
  Wert aus 64 Hex-Zeichen mit Modus 0600. NixOS verwaltet es nicht.
  Versuche nicht, es als Store-Pfad zu deklarieren.
- **`/etc/rancher` muss persistent bleiben** (`impermanence.nix`). `/var`
  deckt den Großteil des Zustands beider Kubernetes-Instanzen ab, aber der
  Agent schreibt bei seinem ersten Join `/etc/rancher/node/password`, und
  der Server speichert einen Hash davon unter dem Node-Namen. Auf einem
  tmpfs-Root wird diese Datei bei jedem Boot neu erzeugt, und der Server
  lehnt den erneuten Join dann ab ("Node password rejected"). Streich
  die Zeile, und die Box fällt beim ersten Neustart nach der Registrierung
  wortlos aus dem Mesh.
- **`--disable`, `--flannel-backend` und `--disable-network-policy` sind
  reine Server-Flags.** `k3s agent` bricht bei einem unbekannten Flag hart
  ab, wer also eines davon an einen Agent übergibt, schickt die Unit in eine
  endlose Crash-Schleife, auf einer Box ohne Shell. Der nixpkgs-eigene Test
  `nixos/tests/rancher/multi-node.nix` gibt seinen Server-Nodes `disable`
  und seinem Agent-Node keines von beiden, und die Beschreibung von `role`
  in rke2 sagt dasselbe. Knüpfe jedes reine Server-Flag an die Rolle.
- **Die beiden Kubernetes-Instanzen sind absichtlich getrennte Cluster.**
  `services.k3s` (Rolle `server`) betreibt Nextcloud und Forgejo *dieser
  Box*. `services.rke2` (Rolle `agent`) tritt dem Mesh der Edge bei. Das
  Kubelet eines Agents kann nicht starten, solange sein Server nicht
  erreichbar ist, und `midnight-reboot.timer` feuert bedingungslos um 00:07.
  Lägen die eigenen Dienste der Box im Cluster der Edge, würde jeder
  Ausfall über Mitternacht sie lahmlegen, also "vereinfache" das nicht
  zu einem Cluster. Es ist rke2 statt eines zweiten k3s, weil nixpkgs beide
  aus einem nach Namen parametrisierten Generator baut, sodass ihre
  Zustandsverzeichnisse (`/var/lib/rancher/{k3s,rke2}`) und Unit-Namen nicht
  kollidieren. Es gibt keine Option `dataDir`, mit der ein zweites k3s
  funktionieren würde.
- **`hostNetwork`-Pods haben keine unterscheidbare Quelladresse.** Der
  lokale Cluster läuft mit `--flannel-backend=none`, seine Pods teilen also
  den Netzwerk-Namespace des Hosts, und nginx sieht sie als `127.0.0.1` oder
  als LAN-IP. Das alte nspawn-Design gab Containern ein eigenes Subnetz, das
  die `lanOnly`-Sperre als Erstes ablehnte. Diese Schicht gibt es nicht
  mehr. Schreib nie eine Regel `deny <podCidr>`, denn sie kann nicht
  greifen.
- **Die Edge erzwingt das Rechenfenster, in der Zeitzone der Appliance.**
  Nur die Edge kann den NoSchedule-Taint schreiben (NodeRestriction lässt
  niemand anderen), also läuft der Vergleich auf einer Maschine, die nicht
  dem Besitzer gehört: ein Edge-VPS auf UTC gegenüber einer Appliance, die
  mit `Europe/Berlin` ausgeliefert wird. Die Zone reist deshalb mit den
  beiden Grenzen (`--window-tz`, `ComputeWindow.tz`), und die Edge wertet
  jeden Node mit `TZ="$tz" date` aus. Vorher las sie ihre eigene Uhr. Ein in
  Berlin eingegebenes Fenster von 23:00 bis 07:00 wurde im Winter von 00:00
  bis 08:00 und im Sommer von 01:00 bis 09:00 durchgesetzt und verschob sich
  bei jeder Zeitumstellung um eine Stunde. Damit bekamen die Pods Fremder
  die ersten Stunden des Arbeitstags des Besitzers, genau das, was die
  Funktion verhindern soll. Zieh `now` nicht wieder aus der
  Schleife über die Nodes in `modules/edge.nix` heraus. Es ist pro Node,
  weil die Zone es ist.
- **Der Markt ist das dritte Opt-in des Registrars.** `/market/*` (Stripe
  Connect, `backend-registrar/src/market.rs`, `wiki/Market.md`) antwortet
  503, solange `losos.edge.market.enable` nicht gesetzt ist, und 403, solange
  `losos.edge.tenants.<id>.market` nicht gesetzt ist. `tenantsJson` in
  `modules/edge.nix` legt seine Attribute fest, daher muss der Schlüssel
  `market` dort aufgeführt bleiben, sonst bekommt jeder Handel ein 403, das
  niemand erklärt.
  - Eine bezahlte Bestellung ist ein Anspruch. Speicherbestellungen bekommen
    einen Namespace und ein PVC im Mesh; das Anlegen ist idempotent, ein 409
    gilt als erledigt, und nichts wird je gelöscht. Rechenleistung ist nur
    eine Gutschrift im Ledger.
  - Angebote und Bestellungen hängen davon ab, was der Node des Verkäufers
    bereits teilt (`Sharing`, aus den Rechenfenstern der Registry). Der
    Markt ist ein Weg, für das Mesh bezahlt zu werden, kein zweites Produkt.
  - Die Admin UI erreicht ihn über das Relay `/api/market*` von lososd
    (`backend/src/market.rs`). Das Relay antwortet 200 `{available:false}`
    statt 404, wenn der Markt aus ist, weil die SPA sich ein 404 als
    "route not served" merkt.
  - Nur die Unit `losos-stripe-gate` (`losos-registrar stripe-gate`) hält
    den Stripe-Schlüssel. Sie bekommt die versiegelten Blobs
    (`losos.edge.market.{stripeSecretKey,webhookSecret}Sealed`) über
    `LoadCredentialEncrypted=`. Der Registrar spricht mit ihr über
    `/run/losos-stripe-gate/gate.sock` und sieht den Schlüssel nie. Füge
    keine Operation hinzu, die beliebige Stripe-Aufrufe weiterleitet,
    und halte die Request-Prüfungen des Gates (Ziel, Währung,
    Gebührenobergrenze, Session-Lebensdauer, https-Endpunkt) strenger als
    das, was der Registrar gerade sendet. Ein fehlender Blob überspringt das
    Gate (`ConditionPathExists`), und `/market/*` antwortet 503.
  - Das Onboarding schreibt die Box-UUID in das Stripe-Konto. Sie ist eine
    SHA-256-Ableitung des Wiederherstellungscodes (`backend/src/boxid.rs`),
    nie der Code selbst.
- **Jede Edge öffnet das Teilen; nur eine *offizielle* Edge öffnet den
  Markt.** lososd sucht nach Edges (`backend/src/edge.rs`: DNS-SD
  `_losos-edge._tcp` plus `losos.proxy.registrarUrl`, geprüft auf
  `/health`). Ist keine in Reichweite, verweigert es, `sharingMyStorage`
  oder `cluster.enable` einzuschalten (409 `edgeRequired`). Die Verweigerung
  gilt nur fürs Einschalten, eine Box kann also immer gehen.
  - Zusätzlich stellt lososd jeder antwortenden Edge eine Challenge
    (`GET /identity?nonce=`). Eine Edge gilt nur dann als offiziell, wenn
    ihr Zertifikat vom LosOS-Root-Schlüssel in
    `keys/official-edge-root.pub` (`losos.proxy.officialRootKeyFile`)
    signiert ist, diese URL nennt und nicht abgelaufen ist, und der
    Schlüssel des Zertifikats die Nonce signiert hat. Das Markt-Relay lehnt
    alles andere ab (`{available:false,
    reason:"noOfficialEdge"}`, 409 `officialEdgeRequired`).
  - Die committete Schlüsseldatei ist **absichtlich leer**, bis der Besitzer
    den öffentlichen Schlüssel hineinschreibt. Ohne Root ist nichts
    offiziell, und der Markt ist aus. Der private Schlüssel liegt offline
    beim Besitzer und wird nie committet.
  - `losos-registrar identity {keygen,sign,show,verify}` sind die
    Low-Level-Befehle (`backend-registrar/src/identity.rs`).
    `tests/edge-lan.nix` baut damit beim Build eine eigene Root, statt der
    ausgelieferten Datei zu vertrauen. `losos.edge.identity.{keyFile,certFile}`
    sind die Seite der Edge.
  - Die Zeremonie, die ein Operator ausführt, ist `losos-registrar provision
    {whoami,root-keygen,publish,edge,verify}`
    (`backend-registrar/src/provision.rs`), **auf dem eigenen Rechner des
    Operators**. Jedes Verb, das den Root-Schlüssel erzeugt oder verwendet,
    meldet den Operator zuerst über den OAuth Device Flow von GitHub an (nur
    öffentliche Client-ID, kein Secret). Es verweigert, wenn die *numerische
    ID* des Kontos nicht in `backend-registrar/operators.json` steht, das in
    das Binary einkompiliert ist.
  - `provision edge` liest den eigenen öffentlichen Schlüssel der Edge von
    `GET /identity/public-key`; der Registrar erzeugt diesen Schlüssel beim
    ersten Start, und die private Hälfte verlässt die Edge nie. Das Tool
    signiert das Zertifikat, pusht es mit dem GitHub-Token der Anmeldung an
    `POST /identity/cert` und führt die vier Prüfungen aus. Die Edge prüft
    das Token gegen dieselbe einkompilierte Allowlist, bevor sie irgendetwas
    installiert (`server::identity_push`). Es gibt kein SSH.
    `root-keygen --publish` und `publish` öffnen mit derselben Anmeldung
    (`public_repo`) den PR, der `keys/official-edge-root.pub` füllt.
    `tests/provision.rs` führt das alles gegen ein gefälschtes GitHub und
    einen echten Registrar aus.
  - Die Allowlist entscheidet, wem die Werkzeuge dienen. Der Root-Schlüssel
    bleibt das ganze Geheimnis.
  - Forgejo Actions ist in beiden Forgejo-Modi **aus**, und die Box betreibt
    keinen Runner. Die frühere Zeremonie von der Box aus (Actions-Workflows
    plus `modules/git-runner.nix`) ist weg, und
    `provisioning/edge-identity/README.md` ist das Runbook des Operators.
  - Der Rechner des Operators hat keinen Nix-Store, also ist
    `.#losos-registrar-static` (`pkgsStatic`, musl, gleicher `cargoHash`)
    das Binary dafür. Der CI-Job `publish-tool` pusht es nach GHCR als
    `images:<channel>-x86_64` (`losos-registrar` und `SHA256SUMS`, je eine
    Schicht), und der Proxy liefert es unter
    `/updates/<channel>/x86_64/<file>` aus. Das ist die einzige Route des
    LosOS-Desktop-Proxys, die einfache Dateien ausgibt. Das Runbook enthält
    die curl-Zeile.
- **`system.stateVersion = "26.11"` wird einmal gesetzt.** Es entspricht dem
  nixos-unstable, dem dieser Flake folgt. Ändere es nicht.
- **Der Symlink `result` ist ein Artefakt von `nix build`** und zeigt in
  `/nix/store`. Er steht in der gitignore und wird nie committet.
