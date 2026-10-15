---
title: Zabudnuté heslo
sidebar_position: 5
slug: /troubleshooting/forgot-password
---

# Zabudnuté heslo

<div className="losos-symptom">

**Čo vidíte:** „Toto heslo nebolo prijaté.“ v dialógu odomknutia alebo na
prihlasovacej stránke LosOS cloud.

</div>

## Najprv sa uistite, že ide o heslo \{#first-make-sure-it-is-the-password}

- **LosOS cloud nebeží.** Dialóg odomknutia potom namiesto odmietnutia hesla
  hlási „LosOS cloud práve nebeží, takže heslo sa teraz nedá overiť“.
  Po reštarte počkajte pár minút alebo použite náhradný kľúč, ako je
  opísané nižšie.
- **Zariadenie spred 7. októbra 2026** na správcovských stránkach odmietalo
  každé heslo, aj keď ho LosOS cloud prijal, pretože heslo sa ku kontrole
  vôbec nedostalo. Opravil to pull request #71 a oprava príde s nočnou
  aktualizáciou. Kým nepríde, funguje náhradný kľúč.
- **Príliš veľa pokusov.** Po desiatich nesprávnych pokusoch je počítač
  zablokovaný na čoraz dlhší čas. Počkajte pár minút.

## Máte náhradný správcovský kľúč \{#you-have-the-spare-admin-key}

![Dialóg odomknutia, ktorý žiada náhradný správcovský kľúč.](@site/docs/img/unlock-spare-key.png)

V dialógu odomknutia zvoľte **Použiť namiesto toho náhradný správcovský
kľúč** a vložte 64-znakový kľúč z vytlačeného hárku. Správcovské stránky sa
otvoria.

Kľúčom môžete nastaviť aj nové heslo vlastníka pre LosOS cloud, a to cez tú
istú cestu, ktorú použil sprievodca. Na paneloch na to zatiaľ nie je
tlačidlo, takže z počítača v LAN spustíte jeden príkaz. Nahraďte v ňom kľúč,
adresu a heslo. Nové heslo musí spĺňať štyri pravidlá:

```bash
curl -H "Authorization: Bearer <spare key>" -H "Content-Type: application/json" \
  -d '{"password": "Correct-horse battery staple 1"}' \
  http://<address>/api/set-password
```

Odpoveď uvedie účet, ktorého heslo sa zmenilo. Prihláste sa do LosOS cloud
novým heslom. Od tej chvíle ho prijímajú aj správcovské stránky.

## Nemáte ani jedno \{#you-have-neither}

LosOS cloud heslo overuje a drží jeho jedinú kópiu. Odkaz na obnovenie
neexistuje, pretože zariadenie neposiela poštu, a nie je ani shell, v ktorom
by sa heslo dalo obnoviť. Skôr ako preinštalujete, zistite, či má rolu
správcu iný účet v LosOS cloud. Ten môže heslo vlastníka obnoviť na stránke
Používatelia v LosOS cloud. Inak je jedinou cestou späť **preinštalovanie**,
ktoré vymaže disk, a potom obnovenie súborov z inej kópie, ktorú si
uchovávate, napríklad z priečinka desktopového klienta alebo z nahrávok
z telefónu.

## Ako tomu nabudúce predísť \{#preventing-the-next-time}

Náhradný kľúč si vytlačte, keď ho sprievodca ukáže, a uchovávajte ho mimo
zariadenia. Heslo si uložte do správcu hesiel. Dajte druhej dôveryhodnej
osobe správcovský účet v LosOS cloud.
