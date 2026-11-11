---
title: Terminálový klient
sidebar_position: 10
slug: /manual/terminal
---

# Terminálový klient

![Prehľad v prehliadači; terminálový klient ukazuje tie isté panely.](@site/docs/img/overview-sk.png)

`losos` je terminálový program, ktorý spravuje zariadenie z iného počítača.
Ukazuje tie isté panely ako správcovské stránky, tými istými slovami a v
tých istých farbách, a nepotrebuje SSH, pretože zariadenie ho nemá. Každú
požiadavku, ktorú pošle, podpíše váš vlastný kľúč GPG a zašifruje ju pre
zariadenie. Zariadenie odpovedá rovnako, takže požiadavku prečíta iba
zariadenie a odpoveď iba vy.

Beží na 64-bitovom x86 Linuxe.

## Pred inštaláciou zariadenia: dajte svoj kľúč na inštalačné médium \{#before-you-install-the-box-put-your-key-on-the-stick}

Zariadenie prijíma iba požiadavky podpísané kľúčom GPG, ktorý dostalo pri
inštalácii. Na bežiacom zariadení ho nič neskôr pridať nedokáže. Keby to
išlo, ktokoľvek, kto by sa dostal do správcovských stránok, by si mohol
pridať vlastný kľúč a potom si k nemu vytvoriť token. Terminálový klient
preto vyžaduje trochu plánovania: inštalačné médium, z ktorého inštalujete,
musí niesť váš kľúč.

:::warning[Médium zo stránky vydaní nenesie kľúč]
Inštalačné médium zo stránky vydaní žiadny kľúč nenesie. Zariadenie
nainštalované z neho funguje vo všetkom ostatnom a jeho panel Prístupové
tokeny hovorí, že pri inštalácii nebol vložený žiadny kľúč, a tokeny
nevytvára. Jediný spôsob, ako takému zariadeniu dať kľúč, je nainštalovať
ho znova z inštalačného média, ktoré kľúč nesie, a inštalácia ho zmaže.
:::

Potrebujete kľúč GPG s podkľúčom na šifrovanie, pretože zariadenie mu
šifruje svoje odpovede. `gpg --full-generate-key` taký vytvorí predvolene.
Kľúč, ktorému by zariadenie nedokázalo odpovedať, zastaví inštalátor skôr,
než sa dotkne disku. Verejnú polovicu exportujte do nového, prázdneho
priečinka:

```sh
mkdir my-losos-stick && cd my-losos-stick
gpg --armor --export you@example.com > owner.asc
```

Vedľa neho uložte toto ako `flake.nix`:

```nix
{
  inputs.losos.url = "github:dasmatus/losos";
  outputs = { losos, ... }:
    let
      key = builtins.readFile ./owner.asc;
      stick = losos.nixosConfigurations.iso.extendModules {
        modules = [ { losos.access.gpgKeys = [ key ]; } ];
      };
    in {
      packages.x86_64-linux.default =
        stick.config.system.build.isoImage;
    };
}
```

Potom inštalačné médium zostavte pomocou [Nix](https://nixos.org/download/) na
64-bitovom x86 počítači s Linuxom:

```sh
nix --extra-experimental-features 'nix-command flakes' build
```

Zostavenie kompiluje programy LosOS, takže prvé spustenie chvíľu trvá. ISO
skončí v `result/iso/`. Zapíšte ho na USB kľúč a
[nainštalujte](/start/install.md) ako obvykle. Po prvom spustení panel
Prístupové tokeny ukáže váš kľúč pod **Vložené kľúče GPG**. Fungujú aj dva
kľúče: uveďte oba, ako v `[ key (builtins.readFile ./other.asc) ]`.

:::note[Vlastné inštalačné médium nie je podpísané]
Inštalačné médium, ktoré si zostavíte sami, nie je podpísané, takže aby z neho
počítač naštartoval, vypnite vo firmvéri Secure Boot.
:::

## Vytvorenie tokenu \{#make-a-token}

V správcovských stránkach otvorte **Nastavenia → Prístupové tokeny**. Panel
ukazuje:

- **Kľúč tohto zariadenia**, odtlačok, ktorý terminál overí pri prvom
  pripojení;
- **Vložené kľúče GPG**, kľúče, s ktorými bolo zariadenie nainštalované;
- **Tokeny**, tie už vytvorené, s časom posledného použitia každého z nich.

Pomenujte nový token, vyberte kľúč, ku ktorému patrí, zvoľte **Iba čítanie**
alebo **Plný prístup** a vyberte, kedy vyprší. **Vytvoriť token** ho ukáže
raz. Vtedy si ho skopírujte. Zariadenie si drží iba jeho hash a znova ho
ukázať nedokáže.

Token funguje iba v požiadavke podpísanej kľúčom, pre ktorý bol vytvorený,
takže token odpísaný z obrazovky je bez toho kľúča nanič. Token iba na
čítanie si môže pozrieť každý panel a nič nezmení. Žiadny token nedokáže
vytvárať, vypisovať ani rušiť tokeny a žiadny sa nedostane k sprievodcovi
nastavením ani k prihláseniu. **Zrušiť** token okamžite ukončí.

## Inštalácia klienta \{#install-the-client}

```sh
curl -fsSLO https://nightly.link/dasmatus/losos/workflows/ci/main/losos-pages-updates.zip
unzip -o losos-pages-updates.zip -d losos-pages-updates
sh losos-pages-updates/install.sh
```

Zip je najnovšie zostavenie klienta z GitHubu LosOS, s programom, jeho
kontrolným súčtom a skriptom. Skript porovná program s kontrolným súčtom a
uloží `losos` do `~/.local/bin`, alebo do `/usr/local/bin`, keď ho spustíte
ako root. Nikdy si nepýta heslo. Ak chcete, najprv si ho prečítajte. Je
krátky a hneď na začiatku hovorí, čo robí.

`losos` drží váš kľúč a tokeny v kľúčenke pracovného prostredia (GNOME
Keyring, KWallet alebo KeePassXC) cez `secret-tool`. Na Debiane a Ubuntu je
to balík `libsecret-tools`, na Fedore a Archu `libsecret`. Do obyčajného
súboru ich nikdy nezapíše.

## Pripojenie \{#connect}

```sh
losos key import --gpg you@example.com
losos login http://192.168.1.23
losos
```

`key import --gpg` skopíruje váš tajný kľúč z GnuPG do kľúčenky. Ak tajný
kľúč držíte v súbore, `losos key import secret.asc` urobí to isté zo
súboru.

`login` berie adresu zariadenia z jeho obrazovky. Obyčajné `http://` tu
stačí: požiadavka aj odpoveď sú šifrované od konca po koniec, nech ich
prenáša čokoľvek. Funguje aj `https://<name>.local`. Certifikát si zariadenie
vytvorilo samo, takže `losos` pri prihlásení prijme ten, ktorý vidí, a
zapamätá si ho. Ak zariadenie neskôr ukáže iný certifikát, `losos` sa
zastaví a povie to. Keď viete, prečo sa zmenil, spustite `login` znova a
nový certifikát prijmete.

`login` si od zariadenia vypýta jeho kľúč a ukáže odtlačok v skupinách po
štyroch znakoch. Skôr než odpoviete áno, overte, že sa zhoduje s **Kľúč
tohto zariadenia** na paneli Prístupové tokeny. Potom vložte token, ktorý
`losos` nevypisuje. Odvtedy si `losos` kľúč zariadenia pamätá a ak sa
niekedy zmení, odmietne sa so zariadením rozprávať.

Samotný príkaz `losos` otvorí zobrazenie na celú obrazovku: bočný panel
vľavo, panel vpravo, klávesy dole. Šípky alebo `j` a `k` posúvajú výber,
Enter zmení nastavenie, Tab prepína medzi bočným panelom a panelom, `/`
hľadá v paneloch, `?` vypíše zvyšok a `q` program ukončí. Zmena zostane
návrhom, kým ju klávesom `a` nepoužijete, a zariadenie sa pred zostavením
opýta, rovnako ako správcovské stránky.

:::note[Čo zostáva v správcovských stránkach]
Prístupové tokeny, trh, Diagramy a všetko, čo maže dáta, zostáva v správcovských
stránkach.
:::

## Mimo domova \{#away-from-home}

Keď sa zariadenie dostane na internet cez edge, `login` si zapamätá aj
adresu zariadenia na edgi a `losos hosts` ju ukáže ako **mimo domova**. Keď
vlastná adresa zariadenia nikam nevedie, lebo tento počítač je v inej
sieti, `losos` pošle volanie na túto adresu. Edge ho odovzdá ďalej bez
toho, aby ho čítal: stále je podpísané vaším kľúčom a zašifrované pre kľúč
zariadenia, a zariadenie odpovedá rovnako. Edge má certifikát, ktorému web
dôveruje, takže nie je čo ďalšie pripínať.

Takto cez edge idú iba volania terminálu. Ostatné správcovské stránky
zostávajú vo vlastnej sieti zariadenia. Ak má aj terminál zostať tam,
vypnite `losos.access.throughEdge` na paneli Rozšírené.

## LosOS Cloud a LosOS Git z tohto počítača \{#use-losos-cloud-and-losos-git-from-this-computer}

```sh
losos cloud setup
losos git setup
```

Každý z príkazov si vypýta vaše heslo LosOS, to, ktorým sa prihlasujete do
LosOS Cloud, a nastaví počítač, na ktorom ho spustíte. Panel **Tento
počítač** v zobrazení na celú obrazovku urobí to isté jedným klávesom.

`cloud setup` si od zariadenia vypýta heslo aplikácie iba pre tento počítač
a uloží ho do kľúčenky tam, kde ho hľadá správca súborov. LosOS Cloud sa
potom objaví v bočnom paneli správcu súborov (GNOME Files a ďalší správcovia
súborov, ktorí čítajú záložky GTK) a po otvorení ukáže vaše súbory bez
pýtania hesla. Správca súborov sa pri prvom pripojení raz opýta, či prijať
vlastný certifikát zariadenia.

`git setup` si od zariadenia vypýta token LosOS Git iba pre tento počítač a
uloží ho do kľúčenky. git ho potom dostane od `losos` vždy, keď sa rozpráva
so zariadením, takže klonovanie a push nepotrebujú heslo:

```sh
git clone https://mattbox.local/git/notshared/notes.git
```

Keď má zariadenie vlastný certifikát, git dostane pokyn prijať presne kľúč
toho certifikátu, rovnakou kontrolou, akú robí `losos`.

Obe sú pomenované podľa počítača, napríklad „LosOS on laptop“. `--device`
zvolí iné meno. Ak chcete počítaču prístup odobrať, vymažte jeho položku v
časti **Zabezpečenie** v nastaveniach LosOS Cloud alebo v časti
**Aplikácie** v nastaveniach LosOS Git. Opätovné spustenie nastavenia
nahradí heslo alebo token toho počítača.

## V skripte \{#in-a-script}

`losos call` pošle jednu požiadavku a vypíše odpoveď:

```sh
losos call GET /api/state
```

Keď zariadenie odpovie chybou, skončí s kódom 1, takže skript to môže
skontrolovať. `losos --help` vypíše všetky príkazy.
