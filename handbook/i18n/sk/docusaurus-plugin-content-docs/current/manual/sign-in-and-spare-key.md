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

Prihlásenie platí 12 hodín a skončí skôr, ak dve hodiny nič nerobíte. Ak v
LosOS Login zaškrtnete, aby si vás zapamätal, zostanete na tom počítači
prihlásení mesiac. **Odhlásiť sa** v správcovských stránkach ho ukončí hneď.

:::note[Nesprávne pokusy]
Po niekoľkých nesprávnych heslách za sebou LosOS Login to meno na pár minút
odmietne. Počkajte a skúste to znova.
:::

## LosOS cloud a LosOS Git \{#losos-cloud-and-losos-git}

Na vlastnej sieti zariadenia vás LosOS cloud aj LosOS Git prihlásia cez
LosOS Login. Keď otvoríte niektorú z nich neprihlásení, ocitnete sa v LosOS
Login; prihláste sa tam raz a pustia vás obe aplikácie, bez druhého hesla.

Človek, ktorý zariadenie nastavil, príde do oboch aplikácií ako účet
vlastníka, ten, ktorý drží súbory a repozitáre spred tejto zmeny. Ktokoľvek
iný dostane v každej aplikácii vlastný účet, keď ju prvýkrát otvorí,
pomenovaný podľa svojho prihlasovacieho mena.

Aplikácie si nechávajú vlastnú prihlasovaciu stránku tam, kde LosOS Login
pomôcť nemôže:

- cez internet (verejnú adresu zariadenia), kde LosOS Login neodpovedá;
- kým sa LosOS Login po štarte zariadenia ešte spúšťa;
- v LosOS cloud na `/nextcloud/login?direct=1`, ak o ňu požiadate.

Tam sa vlastník prihlási svojím heslom ako doteraz. Aplikácie pre počítač a
telefón a `git` ďalej používajú heslá aplikácií a tokeny. Sieťový priečinok
LosOS cloud berie v sieti zariadenia vaše meno a heslo z LosOS Login a cez
internet heslo aplikácie; pozrite
[Súbory vo vašom počítači](files-on-your-computer).

**Odhlásiť sa** v LosOS cloud vás odhlási aj z LosOS Login. **Odhlásiť sa**
v LosOS Git nie: kým ste prihlásení v LosOS Login, LosOS Git vás hneď
prihlási znova. Z oboch odídete odhlásením z LosOS Login (v správcovských
stránkach alebo v LosOS cloud). Karta aplikácie, ktorá je už otvorená, zostane
prihlásená, kým sa neodhlásite aj z tej aplikácie.

## Kto čo vidí \{#who-sees-what}

Správcovské stránky sú rozdelené na časti: úložisko, záloha, aplikácie, trh
(aj s virtuálnymi strojmi), sieť (vlastné domény), vzhľad, Laboratórium a
prístupové tokeny. Správca môže používať všetky. Iba správca môže použiť
nastavenia, otvoriť Mesh, Hardvér, Zabezpečenie, Rozšírené, História,
O zariadení a Reset, zmazať alebo obnoviť zariadenie a pozrieť si kód na
obnovenie. Kto nie je správca, dostane časti, ktoré mu boli pridelené, alebo
číta všetky časti bez možnosti niečo zmeniť. Prehľad vidí každý prihlásený.
Správcovské stránky skryjú panely, ktoré daný človek nemôže použiť.

Prvý človek na zariadení je vždy správca. Stránku na pridávanie ďalších
ľudí správcovské stránky zatiaľ nemajú.

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

Keď je certifikát nainštalovaný a pripájate sa cez HTTPS, LosOS cloud
ponúkne prístupový kľúč (passkey). Telefón alebo notebook sa potom do LosOS
cloud prihlasuje vlastným odomknutím namiesto hesla. LosOS Login prístupové
kľúče neprijíma a vždy si pýta meno a heslo. Prístupový kľúč nie je druhý
náhradný kľúč.

## Okno na prevzatie \{#the-claim-window}

Nové zariadenie nemá vlastníka. Prvý prehliadač v LAN, ktorý dokončí
sprievodcu, sa stane vlastníkom a prvým správcom a okno sa zavrie.
Sprievodcu preto spustite čoskoro po inštalácii, v sieti, ktorej dôverujete.
Nič mimo LAN zariadenie prevziať nedokáže a znova ho prevziať umožní iba
preinštalovanie.
