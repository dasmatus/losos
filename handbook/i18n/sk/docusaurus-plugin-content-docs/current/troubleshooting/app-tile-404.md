---
title: Dlaždica aplikácie vráti 404 alebo „nedôveryhodnú doménu“
sidebar_position: 8
slug: /troubleshooting/app-tile-404
---

# Dlaždica aplikácie vráti 404 alebo „nedôveryhodnú doménu“

<div className="losos-symptom">

**Čo vidíte:** kliknutie na **Úlohy**, **Kalendár** alebo inú dlaždicu na
Prehľade otvorí holú stránku „Not Found“, alebo LosOS cloud ukáže „Prístup
cez nedôveryhodnú doménu“.

</div>

![Panel Aplikácie s LosOS cloud a LosOS Git; dlaždice aplikácií na Prehľade otvárajú tie isté aplikácie.](@site/docs/img/apps-sk.png)

## „Not Found“ z dlaždice aplikácie \{#not-found-from-an-app-tile}

Príčiny sú dve. Pull requesty #65 a #66 opravili obe 6. októbra 2026 a nočná
aktualizácia opravy prinesie:

- Dlaždice odkazovali na krátky tvar `/nextcloud/apps/<app>/`, na ktorý
  webový server LosOS cloud v predvolenom
  [režime aplikácií](../types/app-modes) neodpovedal. Dlaždice teraz vedú
  cez `/nextcloud/index.php/apps/<app>/`, na ktorý odpovedá každá
  konfigurácia.
- Samotná aplikácia v LosOS cloud nie je nainštalovaná alebo je vypnutá.
  Otvorte LosOS cloud → Aplikácie a zapnite ju.

:::tip[Kým aktualizácia nepríde]
Kým aktualizácia nepríde, otvorte LosOS cloud z dlaždice **Súbory** alebo na
`/nextcloud` a použite jeho vlastnú ponuku aplikácií. Jej odkazy fungujú.
:::

## „Prístup cez nedôveryhodnú doménu“ \{#access-through-untrusted-domain}

LosOS cloud odpovedá iba na mená, ktorým dôveruje. Pri každej požiadavke mu
zariadenie oznámi, na ktorú adresu požiadavka prišla, takže funguje
`<name>.local` aj IP adresa. Zástupný znak mu zariadenie nikdy neodovzdá.
Túto stránku uvidíte, keď:

- ste sa k zariadeniu dostali menom, ktoré nie je jeho vlastné: aliasom
  z routera, záznamom v súbore hosts pod iným menom alebo doménou cez edge,
  o ktorej zariadenie nevie. Použite `<name>.local` alebo adresu.
- ste pred chvíľou zmenili meno zariadenia a LosOS cloud sa ešte
  nereštartoval. Počkajte, kým sa prestavba dokončí.
- ste sa IP adresou dostali k zariadeniu spred 6. októbra 2026. Dôveru pre
  každú požiadavku zvlášť pridal pull request #65. Do aktualizácie
  používajte `<name>.local`.
