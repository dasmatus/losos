---
title: Terminálový klient
sidebar_position: 10
slug: /manual/terminal
---

# Terminálový klient

![Prehľad v prehliadači; terminálový klient ukazuje tie isté panely.](@site/docs/img/overview-sk.png)

`losos` je terminálový program, ktorý spravuje zariadenie z iného počítača.
Ukazuje tie isté panely ako správcovské stránky, tými istými slovami a v
tých istých farbách, a nepotrebuje SSH, pretože zariadenie ho nemá.

Prihlasuje sa cez LosOS Login, vlastné prihlásenie zariadenia, tak ako
správcovské stránky, LosOS Cloud a LosOS Git. Schválite ho v prehliadači.
Odvtedy terminál koná ako vy: vaša rola a časti zariadenia, ktoré vám boli
dané. Nemá vlastné heslo a žiadny prístupový token na odpísanie z panela
Prístupové tokeny.

Beží na 64-bitovom x86 Linuxe.

## Nainštalujte klienta

```sh
curl -fsSLO https://nightly.link/dasmatus/losos/workflows/ci/main/losos-pages-updates.zip
unzip -o losos-pages-updates.zip -d losos-pages-updates
node losos-pages-updates/install.mjs
```

Archív zip je najnovší zostavený klient z GitHubu LosOS, s programom, jeho
kontrolným súčtom a skriptom. Skript program porovná so súčtom a vloží
`losos` do `~/.local/bin`, alebo do `/usr/local/bin`, keď ho spustíte ako
root. Nikdy nepýta heslo. Ak chcete, prečítajte si ho. Je krátky a hore hovorí,
čo robí.

`losos` drží prihlásenie v kľúčenke plochy (GNOME Keyring, KWallet alebo
KeePassXC) cez `secret-tool`. Na Debiane a Ubuntu je to balík
`libsecret-tools`, na Fedore a Archu `libsecret`. Tokeny nikdy nezapisuje do
obyčajného súboru.

## Prihlásenie

```sh
losos login https://mattbox.local
losos
```

`login` berie adresu zariadenia z jeho obrazovky. Použite `https://`. LosOS
Login na holom `http://` neodpovedá; zariadenie tú adresu pošle ďalej na
`https://`.

Pri prvom raze, ak má zariadenie vlastný certifikát, `losos` ukáže odtlačok
SHA-256 certifikátu. Porovnajte ho s tým, ktorý pre zariadenie ukazuje váš
prehliadač, a ak sa zhodujú, povedzte áno. Odvtedy `losos` hovorí len so
zariadením s týmto certifikátom. Ak zariadenie neskôr ukáže iný, `losos` sa
zastaví a povie to. Spustite `login` znova, keď viete, prečo sa zmenil.

Potom sa `losos` prihlási cez LosOS Login. Vypíše odkaz a kód:

```text
losos sa do https://mattbox.local prihlasuje cez LosOS Login. Otvorte tento odkaz v prehliadači, prihláste sa a schváľte terminál:
  https://mattbox.local/auth/consent/openid/device-authorization?user_code=WDJBMJHT
Stránka ukazuje kód WDJBMJHT. Čaká sa na schválenie…
```

Otvorte odkaz, prihláste sa do LosOS Login ako vždy, s druhým krokom, ak ho
máte, skontrolujte, že stránka ukazuje ten istý kód, a zvoľte **Accept**.
Terminál pokračuje sám.

Heslo do terminálu nikdy nepíšete. LosOS Login mu dá dva tokeny: jeden, ktorý
zariadenie otvorí na hodinu, a jeden, ktorý získa nový pár, keď ten vyprší.
Ak sa terminál 30 dní nepoužije, musí sa prihlásiť znova. Oba zostanú v
kľúčenke. Druhý `losos` na tomto počítači ich obnovuje spoločne, takže jeden
nemôže druhého omylom odhlásiť.

Terminál koná ako vy. Správca vidí každý panel, ktorý má. Ktokoľvek iný vidí
časti, ktoré mu boli dané, tie isté, ktoré mu ukazujú správcovské stránky, a
zmena, ktorú nemôže urobiť, je odmietnutá. Osobu, ktorú LosOS Login drží na
jej vlastnom účte, terminál nepustí, kým to nedokončí v časti **Vaše konto**
na správcovských stránkach.

```sh
losos logout https://mattbox.local
```

`logout` oba tokeny vráti do LosOS Login a zabudne ich. Zariadenie ich hneď
prestane brať.

Samotný `losos` otvorí celé zobrazenie: bočný panel vľavo, panel vpravo,
klávesy dole. Hlavička povie, kto je prihlásený. Šípky alebo `j` a `k`
pohybujú, Enter mení nastavenie, Tab prechádza medzi bočným panelom a
panelom, `/` panely hľadá, `?` vypíše zvyšok a `q` skončí. Zmena zostane
návrhom, kým nestlačíte `a` a nepoužijete ju, a zariadenie sa pred
prestavaním opýta, ako správcovské stránky.

:::note[Čo zostáva na správcovských stránkach]
Prístupové tokeny, trh, Diagramy a všetko, čo maže dáta, zostáva na
správcovských stránkach.
:::

## Staré prihlásenie

Počítač, ktorý sa kedysi prihlásil prístupovým tokenom a kľúčom GPG, má to
prihlásenie ešte v kľúčenke. `losos` ho nepoužíva. Povie, že prihlásenie je
starý prístupový token a že `losos login` sa namiesto toho prihlási cez LosOS
Login. `losos logout` staré prihlásenie odstráni. Dnu vedie jedna cesta.

## Vo vlastnej sieti zariadenia

Volania terminálu zostávajú vo vlastnej sieti zariadenia, tak ako
správcovské stránky. LosOS Login, LosOS Cloud a LosOS Git odpovedajú cez
internet. Terminál svoje volania touto cestou neposiela, pretože nesú
prihlásenie.

## LosOS Cloud a LosOS Git z tohto počítača \{#use-losos-cloud-and-losos-git-from-this-computer}

```sh
losos cloud setup
losos git setup
```

Každý sa opýta na heslo do LosOS, to, ktorým sa prihlasujete do LosOS Login,
a nastaví počítač, na ktorom ho spustíte. Panel **Tento počítač** v celom
zobrazení urobí to isté jedným klávesom. Spustiť ich môže správca.

`cloud setup` požiada zariadenie o heslo aplikácie len pre tento počítač a
drží ho v kľúčenke, kde ho hľadá správca súborov. LosOS Cloud sa potom
ukáže v bočnom paneli správcu súborov (GNOME Files a iné, ktoré čítajú
záložky GTK) a otvorenie ukáže vaše súbory bez pýtania hesla. Správca
súborov sa raz opýta, či prijme vlastný certifikát zariadenia, keď sa prvý
raz pripojí.

`git setup` požiada zariadenie o token LosOS Git len pre tento počítač a
drží ho v kľúčenke. git ho potom berie z `losos`, kedykoľvek hovorí so
zariadením, takže klonovanie a odosielanie heslo nepotrebujú:

```sh
git clone https://mattbox.local/git/notshared/notes.git
```

Keď má zariadenie vlastný certifikát, klient povie gitu, aby prijal presne
kľúč toho certifikátu, tú istú kontrolu, ktorú robí `losos`.

Oba sa volajú podľa počítača, napríklad „LosOS on laptop“. `--device` vyberie
iné meno. Prístup počítača zoberiete vymazaním jeho položky v časti
**Zabezpečenie** v nastaveniach LosOS Cloud alebo v časti **Aplikácie** v
LosOS Git. Nové nastavenie nahradí heslo alebo token toho počítača.

## V skripte

`losos call` pošle jednu požiadavku a vypíše odpoveď:

```sh
losos call GET /api/state
```

Skončí s 1, keď zariadenie odpovie chybou, takže to skript vie skontrolovať.
`losos --help` vypíše každý príkaz.

`login` v skripte vypíše ten istý odkaz a čaká. Keď je certifikát vlastný
certifikát zariadenia, odovzdajte jeho SHA-256 ako `--fingerprint`, ten,
ktorý ukazuje prehliadač, aby sa skript nemusel pýtať.
