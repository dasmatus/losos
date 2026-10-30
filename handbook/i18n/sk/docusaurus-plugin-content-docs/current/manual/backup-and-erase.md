---
title: Záloha a vymazanie
sidebar_position: 8
slug: /manual/backup-and-erase
---

# Záloha a vymazanie

Zariadenie dokáže skopírovať všetko, čo má, do úložného bucketu, ktorý si
prenajmete, a dokáže sa úplne vymazať. Obe funkcie spolupracujú:
zálohujete, vymažete a kód obnovy všetko vráti, na tom istom zariadení
alebo na novom.

## Bucket \{#the-bucket}

**Nastavenia → Záloha** prijme akýkoľvek bucket kompatibilný s S3: Amazon S3
(vrátane Glacier), Backblaze B2, Wasabi, Cloudflare R2 alebo MinIO vo
vašej vlastnej sieti. Od poskytovateľa potrebujete päť vecí:

- **adresu**, napríklad `https://s3.eu-central-1.amazonaws.com`;
- názov **bucketu**;
- **priečinok** v ňom, ak ho chcete;
- **región**, ak ho poskytovateľ vyžaduje;
- **prístupový kľúč** a jeho **tajný kľúč** s právom čítať a zapisovať do
  toho bucketu.

Adresa musí začínať `https://`. Obyčajné `http://` zariadenie prijme iba pre
adresu vo vašej vlastnej sieti, napríklad NAS. Po uložení sa tajný kľúč už
nikdy neukáže. Keď budete neskôr bucket meniť, nechajte pole tajného kľúča
prázdne a uložený kľúč zostane.

![Nastavenia, Záloha: kam idú zálohy, s adresou a kľúčmi bucketu, a kód obnovy za tlačidlom Zobraziť.](@site/docs/img/settings-backup-sk.png)

### Glacier na Amazon S3 \{#glacier-on-amazon-s3}

S adresou Amazon S3 (`https://s3.<region>.amazonaws.com`) pole **Úložisko**
uloží zálohy do jednej z tried úložiska Glacier. Ich uchovávanie stojí menej
a čítanie späť viac:

| Úložisko | Obnova začne | Vhodné pre |
|---|---|---|
| Štandardné | hneď | zálohy, ktoré môžete potrebovať hocikedy |
| Glacier Instant Retrieval | hneď, AWS si účtuje každé čítanie | lacnejšiu kópiu, ktorú čítate zriedka |
| Glacier Flexible Retrieval | po 3 až 5 hodinách | kópiu pre prípad, že zariadenie zmizne |
| Glacier Deep Archive | až po 12 hodinách | najlacnejšiu kópiu, na najhorší deň |

![Nastavenia, Záloha: formulár bucketu s Glacier Deep Archive vybraným v poli Úložisko.](@site/docs/img/settings-backup-glacier-sk.png)

Do Glacier idú iba súbory. Malý index so zoznamom záloh zostáva v triede
Štandardné, takže zariadenie stále dokáže okamžite overiť kód obnovy a
upratať staré zálohy. Pri Flexible Retrieval a Deep Archive obnova najprv
požiada AWS o sprístupnenie zálohy a čaká; LosOS Cloud a LosOS Git medzitým
bežia, kým sa súbory nevrátia. AWS si sprístupnenie účtuje a triedy Glacier
majú aj minimálnu dobu uloženia, takže záloha upratovaná skôr sa za ňu aj
tak zaplatí.

:::warning[Žiadne pravidlo životného cyklu na buckete]
Triedu vyberte tu, nie pravidlom životného cyklu na buckete. Pravidlo pre
celý bucket presunie do Glacier aj index a potom sa žiadna záloha nedá
vypísať ani otvoriť, kým ho AWS nesprístupní.
:::

## Kód obnovy \{#the-recovery-code}

Každú zálohu zariadenie zašifruje ešte predtým, než ho opustí, a kľúčom je
**kód obnovy** zariadenia. Vlastník bucketu vidí iba dáta, ktoré sa bez neho
nedajú prečítať, a bez kódu zálohu neotvorí nikto, ani LosOS. Zobrazí ho
**Zobraziť** v časti *Kľúč k vašim zálohám*. Zapíšte si ho niekam inam ako na
zariadenie.

## Čo obsahuje záloha \{#what-a-backup-holds}

- súbory LosOS Cloud;
- repozitáre LosOS Git;
- databázy oboch;
- schránky LosOS Mail, ak zariadenie má Poštu (poštový server sa na pár
  sekúnd kopírovania zastaví);
- priečinok, ktorý zariadenie zdieľa s mesh, ak je zdieľanie zapnuté (na
  kópiu sa otvorí a potom znova zamkne);
- každé nastavenie a vzhľad domovskej stránky.

Náhľady, ktoré LosOS Cloud dokáže nakresliť znova, v zálohe nie sú.
**Zálohovať teraz** spustí zálohu. Prvá skopíruje všetko, ďalšie posielajú
iba zmeny. Bucket uchováva posledných sedem.

## Obnova \{#restoring}

V časti **Obnova** napíšte kód obnovy zariadenia, ktoré zálohu vytvorilo, a
stlačte **Obnoviť…**. Vráti sa najnovšia záloha v buckete:

- to, čo je teraz na zariadení, nahradí obsah zálohy;
- LosOS Cloud a LosOS Git sa zastavia, kým sa súbory vracajú na miesto, a
  potom sa znova spustia. Zálohu z Glacier Flexible Retrieval alebo Deep
  Archive treba najprv sprístupniť, čo trvá hodiny;
- zariadenie použije nastavenia zo zálohy, čím sa znova zostaví;
- odvtedy si zariadenie ponechá kód, ktorý ste napísali, takže jeho ďalšia
  záloha pôjde na to isté miesto.

Potom sa prihláste heslom, ktoré ste mali, keď záloha vznikla. Nesprávny kód
skončí chybou „the recovery code does not open this backup“ a na zariadení
sa nič nezmení.

:::tip[Obnova na novom zariadení]
Na zariadení, ktoré bolo vymazané alebo nanovo nainštalované, najprv
prejdite sprievodcom, potom v časti Záloha nastavte ten istý bucket a obnovte
zálohu odtiaľ.
:::

## Vymazanie zariadenia \{#erasing-the-box}

**Nastavenia → Reset → Vymazať…** zmaže dáta vlastníka, nielen nastavenia:
súbory, repozitáre, zdieľaný priečinok, každé nastavenie, heslo, náhradný
kľúč a kód obnovy. LosOS zostane nainštalovaný a zariadenie privíta
ďalšieho človeka sprievodcom nastavením.

![Dialóg vymazania: čo zmizne, čo sa mimo zariadenia uvoľní a prepínač Najprv zálohovať.](@site/docs/img/settings-erase-dialog-sk.png)

Zariadenie postupne:

1. najprv zálohuje do bucketu, ak je zapnuté **Najprv zálohovať**. Neúspešná
   záloha vymazanie zastaví a nič sa nezmaže;
2. odpočítava, pätnásť minút, ak `losos.reset.graceMinutes` na paneli
   Rozšírené neurčuje inak.
   **Zrušiť vymazanie** všetko zastaví až do konca odpočítavania a dovtedy sa
   nič nezmenilo;
3. uvoľní to, čo drží mimo seba: svoje vlastné domény, ponuky na trhu a
   miesto na edge;
4. vráti všetky nastavenia, reštartuje sa a na začiatku ďalšieho štartu, skôr
   než beží akákoľvek aplikácia, zmaže dáta.

![Odpočítavanie s tlačidlom Zrušiť vymazanie.](@site/docs/img/settings-erase-countdown-sk.png)

:::danger[Od kroku 3 niet cesty späť]
Od kroku 3 sa vymazanie zastaviť nedá. Krok, ktorý edge nepotvrdí,
zariadenie zaznamená a vo vymazaní pokračuje. Po reštarte Nastavenia →
Reset ukážu, čo posledné vymazanie uvoľnilo, ako počty, a povedia, kedy edge
niektorý krok nepotvrdil.
:::

Zálohy, ktoré už sú v buckete, tam zostanú. Vymazanie sa bucketu nikdy
nedotkne.

## Vymazanie, reset alebo preinštalovanie \{#erase-reset-or-reinstall}

| | Nastavenia | Súbory a repozitáre | Heslo a kódy | Mimo zariadenia |
|---|---|---|---|---|
| **Reset** | späť na predvolené | zostanú | zostanú | zostanú |
| **Vymazanie** | späť na predvolené | zmazané | zmazané | uvoľnené |
| **Preinštalovanie** | späť na predvolené | zmazané | zmazané | ostanú, ako boli |

[Reset a preinštalovanie](./reset-and-reinstall) opisuje ďalšie dve.
