---
title: Edge ako výstup
sidebar_position: 6.2
mdx:
  format: md
---

# Edge ako výstup

Edge môže byť pre počítače ľudí, ktorým slúži, aj cestou von do internetu,
tak ako VPN. Využíva to LosOS Desktop: jeho sieťový stroj si vyberie edge
rovnako ako zariadenie ([Federácia edge](/in-depth/edge-federation.md)) a
posiela cezeň prevádzku počítača, takže stránky vidia adresu edge, nie
domácu linku človeka.

Kým ho operátor nezapne, je vypnutý, a aj potom platí len pre nájomcov,
ktorých operátor menuje:

```nix
losos.edge.exit.enable = true;
losos.edge.tenants.alice.exit = true;
```

## Ako sa počítač pripojí

Nájomca zaregistruje kľúč WireGuard cez `POST /exit`, pošle svoje id,
token a kľúč. Edge odpovie:

| Pole | Čo to je |
| --- | --- |
| `address` | jediná adresa počítača na rozhraní `losos-exit`, platí, kým id zostáva |
| `server_public_key`, `endpoint` | kľúč WireGuard edge a kam ho volať (`losos.edge.exit.endpoint`, predvolene verejná doména na UDP 51820) |
| `allowed_ips` | `0.0.0.0/0` a `::/0`: všetko ide do tunela |
| `onion`, `onion_port` | vlastná onion adresa edge pre dvere SOCKS nižšie, ak ju má |
| `socks_username`, `socks_password` | prihlasovacie údaje pre tie dvere; heslo je pri každej registrácii nové |

`POST /exit/leave` počítač zase odhlási. Nájomca bez `exit` dostane 403,
kľúč už zaregistrovaný pre iné id 400 a plná podsieť 503.

Registrátor drží zaregistrované počítače v `/var/lib/losos-exit/peers.json`.
`losos-exit-sync` ich pri každej zmene súboru dá na rozhranie a odoberie
tie, ktoré odišli. Súkromný kľúč WireGuard edge zostáva v
`/var/lib/losos-exit-key`, ktorý číta len root.

## Čo pustí von

Len internet. Edge zahodí všetko, čo počítač z výstupu pošle na súkromné,
loopback, carrier-grade NAT, link-local, dokumentačné, multicast alebo
rezervované adresy, a všetko pre iný počítač na tom istom výstupe. Bez
toho by vlastná sieť edge, metadátová služba cloudu a každý iný nájomca
boli o jeden skok ďalej. IPv6 sa nerozdáva a nič z neho sa nepreposiela.

## Najprv cez Tor

S `losos.edge.exit.onion.enable` tá istá registrácia otvorí aj server
SOCKS5, `exit-socks`, za vlastnou onion službou edge. Je pre počítač,
ktorého prevádzka už ide cez Tor (schizo režim LosOS Desktop): Tor
donesie spojenie k edge a edge ho nadviaže. Stránka vidí edge a edge
vidí len Tor.

Server žiada meno a heslo z registrácie a heslo porovná s odtlačkom
SHA-256; súbor nič iné nedrží. Pripája sa len na verejné adresy a
odmietne meno, ktoré sa preloží na akúkoľvek adresu, ktorá verejná nie
je. Beží ako vlastný používateľ, číta len súbor nájomcov a systemd mu
súkromné rozsahy zakáže druhýkrát.

:::warning[Edge vidí, kam idete]
Výstup skryje domácu adresu pred stránkami, ktoré človek navštívi, nie
pred edge. Operátor edge vidí, kam prevádzka ide, ako každý poskytovateľ
VPN. Používajte edge, ktorému dôverujete, alebo najprv Tor.
:::
