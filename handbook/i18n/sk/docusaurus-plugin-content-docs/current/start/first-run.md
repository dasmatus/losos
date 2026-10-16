---
title: Prvé spustenie
sidebar_position: 3
---

# Prvé spustenie

Keď prvýkrát otvoríte adresu zariadenia, admin stránky ukážu sprievodcu
s tromi krokmi. Prejdite ho čoskoro po inštalácii, z počítača vo vašej
vlastnej sieti. Kto sprievodcu dokončí prvý, ten zariadenie vlastní.

## Krok 1: Dôverujte tomuto zariadeniu \{#step-1-trust-this-box}

Zariadenie poskytuje HTTPS s certifikátom, ktorý si vytvorilo samo. Váš
prehliadač tento certifikát nikdy nevidel, takže mu zatiaľ nedôveruje.
Všetko po tomto kroku by malo ísť cez šifrované spojenie, aj heslo, ktoré
idete nastaviť. Preto sprievodca chce, aby ste certifikát nainštalovali
ako prvý.

<img src={require("@site/docs/img/wizard-step1-trust-sk.png").default} alt="Krok 1 sprievodcu: jednoriadkové inštalátory pre macOS, Linux a Windows, stiahnutie certifikátu a odtlačok na porovnanie." width="520" />

Najrýchlejšie je to jedným riadkom v termináli, ukázaným pre počítač, na
ktorom práve ste:

```bash title="macOS a Linux"
curl -fsSL http://<address>/setup/trust.sh | sh
```

```powershell title="Windows (PowerShell)"
irm http://<address>/setup/trust.ps1 | iex
```

Čo tento riadok robí a čo nie:

- Pridá **jeden certifikát** do úložísk, ktoré čítajú vaše prehliadače, iba
  pre vášho používateľa: prihlasovaciu kľúčenku na macOS, úložisko Trusted
  Root používateľa na Windows a úložiská NSS, ktoré na Linuxe používajú Chrome
  a Firefox. Neinštaluje žiadny softvér a nikdy nežiada administrátorské
  práva. Vrátite ho tak, že tú jednu položku zmažete.
- Prichádza **zo zariadenia, vo vašej vlastnej sieti**, nie z internetu. Je
  krátky a odkaz si môžete najprv otvoriť v karte a prečítať.
- Vypíše **odtlačok** certifikátu, ktorý porovnáte s tým na stránke
  sprievodcu.
- macOS sa raz opýta na heslo vášho účtu. Pýta sa ho kľúčenka, nie skript.
  Windows ukáže vlastné okno „nainštalovať tento koreňový certifikát?“.

Telefóny terminál nemajú. Na telefóne si z tej istej stránky stiahnite
`losos-ca.crt` a v nastaveniach telefónu ho pridajte ako dôveryhodný
certifikát. Rovnako to môžete urobiť ručne aj na počítači.

Potom reštartujte prehliadač a znova otvorte stránku na `https://<name>.local`
alebo `https://<address>`. Tento krok môžete aj preskočiť a vrátiť sa k nemu
neskôr. Ak sa niečo pokazí, pozrite si stránku
[Varovanie o certifikáte](../troubleshooting/certificate-warning).

## Krok 2: Vyberte si spôsob prihlásenia \{#step-2-choose-how-you-sign-in}

Nastavte **heslo vlastníka**. Musí mať aspoň 12 znakov, malé písmeno, veľké
písmeno, číslicu a symbol, napríklad `-` alebo `!`. Sprievodca počas
písania odškrtáva každé pravidlo. Toto jedno heslo otvára LosOS cloud,
LosOS Git aj admin stránky.

<img src={require("@site/docs/img/wizard-step2-password-sk.png").default} alt="Krok 2 so zadaným heslom: každé zo štyroch pravidiel je odškrtnuté, keď je splnené." width="520" />

Krok čaká, kým LosOS cloud dokončí svoje vlastné prvé spustenie, čo na
novom zariadení trvá niekoľko minút. Medzitým stránka ukazuje „Čaká sa“
a dôvod. Pokračuje sama.

<img src={require("@site/docs/img/wizard-step2-waiting-sk.png").default} alt="Krok 2, kým sa LosOS cloud ešte inštaluje: stránka hovorí, na čo čaká, a pokračuje sama." width="520" />

Keď je heslo nastavené, stránka raz ukáže **náhradný správcovský kľúč**:
64 znakov s tlačidlami **Kopírovať kľúč** a **Tlačiť**. Vytlačte si ho alebo
si ho zapíšte a uchovávajte ho mimo zariadenia. Pokrýva jedinú situáciu,
ktorú heslo nezvládne: keď LosOS cloud nebeží. Stránka ho už znova ukázať
nevie. Pozrite si
[Prihlásenie a náhradný kľúč](../manual/sign-in-and-spare-key).

<img src={require("@site/docs/img/wizard-step2-spare-key-sk.png").default} alt="Krok 2 po nastavení hesla: prihlasovacie meno a náhradný správcovský kľúč, ukázaný raz, s tlačidlami Kopírovať a Tlačiť." width="520" />

Na spojení cez HTTPS krok ponúkne aj **prístupový kľúč** (passkey), aby sa
telefón alebo notebook mohli prihlásiť bez písania hesla.

## Krok 3: Prihlásiť sa \{#step-3-sign-in}

Sprievodca otvorí LosOS cloud priamo na stránke a vy sa prihlásite heslom,
ktoré ste práve nastavili. Potom vás sprievodca prenesie na prehľad admin
stránok. Odtiaľto pokračujte stránkou [Admin stránky](../manual/admin-pages).

Pri prvom otvorení sa nad prehľadom spustí krátka prehliadka. Má šesť
krokov a každý ukazuje malý pohyblivý obrázok stránky s položkou bočného
panela, kde daná funkcia je. Kroky prechádzajú aplikácie, pridávanie
ďalších, úložisko, požičiavanie zariadenia v noci, nočnú aktualizáciu a to,
kde prehliadku znova nájdete. **Preskočiť prehliadku** ju zavrie a v tom
prehliadači sa už sama neotvorí. **Spustiť prehliadku** v časti Nastavenia,
O zariadení ju vráti.

![Prvý krok prehliadky nad prehľadom: obrázok stránky s otvorenou aplikáciou Súbory, „1 z 6“ a tlačidlá Preskočiť prehliadku a Ďalej.](@site/docs/img/tour-sk.png)

![Krok 3: Ste prihlásení. To bol posledný krok.](@site/docs/img/wizard-step3-sign-in-sk.png)
