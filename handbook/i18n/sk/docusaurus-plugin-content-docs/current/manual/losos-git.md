---
title: LosOS Git
sidebar_position: 4.6
slug: /manual/losos-git
---

# LosOS Git

![Repozitár v LosOS Git: súbory a README, v hlavičke tlačidlo Chat a adresa na klonovanie.](@site/docs/img/git-repository.png)

LosOS Git je vlastný Git server zariadenia. Drží vaše repozitáre, prijíma
pushe a klonovanie cez HTTPS a má stránky na čítanie kódu, otváranie pull
requestov a ich review.

Otvorte ho na `https://<name>.local/git/`. `<name>` je meno zariadenia,
také ako na jeho obrazovke. Prihlásite sa cez LosOS Login. Kto zariadenie
nastavil, je vlastník a jeho repozitáre sú pod `notshared`.

## Repozitáre \{#repositories}

Prvá stránka ukazuje vaše repozitáre, tie, ktoré s vami niekto zdieľa, a
tie, ktoré môže čítať každý na zariadení. **Nový repozitár** vytvorí ďalší
a spýta sa, kto ho uvidí. Prázdny repozitár ukáže príkazy, ktorými doň
pushnete prvú vetvu.

Stránky repozitára sú v bočnom paneli:

- **Kód** ukazuje súbory vetvy a pod nimi README. Hore vyberiete inú
  vetvu, alebo otvoríte súbor a čítate ho s číslami riadkov. Súbory
  Markdown sa zobrazia ako formátovaný text.
- **Pull requesty** zobrazujú otvorené a zatvorené.
- **Commity** a **Vetvy** ukazujú históriu. **Porovnať** pri vetve ukáže,
  čo v nej je a v predvolenej vetve nie.
- **Pracovné stromy** zobrazujú checkouty, ktoré do repozitára pushujú
  (pozri nižšie).
- **Nastavenia** premenujú repozitár, zmenia jeho popis a to, kto ho vidí.
  Vidí ich len vlastník repozitára a jeho správcovia.

### Kto vidí repozitár \{#who-can-see-a-repository}

Každý repozitár má jedno z troch nastavení, ktoré ukazuje odznak pri jeho
mene:

- **Súkromný**: vy a ľudia, ktorých pridáte. Je predvolené.
- **Tento box**: každý, kto má na zariadení účet, keď sa prihlási.
- **Verejný**: ktokoľvek, kto sa dostane k zariadeniu, aj bez
  prihlásenia. Môže čítať kód, históriu a pull requesty a klonovať ho.
  Cez edge je to ktokoľvek na internete.

Pri každom nastavení môžete pushovať len vy a ľudia, ktorých pridáte. Kto
otvorí `/git/` bez prihlásenia, uvidí verejné repozitáre a tlačidlo
**Prihlásiť sa**.

## Klonovanie a push \{#clone-and-push}

Adresa repozitára je navrchu jeho stránok, s tlačidlom, ktoré ju skopíruje:

```sh
git clone https://<name>.local/git/notshared/notes.git
```

git sa spýta na meno a token. `losos git setup` z
[terminálového klienta](terminal#use-losos-cloud-and-losos-git-from-this-computer)
vytvorí token pre jeden počítač a odovzdá ho gitu, takže klonovanie a push
potom heslo nepotrebujú.

## Úlohy \{#issues}

Chyby, práce a nápady k repozitáru aj k práci, ktorá nie je kód, sú v
zozname úloh pod **Úlohy** v bočnom paneli. **Úlohy** repozitára otvoria
jeho projekt. Pozri [Úlohy v LosOS Git](issues).

## Pull requesty \{#pull-requests}

Pushnite vetvu a stlačte **Otvoriť pull request** na stránke Kód alebo
**Porovnať** na stránke Vetvy. Vyberte vetvu, do ktorej sa má zlúčiť, dajte
mu názov a popis a stlačte **Otvoriť pull request**.

Pull request má tri karty:

- **Rozhovor** drží popis, komentáre a review v poradí, v akom prišli, a
  pole na zlúčenie.
- **Commity** zobrazujú commity vo vetve.
- **Zmenené súbory** ukazujú zmeny, súbor po súbore.

### Review \{#review}

V **Zmenených súboroch** ukážte na riadok a stlačte **+**, čím ho
okomentujete. **Komentovať hneď** komentár hneď odošle. **Pridať do
review** ho podrží vo vašom review, kým ho neodošlete. Keď skončíte,
vyberte **Komentár**, **Schváliť** alebo **Žiadať zmeny**, ak chcete,
napíšte zhrnutie, a stlačte **Odoslať review**. Kto pull request otvoril,
môže ten svoj len komentovať.

Komentár k riadku začne vlákno. Odpovedzte pod ním a keď je vec vybavená,
stlačte **Vyriešiť**; vyriešené vlákno sa zbalí a **Znova otvoriť** ho
vráti. Vlákno, ktorého riadok sa odvtedy zmenil, je označené ako zastarané.

**Recenzenti** vpravo požiadajú ľudí o review a ukážu, ako na tom kto je.

### Kontroly \{#checks}

Keď nástroj ohlási výsledok pre najnovší commit vetvy, napríklad build
alebo beh testov, výsledok sa ukáže na karte Rozhovor nad poľom na
zlúčenie.

### Zlúčenie \{#merge}

Pole na zlúčenie ponúka tri spôsoby:

- **Merge commit** zachová všetky commity a pridá jeden, ktorý vetvy spojí.
- **Squash** dá všetky zmeny vetvy do jedného commitu.
- **Fast-forward** posunie základnú vetvu na vetvu so zmenami, ak základ
  nemá nič, čo vetve chýba.

**Potom zmazať `<branch>`** vetvu po zlúčení odstráni. Zlúčenie s
konfliktmi zariadenie odmietne; zlúčte základnú vetvu do svojej, pushnite
a skúste znova. **Zatvoriť bez zlúčenia** odloží pull request bokom a
**Znova otvoriť** ho vráti.

## Pracovné stromy \{#worktrees}

Pracovný strom je jeden checkout repozitára na jednom počítači, vytvorený
cez `git worktree add`. Každý pomenujte a povedzte, na ktorom počítači je,
a každý push z neho ponesie oboje:

```sh
git config extensions.worktreeConfig true
git worktree add ../fix-backup -b fix-backup
cd ../fix-backup
git config --worktree push.pushOption "worktree=fix-backup"
git config --worktree --add push.pushOption "computer=laptop"
git push -u origin fix-backup
```

**Pracovné stromy** ho potom ukážu s jeho vetvou, posledným pushom a pull
requestom pre túto vetvu, ak nejaký je. Pull request, ktorého vetvu pushuje
pomenovaný pracovný strom, ukáže meno toho stromu vo svojej hlavičke.
**Zabudnúť** strom zo zoznamu vyradí; ďalší push ho tam vráti.

## Ľudia z iných boxov \{#people-of-other-boxes}

Niekto, kto má účet na inom LosOS boxe vo vašej sieti, sa môže zapojiť do
vašich verejných repozitárov bez účtu na vašom zariadení. Prihlási sa svojou
chatovou adresou, tou, ktorú LosOS Chat ukazuje pod jeho menom, napríklad
`@eva:kitchenbox.local`.

1. Na stránke repozitára na vašom zariadení stlačí **Použiť chatovú
   adresu**, napíše adresu a stlačí **Pokračovať na tvojom boxe**.
2. Otvorí sa jeho vlastný box a spýta sa, či má vášmu povedať, kto je.
   Stlačí **Povedať boxu `<name>.local`**.
3. Je späť na vašom zariadení, prihlásený ako `@eva:kitchenbox.local`.

![Evin vlastný box sa pýta, skôr než povie boxu mattbox.local, kto je.](@site/docs/img/git-other-box-vouch.png)

Jeho box vášmu povie len chatovú adresu a meno. Vaše zariadenie si odpoveď
overí u chatového servera jeho boxu, skôr než jej uverí. Ľudia s účtom na
vašom zariadení sa prihlasujú cez LosOS Login ako doteraz.

Takto prihlásený môže komentovať, robiť recenzie a otvárať pull requesty.
Do vašich vetiev pushovať nemôže. Pushuje do vetiev, ktorých názov začína
jeho prihlasovacím menom tu, napríklad `eva@kitchenbox.local/hours`, a o
zmenu požiada v pull requeste. **Kopírovať heslo pre git** na stránke
repozitára skopíruje heslo, ktoré git pýta; meno môže byť akékoľvek.

```sh
git push https://<name>.local/git/notshared/site.git HEAD:eva@kitchenbox.local/hours
```

Zlučujete len vy a vaši správcovia. **Odhlásiť sa** ukončí jeho návštevu na
tomto počítači.

![Pull request z Evinej vlastnej vetvy s odpoveďou vlastníka repozitára.](@site/docs/img/git-other-box-pull.png)

## Miestnosť v chate pre repozitár \{#a-chat-room-for-a-repository}

Keď je na zariadení LosOS Chat, **Nastavenia** repozitára majú prepínač
**Rozprávať sa o tomto repozitári v LosOS Chat**. Zapnite ho a zariadenie
vytvorí pre repozitár miestnosť vo vašom chate, s LosOS Git v nej. LosOS
Git tam píše každý push, pull request, komentár, zlúčenie a výsledok
kontroly.

![Nastavenia repozitára so zapnutým prepínačom Rozprávať sa o tomto repozitári v LosOS Chat a odkazom na miestnosť.](@site/docs/img/git-repository-room.png)

V hlavičke repozitára potom bude tlačidlo **Chat** pre každého
prihláseného. Do miestnosti verejného repozitára sa môže pridať ktokoľvek,
aj z chatu na svojom boxe. Do súkromného sa ľudia pridajú, keď ich pozvete
z chatu.

Vypnutím prepínača príspevky prestanú. Miestnosť a to, čo v nej je, vo
vašom chate zostanú.

## Nastavenia \{#settings}

`losos.git.enable` vypne LosOS Git. `losos.git.site.requireSignIn`
vyžiada od každého prihlásenie skôr, než LosOS Git niečo ukáže, aj
verejné repozitáre. Obe sú na paneli **Rozšírené**.
