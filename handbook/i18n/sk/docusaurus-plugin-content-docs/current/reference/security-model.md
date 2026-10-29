---
title: Bezpečnostný model
sidebar_position: 4
---

# Bezpečnostný model

Táto stránka je zhrnutie pre vlastníkov. Úplný dokument je
[`docs/security-model.md`](https://github.com/dasmatus/losos/blob/main/docs/security-model.md)
v repozitári. Je verzovaný spolu s kódom a je napísaný tak, aby ste s ním
mohli polemizovať.

## Hranice dôvery \{#trust-boundaries}

1. **Internet → edge.** Do internetu je obrátený iba edge. Zariadenie
   otvára tunel smerom von a zvonku nepočúva na ničom.
2. **Edge → zariadenie.** TLS 1.3 šifruje tunel od konca po koniec, cez QUIC
   alebo, kde je UDP zablokované, cez TCP. Zariadenie si pri prvom spojení
   zapamätá certifikát edge a zmenený certifikát odmietne.
   V tuneli zostávajú spojenia návštevníkov šifrované až po vlastný
   certifikát zariadenia: edge ich smeruje podľa názvu a čítať ich nemôže.
   Vidí však, na aký názov sa návštevník pýtal, kedy, koľko dát prešlo a
   domácu adresu zariadenia. Pozri
   [Edge nevidí do prevádzky](../in-depth/master-proxy.md#the-edge-cannot-read-the-traffic).
3. **LAN → zariadenie.** Zariadenie poskytuje HTTPS s vlastným certifikátom,
   ktorému na každom počítači raz dôverujete. Kým to neurobíte, niekto na tej
   istej Wi-Fi, kto dokáže zachytávať prevádzku, môže zachytiť prvé použitie.
4. **Vaše súbory ↔ kópie mesh siete.** Zariadenie má dva účty s oddelenými
   skupinami a domovskými adresármi v režime 700. Adresár mesh siete je
   šifrovaný vlastným kľúčom, ktorý zariadenie načíta iba počas zdieľania.
   Tento kľúč je na zariadení, ktoré miesto požičiava. Ak chcete mať v mesh
   sieti vlastné dáta, zálohujte ich na úložisko, ktoré ste si tam kúpili:
   vaše zariadenie ich pred odoslaním zašifruje vaším kódom obnovy
   a požičiavatelia aj edge majú len šifrovaný text.
5. **Oficiálny edge ↔ akýkoľvek edge.** Zariadenie si každý edge overí
   samo, skôr ako cez neho obchoduje: pas OpenPGP jeho vlastníka, TPM, ktorý
   ukazuje, že nezmenený naštartoval zverejnený obraz brány LosOS, a
   nastavenia, ktoré posielajú podiel LosOS na adresy, ktoré zariadenie má.
   Obchodovanie potrebuje edge, ktorý prejde, všetko ostatné funguje s
   akýmkoľvek edge. Žiadny kľúč LosOS z edge oficiálny neurobí, takže niet
   súkromného kľúča, ktorý by sa dal ukradnúť. Edge pristihnutý pri
   manipulácii zariadenie, ktoré ho pristihlo, už nikdy nepoužije.

![Hranice zariadenia: jedny vstupné dvere, riadiace API iba na loopbacku, tunel otvorený smerom von k edge a jeden šifrovaný zväzok.](@site/docs/img/box-architecture-sk.svg)

## Čo je admin kľúč \{#what-the-admin-key-is}

Náhradný kľúč so 64 znakmi má silu roota: môže zapísať akékoľvek nastavenie
a zostaviť zariadenie nanovo. Zariadenie ho vytvorí a raz ukáže na stránke
sprievodcu. Potom ho vydá iba prehliadaču, ktorý preukáže heslo vlastníka.
Riadiace API počúva iba na loopbacku zariadenia, za ochranou iba pre LAN.
Desať neúspešných pokusov adresu pribrzdí. Zariadenie vedie auditný log zmien
nastavení, obnovení hesla a resetov.

## Hardening \{#hardening}

Predvolene zapnuté: parametre a sysctl na spevnenie jadra, zoznam zakázaných
modulov jadra, `/tmp` v tmpfs a systemd sandbox pre webový server, mDNS
a riadiaci démon. Štyri ochrany, ktoré môžu niečo pokaziť, sa zapínajú na
paneli Zabezpečenie: AppArmor, spevnený alokátor pamäte, vypnuté SMT,
USBGuard. [Hardening](/in-depth/hardening.md) opisuje každý prepínač a čo
stojí.

![Nastavenia, Zabezpečenie: štyri ďalšie ochrany, ktoré sú predvolene vypnuté, každá s tým, čo stojí.](@site/docs/img/settings-security-sk.png)

## Známe obmedzenia bez prikrášľovania \{#known-limitations-stated-plainly}

- **Inštalačné médium je podpísané, nainštalovaný systém nie.** Firmvér
  so zapísaným certifikátom LosOS overí zavádzač na kľúči, jeden podpísaný
  obraz s jadrom, initrd a príkazovým riadkom. Ten pred pripojením overí
  hash obrazu systému, takže upravený kľúč sa nespustí. Vlastný zavádzač
  zariadenia a jeho nočné jadrá podpísané nie sú. Zariadenie preto beží
  s vypnutým Secure Boot a jeho zavádzací reťazec chráni iba fyzický prístup.
  Podrobnosti sú v [Secure Boot a podpísané médiá](../start/secure-boot).

- **Jeden origin.** Admin stránky, LosOS Cloud a LosOS Git zdieľajú jednu
  adresu. Diera typu cross-site scripting v ktorejkoľvek aplikácii by mohla
  prečítať admin kľúč z karty, ktorá ho má. Obranou je prísna politika
  content-security na admin stránkach, ochrana iba pre LAN a aktualizované
  aplikácie. Projekt zvažoval samostatný názov pre admin stránky a ponechal
  jednu adresu.
- **Krádež celého zariadenia.** TPM vydá diskový kľúč akémukoľvek softvéru
  spustenému na tom počítači, takže zlodej so zariadením ho prečíta.
  Šifrovanie chráni iba disk vybratý samostatne. Na zariadení bez TPM je
  čitateľný aj samotný disk. Zariadenie nainštalované s `--tang` sa otvorí
  iba s kľúčovým serverom vo vlastnej sieti a inde pýta kód na obnovu.
  Zariadenie nainštalované s `--tang-edge` potrebuje odpoveď svojho edgeu a
  majiteľ môže edgeu povedať, aby ukradnutému zariadeniu prestal odpovedať.
  Rozdiel vysvetľuje [TPM a kľúč disku](tpm).
- **Auditný log** sa nerotuje a root ho môže prepísať.
- **Obmedzovanie (throttling) je podľa adresy**, takže útočník v LAN, ktorý
  podvrhuje adresy, dostane zakaždým nový limit.
- **Ručne napísaný widget** beží v každom prehliadači, ktorý zariadenie
  otvorí. Je v sandboxe, oddelený od admin kľúča a API, ale môže zobraziť
  čokoľvek, čo autor napísal, a stiahnuť čokoľvek z internetu.
