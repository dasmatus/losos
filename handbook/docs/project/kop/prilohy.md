---
title: Prílohy
sidebar_position: 11
---

# Prílohy

## Príloha A: Slovník pojmov

| Pojem                 | Význam                                                                                                  |
| --------------------- | ------------------------------------------------------------------------------------------------------- |
| box                   | počítač s nainštalovaným LosOS (appliance)                                                               |
| edge                  | server, ku ktorému box drží tunel: verejná adresa, mesh, trh                                            |
| mesh                  | sieť boxov jedného edge, ktoré si zdieľajú voľný disk a procesor                                        |
| trh                   | voliteľný predaj a nákup zdieľanej kapacity cez Stripe Connect, 4 % platforme                          |
| LosOS cloud, LosOS Git| mená, pod ktorými majiteľ vidí Nextcloud a Forgejo                                                       |
| administračné stránky | webové rozhranie boxu na `/`, len z LAN                                                                  |
| sprievodca            | trojkrokové prvé spustenie: certifikát, heslo, prihlásenie                                              |
| náhradný kľúč         | 64-znakový administračný kľúč, zobrazený raz; mocný ako root                                             |
| claim                 | zabratie čerstvého boxu prvým, kto dokončí sprievodcu                                                   |
| impermanence          | model, v ktorom koreň je tmpfs a prežije len zoznam adresárov na `/persist`                              |
| `/persist`            | šifrovaný zväzok (LVM → LUKS2 → ext4) so všetkým trvalým stavom                                          |
| TPM 2.0               | čip, do ktorého inštalátor zapečatí kľúč disku                                                          |
| keyfile režim         | odomykanie kľúčom v initrd na nešifrovanom boot oddiele (bez TPM)                                       |
| flake                 | deklaratívny popis celého systému v Nixe                                                                 |
| generácia             | jedna zostavená verzia systému; posledných päť ostáva v boot menu                                        |
| prestavba (rebuild)   | zostavenie a prepnutie systému z popisu; cesta pre Použiť, nočnú aktualizáciu aj reset                   |
| `overrides.nix`       | súbor s voľbami majiteľa, ktorý zapisuje démon                                                          |
| `install-target.nix`  | súbor s diskami, firmvérom a režimom odomykania boxu, ktorý zapisuje inštalátor                          |
| lososd, losos-ctl     | riadiaci démon a jeho fasáda v príkazovom riadku                                                         |
| k3s, rke2             | dva Kubernetes runtime: vlastný cluster boxu a agent meshu                                               |
| Longhorn              | replikované blokové úložisko meshu                                                                       |
| fscrypt               | šifrovanie adresára `shared` vlastným kľúčom, načítaným len počas zdieľania                             |
| okno                  | hodiny, v ktorých box požičiava procesor, v časovom pásme boxu                                           |
| KSPP                  | Kernel Self Protection Project, zdroj odporúčaní pre hardening jadra                                     |
| CSP                   | Content-Security-Policy, hlavička obmedzujúca, čo smie stránka načítať a spustiť                         |
| widget                | dlaždica na nástenke Prehľadu; ručne písané bežia v izolovanom rámci                                     |

## Príloha B: Testy a kontroly

**Testy vo virtuálnych strojoch** (`tests/*.nix`, `nix build
.#checks.x86_64-linux.<názov>`): `install`, `tpm`, `impermanence`,
`hardening`, `resize`, `admin-vm` (`losos-admin-daemon`), `front-vhost`,
`setup`, `tls`, `keyring`, `console`, `cluster-vm`, `edge-vm`,
`market-vm`, `nextcloud-httpd`, `admin-ui`, `design-system`.

**Pri vyhodnotení**: `tests/invariants.nix` (`losos-invariants`).

**Jednotkové**: `cargo test` v `backend/`, `backend-registrar/`,
`edge-vercel/`; `cargo clippy --all-targets -- -D warnings`; `cargo fmt
--check`.

**V prehliadači**: `admin-ui/app/tests/{wizard,app,toasts,look}.browser.mjs`.

**CI** (`.github/workflows/ci.yml`): pins, clippy, test, eval, losos-ctl,
registrar-and-ui (vrátane príručky), images, publish-cache, iso (zostavenie
a boot pod OVMF a SeaBIOS), release-media pri značke `v*`.
`handbook.yml`: typecheck, check (odkazy, tokeny), build, deploy na Pages,
PDF tohto dokumentu.

**Dev prostredie**: `devenv shell`, `devenv test` (všetko okrem VM testov
a ISO).

## Príloha C: Štruktúra repozitára

```text
flake.nix, flake.lock      systémy iso a install, modul edge, balíky, checks
modules/                   NixOS moduly; voľby v options.nix, predvolené v defaults.nix
  boot, disko, impermanence, hardening, configuration, services, workloads,
  cluster, containers (nginx), daemon, setup, tls, updates, console, edge, proxy, …
backend/                   lososd + losos-ctl (Rust): installer, grow, setup, signin, look, market, supervisor
backend-registrar/         registrátor edge (Rust): server, join, window, market, stripe_gate, identity
edge-vercel/               riadiaca rovina edge ako funkcia Vercel (demo)
admin-ui/app/              administračné stránky (React, Vite, Tailwind, shadcn/ui)
admin-ui/themes/           tokens.css, témy Nextcloud a Forgejo, logá
handbook/                  príručka majiteľa (Docusaurus) a tento dokument (docs/project/kop)
tests/                     VM testy, invarianty, iso-boot.py
wiki/                      zdroj vývojárskej wiki
docs/                      bezpečnostný model, baseline, návrhové špecifikácie
flake/                     balíky, obrazy OCI, obrazy diskov, fast-build (mold, ccache, sccache)
.github/                   CI, akcie, šablóna pull requestu, snímky k PR
```

## Príloha D: Zdroje

- Repozitár projektu: https://github.com/dasmatus/losos (kód, wiki, vydania)
- Príručka majiteľa: https://losos.dasmat.us
- NixOS manual a nixpkgs: https://nixos.org/manual/nixos/unstable/
- impermanence: https://github.com/nix-community/impermanence
- disko: https://github.com/nix-community/disko
- Kernel Self Protection Project, odporúčané nastavenia:
  https://kspp.github.io/Recommended_Settings
- systemd-cryptenroll(1), crypttab(5), systemd-cryptsetup
- Linux kernel documentation: fscrypt, LUKS2 (cryptsetup)
- k3s: https://docs.k3s.io; rke2: https://docs.rke2.io
- Longhorn: https://longhorn.io/docs
- Nextcloud Server Administration Manual: https://docs.nextcloud.com
- Forgejo documentation: https://forgejo.org/docs
- Traefik: https://doc.traefik.io/traefik; rathole: https://github.com/rapiz1/rathole
- Stripe Connect, destination charges: https://docs.stripe.com/connect
- Docusaurus: https://docusaurus.io; pandoc: https://pandoc.org; Typst: https://typst.app/docs
- Stredná priemyselná škola elektrotechnická, Hálova 16: Maturitná skúška,
  informačný materiál školy (predbežný harmonogram 2026/2027)
