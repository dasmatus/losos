---
title: Prihlásenie a náhradný kľúč
sidebar_position: 2
slug: /manual/sign-in-and-spare-key
---

# Prihlásenie a náhradný kľúč

Každý sa do správcovských stránok prihlasuje v LosOS Login, prihlasovacej
stránke zariadenia, vlastným prihlasovacím menom a heslom. Zariadenie má
okrem toho jeden náhradný správcovský kľúč, ktorý sa raz ukáže počas
nastavenia.

## Vaše prihlasovacie meno a heslo \{#your-sign-in-name-and-password}

Krok sprievodcu s heslom si pýta prihlasovacie meno a heslo. Kto ho dokončí,
stane sa prvým správcom zariadenia. To isté heslo sa stane aj heslom
vlastníka do LosOS cloud, takže jedno heslo otvára oboje.

Prihlasovacie meno sa píše malými písmenami. Začína písmenom, potom
nasledujú písmená, číslice, `.`, `_` alebo `-`, najviac 32 znakov. Nové
heslo potrebuje aspoň 12 znakov, malé písmeno, veľké písmeno, číslicu a
symbol.

Zariadenie si každé heslo drží iba ako hash. Odkaz na obnovenie neexistuje,
pretože zariadenie neposiela poštu. Ak heslo stratíte, pozrite
[Zabudnuté heslo](../troubleshooting/forgot-password).

## LosOS Login \{#losos-login}

LosOS Login je prihlasovacia stránka zariadenia na
`https://<zariadenie>/auth/`, vo farbách LosOS. Keď otvoríte správcovské
stránky neprihlásení, dialóg s nadpisom **Prihláste sa do tohto zariadenia**
má tlačidlo **Prihlásiť sa**. To vás zavedie do LosOS Login a po prihlásení
vráti späť. Kým sa LosOS Login ešte spúšťa, hneď po štarte zariadenia,
správcovské stránky ukazujú „LosOS Login sa spúšťa“ s točiacim sa kolieskom
a niekoľkými zaujímavosťami o LosOS a potom pokračujú samy.

![Dialóg Prihláste sa do tohto zariadenia s tlačidlom Prihlásiť sa a odkazom na použitie náhradného správcovského kľúča.](@site/docs/img/unlock-dialog-sk.png)

LosOS Login funguje iba cez HTTPS. Obyčajnú adresu `http://` na
ňu zariadenie presmeruje na `https://`. Kým nie je nainštalovaný certifikát
zariadenia, prehliadač najprv zobrazí varovanie; pozrite
[Varovanie o certifikáte](../troubleshooting/certificate-warning).

Ak ste si k účtu pridali overovaciu aplikáciu alebo prístupový kľúč, LosOS
Login si ich po hesle vypýta. Pozrite [Váš účet](#your-account).

Prihlásenie platí 12 hodín a skončí skôr, ak dve hodiny nič nerobíte. Ak v
LosOS Login zaškrtnete, aby si vás zapamätal, zostanete na tom počítači
prihlásení mesiac. **Odhlásiť sa** v správcovských stránkach ho ukončí hneď.

:::note[Nesprávne pokusy]
Po niekoľkých nesprávnych heslách za sebou LosOS Login to meno na pár minút
odmietne. Počkajte a skúste to znova.
:::

## LosOS cloud a LosOS Git \{#losos-cloud-and-losos-git}

LosOS cloud aj LosOS Git vás prihlásia cez LosOS Login, na vlastnej sieti
zariadenia aj cez internet. Keď otvoríte niektorú z nich neprihlásení,
ocitnete sa v LosOS Login; prihláste sa tam raz a pustia vás obe aplikácie,
bez druhého hesla. Ani jedna aplikácia neukazuje vlastnú prihlasovaciu
stránku.

Človek, ktorý zariadenie nastavil, príde do oboch aplikácií ako účet
vlastníka, ten, ktorý drží súbory a repozitáre spred tejto zmeny. Ktokoľvek
iný dostane v každej aplikácii vlastný účet, keď ju prvýkrát otvorí,
pomenovaný podľa svojho prihlasovacieho mena. Obe aplikácie ukazujú celé
meno a e-mailovú adresu z LosOS Login a zmenia ich, keď ich zmeníte v
[Vašom účte](#your-account).

Cez internet LosOS Login odpovedá na verejnej adrese zariadenia: na jeho
mene na okrajovom serveri alebo na vlastnej doméne, keď už vedie na
zariadenie. Hneď po štarte, kým sa LosOS Login ešte spúšťa, aplikácie
ukážu stránku, ktorá to oznamuje, a každých pár sekúnd to skúsia znova
samy.

Aplikácie pre počítač a telefón a `git` ďalej používajú heslá aplikácií a
tokeny. Sieťový priečinok LosOS cloud berie v sieti zariadenia vaše meno a
heslo z LosOS Login a cez internet heslo aplikácie; pozrite
[Súbory vo vašom počítači](files-on-your-computer).

**Odhlásiť sa** v LosOS cloud vás odhlási aj z LosOS Login. LosOS Git si
nevedie vlastné prihlásenie: na každej stránke sa pýta LosOS Login, takže
odhlásenie z LosOS Login (v správcovských stránkach alebo v LosOS cloud)
vás odhlási aj z neho. Karta LosOS cloud, ktorá je už otvorená, zostane
prihlásená, kým sa neodhlásite aj z LosOS cloud.

## Kto čo vidí \{#who-sees-what}

Správcovské stránky sú rozdelené na časti: úložisko, záloha, aplikácie, trh
(aj s virtuálnymi strojmi), sieť (vlastné domény), vzhľad, Laboratórium a
prístupové tokeny. Správca môže používať všetky. Iba správca môže použiť
nastavenia, otvoriť Mesh, Hardvér, Zabezpečenie, Používatelia, Rozšírené, História,
O zariadení a Reset, zmazať alebo obnoviť zariadenie a pozrieť si kód na
obnovenie. Kto nie je správca, dostane časti, ktoré mu boli pridelené, alebo
číta všetky časti bez možnosti niečo zmeniť. Prehľad vidí každý prihlásený.
Správcovské stránky skryjú panely, ktoré daný človek nemôže použiť.

Ďalšia časť, **Vlastný účet**, otvára panel **Váš účet**, kde si človek
spravuje vlastné meno, e-mailovú adresu, heslo a druhé faktory. Noví ľudia
ju dostanú automaticky. Bez nej si človek nemôže pridať overovaciu
aplikáciu ani prístupový kľúč a heslo mu mení správca.

## Pridávanie ľudí \{#adding-people}

Prvý človek na zariadení je vždy správca. Ďalších ľudí pridáva správca v
**Nastavenia > Používatelia**. Tento panel je tam vtedy, keď zariadenie
prihlasuje ľudí samo cez LosOS Login. Zariadenie, ktorého prihlasovanie beží
inde, panel Používatelia nemá.

Nového človeka pridáte vyplnením **Pridať človeka**: prihlasovacie meno,
celé meno, heslo podľa rovnakých pravidiel ako to vaše a prístup.

- **Vybrané časti**: vidí a mení iba zapnuté časti, napríklad Úložisko a
  Záloha.
- **Vidí všetko**: vidí každý panel a mení iba zapnuté časti.
- **Správca**: môže meniť všetko vrátane toho, kto sa môže prihlásiť.

![Panel Používatelia: traja ľudia s ich prístupom a pod nimi formulár Pridať človeka.](@site/docs/img/settings-users-sk.png)

Nový človek má **Vlastný účet** zapnutý. Meno a heslo mu odovzdajte sami. Prihlási sa cez LosOS Login. LosOS cloud aj
LosOS Git mu pri prvom otvorení vytvoria vlastný účet.

Kto má overovaciu aplikáciu alebo prístupový kľúč, má štítok
**Dvojstupňové**. Koho zadržiava pravidlo prihlásenia, má štítok
**Zadržaný** a pod menom dôvod.

Každý človek v zozname má tieto tlačidlá:

- **Prístup** mení, čo môže robiť. Platí od jeho ďalšieho kliknutia, bez
  nového prihlásenia.
- **Nové heslo** je pre toho, kto svoje stratil. Staré prestane platiť hneď.
- **Obnoviť dvojstupňové** mu odstráni overovaciu aplikáciu a prístupové
  kľúče, keď stratil telefón alebo kľúč. Prihlasuje sa potom iba heslom,
  kým si nepridá nové. Zobrazí sa iba pri tom, kto nejaké má.
- **Odstrániť fotku** a **Odstrániť banner** stiahnu obrázok, ktorý by
  ostatní nemali vidieť. Zobrazia sa iba pri tom, kto nejaký má.
- **Odstrániť** mu hneď zastaví prihlasovanie do správcovských stránok aj do
  oboch aplikácií a zmaže jeho profil. Jeho účty a súbory v LosOS cloud a
  LosOS Git na zariadení zostanú.

Váš vlastný riadok tlačidlá nemá. Riadok vlastníka má iba **Obnoviť
dvojstupňové**, pre iného správcu. Vlastníka tu nikto nemôže odstrániť,
zmeniť mu prístup ani nastaviť heslo; pri stratenom hesle vlastníka pozrite
[Zabudnuté heslo](../troubleshooting/forgot-password). Zariadenie si vždy
ponechá aspoň jedného správcu.

## Váš účet \{#your-account}

**Nastavenia > Váš účet** je vlastná stránka každého človeka. Má ju každý s
časťou Vlastný účet, aj správcovia, a nikdy nie náhradný kľúč, ktorý účet
nemá.

![Váš účet: vaše údaje, heslo, dvojstupňové prihlásenie a jednorazový kód.](@site/docs/img/settings-account-sk.png)

- **Profil**: fotka, banner, farba, vaše zámená a riadok o vás. Každý, kto
  je prihlásený do zariadenia, ich vidí vedľa vášho mena: v hornej lište, v
  table Používatelia a v aplikáciách zariadenia. Vyberte fotku alebo banner,
  ťahaním a priblížením nastavte časť, ktorú chcete, a zvoľte **Použiť**.
  Zariadenie obrázok znova oreže a uloží ako nový súbor, takže miesto a
  fotoaparát, ktorým bola fotka nasnímaná, zostanú vo vašom zariadení. Vedľa
  profilu je vždy aj vaše prihlasovacie meno, takže sa za vás fotkou nikto
  nevydá.
- **Vaše údaje**: celé meno a e-mailová adresa. LosOS cloud aj LosOS Git
  ukazujú tie isté. Prihlasovacie meno môže zmeniť iba správca.
- **Heslo**: kedy ste ho naposledy zmenili a kedy vyprší, ak má zariadenie
  také pravidlo. Nové heslo sa riadi rovnakými pravidlami ako prvé a otvára
  aj LosOS cloud a LosOS Git.
- **Dvojstupňové prihlásenie**: či máte overovaciu aplikáciu a koľko
  prístupových kľúčov. **Otvoriť LosOS Login** otvorí vlastnú stránku LosOS
  Login na novej karte, kde ich pridáte alebo odstránite. S ktorýmkoľvek z
  nich si ho LosOS Login vypýta po hesle pri každom prihlásení.
- **Jednorazový kód**: LosOS Login si pred zmenou druhých faktorov pýta kód,
  ktorý by inak poslal e-mailom. Zariadenie e-maily neposiela, preto si kód
  vypýtajte na stránke LosOS Login, vráťte sa sem, napíšte svoje heslo a
  zvoľte **Zobraziť kód**.

## Pravidlá prihlásenia \{#sign-in-rules}

Správca nastavuje dve pravidlá pre všetkých v **Nastavenia > Používatelia >
Pravidlá prihlásenia**:

- **Vyžadovať dvojstupňové prihlásenie**: každý si pridá overovaciu
  aplikáciu alebo prístupový kľúč.
- **Heslá vypršia**: heslo platí zadaný počet dní, od 1 do 1200, rátané od
  chvíle, keď si ho človek naposledy nastavil.

Koho pravidlo zachytí, je zadržaný. Stále sa prihlási, ale správcovské
stránky mu ukážu iba **Váš účet** s poznámkou, čo treba urobiť, a LosOS
Login ho nepustí do LosOS cloud ani LosOS Git. Keď si zvolí nové heslo alebo
pridá druhý faktor, zvyšok sa znova otvorí. Po novom druhom faktore si ho
LosOS Login raz vypýta pred ďalšou stránkou. Zadržaný človek, ktorý otvorí
LosOS cloud alebo LosOS Git, uvidí stránku, ktorá ho pošle do jeho účtu; ten sa otvára na vlastnej sieti zariadenia.

Kým začnete vyžadovať dvojstupňové prihlásenie, pridajte si overovaciu
aplikáciu alebo prístupový kľúč do vlastného účtu. Zariadenie pravidlo
odmietne, kým ho nemáte, aby nezadržalo aj vás. Náhradný správcovský kľúč
nie je zadržaný nikdy.

## Náhradný správcovský kľúč \{#the-spare-admin-key}

Zariadenie si pri prvom štarte vytvorilo kľúč so 64 znakmi. Sprievodca vám
ho raz ukázal, keď ste nastavili heslo, s tlačidlami **Kopírovať** a
**Tlačiť**. Kľúč otvorí každý panel ako správca, bez LosOS Login. Funguje
teda aj v deň, keď LosOS Login nefunguje.

V dialógu **Prihláste sa do tohto zariadenia** zvoľte **Použiť namiesto
toho náhradný správcovský kľúč** a vložte kľúč. Karta zostane odomknutá,
kým ju nezatvoríte.

![Dialóg, ktorý si pýta náhradný správcovský kľúč so 64 znakmi.](@site/docs/img/unlock-spare-key-sk.png)

![Nesprávny náhradný kľúč: dialóg hovorí, že kľúč nebol prijatý, a zostane otvorený.](@site/docs/img/unlock-refused-sk.png)

Po desiatich nesprávnych kľúčoch z jedného počítača zariadenie tento
počítač zablokuje. Blokovanie začína na jednej sekunde a zdvojnásobuje sa
až do piatich minút.

:::warning[Náhradný kľúč držte ďalej od zariadenia]
Vytlačený list držte ďalej od zariadenia. Ktokoľvek s kľúčom môže zmeniť
každé nastavenie, čo sa rovná rootovi. Zariadenie kľúč nedokáže ukázať znova
ani ho vymeniť, pretože nemá shell. Ak stratíte heslo *aj* náhradný kľúč,
musíte zariadenie preinštalovať z USB kľúča, čo zmaže disk.
:::

## Zariadenie nastavené skôr \{#a-box-set-up-earlier}

Zariadenie nastavené predtým, než mali ľudia vlastné prihlasovacie mená,
nemá nikoho, za koho by sa dalo prihlásiť. Jeho dialóg hovorí **Vytvorte si
prihlásenie** a pýta si prihlasovacie meno, heslo a náhradný správcovský
kľúč. Kľúč dokazuje, že zariadenie je vaše, a stanete sa jeho prvým
správcom. Odvtedy sa prihlasujte týmto menom a heslom.

## Prístupové kľúče \{#passkeys}

LosOS Login prijíma prístupové kľúče (passkeys). Pridáte si ho na stránke
LosOS Login z [Vášho účtu](#your-account), cez HTTPS, keď je certifikát
zariadenia nainštalovaný. Na prihlasovacej stránke vás potom telefón alebo
notebook prihlási vlastným odomknutím namiesto mena a hesla, do
správcovských stránok, LosOS cloud aj LosOS Git. Prístupový kľúč nie je
druhý náhradný kľúč: otvára iba váš vlastný účet.

## Okno na prevzatie \{#the-claim-window}

Nové zariadenie nemá vlastníka. Prvý prehliadač v LAN, ktorý dokončí
sprievodcu, sa stane vlastníkom a prvým správcom a okno sa zavrie.
Sprievodcu preto spustite čoskoro po inštalácii, v sieti, ktorej dôverujete.
Nič mimo LAN zariadenie prevziať nedokáže a znova ho prevziať umožní iba
preinštalovanie.
