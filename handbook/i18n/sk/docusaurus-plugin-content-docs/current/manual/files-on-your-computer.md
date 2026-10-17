---
title: Súbory vo vašom počítači
sidebar_position: 4.5
slug: /manual/files-on-your-computer
---

# Súbory vo vašom počítači

Vaše súbory v LosOS cloud sú aj sieťovým priečinkom. Každý počítač v sieti
zariadenia ho otvorí vo svojom správcovi súborov, môže doň súbory preťahovať,
brať ich odtiaľ a otvárať ich priamo na mieste, bez inštalácie čohokoľvek.
Priečinok je zdieľanie WebDAV od LosOS cloud na tejto adrese:

```text
https://<name>.local/nextcloud/remote.php/webdav/
```

`<name>` je meno zariadenia, také ako na jeho obrazovke. V zdieľaní sú tie
isté súbory ako v aplikácii Súbory v LosOS cloud.

## Skôr než začnete \{#before-you-start}

Nainštalujte do počítača certifikát zariadenia, ako v
[prvom spustení, kroku 1](../start/first-run#step-1-trust-this-box). Bez
neho správca súborov napíše, že zariadenie nevie overiť, a spýta sa, či
pokračovať. Windows sa nepripojí vôbec.

## Aké meno a heslo \{#which-name-and-password}

V sieti zariadenia zadajte prihlasovacie meno a heslo, ktoré používate v
LosOS Login. Kto zariadenie nastavil, dostane súbory vlastníka. Ktokoľvek iný
dostane svoje vlastné a pri prvom otvorení zdieľania mu LosOS cloud založí
účet.

Cez internet LosOS Login neodpovedá, a tak si zdieľanie namiesto toho pýta
heslo aplikácie. Vytvoríte ho v LosOS cloud v časti **Nastavenia** >
**Zabezpečenie** > **Vytvoriť nové heslo aplikácie**. LosOS cloud potom
ukáže meno a heslo, ktoré treba zadať. Heslo aplikácie funguje aj v sieti
zariadenia a dá sa zrušiť samostatne.

## Linux \{#linux}

GNOME Súbory a KDE Dolphin nájdu zariadenie samy.

- **GNOME Súbory:** otvorte **Iné umiestnenia**. V časti **Siete** kliknite
  na **LosOS cloud on `<name>`** a zadajte meno a heslo.
- **Dolphin:** v paneli miest otvorte **Sieť** a potom **LosOS cloud on
  `<name>`**.

Ak chcete adresu zadať sami, napíšte
`davs://<name>.local/nextcloud/remote.php/webdav/` do poľa **Pripojiť sa k
serveru** v GNOME Súboroch alebo
`webdavs://<name>.local/nextcloud/remote.php/webdav/` do riadka s adresou v
Dolphine.

Keď si heslo necháte zapamätať, priečinok sa nabudúce otvorí bez pýtania.
GNOME Súbory ho vedia pridať aj do bočného panela ako záložku.

## Windows \{#windows}

Prieskumník súborov zdieľanie v časti **Sieť** neukazuje. Pridajte ho podľa
adresy:

1. Otvorte **Tento počítač** a potom **Pripojiť sieťovú jednotku** (vo
   Windows 11 v ponuke **...**).
2. Vyberte písmeno jednotky. Do poľa **Priečinok** napíšte
   `https://<name>.local/nextcloud/remote.php/webdav/`.
3. Zaškrtnite **Pripojiť pomocou iných poverení** a **Znova pripojiť pri
   prihlásení** a kliknite na **Dokončiť**.
4. Zadajte meno a heslo a zaškrtnite **Zapamätať si moje poverenia**.

Zdieľanie sa potom objaví v časti **Tento počítač** s vlastným písmenom
jednotky.

:::note[Veľké súbory vo Windows]
Vlastný klient WebDAV vo Windows odmieta súbory väčšie než asi 50 MB, kým sa
jeho limit nezvýši. Na veľké súbory alebo celé priečinky použite aplikáciu
pre počítač nižšie.
:::

## macOS \{#macos}

Finder zdieľanie v časti **Sieť** tiež neukazuje. Pridajte ho podľa adresy:

1. Vo Finderi vyberte **Otvoriť** > **Pripojiť k serveru** (Command-K).
2. Napíšte `https://<name>.local/nextcloud/remote.php/webdav/` a kliknite
   na **Pripojiť**.
3. Vyberte **Registrovaný užívateľ**, zadajte meno a heslo a zaškrtnite
   **Uložiť toto heslo do kľúčenky**.

Ak pred **Pripojiť** kliknete na **+**, adresa zostane v zozname obľúbených
serverov.

## Aplikácie pre počítač a telefón \{#the-desktop-and-phone-apps}

Zdieľanie slúži na otváranie súborov tam, kde sú. Na synchronizáciu kópie
priečinka alebo na nahrávanie fotiek z telefónu použite aplikácie LosOS cloud
(klienta Nextcloud pre počítač a aplikácie Nextcloud pre telefón). Zadajte im
adresu `https://<name>.local/nextcloud`. Na prihlásenie otvoria LosOS Login
v prehliadači a potom si nechajú vlastné heslo aplikácie.

Na počítači s Linuxom a terminálovým klientom `losos` nastaví správcu súborov
za vás `losos cloud setup`; pozrite [Terminálový klient](terminal).
