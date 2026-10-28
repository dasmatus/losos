---
title: Adresy a porty
sidebar_position: 1
---

# Adresy a porty

## Na zariadení \{#on-the-box}

![Vnútri zariadenia: nginx drží porty 80 a 443 a podľa cesty smeruje na admin stránky, riadiace API, LosOS Cloud, LosOS Git a túto príručku.](@site/docs/img/box-architecture-sk.svg)

| Cesta                         | Čo                                                  | Kto ju môže otvoriť                 |
| ----------------------------- | --------------------------------------------------- | ----------------------------------- |
| `/`                           | Admin stránky                                       | iba lokálna sieť                    |
| `/handbook/`                  | Táto príručka                                       | iba lokálna sieť                    |
| `/api/…`                      | Riadiace API zariadenia (JSON, vyžaduje admin kľúč) | iba lokálna sieť                    |
| `/api/health`                 | `{"ok":true}`, keď riadiaci démon beží              | iba lokálna sieť, bez kľúča         |
| `/setup/state.json`           | Názov, adresa, HTTPS, odtlačok certifikátu          | iba lokálna sieť                    |
| `/setup/losos-ca.crt`         | Certifikát zariadenia                               | iba lokálna sieť                    |
| `/setup/trust.sh`, `/setup/trust.ps1` | Jednoriadkové inštalátory certifikátu       | iba lokálna sieť                    |
| `/cloud/`                     | LosOS Cloud                                         | ktokoľvek, aj cez edge              |
| `/remote.php/`, `/status.php`, `/ocs/`, `/index.php/` | To, čo volajú aplikácie Nextcloud, kalendárové aplikácie a správcovia súborov | ktokoľvek, aj cez edge |
| `/nextcloud/`                 | To isté, na adrese, ktorú používal Nextcloud zariadenia | ktokoľvek, aj cez edge          |
| `/git/`                       | LosOS Git                                           | ktokoľvek, aj cez edge              |

:::note[Čo znamená lokálna sieť]
„Iba lokálna sieť“ znamená zdrojovú adresu zo súkromného rozsahu
(`10.…`, `172.16.…` až `172.31.…`, `192.168.…`), ktorá nie je vlastnou
adresou zariadenia ani nepatrí do rozsahu podov mesh siete. Všetko ostatné
vrátane prevádzky, ktorá prichádza cez tunel edge, dostane **403**.
:::

## Porty \{#ports}

| Port    | Protokol | Čo                                                     |
| ------- | -------- | ------------------------------------------------------ |
| 80      | HTTP     | Všetko vyššie                                          |
| 443     | HTTPS    | To isté s vlastným certifikátom zariadenia (`losos.tls.enable`, predvolene zapnuté) |
| 443 (UDP) | HTTP/3 | To isté cez QUIC, prehliadačom ho ponúka hlavička `Alt-Svc`; HTTPS cez TCP zostáva ako záloha |
| 5353    | mDNS     | Ohlasovanie `<name>.local` a boxu ako služby `_losos._tcp` |
| 445     | SMB      | Súbory každého človeka v LosOS Cloud ako sieťový priečinok Windows (`losos.cloud.windows.enable`, predvolene zapnuté); iba lokálna sieť |
| 3702 (UDP), 5357 | WS-Discovery | Zobrazenie zariadenia v Sieti v Prieskumníkovi Windows, kým je priečinok zapnutý |
| 5355    | LLMNR    | Odpoveď Windows, keď sa pýta na zariadenie jeho holým názvom, kým je priečinok zapnutý |
| 8082    | HTTP     | Riadiace API, iba na loopbacku zariadenia; nginx naň presmeruje `/api/` |
| 8480, 8443 | HTTP, HTTPS | Čo Tor a I2P doručia pre vlastné adresy zariadenia (`losos.hidden.tor.enable`, `losos.hidden.i2p.enable`), iba na loopbacku zariadenia. Pozri [Tor a I2P](../manual/tor-and-i2p.md) |

Kým je priečinok zapnutý, záznam mDNS pre LosOS Cloud je priečinok SMB
(`_smb._tcp`), takže Finder a správcovia súborov v Linuxe ukážu zariadenie
raz.

Nič iné otvorené nie je. Zariadenie sa k mesh sieti a k edge pripája smerom
von. Tunel otvára a ku clustru sa pripája samo, takže doma neotvárate žiadny
prichádzajúci port.

## Na edge \{#on-an-edge}

| Port         | Protokol | Čo                                                      |
| ------------ | -------- | ------------------------------------------------------- |
| 80, 443      | HTTP(S)  | Traefik: verejné meno každého zariadenia                 |
| 2333 (UDP)   | QUIC     | Tunel, ktorý zariadenia vytáčajú (`losos.edge.tunnelBindPort`) |
| 2333 (TCP)   | TLS      | Ten istý tunel pre zariadenia v sieti, ktorá zahadzuje UDP |
| 7883 (UDP)   | QUIC     | Relay hovorov (`losos.edge.calls.relay.port`)            |

## Názvy \{#names}

| Názov                       | Čo                                                       |
| --------------------------- | -------------------------------------------------------- |
| `<name>.local`              | Zariadenie vo vlastnej sieti, cez mDNS                   |
| `<name>.<edge domain>`      | Zariadenie cez svoj edge, pre LosOS Cloud a LosOS Git    |
| `losos-edge.dasmat.us`      | Riadiaca časť edge LosOS a stránka „find my box“         |
| `proxy.losos.dasmat.us`     | Binárna cache, z ktorej sa zariadenie aktualizuje        |
| `127.0.0.1:8787`            | Lokálna Nix cache z GHCR na zariadení a inštalačnom médiu |
| `losos.dasmat.us/proxy`     | Kópia tejto cache, použije sa, keď prvá neodpovedá       |
| `losos.dasmat.us`           | Táto príručka na webe                                    |
