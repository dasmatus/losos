---
title: Prihlasovacie údaje
sidebar_position: 4.6
slug: /reference/credentials
---

# Prihlasovacie údaje

Zariadenie si uchováva niekoľko tajomstiev pre seba: náhradný kľúč k
stránkam správy, kľúče LosOS Login, tajomstvo, ktorým sa otvára LosOS Mail,
a kľúče vášho bucketu na zálohy. Žiadne z nich nie je obyčajný súbor. Každé
je prihlasovací údaj, zašifrovaný a zapečatený pre toto zariadenie, a systém
ho odovzdá iba službám, ktoré ho čítajú.

**Nastavenia → Prihlasovacie údaje** ich vypíšu. Ukážu, na čo každý slúži,
čo ho číta, pre čo je zapečatený a kedy sa naposledy zmenil. Hodnotu nikdy
neukážu a nič na stránke ju nemôže prezradiť.

![Nastavenia, Prihlasovacie údaje: k čomu sú tajomstvá zapečatené, potom každé s tým, na čo slúži, ako vzniklo, kedy sa zmenilo a čo ho číta, a Vytvoriť nový pri tých, ktoré sa dajú vymeniť.](@site/docs/img/settings-credentials-sk.png)

## Kde sú uložené

Každý prihlasovací údaj je jeden súbor v `/etc/credstore.encrypted`,
zašifrovaný pomocou `systemd-creds`. Adresár sa zachováva cez reštarty na
zašifrovanom zväzku (pozrite [Čo prežije reštart](what-survives-a-reboot.md))
a vymazanie zariadenia ho odstráni.

Súbor je zapečatený jedným z dvoch kľúčov:

- **TPM**, ak ho zariadenie má. Kľúč drží čip, takže kópia súboru je inde
  nepoužiteľná, rovnako ako kópia celého disku. Pečať nie je viazaná na
  merania firmvéru, z dôvodu, ktorý pre kľúč disku uvádza
  [stránka o TPM](tpm.md): zariadenie sa v noci aktualizuje samo a nikto pri
  ňom nie je, aby ho ručne odomkol.
- **Vlastný kľúč zariadenia**, `/var/lib/systemd/credential.secret`, ktorý
  môže čítať iba root, ak zariadenie TPM nemá. Kópia jedného súboru je sama
  osebe stále nepoužiteľná, ale kópia celého zväzku si kľúč odnesie so sebou.

Panel ukáže, ktorý z nich toto zariadenie používa.

Každý súbor nesie aj svoj vlastný názov, ktorý sa pri otváraní kontroluje,
takže súbor skopírovaný na miesto iného prihlasovacieho údaja sa odmietne,
namiesto aby sa použil.

## Ako ho služba dostane

Keď sa služba spustí, systemd otvorí prihlasovacie údaje, ktoré smie
dostať, a vloží ich do adresára len pre túto službu, ktorý je iba v pamäti.
Služba sa dozvie, kde ich nájde, a nič viac. Keď sa zastaví, adresár zmizne.
Žiadna iná služba ani nikto prihlásený do zariadenia ho nemôže čítať.

## Nová hodnota

Niektoré prihlasovacie údaje sa dajú v paneli vytvoriť nanovo: **Vytvoriť
nový** sa najprv opýta, potom zariadenie vytvorí náhodnú hodnotu, zapečatí ju
a reštartuje to, čo ju číta. Nový kľúč relácií pre LosOS Login napríklad
odhlási všetkých.

Ostatné nie, a panel povie prečo:

- Náhradný kľúč: zapísali ste si ho pri nastavení zariadenia a nový by vám
  nechal kľúč, ktorý už nefunguje.
- Kľúč úložiska LosOS Login: šifruje druhé faktory všetkých a nový by ich
  stratil.
- Kľúče, ktoré LosOS Login dáva pomocníkovi pre nástroje AI: nový by odhlásil
  každého pomocníka.
- Tajomstvo LosOS Mail: stránky správy ho držia od chvíle, keď sa spustia.
  Tajomstvo, ktorým sa stránky správy pýtajú LosOS Login, sa vytvorí znova
  samo vždy, keď je to potrebné.
- Kľúče bucketu: tie zadávate v paneli Záloha, na ktorý panel Prihlasovacie
  údaje odkazuje.

V termináli ukáže `losos credentials list` ten istý zoznam a
`losos credentials rotate <name>` vytvorí jeden nanovo.

## Zariadenia nastavené predtým

Zariadenie nastavené pred úložiskom prihlasovacích údajov uchovávalo tieto
tajomstvá ako obyčajné súbory v `/var/secrets`. Pri prvom štarte s úložiskom
zapečatí každú hodnotu tak, ako je, aby sa nič, čo ju už používa,
nepokazilo, potom obyčajný súbor prepíše a vymaže. Panel označí každý údaj,
ktorý takto presunul.

## Keď sa prihlasovací údaj nedá otvoriť

Ak bol TPM vymazaný alebo sa vlastný kľúč zariadenia stratil, zapečatený
súbor sa už neotvorí. Zariadenie súbor odloží bokom, vytvorí novú hodnotu a
panel ju označí ako pri štarte vytvorenú znova. Znova vytvorený náhradný
kľúč sa nikde neukáže, preto sa namiesto neho prihláste svojím heslom.

## Pred čím nechráni

Úložisko chráni to, čo je na disku: záložnú kópiu, disk vybratý zo
zariadenia, chybu, ktorou služba prečíta súbory, ktoré by nemala. Nechráni
pred rootom na bežiacom zariadení, ktorý môže systém požiadať, aby otvoril
ktorýkoľvek prihlasovací údaj. Preto zariadenie nemá shell ani SSH.
