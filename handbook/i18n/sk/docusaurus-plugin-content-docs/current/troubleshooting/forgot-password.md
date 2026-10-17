---
title: Zabudnuté heslo
sidebar_position: 5
slug: /troubleshooting/forgot-password
---

# Zabudnuté heslo

<div className="losos-symptom">

**Čo vidíte:** LosOS Login, prihlasovacia stránka zariadenia na
`https://<zariadenie>/auth/`, neprijme vaše prihlasovacie meno a heslo alebo
si pýta overovaciu aplikáciu či prístupový kľúč, ktorý už nemáte.

</div>

## Najprv sa uistite, že ide o heslo \{#first-make-sure-it-is-the-password}

- **Prihlasovacie meno.** Prihlasovacie mená sa píšu malými písmenami.
  Skontrolujte meno aj heslo.
- **Príliš veľa pokusov.** Po niekoľkých nesprávnych heslách za sebou
  LosOS Login to meno na pár minút odmietne, aj so správnym heslom.
  Počkajte a skúste to znova.
- **Zariadenie nastavené skôr** ešte nemá prihlasovacie mená. Jeho dialóg
  hovorí **Vytvorte si prihlásenie** a potrebuje náhradný správcovský kľúč;
  pozrite
  [Zariadenie nastavené skôr](../manual/sign-in-and-spare-key#a-box-set-up-earlier).

## Zariadenie nastavil niekto iný \{#someone-else-set-up-the-box}

Opýtajte sa správcu. V **Nastavenia > Používatelia** vám tlačidlom **Nové
heslo** nastaví nové heslo; pozrite
[Pridávanie ľudí](../manual/sign-in-and-spare-key#adding-people). Náhradný
kľúč treba iba na heslo vlastníka, ako je opísané nižšie.

## Stratili ste telefón alebo prístupový kľúč \{#you-lost-your-phone-or-passkey}

LosOS Login si po hesle pýta druhý krok a telefón alebo kľúč už nemáte.
Správca vám v **Nastavenia > Používatelia** tlačidlom **Obnoviť
dvojstupňové** odstráni overovaciu aplikáciu aj prístupové kľúče; potom sa
prihlásite iba heslom a nové si pridáte vo **Vašom účte**. Vlastník požiada
iného správcu alebo použije náhradný správcovský kľúč, ktorý obnoví aj
vlastníka. Pozrite [Váš účet](../manual/sign-in-and-spare-key#your-account).

## Máte náhradný správcovský kľúč \{#you-have-the-spare-admin-key}

![Dialóg, ktorý žiada náhradný správcovský kľúč.](@site/docs/img/unlock-spare-key-sk.png)

V dialógu **Prihláste sa do tohto zariadenia** zvoľte **Použiť namiesto
toho náhradný správcovský kľúč** a vložte 64-znakový kľúč z vytlačeného
hárku. Správcovské stránky sa otvoria.

Kľúčom môžete nastaviť aj nové heslo vlastníka, teda toho, kto zariadenie
nastavil, a to cez tú istú cestu, ktorú použil sprievodca. Zmení naraz
prihlasovacie heslo vlastníka aj jeho heslo do LosOS cloud. Na paneloch na
to zatiaľ nie je tlačidlo, takže z počítača v LAN spustíte jeden príkaz.
Nahraďte v ňom kľúč, adresu a heslo. Nové heslo musí spĺňať štyri pravidlá:

```bash
curl -H "Authorization: Bearer <spare key>" -H "Content-Type: application/json" \
  -d '{"password": "Correct-horse battery staple 1"}' \
  http://<address>/api/set-password
```

Odpoveď uvedie účet LosOS cloud, ktorého heslo sa zmenilo. V LosOS Login sa
prihláste svojím prihlasovacím menom a novým heslom; LosOS cloud a LosOS Git
sa pridajú.

## Nemáte ani jedno \{#you-have-neither}

Zariadenie si vaše heslo drží iba ako hash. Odkaz na obnovenie neexistuje,
pretože zariadenie neposiela poštu, a nie je ani shell, v ktorom by sa heslo
dalo obnoviť. Skôr ako preinštalujete, zistite, či je na zariadení správcom
aj niekto iný: ten nastaví nové heslo komukoľvek okrem vlastníka. Pre
vlastníka je jedinou cestou späť **preinštalovanie**, ktoré vymaže disk, a potom obnovenie súborov z inej
kópie, ktorú si uchovávate, napríklad z priečinka desktopového klienta alebo
z nahrávok z telefónu.

## Ako tomu nabudúce predísť \{#preventing-the-next-time}

Náhradný kľúč si vytlačte, keď ho sprievodca ukáže, a uchovávajte ho mimo
zariadenia. Heslo si uložte do správcu hesiel. Dajte druhej dôveryhodnej
osobe správcovský účet v **Nastavenia > Používatelia**.
