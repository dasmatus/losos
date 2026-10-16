---
title: Varovanie o certifikáte
sidebar_position: 4
slug: /troubleshooting/certificate-warning
---

# Varovanie o certifikáte

<div className="losos-symptom">

**Čo vidíte:** „Vaše pripojenie nie je súkromné“, „Varovanie: potenciálne
bezpečnostné riziko“, `NET::ERR_CERT_AUTHORITY_INVALID` alebo prečiarknutý
zámok na adrese `https://` zariadenia.

</div>

## Prečo \{#why}

Zariadenie si certifikát podpisuje samo, pretože pre meno `.local` ho žiadna
verejná autorita vydať nemôže. Kým prehliadaču nepoviete, aby tomuto
certifikátu dôveroval, každá stránka `https://` varuje. Obyčajné
`http://<address>` nevaruje nikdy a vo vašej vlastnej LAN je v poriadku.
HTTPS je dôležité pre prístupové kľúče (passkeys) a bráni tomu, aby heslo
prechádzalo sieťou nešifrované.

## Riešenie: certifikát nainštalujte raz na každý počítač \{#the-fix-install-the-certificate-once-per-computer}

Prvý krok sprievodcu ukazuje jednoriadkový inštalátor. Po nastavení
zariadenie tie isté súbory poskytuje ďalej, iba do LAN:

<img src={require("@site/docs/img/wizard-step1-trust-sk.png").default} alt="Krok sprievodcu s certifikátom, kde sú jednoriadkové inštalátory a odtlačok." width="520" />

```bash title="macOS a Linux"
curl -fsSL http://<address>/setup/trust.sh | sh
```

```powershell title="Windows (PowerShell)"
irm http://<address>/setup/trust.ps1 | iex
```

Skript pridá jeden certifikát do úložísk prehliadačov vášho používateľa a
vypíše jeho odtlačok. Porovnajte ho s `http://<address>/setup/state.json`.
Potom **reštartujte prehliadač** a otvorte `https://<name>.local`.

:::note[Ručné pridanie certifikátu]
Ak to chcete urobiť ručne, stiahnite `http://<address>/setup/losos-ca.crt`
a pridajte ho ako dôveryhodnú certifikačnú autoritu. Na macOS na to slúži
Prístup ku kľúčenkám (Keychain Access). Vo Windows je to `certmgr.msc` →
Dôveryhodné koreňové certifikačné autority. Firefox má vlastné nastavenia
certifikátov a telefón ich má v nastaveniach zabezpečenia.
:::

## Varuje aj po inštalácii \{#still-warning-after-installing}

- **Otvorili ste `https://<address>`.** Certifikát platí iba pre
  `<name>.local`, takže IP adresa varuje vždy. Použite meno alebo obyčajné
  `http://` s adresou.
- **Firefox na Linuxe** má vlastné úložisko pre každý profil. Skript pokryje
  každý profil, ktorý nájde, no pre profil vytvorený neskôr ho treba
  spustiť znova.
- **Zariadenie bolo preinštalované** alebo sa zmenilo jeho meno, takže má
  nový certifikát. Spustite skript znova. Starý záznam neuškodí.
- **Platnosť certifikátu vypršala.** Platí dva roky od inštalácie.
  Zariadenie si po vypršaní vytvorí nový a skript nainštaluje ten nový.
- **Chrome hlási, že certifikát „nie je platný pre tento názov“** po
  premenovaní: rovnaká príčina, rovnaké riešenie.

## Nemám to jednoducho preklikať? \{#should-i-just-click-through}

Vo vašej vlastnej LAN, pri zariadení, ktoré ste si sami nainštalovali, nie
je jedno preklikanie nebezpečné. Keď však prehliadaču poviete, aby varovanie
ignoroval, bude ignorovať aj skutočného útočníka na tej istej Wi-Fi a
prístupové kľúče (passkeys) neponúkne. Radšej nainštalujte certifikát.
Stačí na to jeden riadok.
