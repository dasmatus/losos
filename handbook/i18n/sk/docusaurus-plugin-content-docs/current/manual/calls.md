---
title: Hovory
sidebar_position: 11
slug: /manual/calls
---

# Hovory

Ľudia na zariadení si môžu z LosOS chatu volať, hlasom alebo s videom, v
priamej správe alebo v miestnosti s najviac ôsmimi ľuďmi. Hovor ide cez
zariadenie a zariadenie ho nevidí ani nepočuje. Každý snímok obrazu a zvuku
zašifruje prehliadač, ktorý ho posiela, a dešifrujú ho až prehliadače,
ktoré ho prijímajú.

Hovory sa riadia štandardom MatrixRTC, ktorý Matrix používa na hovory v
miestnostiach, ako vlastný typ hovoru LosOS, `org.losos.call`, s jedným
hovorom na miestnosť. Každý v hovore zverejní členstvo, ktoré uvádza server
hovorov zariadenia, `losos-calls`. Aplikácia Matrix, ktorá tento typ hovoru
nepozná, ho preto nechá tak a neponúkne **Pripojiť sa**, ktoré by
nefungovalo. Zvonenie, odmietnutie a výmena kľúčov používajú vlastné
udalosti MatrixRTC. Obraz a zvuk idú cez `losos-calls`, nie cez LiveKit.

## Kedy sú hovory k dispozícii \{#when-calls-are-there}

Hovory patria k chatu, takže sú všade tam, kde je chat, so sieťou mesh aj
okrajovým serverom, alebo bez nich. Potrebujú aj
**Nastavenia → Rozšírené → `losos.calls.enable`**, ktoré je zapnuté, kým ho
nevypnete. Ak je chat alebo toto vypnuté, miestnosti nemajú tlačidlá na
volanie a zariadenie pre hovory nespúšťa nič. Keď niečo z toho znova
zapnete, hovory sa vrátia po ďalšom Použiť.

## Ako začať hovor \{#starting-a-call}

Otvorte miestnosť alebo priamu správu. V hlavičke sú dve tlačidlá,
**Hlasový hovor** a **Videohovor**. Prehliadač sa raz opýta na mikrofón, a
na kameru, keď začínate s videom.

Hovor kameru nepotrebuje. Ak prehliadač žiadnu neotvorí, lebo žiadna nie je
alebo ste ju nepovolili, pripojíte sa s vypnutou kamerou a ukáže sa o tom
správa; ostatní vás počujú ako zvyčajne. Bez mikrofónu sa pripojíte len ako
poslucháč a tlačidlo mikrofónu sa naň opýta znova.

V priamej správe sa druhému človeku v rohu stránky chatu ukáže karta,
ktorá zvoní minútu a pol. V miestnosti sa ukáže všetkým, ak ten, kto hovor
začína, smie upozorniť celú miestnosť, čo je predvolene správca miestnosti;
inak ostatní uvidia hovor v časovej osi. **Prijať** pripojí tak, ako sa
hovor začal. Pri videohovore **Prijať bez videa** pripojí len s mikrofónom.
**Odmietnuť** kartu skryje. V priamej správe sa to dozvie aj volajúci a
jeho hovor sa zatvorí.

Časová os miestnosti ukáže, že sa začal hovor. Kým hovor trvá, riadok má
tlačidlo **Pripojiť sa**, takže sa môže pripojiť aj ten, kto zvonenie
zmeškal alebo z hovoru odišiel. Keď sa niekomu zavrie prehliadač alebo
stratí sieť, ostatní uvidia, že odišiel, najneskôr do pol minúty. Keď odíde
posledný človek, časová os ukáže,
že sa hovor skončil. Iné aplikácie Matrix namiesto toho ukážu krátke
oznámenie s odkazom, ktorý otvorí hovor v LosOS Chat, kde sa ľudia
prihlásia cez LosOS Login.

Miestnosti a priame správy vytvorené v LosOS Chat sú na hovory pripravené.
V miestnosti vytvorenej staršou verziou LosOS zapne hovory správca
miestnosti tým, že začne prvý. Dovtedy dostane každý iný, kto to skúsi,
správu, ktorá to vysvetľuje.

## Počas hovoru \{#during-a-call}

Každý človek má svoju dlaždicu. Dlaždica ukazuje jeho kameru, alebo jeho
obrázok, kým má kameru vypnutú. Tlačidlá dole vypínajú a zapínajú váš
mikrofón a kameru, zdieľajú obrazovku, prepínajú pozadie, pozývajú hosťa,
menia hlas, pridávajú LosOS, otvárajú režim podcastu a ukončia vašu účasť v hovore. Vypnutie kamery ju
zastaví, takže zhasne jej svetlo, a kým ju znova nezapnete, neposiela sa
nič.

Váš hlas sa posiela rýchlosťou 128 kb/s v celom počuteľnom rozsahu, so
zapnutým potlačením ozveny a šumu v prehliadači. Kamera posiela plynulých
30 snímok za sekundu. Keď spojenie nestačí, najprv zníži ostrosť a pohyb
nechá plynulý; zdieľaná obrazovka robí opak a text nechá ostrý.

Štítok **Šifrované end-to-end** hore ukazuje, koľko snímok váš prehliadač
poslal zašifrovaných a koľko prijal a otvoril.

## Váš hlas \{#your-voice}

**Zmeniť môj hlas** mení, ako vás počujú ostatní: **Hlbší**, **Vyšší**,
**Obor**, **Veverička** alebo **Robot**, a **Prirodzený** je váš vlastný
hlas. Deje sa to vo vašom prehliadači pred zašifrovaním hovoru, takže
zariadenie aj ostatní dostanú len zmenený hlas a nikto nepozná, že nie je
váš. Voľba sa zapamätá pre ďalšie hovory, kým nezvolíte **Prirodzený**.

## Vaše pozadie \{#your-background}

Keď máte zapnutú kameru, miestnosť za vami je čierna. Prehliadač vás na
každom snímku nájde a všetko ostatné začierni ešte predtým, ako snímok
zašifruje, takže vašu miestnosť nedostane ani zariadenie, ani ostatní.
Tlačidlo **Ukázať miestnosť za vami** posiela kameru tak, ako je, a
**Zatemniť pozadie** čiernu znova zapne. Prehliadač si vašu voľbu
zapamätá na ďalší hovor.

Ak prehliadač pozadie začierniť nevie, kamera namiesto vašej miestnosti
nepošle nič a ukáže sa o tom správa.

## Zdieľanie obrazovky \{#sharing-your-screen}

**Zdieľať obrazovku** sa prehliadača opýta, ktorú obrazovku, okno alebo
kartu zdieľať. Kým niekto zdieľa, jeho obrazovka vyplní stred hovoru a
ľudia sa presunú do pásu vedľa nej. **Prestať zdieľať** alebo tlačidlo
zastavenia v samotnom prehliadači zdieľanie ukončí. Obrazovka je
zašifrovaná tými istými kľúčmi ako kamera.

## Režim podcastu \{#podcast-mode}

Režim podcastu vysiela hovor na adresu RTMP, napríklad tú, ktorú vám s
kľúčom streamu dá YouTube alebo Twitch. **Režim podcastu** otvorí panel s
týmito poľami:

- adresa RTMP, ktorá začína `rtmp://` alebo `rtmps://`;
- názov a podnadpis, ktoré sa kreslia pod obraz;
- prekrytie, obrázok PNG kreslený navrch, napríklad logo alebo rámik.

Vysielanie ukazuje ľudí v mriežke, alebo zdieľanú obrazovku v strede a
ľudí vedľa nej, rovnako ako hovor. **Začať vysielať** ho spustí a
**Ukončiť vysielanie** zastaví. Hovor naraz vysiela len jeden človek a
všetci v hovore vidia počas vysielania štítok **Naživo**.

Živé vysielanie je verejné, takže je to jediná časť hovoru, ktorá nie je
šifrovaná end-to-end. Váš prehliadač zloží obraz a zvuk z hovoru, ktorý už
dešifroval, a pošle ich do zariadenia, ktoré ich odovzdá na adresu RTMP.
Kľúč streamu zostáva vo vašom prehliadači a v zariadení a nikdy sa
nezapíše do záznamov zariadenia.

## Hostia \{#guests}

Niekto bez účtu na zariadení sa môže pripojiť k jednému hovoru cez odkaz
pre hosťa. **Pozvať hosťa** odkaz vytvorí a ukáže ho s tlačidlom
**Kopírovať odkaz**. Pošlite ho osobe, ktorú pozývate. Otvorí ho v
prehliadači, zadá meno a pripojí sa. Ostatní ju uvidia ako *meno (hosť)*.

Odkaz pre hosťa:

- otvorí len tento hovor, na 12 hodín;
- pustí naraz len jedného hosťa;
- nedáva účet, chat ani prístup k ničomu inému na zariadení;
- pre hosťa prestane fungovať, keď z hovoru odídu všetci z miestnosti.

Odkazy pre hostí vydáva oficiálny edge LosOS na verejnom mene zariadenia,
takže zariadenie bez neho ich vytvoriť nevie. Edge odkaz vytvorí, ale
hovor nepočuje. Časť za `#` je tajomstvo, ktoré vytvorí váš prehliadač, a
prehliadač túto časť nikdy nepošle na server. Váš prehliadač ju dá
ostatným v hovore v šifrovaných správach chatu. Prehliadač hosťa a
prehliadač každého človeka si ňou navzájom overia, kto je kto, potom sa
dohodnú na kľúči len pre dvojicu, s kľúčmi vytvorenými len pre tento
hovor, a až potom si odovzdajú kľúče hovoru. Kto má odkaz bez časti za
`#`, dostane sa do hovoru, ale nič nevidí ani nepočuje.

## LosOS v hovore \{#losos-in-the-call}

**Pridať LosOS** privedie do hovoru asistenta zariadenia. Počúva, a keď
niekto povie jeho meno, odpovie nahlas, všetkým, vlastným hlasom. Povedzte
„LosOS, čo je v piatok na zozname?“ a odpovie. Asi dvadsať sekúnd po
odpovedi môže ten istý človek pokračovať bez mena. „Stop“ ho umlčí, nech
to povie ktokoľvek, rovnako ako **Prestať hovoriť** v jeho paneli.
**Odstrániť LosOS** ho z hovoru vyberie.

Jeho dlaždica má namiesto obrázka robota a vidí ju každý v hovore, takže
nikoho nepočúva bez toho, aby to vedel.

Hovory sú šifrované od konca ku koncu, takže ich zariadenie nepočuje, a
nepočuje ich ani asistent, ktorý beží na zariadení. LosOS sa preto pripojí
z prehliadača toho, kto ho pridal, ako druhý člen hovoru, s kľúčmi, ktoré
tento človek už má. Počúva a hovorí práve tento prehliadač:

- Whisper, rozpoznávač reči, mení hlas každého na text. Každý človek
  prichádza vo vlastnom prúde, takže sa vie, kto čo povedal.
- Kokoro, syntetizátor reči, hovorí odpoveď.
- Oba bežia ako WebAssembly na počítači toho človeka. Novší počítač
  stíha, na pomalom prídu odpovede neskôr.

Prvýkrát si prehliadač stiahne dva modely, spolu asi 150 MB, zo zariadenia
a ponechá si ich. Z ničoho iného sa nič nesťahuje. Potom už hovor začne
bez sťahovania.

Z prehliadača odchádza otázka, ako text, k asistentovi zariadenia, LosOS
Tag, spolu s posledným úsekom rozhovoru, aby „čo si o tom myslíš?“ malo
o čom byť. Zaznamenaný zvuk zostáva v prehliadači a zmizne hneď, ako sa
prečíta. Keď človek, ktorý LosOS pridal, opustí hovor alebo zavrie stránku,
LosOS odíde s ním.

Odpoveď sa číta nahlas všetkým v hovore, takže asistent predvolene
nepoužíva súbory, poštu ani miestnosti toho, kto ho pridal. **Môže použiť
moje súbory, poštu a miestnosti** v jeho paneli to pre tento hovor zapne.

LosOS zatiaľ rozumie a hovorí po anglicky. Je zapnutý, kým ho nevypnete v
**Nastavenia → Pokročilé → `losos.calls.agent.enable`**; po vypnutí
zariadenie nedodáva rečové modely a tlačidlo zmizne.

## Hovor s vašimi asistentmi \{#calling-your-assistants}

Každý asistent, ktorého vytvoríte v LosOS Tag, je v LosOS Chate človek ako
každý iný a chat s ním je obyčajný chat, s tlačidlom telefónu a kamery v
záhlaví. Stlačte jedno z nich a hovor sa začne s asistentom, ktorý v ňom už
je: netreba hovoriť jeho meno ani stláčať **Pridať LosOS**. Pri kamere
odpovedá s tvárou, ktorá žmurká, pri premýšľaní hľadí nahor a otvára ústa
podľa hlasitosti hlasu. Pri telefóne je to hlasový hovor.

Každý asistent má hlas, ktorý zostáva z hovoru na hovor rovnaký a vyberá sa
podľa mena. Stačí hovoriť. Odpovedá s tým, čo smiete čítať aj vy, lebo
odpoveď počujete len vy, takže **Môže použiť moje súbory, poštu a
miestnosti** je od začiatku zapnuté a v jeho paneli to môžete vypnúť. Čo
vie a čo smie, nastavíte v **Nastavenia → LosOS Tag** pri danom asistentovi.

Funguje ako LosOS v skupinovom hovore. Pripája sa z vášho prehliadača, takže
hovor zostane šifrovaný, a potrebuje tie isté dva rečové modely. Vidí váš
hlas, nie váš obraz: kamera ide ľuďom v hovore a asistentovi nikdy. Keď
zložíte, je preč.

## Čo vidí zariadenie \{#what-the-box-sees}

Zariadenie každý snímok posunie ostatným v hovore bez toho, aby ho otvorilo.
Vie, kto je v ktorom hovore a kedy, aké veľké sú snímky a ako často
prichádzajú. Obraz ani zvuk nikdy nemá, okrem vysielania podcastu, ktoré
mu niekto pošle, aby ho odovzdalo ďalej.

Pridanie LosOS na tom pre zariadenie nič nemení: pripojí sa z prehliadača
a drží rovnaký druh kľúčov, takže zariadenie stále vidí iba zašifrované
snímky. Počuje to, čo sa rozhodne priniesť ten, kto ho pridal.

Prehliadač každého človeka si pre hovor vytvorí vlastný kľúč. Prehliadače
si kľúče odovzdávajú v šifrovaných správach chatu medzi zariadeniami, ktoré
zariadenie LosOS prenáša, ale nevie ich prečítať. Kľúče sa počas hovoru
menia:

- Keď sa niekto pripojí, všetci prejdú na ďalší kľúč a nový človek dostane
  ten. Neotvorí nič, čo sa poslalo, kým neprišiel.
- Keď niekto odíde, všetci si vytvoria nový náhodný kľúč a dajú ho len
  tým, ktorí zostali. Ten, kto odišiel, neotvorí nič, čo sa pošle potom.
- Každých päť minút všetci aj tak prejdú na ďalší kľúč.

Kľúče sú len v pamäti prehliadača a s koncom hovoru zmiznú. Nič sa
neukladá, takže kľúč, ktorý by sa našiel neskôr, z hovoru neotvorí nič.

Jednu hranicu je dobré poznať. Prehliadač posiela svoj kľúč zariadeniam
v hovore tak, ako ich uvádza server chatu na zariadení LosOS.
Niekto, kto by ovládol softvér zariadenia, by mohol uviesť aj vlastné
zariadenie. Overte si navzájom zariadenia v chate (**Zabezpečené správy**,
potom **Overiť**), aby ste vedeli, že zariadenie naozaj patrí tomu druhému.

## Prehliadače \{#browsers}

Aby sa prehliadač mohol pripojiť k hovoru, musí vedieť šifrovať snímky.
Aktuálny Firefox, Chrome, Edge a Safari to vedia. Prehliadač, ktorý to
nevie, neukáže tlačidlá na volanie a nikdy nepošle snímok bez šifrovania.

## Kde hovory fungujú \{#where-calls-work}

Na zariadení vo vlastnej sieti fungujú hovory medzi počítačmi, ktoré sa k
nemu dostanú v tejto sieti. Zariadenie prijíma médiá na porte UDP 7882
(**Nastavenia → Rozšírené → `losos.calls.udpPort`**), ktorý je na jeho
firewalle otvorený, kým sú hovory zapnuté.

Na zariadení, ktoré ide cez edge, idú cez edge aj médiá. Zariadenie
udržiava s edgeom jedno spojenie QUIC a samo neotvára žiadny port, a
ľudia aj hostia posielajú médiá na port na edgei. Snímky sú tam rovnako
zašifrované ako na zariadení.
