---
title: Správcovské stránky
sidebar_position: 1
---

# Správcovské stránky

Správcovské stránky sú vlastná webová stránka zariadenia na
`http://<address>/`. Zariadenie ich poskytuje iba vo vašej lokálnej sieti,
aj keď ho edge zverejňuje. Každá karta vás požiada o prihlásenie vlastným
menom a heslom v LosOS Login, prihlasovacej stránke zariadenia na
`https://<address>/auth/`, skôr, než ukáže čokoľvek, čo sa dá zmeniť.

## Sekcie \{#the-sections}

Bočný panel vľavo má šesť sekcií a pod nimi to, čo ste si pripli. Zbalený sa
zúži na pás ikon. Na telefóne ho nahradí lišta pri spodnom okraji obrazovky,
na dosah palca: hlavné sekcie a potom **Viac**, ktoré vysunie celý bočný panel
zdola. Zatvoríte ho potiahnutím nadol. Ak na zariadení beží LosOS Chat a
LosOS Mail, zaberú dve miesta v lište.

![Prehľad: bočný panel vľavo, meno zariadenia a či odpovedá, dlaždice aplikácií, karta úložiska a nástenka.](@site/docs/img/overview-sk.png)

| Sekcia         | Čo tam je                                                                                                   |
| -------------- | ----------------------------------------------------------------------------------------------------------- |
| **Prehľad**    | Či zariadenie odpovedá a ako dlho, karta úložiska, dlaždice aplikácií a vaša nástenka widgetov.              |
| **Aplikácie**  | Aplikácie na tomto zariadení s ich nastaveniami, vyhľadávanie v Artifact Hub a aplikácie nainštalované odtiaľ.    |
| **Úložisko**   | Ako je disk plný, čo zaberá miesto a rezerva, ktorú si môžete vziať.                                         |
| **Mesh**       | Pripojenie k ďalším zariadeniam, hodiny, keď toto zariadenie požičiava svoj voľný čas, **Trh** so zdieľaním disku a **Stroje**. |
| **Diagramy**     | LosOS Diagrams: diagramy, plány a tabule ako karty a sieť tohto zariadenia nakreslená a simulovaná.    |
| **Nastavenia** | Sieť, Vzhľad, Hardvér, Zabezpečenie, Používatelia, Prístupové tokeny, Prihlasovacie údaje, Rozšírené, História, Záloha, O zariadení a Reset, každé ako samostatný panel.   |

<img src={require("@site/docs/img/overview-phone-sk.png").default} alt="Prehľad na telefóne: Prehľad, Aplikácie, Úložisko a Mesh v lište pri spodnom okraji, všetko ostatné pod Viac." width="260" /> <img src={require("@site/docs/img/overview-phone-sheet-sk.png").default} alt="Ten istý telefón po ťuknutí na Viac: všetky sekcie v paneli, ktorý sa vysunie zdola." width="260" />

## Vyhľadávanie \{#search}

Lupa v hornej časti bočného panela otvorí jeden zoznam, ktorý nájde stránky,
aplikácie, súbory, repozitáre aj úlohy. Na ktorejkoľvek správcovskej stránke ho
otvorí aj Ctrl+Shift+P (na Macu Cmd+Shift+P) a tiež dvojbodka napísaná
kdekoľvek mimo textového poľa. Nájde:

- sekcie a panely nastavení, podľa mena alebo podľa slova ako „gpu“ či
  „hostname“, v jazyku na obrazovke alebo v angličtine
- aplikácie v LosOS Cloud a LosOS Git
- súbory a priečinky v LosOS Cloud, podľa mena
- repozitáre v LosOS Git
- úlohy v LosOS Git, podľa slov alebo kľúča ako `BT-12`; keď nič
  nenapíšete, otvorené úlohy pridelené vám
- text vnútri súborov, keď napíšete tri písmená: vo vašich súboroch v LosOS
  Cloud a na hlavnej vetve každého repozitára v LosOS Git
- vaše pripnuté položky
- aplikácie na inštaláciu z Artifact Hub; keď jednu vyberiete, otvorí sa
  panel Aplikácie s jej inštalačným dialógom
- niekoľko akcií: svetlá alebo tmavá téma, jazyk, prehliadka, táto príručka
  a odhlásenie

Šípky sa pohybujú po zozname, Enter otvorí zvýraznenú položku a Escape
zoznam zavrie. Súbory a repozitáre sa otvárajú vo vlastnej aplikácii a
Ctrl+Enter alebo Cmd+Enter otvorí položku na novej karte.

:::note[Vyhľadávanie súborov potrebuje heslo aplikácie zariadenia]
Vyhľadávanie súborov používa heslo aplikácie, ktoré zariadenie dostane od
LosOS Cloud, keď sa nastaví heslo vlastníka, a to urobí sprievodca. Bez neho
to zoznam napíše tam, kde by boli súbory; pozrite
[Ako zariadenie vypisuje vaše súbory](#pins) nižšie.
:::

Aby mohlo hľadať vnútri vašich súborov v LosOS Cloud, zariadenie si vedie
index ich textu. Každé dve minúty skontroluje nové a zmenené súbory a číta
iba tie. Číta obyčajný text a kód, Markdown, CSV, JSON a HTML, súbory
OpenDocument, súbory Wordu, Excelu a PowerPointu v súčasných formátoch
(.docx, .xlsx, .pptx) a PDF. Obrázky, zvuk, video a archívy preskakuje a
z každého súboru si ponechá iba prvý megabajt textu. Hneď po prvom štarte
zoznam píše, že zariadenie ešte číta vaše súbory. Index zostáva na
zariadení, výmaz do továrenského stavu ho zmaže a `losos.search.enable` na
paneli Rozšírené ho vypne. LosOS Git index nepotrebuje: každé vyhľadávanie
číta repozitáre priamo.

## Ponuka pravého tlačidla \{#the-right-click-menu}

Kliknite pravým tlačidlom na stránkach správy, v LosOS Cloud alebo v Pošte
a ponuka ukáže, čo môžete urobiť s tým, čo je pod ukazovateľom. Na
dotykovej obrazovke podržte prst. Z klávesnice ju otvorí Shift+F10 alebo
kláves Menu pre to, čo má práve fokus, a šípky sa ňou pohybujú.

- **Prázdne miesto**: Späť, Dopredu, Načítať znova, vyhľadávanie, svetlý
  alebo tmavý vzhľad a táto príručka.
- **Označený text**: Kopírovať a hľadanie označených slov.
- **Odkaz**: otvoriť na novej karte alebo skopírovať adresu.
- **Dlaždica aplikácie alebo pripnutá položka**: otvoriť na novej karte,
  pripnúť alebo odopnúť.
- **Súbor alebo priečinok v LosOS Cloud**: otvoriť, stiahnuť, premenovať,
  presunúť, skopírovať, staršie verzie, hviezdička, zdieľať, skopírovať odkaz,
  vymazať. Keď je zaškrtnutých viac súborov, ponuka pracuje so všetkými.
  Miesto okolo zoznamu ponúka nový priečinok a nahrávanie, súbor v koši
  obnovenie alebo vymazanie natrvalo.
- **Správa v Pošte**: otvoriť, odpovedať, odpovedať všetkým, preposlať,
  označiť ako prečítanú alebo neprečítanú, označiť, archivovať, presunúť
  do iného priečinka, odstrániť.
- **Človek v Kontaktoch**: pošta, chat, hovor alebo video, upraviť,
  exportovať ako `.vcf`, odstrániť. Človek v zozname **Na tomto boxe**
  ponúka namiesto posledných troch **Pridať do kontaktov**.

![Ponuka na prázdnom mieste Prehľadu s otvoreným Vzhľadom: svetlý, tmavý a podľa prehliadača.](@site/docs/img/context-menu-page-sk.png)

![Ponuka súboru v LosOS Cloud: Otvoriť, Stiahnuť, Premenovať s F2, Presunúť, Kopírovať, Verzie, Pridať do obľúbených, Zdieľať, Kopírovať odkaz, Vytvoriť úlohu a Odstrániť s Del.](@site/docs/img/context-menu-file-sk.png)

Klávesy vedľa položky fungujú aj bez ponuky. V LosOS Cloud F2 premenuje
zaškrtnutý súbor a Delete ho presunie do koša (na Macu Cmd+Backspace).
Textové polia si nechávajú ponuku prehliadača, kvôli pravopisu a vkladaniu.
Ponuka ukazuje iba to, čo toto zariadenie vie: položka pre funkciu, ktorú
zariadenie nemá, v nej nie je, ani sivá.

## Lišta nad výberom \{#the-bar-over-a-selection}

Označte slová v dokumente alebo prezentácii Markdown v LosOS Office alebo
v poznámke a po pustení tlačidla myši sa nad nimi objaví malá lišta.
Vyberte karty alebo šípky v Diagramoch a objaví sa nad nimi. Ponúka to,
čo najpravdepodobnejšie urobíte ďalej, takže to nemusíte hľadať na paneli
nástrojov.

- **Text, ktorý môžete upravovať** (LosOS Office a Poznámky v režime
  **Upraviť**): **Opýtať sa AI**, tučné, kurzíva, prečiarknuté,
  zvýraznenie a kód, odkaz na webovú stránku, odkaz na inú poznámku,
  zmienka o osobe alebo súbore a Kopírovať. Stlačené tlačidlo ukazuje, že
  formát už je použitý; ďalším stlačením zmizne. Odkaz si adresu vypýta
  priamo v lište a odkaz na poznámku či zmienka ponúka mená počas písania.
- **Poznámka, ktorú čítate**: **Opýtať sa AI**, Kopírovať a skopírovanie
  odkazu na časť, v ktorej slová sú.
- **Karty a šípky v Diagramoch**: **Opýtať sa AI**, keď karty obsahujú
  text, farba, úprava textu jednej karty, zarovnanie viacerých, šípka
  medzi dvoma kartami, zoskupenie, duplikovanie a odstránenie. Každé
  tlačidlo robí to isté ako tlačidlo v inšpektore, takže ho jedno Späť
  vráti.

Keď na zariadení beží Pošta alebo Chat, lišta nad textom pošle slová aj
e-mailom alebo v chate. **Opýtať sa AI** sa ukazuje iba vtedy, keď je AI
zariadenia zapnutá; pri texte, ktorý môžete upravovať, vloží **Nahradiť
výber** odpoveď namiesto označených slov.

Z klávesnice sa do lišty dostanete klávesmi Alt+F10, šípky sa ňou
pohybujú a Enter alebo medzerník stlačí tlačidlo. Escape vás vráti do
textu alebo diagramu a skryje lištu, kým neoznačíte niečo iné. Lišta
zmizne aj vtedy, keď začnete písať cez výber.

## Pripnuté položky \{#pins}

Pripnite si aplikácie, súbory, repozitáre a úlohy, ktoré otvárate najčastejšie, a
budú v bočnom paneli pod **Pripnuté**. Pripnutá položka sa otvára vo
vlastnej aplikácii: súbor alebo priečinok v LosOS Cloud, repozitár v LosOS
Git, úloha alebo projekt v [úlohách](./issues.md) LosOS Git, aplikácia na Prehľade alebo aplikácia nainštalovaná z vyhľadávania na
vlastnom porte. Zoznam drží zariadenie, takže každý prehliadač, z ktorého sa
prihlásite, ukazuje tie isté pripnuté položky.

- Na Prehľade ukážte na dlaždicu aplikácie a kliknite na špendlík v jej
  rohu. Pripnutá dlaždica si špendlík nechá, farebný.
- Na stránke Aplikácie je pri každej nainštalovanej aplikácii **Pripnúť**.
- Ceruzka pri **Pripnuté** otvorí celý zoznam. Tam pripnutú položku posuniete
  vyššie alebo nižšie, odopnete ju a pripínate zo štyroch kariet: **Aplikácie**,
  **Súbory**, ktorá prechádza priečinkami LosOS Cloud, **Repozitáre** a
  **Úlohy**, ktorá nájde úlohu podľa slov alebo kľúča.
- Na úlohe alebo projekte v LosOS Git ju pripne špendlík navrchu. Úloha je
  v bočnom paneli pod svojím názvom.
- Ukážte na pripnutú položku v bočnom paneli a kliknutím na prečiarknutý
  špendlík ju odopnete. Pripnuté položky nájde aj
  [vyhľadávanie](#search).
- Kliknite pravým tlačidlom na súbor, priečinok, repozitár alebo úlohu vo
  vyhľadávaní, na úlohu v zozname alebo na nástenke LosOS Git, na dlaždicu aplikácie na Prehľade alebo na pripnutú položku
  v bočnom paneli a vyberte **Pripnúť na bočný panel** alebo **Odopnúť
  z bočného panela**. Na dotykovej obrazovke na položke podržte prst.

![Prehľad so šiestimi pripnutými položkami pod Pripnuté v bočnom paneli a označenými pripnutými dlaždicami Kalendár a Poznámky.](@site/docs/img/pins-overview-sk.png)

![Zoznam pripnutých položiek s kartou Súbory otvorenou na priečinku Documents v LosOS Cloud.](@site/docs/img/pins-dialog-sk.png)

:::note[Ako zariadenie vypisuje vaše súbory]
Zariadenie vypisuje vaše súbory s heslom aplikácie, o ktoré požiada LosOS
Cloud, keď sa nastaví heslo vlastníka, a to urobí sprievodca. Heslo
aplikácie sa v nastaveniach zabezpečenia LosOS Cloud zobrazí ako „LosOS
admin page“. Keď ho tam zrušíte, výpis aj vyhľadávanie súborov sa zastavia,
kým sa heslo vlastníka nenastaví znova, a to cestou s náhradným kľúčom na
stránke [Zabudnuté heslo](../troubleshooting/forgot-password#you-have-the-spare-admin-key). Bočný panel pojme 24 pripnutých položiek.
:::

## Prehliadka \{#the-tour}

Keď sa na tieto stránky po nastavení dostanete prvýkrát, nad Prehľadom sa
otvorí krátka prehliadka: šesť krokov na každom zariadení a jeden navyše za
každú funkciu, ktorú toto zariadenie má ([Prvé spustenie](../start/first-run)
ich vymenúva). **Spustiť prehliadku** v
Nastaveniach, O zariadení ju otvorí znova. Ak máte v systéme nastavené
„obmedziť pohyb“, každý krok namiesto animácie ukáže statický obrázok.

## Pruh o sieti mesh \{#the-mesh-banner}

Niektoré funkcie bežia len na zariadení, ktoré zdieľa svoje úložisko a je
pripojené k sieti mesh. Kým neplatí oboje, pruh nad každou stránkou hovorí,
že niektoré funkcie nemusia byť dostupné, a odkazuje na **Mesh**, kde sa
pripojíte, alebo na **Trh**, kde zapnete zdieľanie disku. **Zavrieť** ho v
danom prehliadači skryje. Zmizne sám, keď sa zariadenie pripojí, a vráti
sa, ak zariadenie sieť mesh opustí.

## Upozornenia \{#notifications}

Krátke stavové správy sa objavujú vpravo hore: zmena sa práve používa,
zmena sa podarila, widget pribudol. Otázka, na ktorú musíte odpovedať,
napríklad reset zariadenia alebo použitie rezervy, namiesto toho otvorí
dialóg. Bezpečná voľba je v ňom prvá.

![Panel nastavení so zmenou, ktorá ešte nie je použitá: lišta dole s tlačidlami Zahodiť a Použiť.](@site/docs/img/settings-apply-bar-sk.png)

## Hra počas čakania \{#a-game-while-you-wait}

Niektoré veci trvajú minúty: použitie zmeny, prvé spustenie LosOS Cloud,
štart LosOS Login, inštalácia aplikácie. Kým niektorá z nich beží, tlačidlo
**Zahrajte si, kým čakáte** vedľa priebehu otvorí pod ním malú hru, náhodne
jednu z piatich: Proti prúdu, Úhor, Spájanie, Tehličky a Dvojice. Priebeh
zostáva nad hrou na obrazovke. Nič sa nezačne, kým nestlačíte kláves alebo
neťuknete, hra nemá zvuk a pozastaví sa, keď prejdete na inú kartu. Keď
čakanie skončí, hra sa zastaví a oznámi to a **Zavrieť hru** ju odloží. Ak
máte v systéme zapnuté obmedzenie pohybu, ponúknu sa len Spájanie a Dvojice,
lebo v nich sa medzi ťahmi nič nehýbe. Najlepšie skóre každej hry si
pamätá tento prehliadač.

## Téma a jazyk \{#theme-and-language}

Vaša ponuka vpravo hore, pod vaším menom, má **Vzhľad** („Podľa
prehliadača“, svetlý alebo tmavý) a **Jazyk** (jazyk prehliadača,
angličtina, slovenčina alebo nemčina). Je to tá istá ponuka na každej
stránke zariadenia: na správcovských stránkach, v LosOS Cloud, Office,
Mail, Chat a LosOS Git. Stránka, na ktorú nie ste prihlásení, má obe voľby
za jedným tlačidlom vo svojej lište. Tento prehliadač si obe voľby
zapamätá.

![Prehľad v nemčine a v tmavej téme.](@site/docs/img/overview-de-dark.png)

## Odhlásenie \{#sign-out}

**Odhlásiť sa** ukončí vaše prihlásenie aj v LosOS Login, takže ďalšia
návšteva si znova vypýta meno a heslo. Ak ste stránky otvorili náhradným
kľúčom, zabudne namiesto toho kľúč na tejto karte a zatvorenie karty urobí
to isté. Pozri [Prihlásenie a náhradný kľúč](sign-in-and-spare-key).

## Čo správcovské stránky nedokážu \{#what-the-admin-pages-cannot-do}

Nie je tu prehliadač súborov, terminál ani prehliadač logov. Vaše súbory sú
v LosOS Cloud; karta **Súbory** v zozname pripnutých položiek ukazuje
priečinky iba na to, aby ste z nich mohli pripínať. Zariadenie nemá shell,
takže ho nemajú ani stránky. Zariadenie o sebe podáva správu cez Prehľad,
panel Úložisko a panel O zariadení. Keď to nestačí, stránka
[Keď niečo nefunguje](../troubleshooting) hovorí, čo ďalšie je k dispozícii.
