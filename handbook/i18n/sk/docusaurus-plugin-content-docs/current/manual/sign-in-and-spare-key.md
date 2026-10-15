---
title: Prihlásenie a náhradný kľúč
sidebar_position: 2
slug: /manual/sign-in-and-spare-key
---

# Prihlásenie a náhradný kľúč

Zariadenie má jedno heslo a jeden náhradný kľúč.

## Heslo \{#the-password}

Heslo nastavené v sprievodcovi je heslo vlastníka do LosOS cloud. Odomyká aj
správcovské stránky. Keď ich otvoríte, pýta si ho dialóg s nadpisom
**Odomknite toto zariadenie** a zariadenie ho nechá overiť LosOS cloud cez
vlastné loopback spojenie. Druhú kópiu si zariadenie nedrží. Heslo zmeníte v
LosOS cloud v časti Nastavenia → Osobné → Zabezpečenie a správcovské stránky
nové heslo prijmú hneď.

![Dialóg Odomknite toto zariadenie, ktorý si pýta heslo vlastníka, s odkazom na použitie náhradného správcovského kľúča.](@site/docs/img/unlock-dialog.png)

Nové heslo potrebuje aspoň 12 znakov, malé písmeno, veľké písmeno, číslicu a
symbol. Heslo nastavené predtým, než toto pravidlo existovalo, prihlási
stále.

![Nesprávne heslo: dialóg hovorí, že heslo nebolo prijaté, a zostane otvorený.](@site/docs/img/unlock-refused.png)

Karta zostane odomknutá, kým ju nezatvoríte. Po desiatich nesprávnych
pokusoch z jedného počítača zariadenie tento počítač zablokuje. Blokovanie
začína na jednej sekunde a zdvojnásobuje sa až do piatich minút.

## Náhradný správcovský kľúč \{#the-spare-admin-key}

Správcovské stránky v skutočnosti odomyká kľúč so 64 znakmi, ktorý si
zariadenie vytvorilo pri prvom štarte. Heslo je spôsob, ako ho získať.
Sprievodca vám ten kľúč raz ukázal, keď ste nastavili heslo, s tlačidlami
**Kopírovať** a **Tlačiť**. Je to náhrada pre jediný prípad, ktorý heslo
nepokryje. Keď LosOS cloud nebeží, heslo nemá kto overiť. Dialóg na
odomknutie to vtedy povie a ponúkne **Použiť namiesto toho náhradný
správcovský kľúč**.

![Dialóg na odomknutie, keď LosOS cloud nebeží: heslo nemá kto overiť, preto dialóg ponúka náhradný správcovský kľúč.](@site/docs/img/unlock-dialog-cloud-down.png)

Vytlačený list držte ďalej od zariadenia. Ktokoľvek s kľúčom môže zmeniť
každé nastavenie, čo sa rovná rootovi. Zariadenie kľúč nedokáže ukázať znova
ani ho vymeniť, pretože nemá shell. Ak stratíte heslo *aj* náhradný kľúč,
musíte zariadenie preinštalovať z USB kľúča, čo zmaže disk.

![Dialóg na odomknutie, ktorý si pýta náhradný správcovský kľúč so 64 znakmi.](@site/docs/img/unlock-spare-key.png)

## Prístupové kľúče \{#passkeys}

Keď je certifikát nainštalovaný a pripájate sa cez HTTPS, sprievodca aj
LosOS cloud ponúknu prístupový kľúč (passkey). Telefón alebo notebook sa
potom prihlasuje vlastným odomknutím namiesto hesla. Prístupový kľúč nie je
druhý náhradný kľúč. LosOS cloud ho overuje rovnako ako heslo.

## Okno na prevzatie \{#the-claim-window}

Nové zariadenie nemá vlastníka. Prvý prehliadač v LAN, ktorý dokončí
sprievodcu, sa stane vlastníkom a okno sa zavrie. Sprievodcu preto spustite
čoskoro po inštalácii, v sieti, ktorej dôverujete. Nič mimo LAN zariadenie
prevziať nedokáže a znova ho prevziať umožní iba preinštalovanie.
