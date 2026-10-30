---
title: Záznamy
sidebar_position: 6.5
slug: /manual/logs
---

# Záznamy

**Nastavenia › Záznamy** ukazujú, čo služby zariadenia zapísali do
systémového denníka: riadiaca služba, LosOS Login, webový server, LosOS
Cloud, LosOS Git, aplikácie, zálohy, aktualizácie a ostatné. Keď niečo
zlyhá, práve tu zariadenie povie prečo, a prečítate si to v prehliadači.
Nikto sa nemusí prihlasovať priamo do zariadenia.

Panel môže otvoriť iba správca alebo ten, kto má náhradný kľúč správcu.
Riadky prezrádzajú veľa o každom, kto zariadenie používa.

## Čítanie riadkov

Panel sa otvorí na riadkoch riadiacej služby od posledného spustenia
zariadenia, najnovšie dole. Každý riadok ukazuje čas, závažnosť, službu,
ktorá ho zapísala, a čo povedala. Keď na službu nabehnete myšou, uvidíte jej
celý názov jednotky a číslo procesu.

Filter nad riadkami vyberá:

- **Služba.** Jedna zo služieb zariadenia, **Všetky služby LosOS** alebo
  **Celý denník**. V zozname sú iba služby, ktoré toto zariadenie spúšťa.
- **Závažnosť.** **Chyby**, **Upozornenia a chyby** a tak ďalej až po
  **Všetko**, kde sú aj riadky ladenia.
- **Od.** Toto spustenie, predchádzajúce spustenie, posledná hodina, deň
  alebo týždeň.
- **Obsahuje.** Iba riadky s týmto textom, bez ohľadu na veľké a malé
  písmená.

**Zobraziť staršie riadky** hore načíta riadky pred prvým na obrazovke.
**Sledovať** pridáva nové riadky, ako ich služby zapisujú, kým nestlačíte
**Prestať sledovať**. **Stiahnuť** uloží až 5000 riadkov aktuálneho filtra
ako textový súbor, napríklad do hlásenia chyby.

Filter je v adrese, takže odkaz ako
`https://<adresa>/settings/logs?unit=lososd&priority=3` otvorí chyby
riadiacej služby.

## Čo je skryté

Skôr než riadok opustí denník, zariadenie skryje všetko, čo vyzerá ako
tajomstvo, a namiesto toho ukáže `[hidden]`: heslá a tokeny zapísané ako
`password=…` alebo `"token": "…"`, údaje v hlavičke `Authorization` alebo
`Cookie`, heslo v adrese, súkromné kľúče, známe formáty tokenov a dlhé
reťazce hex alebo base64, ako kľúče zvyčajne vyzerajú. Náhradný kľúč správcu
je skrytý všade, kde sa objaví. To isté platí pre vyhľadávanie aj pre
stiahnutý súbor, takže ani jedno nenájde tajomstvo podľa jeho znakov.

Radšej skryje viac: aj dlhý kontrolný súčet.

## Keď sa stránky správy nenačítajú

Záznamy prináša malá samostatná čítačka, nie riadiaca služba, takže panel
funguje, aj keď riadiaca služba stojí alebo sa reštartuje. Vtedy ho často
potrebujete najviac.

Ak ostatné stránky čakajú alebo ukazujú chybu, otvorte
`https://<adresa>/basic/settings/logs`. Táto stránka nepotrebuje JavaScript
ani riadiacu službu. Ak LosOS Login beží, prihlásite sa ako zvyčajne;
stránka berie aj náhradný kľúč správcu priamo. Povie, keď riadiaca služba
neodpovedá, a odkáže na jej riadky.

## Čo tam nie je

Inštalátor píše na obrazovku počítača, na ktorom beží, nie do denníka, takže
jeho správy na nainštalovanom zariadení nie sú. Ako inštalácia skončila,
ukáže inštalátor na tej obrazovke.
