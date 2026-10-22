---
title: Súbory vo vašom počítači
sidebar_position: 4.5
slug: /manual/files-on-your-computer
---

# Súbory vo vašom počítači

Vaše súbory v LosOS Cloud sú aj sieťovým priečinkom. Každý počítač v sieti
zariadenia ho otvorí vo svojom správcovi súborov, môže doň súbory preťahovať,
brať ich odtiaľ a otvárať ich priamo na mieste, bez inštalácie čohokoľvek.
Dostanete sa k nemu dvoma spôsobmi:

- **Sieťové zdieľanie.** V sieti zariadenia ho Windows, macOS aj Linux samy
  ukážu v časti **Sieť**, s vašimi súbormi v priečinku s vaším menom.
- **Adresa WebDAV.** Funguje v sieti zariadenia aj cez internet a je
  cestou dnu, kým je sieťové zdieľanie vypnuté.

`<name>` nižšie je meno zariadenia, také ako na jeho obrazovke. V oboch sú tie
isté súbory ako v aplikácii Súbory v LosOS Cloud.

## Sieťové zdieľanie \{#the-network-share}

Váš priečinok v zdieľaní je:

```text
\\<name>\<vaše meno>
```

Prihláste sa menom a heslom, ktoré používate v LosOS Login. Každý vidí len
svoj priečinok; kto zariadenie nastavil, dostane súbory vlastníka. Adresu
ukazuje aj vaša stránka **Váš účet**.

Zdieľanie odpovedá len počítačom vo vlastnej sieti zariadenia, nikdy nie cez
internet. Správca ho zapína a vypína v **Nastaveniach** > **Sieť** >
**Windows, Mac a Linux**. Predvolene je zapnuté.

### Windows \{#windows}

1. Otvorte Prieskumník a kliknite na **Sieť**. Ak lišta hlási, že je
   zisťovanie siete vypnuté, kliknite na ňu a zapnite ho pre súkromnú sieť.
2. Dvakrát kliknite na `<name>` a potom na priečinok s vaším menom.
3. Zadajte meno a heslo LosOS, zaškrtnite **Zapamätať si moje poverenia** a
   kliknite na **OK**.

Ak chcete priečinku dať písmeno jednotky, kliknite naň pravým tlačidlom a
vyberte **Pripojiť sieťovú jednotku**.

### macOS \{#macos}

1. V bočnom paneli Findera kliknite v časti **Umiestnenia** na **Sieť** a
   potom na `<name>`.
2. Kliknite na **Pripojiť ako**, vyberte **Registrovaný užívateľ**, zadajte
   meno a heslo LosOS a zaškrtnite **Uložiť toto heslo do kľúčenky**.

Ten istý priečinok otvorí aj **Otvoriť** > **Pripojiť k serveru** s adresou
`smb://<name>.local/<vaše meno>`.

### Linux \{#linux}

- **GNOME Súbory:** otvorte **Iné umiestnenia**. V časti **Siete** kliknite
  na `<name>`, potom na priečinok s vaším menom a zadajte meno a heslo.
- **Dolphin:** v paneli miest otvorte **Sieť** a potom `<name>`.

Adresa `smb://<name>.local/<vaše meno>` funguje v riadku s adresou v oboch.

### Ak zdieľanie heslo odmietne \{#if-your-password-is-refused}

Windows, a sieťové zdieľanie s ním, overuje heslo po svojom, takže zariadenie
musí to vaše raz vidieť, kým ho zdieľanie prijme. Vidí ho, keď zariadenie
nastavujete, keď vám vznikne účet, keď si heslo zmeníte a keď sa prihlásite
na adresu WebDAV. Ak sa nič z toho od zapnutia zdieľania nestalo, otvorte v
LosOS stránku **Váš účet**, nájdite **Vaše súbory vo Windows, Macu a Linuxe**,
zadajte heslo pod **Povoliť Windows vaše heslo** a kliknite na **Povoliť**.

### Čím sa zdieľanie líši \{#what-the-share-does-differently}

Súbory, ktoré v zdieľaní zmeníte, sa hneď zmenia aj v LosOS Cloud, v
prehliadači aj v aplikáciách pre počítač a telefón. Súbor, ktorý tam
vymažete, zmizne natrvalo: obíde kôš LosOS Cloud. A súbor, ktorý tam
prepíšete, si nenechá staršiu verziu.

## Adresa WebDAV \{#the-webdav-address}

```text
https://<name>.local/remote.php/webdav/
```

### Skôr než začnete \{#before-you-start}

Nainštalujte do počítača certifikát zariadenia, ako v
[prvom spustení, kroku 1](../start/first-run#step-1-trust-this-box). Bez
neho správca súborov napíše, že zariadenie nevie overiť, a spýta sa, či
pokračovať. Windows sa nepripojí vôbec.

### Aké meno a heslo \{#which-name-and-password}

V sieti zariadenia zadajte prihlasovacie meno a heslo, ktoré používate v
LosOS Login. Kto zariadenie nastavil, dostane súbory vlastníka. Ktokoľvek iný
dostane svoje vlastné a pri prvom otvorení zdieľania mu LosOS Cloud založí
účet.

Cez internet LosOS Login neodpovedá, a tak si zdieľanie namiesto toho pýta
heslo aplikácie: to, ktoré dostala aplikácia pre počítač alebo telefón pri
prihlásení, alebo to, ktoré vytvoril `losos cloud setup` (nižšie). Heslo
aplikácie funguje aj v sieti zariadenia a **Zariadenia** v LosOS Cloud ho
samostatne odhlásia.

### Linux \{#webdav-linux}

Kým je sieťové zdieľanie vypnuté, GNOME Súbory a KDE Dolphin ukážu namiesto
neho v časti **Sieť** položku **LosOS Cloud on `<name>`**. Ak chcete adresu
zadať sami, napíšte `davs://<name>.local/remote.php/webdav/` do poľa
**Pripojiť sa k serveru** v GNOME Súboroch alebo
`webdavs://<name>.local/remote.php/webdav/` do riadka s adresou v Dolphine.

Keď si heslo necháte zapamätať, priečinok sa nabudúce otvorí bez pýtania.
GNOME Súbory ho vedia pridať aj do bočného panela ako záložku.

### Windows \{#webdav-windows}

1. Otvorte **Tento počítač** a potom **Pripojiť sieťovú jednotku** (vo
   Windows 11 v ponuke **...**).
2. Vyberte písmeno jednotky. Do poľa **Priečinok** napíšte
   `https://<name>.local/remote.php/webdav/`.
3. Zaškrtnite **Pripojiť pomocou iných poverení** a **Znova pripojiť pri
   prihlásení** a kliknite na **Dokončiť**.
4. Zadajte meno a heslo a zaškrtnite **Zapamätať si moje poverenia**.

Zdieľanie sa potom objaví v časti **Tento počítač** s vlastným písmenom
jednotky.

:::note[Veľké súbory vo Windows]
Vlastný klient WebDAV vo Windows odmieta súbory väčšie než asi 50 MB, kým sa
jeho limit nezvýši. V sieti zariadenia použite namiesto neho sieťové
zdieľanie, inde aplikáciu pre počítač nižšie.
:::

### macOS \{#webdav-macos}

1. Vo Finderi vyberte **Otvoriť** > **Pripojiť k serveru** (Command-K).
2. Napíšte `https://<name>.local/remote.php/webdav/` a kliknite
   na **Pripojiť**.
3. Vyberte **Registrovaný užívateľ**, zadajte meno a heslo a zaškrtnite
   **Uložiť toto heslo do kľúčenky**.

Ak pred **Pripojiť** kliknete na **+**, adresa zostane v zozname obľúbených
serverov.

## Aplikácie pre počítač a telefón \{#the-desktop-and-phone-apps}

Zdieľanie slúži na otváranie súborov tam, kde sú. Na synchronizáciu kópie
priečinka alebo na nahrávanie fotiek z telefónu použite klienta Nextcloud pre
počítač a aplikácie Nextcloud pre telefón, ktoré sa synchronizujú s LosOS
Cloud. Zadajte im adresu `https://<name>.local`. Na prihlásenie otvoria LosOS Login
v prehliadači a potom si nechajú vlastné heslo aplikácie.

Na počítači s Linuxom a terminálovým klientom `losos` nastaví správcu súborov
za vás `losos cloud setup`; pozrite [Terminálový klient](terminal).
