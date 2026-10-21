---
title: Aplikácie
sidebar_position: 4
slug: /manual/apps
---

# Aplikácie

## LosOS cloud \{#losos-cloud}

Vaše súbory, fotky, kalendár, kontakty, poznámky, úlohy, hudba a ostatné
aplikácie nižšie, na `http://<address>/nextcloud`. Je to Nextcloud vo
farbách a s menom zariadenia. Prihlásite sa heslom vlastníka. LosOS cloud
dokáže všetko, čo Nextcloud. Desktopový klient synchronizuje priečinok na
vašom počítači, aplikácie v telefóne nahrávajú fotky a kalendáre a kontakty
sa synchronizujú cez CalDAV a CardDAV.

Prehľad má dlaždicu pre každú aplikáciu z ponuky LosOS cloud, v rovnakom
poradí: Súbory, Nástenka, Fotky, Aktivita, Kontakty, Kalendár, Poznámky,
Záložky, Deck, Hudba, Kolektívy, Ankety, Formuláre, Tabuľky, Memories,
Správy, Úlohy a Mapy. Zariadenie ich pri každom štarte všetky zapne, okrem
tých, ktoré ste vypli na paneli **Aplikácie**. Pošta medzi nimi nie je,
pretože zariadenie nemá poštový server, ktorý by mohla čítať. Ak si Poštu
aj tak pridáte z obchodu s aplikáciami, jej dlaždica sa na Prehľade objaví
pri ďalšom načítaní stránky. Odkazy vedú zámerne cez
`index.php`, pretože na túto cestu odpovedá každá konfigurácia.

Rodina alebo kolegovia dostanú účet v LosOS cloud, keď ho prvýkrát otvoria,
prihlásení cez LosOS Login ([Prihlásenie](sign-in-and-spare-key)). Aj účet
vlastníka je bežný účet, nie správca LosOS cloud. To, ktoré aplikácie
bežia, a nastavenia pre všetkých sú namiesto toho na paneli **Aplikácie**,
v sekcii LosOS cloud.

## LosOS Git \{#losos-git}

Vaše repozitáre, na `http://<address>/git/<účet>/<repozitár>.git`. LosOS
Git je vlastný Git server zariadenia. Človek, ktorého pozná LosOS Login, v
ňom má účet od prvej návštevy, pod tým istým menom. Počítač klonuje a
pushuje s vlastným tokenom, ktorý si git pýta ako heslo: vyrobí ho
`losos git setup` ([Terminálový klient](terminal)). Vlastná konfigurácia
zariadenia je súkromný repozitár `notshared/losos-config`
([Správa](../in-depth/administration)). Adresy na klonovanie sa riadia
adresou, cez ktorú ste sa k zariadeniu dostali. Ak zariadenie nie je na kód, vypnite LosOS Git na paneli
**Aplikácie**.

### Federácia \{#federation}

LosOS cloud federuje: súbory a kalendáre môžete zdieľať s ľuďmi, ktorí
majú účty na iných serveroch Nextcloud, podľa ich adresy `user@server`;
vlastník iného zariadenia je `notshared@inezariadenie.local/nextcloud`.
Vypne to prepínač **Zdieľať s inými servermi Nextcloud** v sekcii LosOS
cloud. Kým je zapnutý, LosOS cloud smie siahnuť na mená končiace na
`.local` a na adresy vo vašej lokálnej sieti a prijme certifikát, ktorý si
každé zariadenie vyrobí samo, čo Nextcloud sám odmieta. Tak sa dostane
k ostatným zariadeniam v tej istej sieti.

:::note[Federácia potrebuje zdieľanie disku]
Federácia funguje iba vtedy, keď zariadenie zdieľa svoj disk
(`losos.sharingMyStorage`), pretože iba vtedy je zdieľaná časť disku
odomknutá. So zdieľaním vypnutým sa zariadenie nerozpráva so žiadnym iným
serverom a panel **Aplikácie** ukazuje prepínač federácie sivý.
:::

## Panel Aplikácie \{#the-apps-pane}

Pri každej aplikácii panel ukazuje, či je zapnutá, v akom
[režime](../types/app-modes) beží, a odkaz. Pod nimi majú LosOS cloud aj
LosOS Git vlastnú rozbaľovaciu sekciu, zavretú, kým ju neotvoríte. Pod tým
**Nájsť ďalšie** hľadá
Helm charty v [Artifact Hub](https://artifacthub.io). Výsledky sťahuje
zariadenie, takže vyhľadávanie potrebuje internet. Každý riadok uvádza, kto
chart zverejnil. Nikto na strane LosOS žiadny z nich neskontroloval.

![Panel Aplikácie s jednou nainštalovanou aplikáciou nad Nájsť ďalšie a výsledkami vyhľadávania v Artifact Hub, každý s tým, kto ho zverejnil, a tlačidlom Inštalovať alebo Zmeniť.](@site/docs/img/apps-find-more-sk.png)

## Aplikácie v LosOS cloud \{#losos-clouds-apps}

Sekcia LosOS cloud vypisuje aplikácie vnútri LosOS cloud, každú s
prepínačom. Vypnutá aplikácia zmizne zo všetkých účtov a to, čo v nej ľudia
vytvorili, zostane uložené pre jej návrat. Vypnutá zostane aj po reštarte
zariadenia. Aplikácie zabudované v LosOS cloud čakajú za tlačidlom
**Zobraziť**, lebo ich potrebujete zriedka; nájde ich aj filter. Niekoľko
prepínačov sa pohnúť nedá a riadok povie prečo: LosOS cloud bez nich
nebeží, zariadenie ich potrebuje na prihlásenie alebo sa riadia zdieľaním
disku.

**Pridať z obchodu s aplikáciami** hľadá v obchode s aplikáciami Nextcloud
a tlačidlom **Pridať** aplikáciu pridá. Hľadá zariadenie, takže to potrebuje
internet. Nikto na strane LosOS tieto aplikácie neskontroloval. Aplikácia
pridaná z obchodu má tlačidlo **Odstrániť**, ktoré sa najprv opýta.
Odstránenie zoberie zo zariadenia jej kód a ponechá, čo v nej účty
vytvorili, takže ju opätovné pridanie nájde. Aplikácie, ktoré zariadenie
prináša, sa dajú vypnúť, ale nie odstrániť.

![Sekcia LosOS cloud: aplikácie s prepínačmi, Deck z obchodu s aplikáciami s tlačidlom Odstrániť, prihlasovacia aplikácia uzamknutá s dôvodom a hľadanie Whiteboard v obchode s tlačidlom Pridať.](@site/docs/img/apps-cloud-sk.png)

## Nastavenia pre všetkých \{#settings-for-everyone}

V sekciách LosOS cloud a LosOS Git má panel Aplikácie nastavenia, ktoré
platia pre každý účet v danej aplikácii. Inak by ste ich našli na vlastných
administrátorských stránkach každej aplikácie. Sekcia LosOS cloud začína
svojím prepínačom federácie ([Federácia](#federation)).

Pre LosOS cloud: jazyk prihlasovacej stránky a účtov, ktoré si žiadny
nevybrali, krajina, do ktorej patrí telefónne číslo bez predvoľby krajiny,
či sa súbory dajú zdieľať verejným odkazom, či každý odkaz potrebuje heslo,
po koľkých dňoch odkazy prestanú fungovať, koľko miesta dostane nový účet a
ako dlho sa uchováva kôš a staré verzie. Prázdne pole nechá voľbu na LosOS
cloud.

Pre LosOS Git: či niečo vidia iba prihlásené účty, či nový repozitár
začína ako súkromný, verejný alebo ako posledný repozitár účtu a či push
do repozitára, ktorý ešte neexistuje, ho vytvorí.

![Panel Aplikácie s nastaveniami pre všetkých v LosOS cloud a LosOS Git.](@site/docs/img/apps-site-settings-sk.png)

Ako každé iné nastavenie ich použijete tlačidlom **Použiť**. Zariadenie ich
nastaví znova pri každom štarte aplikácie, takže zmena urobená vnútri
aplikácie nevydrží. Vlastné nastavenia každého účtu zostávajú v aplikácii.

## Inštalácia aplikácie z vyhľadávania \{#installing-an-app-from-the-search}

Riadok, ktorý zariadenie dokáže stiahnuť, má tlačidlo **Inštalovať**.
Otvorí dialóg v dvoch krokoch.

Prvý krok prečíta chart a ukáže tri veci.

- **Názov na tomto zariadení.** Začína ako názov chartu. Pomenúva aj
  priečinok aplikácie, takže ho neskôr zmeniť nemôžete.
- **Spustiť ako.** `notshared` alebo `shared`, dvaja používatelia
  zariadenia. Aplikácia `notshared` drží svoje dáta na strane disku, ktorá
  patrí vlastníkovi, vedľa vašich súborov. Aplikácia `shared` ich drží na
  strane, ktorú zariadenie požičiava do mesh. Kým zariadenie nezdieľa svoj
  disk, dialóg má `shared` sivé.
- **Vlastnosti.** Každé nastavenie zo súboru hodnôt chartu s jeho
  predvolenou hodnotou, zbalené podľa sekcií. Keď chart dodáva schému, každé
  nastavenie dostane aj svoj opis a možnosti. **Ako text** namiesto toho
  ukáže na úpravu celý súbor hodnôt. Vlastnosť zmenená vo formulári má
  prednosť pred tým istým riadkom v texte.

![Inštalačný dialóg: názov, voľba medzi notshared a shared so sivým shared a vlastnosti chartu zbalené podľa sekcií.](@site/docs/img/apps-install-sk.png)

Druhý krok sa ešte raz opýta. Hovorí, že zariadenie stiahne a spustí kód,
ktorý napísal vydavateľ, ako používateľ, ktorého ste vybrali, bez
administrátorských práv. Kým tam nestlačíte **Inštalovať**, zariadenie nič
nedostane.

Zariadenie potom nainštaluje chart do vlastného klastra a upraví ho tak, aby
bežal rovnako ako LosOS cloud a LosOS Git.

- Každá časť aplikácie beží ako vybraný používateľ, nikdy ako root, bez
  ďalších oprávnení a bez prístupu ku klastru.
- Úložisko, o ktoré chart žiada, sa stane priečinkami pod
  `/home/<user>/data/apps/<name>`.
- Aplikácia dostane jeden vlastný port z `losos.apps.ports`, predvolene 30000
  až 30099, a dostane sa k nemu iba vaša lokálna sieť.

:::note[Charty, ktoré zariadenie odmietne]
Chart, ktorý potrebuje viac, zariadenie odmietne a povie prečo. Typickými
dôvodmi sú administrátorské práva, port pod 1024, port, ktorý už používa
iná aplikácia, alebo systémový priečinok. Aplikácia zložená z viacerých
služieb, ktoré sa navzájom volajú menom, sa nemusí spustiť, pretože vlastný
klaster zariadenia nemá vnútornú sieť.
:::

**Nainštalované aplikácie**, nad vyhľadávaním, vypisuje každú aplikáciu s jej
stavom. **Otvoriť** prejde na port aplikácie na adrese, ktorú práve
používate. **Zmeniť** otvorí ten istý dialóg s aktuálnymi vlastnosťami
aplikácie. **Odstrániť** sa najprv opýta a potom aplikáciu zo zariadenia
odstráni. Jej priečinok s dátami zostane, takže inštalácia pod tým istým
názvom dáta znova nájde.

:::note[Inštalácia potrebuje režim záťaže]
Vyhľadávanie ponúka inštaláciu iba vtedy, keď sú Súbory alebo Kód nastavené
na **Oddelene**, čo je [režim záťaže](../types/app-modes). Iba vtedy beží
vlastný klaster zariadenia. Inštalovanie vypnete nastavením
`losos.apps.enable` na false na paneli **Rozšírené**.
:::

## Dôveryhodné adresy \{#trusted-addresses}

LosOS cloud odpovedá na meno zariadenia a na tú adresu, na ktorú prišla
požiadavka. Zariadenie túto adresu posiela spolu s každou požiadavkou. LosOS
cloud nikdy neodpovedá na zástupný znak. Preto zariadenie, na ktoré
pristupujete cez IP adresu, neukáže stránku o nedôveryhodnej doméne. Ak ju
niekedy uvidíte, [Nedôveryhodná doména](../troubleshooting/app-tile-404#access-through-untrusted-domain)
vysvetľuje prečo.
