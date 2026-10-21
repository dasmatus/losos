---
title: Prístup k zariadeniu
sidebar_position: 4
---

# Prístup k zariadeniu

Všetko na zariadení LosOS je webová stránka na jednej adrese. Túto adresu
môžete napísať dvoma spôsobmi a nie sú rovnako spoľahlivé.

## Cez IP adresu (funguje vždy) \{#by-ip-address-always-works}

Banner na obrazovke zariadenia ukazuje `http://192.168.x.y`, adresu, ktorú
zariadeniu pridelil váš router. Odpovedá na nej každá stránka: admin
stránky na `/`, LosOS Cloud na `/nextcloud`, LosOS Git na `/forgejo/` a táto
príručka na `/handbook/`. Keď sa adresa zmení, zariadenie banner prekreslí.

![Banner pod lososom na obrazovke zariadenia, s adresou, ktorú zadáte do prehliadača, a názvom zariadenia .local.](@site/docs/img/tty1-banner.png)

:::tip[Zariadenie bez obrazovky]
Ak zariadenie nemá pripojenú obrazovku, nájdete ho v zozname zariadení vo
vašom routeri pod názvom, ktorý ste mu dali, alebo ako `losos`, kým ešte nie
je nastavené.
:::

## Cez názov (funguje na väčšine počítačov) \{#by-name-works-on-most-computers}

Zariadenie ohlasuje `<name>.local` cez mDNS. Windows, macOS, iPhony,
telefóny s Androidom a väčšina linuxových desktopov ho preložia, takže
`http://<name>.local` otvorí tie isté stránky. Dva druhy počítačov nie:

- Hostiteľ virtuálneho stroja v sieti NAT, napríklad v libvirt alebo
  VirtualBoxe. Hostiteľ ohlásenia hosťa nikdy nevidí.
- Počítač v inej sieti alebo za routerom, ktorý filtruje multicast.

Panel **O zariadení** na admin stránkach ukazuje adresu, ktorú váš prehliadač
práve používa, a na samostatnom riadku názov `.local`, takže vidíte, ktorú
máte. Odkazy, ktoré rozdávajú aplikácie, napríklad odkazy na zdieľanie
a URL na klonovanie, používajú názov. Názov zostáva rovnaký, aj keď router
pridelí novú adresu.

![Panel O zariadení: adresa, ktorú použil tento prehliadač, názov .local, režim úložiska, stav mesh siete a odkaz na túto príručku.](@site/docs/img/settings-about-sk.png)

## Cez HTTPS \{#over-https}

Zariadenie poskytuje `https://` na porte 443 s vlastným certifikátom pre
`<name>.local`, platným dva roky. Keď tento certifikát nainštalujete, ako
v [prvom spustení, kroku 1](first-run#step-1-trust-this-box),
`https://<name>.local` neukáže žiadne varovanie. Certifikát neobsahuje IP
adresu, takže `https://<address>` varuje vždy. Použite namiesto toho
obyčajné `http://<address>` alebo názov.

## „Find my box“ \{#find-my-box}

Ak nechcete čítať obrazovku a prepisovať adresu, otvorte
[losos-edge.dasmat.us/find](https://losos-edge.dasmat.us/find) v Chrome 142
alebo novšom a stlačte **Find my box**. Chrome sa raz opýta, či stránka môže
hľadať zariadenia vo vašej lokálnej sieti. Povoľte to a stránka nájde
zariadenie podľa názvu a dá vám naň odkaz. Vyhľadávanie beží vo vašom
prehliadači a nič o vašej sieti neopustí váš počítač. Firefox a Safari
takéto povolenie nemajú, takže tam stránka namiesto toho ukáže, ako adresu
zadať ručne.

<img src={require("@site/docs/img/find-my-box.png").default} alt="Stránka Find my box po tom, čo Chrome dostal povolenie hľadať v lokálnej sieti: zariadenie nájdené podľa názvu, s odkazom na jeho otvorenie." width="440" />

## Mimo vašej siete \{#from-outside-your-network}

Predvolene nikde. Zariadenie je dostupné iba z lokálnej siete. Nasadenie
s [oficiálnym edge](../types/official-edge) mu dá verejnú adresu cez tunel.
Aj vtedy zostávajú admin stránky iba pre LAN a cez tunel idú von iba LosOS
cloud a LosOS Git.

![Nastavenia, Sieť: názov, Šifrovať pripojenie a Dostupné aj mimo domova, prepínač, ktorý otvorí tunel k edge.](@site/docs/img/settings-network-sk.png)
