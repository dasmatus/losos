[English](CI-and-Releases) · **Slovenčina** · [Deutsch](CI-and-Releases-de)

# CI a vydania

CI je `.github/workflows/ci.yml`. Spúšťa tie isté príkazy ako skripty devenv,
vypísané priamo, pretože spúšťanie jobov cez `devenv shell` bolo príliš
pomalé. Udržiavajte ich zladené ručne.

`.github/actions/setup-nix` nainštaluje Nix a zapíše substituery do
`/etc/nix/nix.conf` ešte pred štartom démona. Nix ignoruje `NIX_CONFIG` od
nedôveryhodných klientov a zostavenie, ktoré minie cache, sa skompiluje zo
zdrojového kódu bez akéhokoľvek upozornenia.

## Joby

- clippy a rustfmt (rustfmt sa kontroluje iba pri pull requestoch)
- testy Rustu pre oba crate
- vyhodnotenie flaku
- zostavenie balíkov
- inštalačné ISO, zostavené pri každom pushi a potom nabootované pod OVMF a
  SeaBIOS cez `tests/iso-boot.py`. Boot prejde, keď inštalátor pošle
  DNS dotaz na `github.com`. Pri vypršaní času job nahrá snímku obrazovky.
- obraz Nextcloudu, iba raz týždenne a na požiadanie
- `losos-registrar` pre vývojársky počítač, statické zostavenie s musl
  pushnuté na GHCR pri každom pushi do main a pri každom tagu a servírované
  proxy (nižšie)

## Vydania

Najprv nastavte `flake/version.nix` na nový tag v main, potom pushnite tag
`v*`, alebo vo webovom rozhraní pripravte koncept vydania s novým tagom `v*`.
Bootovacie menu, bootovacie obrázky, os-release a bannery na konzole ukazujú
hodnotu z tohto súboru a nainštalované zariadenie sa zostavuje z commitnutého
stromu. Job vydania:

1. zapíše tag do `flake/version.nix` (s varovaním, ak bola commitnutá
   hodnota iná), potom zostaví inštalačné ISO a demo QCOW2,
2. podpíše UEFI zavádzač ISO na mieste tajomstvom `SECURE_BOOT_DB_KEY`
   (`losos-sign-iso`) a tým istým kľúčom podpíše `SHA256SUMS`. Ak tajomstvo
   chýba alebo podpisovanie zlyhá, vydanie sa aj tak publikuje:
   ISO je nepodpísané zostavenie, beh nesie varovanie a poznámky k vydaniu
   uvádzajú *Not signed* (nepodpísané) a prečo,
3. pripojí k vydaniu na GitHube ISO, jeho `.sha256`, `SHA256SUMS`,
   `SHA256SUMS.sig` a certifikát (`.pem` a `.cer`),
4. pushne obe médiá na GHCR, keďže QCOW2 presahuje limit GitHubu 2 GiB na
   asset.

Job `iso` v main robí to isté podpisovanie pred svojimi bootovacími časťami.
OVMF s kľúčmi Microsoftu musí médium odmietnuť a to isté úložisko OVMF s
commitnutým certifikátom pridaným vedľa certifikátov Microsoftu ho musí
nabootovať (`tests/iso-boot.py --firmware
uefi-sb-ms|uefi-sb`). Pull request z forku nemá kľúč, takže bootuje
nepodpísané a časť so zaregistrovaným certifikátom preskočí, a rovnako aj
beh, v ktorom podpisovanie zlyhalo. Kľúč sa vytvorí raz na počítači
vlastníka pomocou `provisioning/secure-boot/keygen.sh`.

Vaše poznámky k vydaniu sa zachovajú. Sekcia so stiahnutiami ide medzi dva
značkovacie komentáre a opakovaný beh nahradí iba túto sekciu.

### Médiá pre tag, ktorý žiadne nemá

Vydanie, ktorého beh pre tag zlyhal, môže dostať svoje médiá neskôr.
Otvorte Actions, vyberte workflow `ci`, zvoľte *Run workflow* na `main` a
vyplňte `release_tags`: buď `missing`, pre každé publikované vydanie bez
inštalačného ISO, alebo samotné tagy oddelené medzerami (`v0.1.7
v0.1.8`). Každý tag sa zostaví z vlastného stromu, rovnako ako pri pushi
tagu, dva naraz, a jeho médiá sa pripoja k existujúcemu vydaniu. Názov
vydania a vaše poznámky zostanú, aké sú. Tagy spred obrazu edge gateway
(v0.1.6 a staršie) médiá zostaviť nevedia a s upozornením sa preskočia.

## Binárna cache

CI publikuje podpísané cesty Nix store ako OCI artefakty na
`ghcr.io/dasmatus/losos/nix-cache`. Servíruje ich samostatné nasadenie
[proxy LosOS Desktop](https://github.com/dasmatus/losos-desktop/tree/main/proxy)
s `GHCR_REPOSITORY=dasmatus/losos`.

Nastavenie:

1. `nix key generate-secret --key-name losos-1`
2. Uložte tajný kľúč ako tajomstvo repozitára `NIX_CACHE_SIGNING_KEY`.
3. Uložte verejný kľúč (`nix key convert-secret-to-public`) ako premennú
   repozitára `NIX_CACHE_PUBLIC_KEY`.
4. Uložte HTTPS URL proxy ako premennú repozitára `LOSOS_PROXY_URL`.
   Je to doména pripojená k projektu Vercel `losos-cache-proxy`,
   `https://proxy.losos.dasmat.us`, a nič iné. Staršie meno,
   `losos-proxy.dasmat.us`, tam teraz presmerúva cez 307. Hostiteľské meno,
   ktoré sa iba prekladá na Vercel, odpovie `DEPLOYMENT_NOT_FOUND`; nix potom
   v každom jobe vypíše
   `warning: '<url>' does not appear to be a binary cache`, zostavuje zo
   zdrojov a beh zostane zelený. `setup-nix` teraz kontroluje
   `<url>/nix-cache-info` a keď sa to stane, pridá k behu anotáciu.
5. Zverejnite balík `losos/nix-cache` a `losos/images` (nižšie), keď ho
   prvý push do main vytvorí. GitHub vytvára balíky ako súkromné a proxy
   potom na všetko v nich odpovedá `502 token: 403`.
6. Udržiavajte `losos.cache.substituters` a `losos.cache.trustedPublicKeys` v
   `modules/options.nix` na tej istej URL a kľúči. Ich predvolené hodnoty sú
   `https://proxy.losos.dasmat.us`, vlastný názov toho istého projektu
   `https://losos-cache-proxy.vercel.app` pre prípad, že sa vlastná doména
   nepreloží, a verejný kľúč `losos-1`, takže
   štandardné zariadenie aj inštalačné médium už ťahajú z cache a
   `tests/invariants.nix` zlyhá, ak niektorá z predvolených hodnôt zmizne.
   Výmena podpisového kľúča znamená zmeniť premennú a predvolenú hodnotu
   naraz.

Každý job hlási, čo dostal z cache. `setup-nix` vloží do `PATH` náhradu
(shim) `nix`, ktorá pri každom volaní počíta vlastné riadky nixu
`copying path '…' from '<url>'` a `building '…'`. Do súhrnu krokov jobu
zapíše tabuľku **Nix binary cache use**, jeden riadok na každý príkaz nix,
ktorý niečo skopíroval alebo zostavil, s cestami z cache LosOS, z
`cache.nixos.org`, odinakiaľ a zostavenými na runneri. Navyše vypíše jeden
riadok `LosOS cache: …` do logu jobu hneď za príkazom. Riadok so stĺpcom
LosOS na 0 a nenulovým "built here" znamená, že sa kompilovali vlastné cesty
tohto projektu, hoci ich mal pushnúť niektorý skorší beh. Job, ktorý iba
vyhodnocuje, nepridá žiadny riadok. Shim nemení ani výstup nixu, ani jeho
návratový kód.

Skôr než sa na ňu spoľahnete, skontrolujte `<proxy-url>/nix-cache-info` a
`nix copy --from <proxy-url>
<store-path>`. Nedostupná cache nie je fatálna. Zariadenie počká 5 s a
prejde na `cache.nixos.org` a zostavovanie. Tajný kľúč nikdy nedávajte do
flaku ani na zariadenie.

### Kópia na GitHub Pages

Stránka GitHub Pages, ktorá servíruje príručku, servíruje aj kópiu cache na
`https://losos.dasmat.us/proxy` pre prípad, že proxy na Verceli neodpovedá.
Pages nespúšťa žiadny kód, takže kópia je obyčajná súborová cache. Pri
každom pushi do main job `publish-cache` zapíše cesty, ktoré publikuje,
príkazom `nix copy --to file://`, podpíše ich tým istým kľúčom `losos-1` a
vynechá každú cestu, ktorú už servíruje cache.nixos.org. Workflow `handbook`
sa spustí znova, keď `ci` na main skončí, a nasadí najnovšiu kópiu pod
`/proxy`.

Má tri obmedzenia:

- Obsahuje len najnovšie zostavenie main. Zariadenie na staršej revízii
  nájde svoje cesty len na proxy.
- Stránka Pages môže mať najviac 1 GB, takže kópia drží NAR súbory pod 800
  MiB a ako prvé vynecháva najväčšie cesty. Súhrn krokov jobu menuje, čo
  vynechala. Obraz Nextcloudu, zostavovaný raz týždenne, v nej nie je nikdy.
- Jej `nix-cache-info` uvádza `Priority: 45`, za proxy (30) a
  cache.nixos.org (40), takže ju nix skúša až po oboch.

`losos.cache.substituters` predvolene uvádza obe. Zdroj Pages v repozitári
musí byť "GitHub Actions" (Settings, Pages), rovnako ako pre príručku.

## Nástroj pre vývojársky počítač

Kľúčová ceremónia ([Master proxy, oficiálne edge](Master-Proxy-sk#oficiálne-edge))
spúšťa `losos-registrar provision` na vlastnom počítači operátora, ktorý
nemá Nix store. `.#losos-registrar-static` je ten istý crate staticky
zlinkovaný s musl. Job `registrar-and-ui` ho zostaví a job `publish-tool` ho
pushne na GHCR ako OCI artefakt
`ghcr.io/dasmatus/losos/images:<channel>-x86_64`, jedna vrstva na súbor,
`losos-registrar` a `SHA256SUMS`. Presne v tomto tvare ich servíruje cesta
proxy `/updates/<channel>/<arch>/<file>`. Vyberie vrstvu, ktorej titulok je
názov súboru, a presmeruje na úložisko GHCR, okrem `SHA256SUMS`, ktorý
servíruje priamo. Rovnakú cestu číta sysupdate v LosOS Desktop. Kanál je
`main` pre push do main, tag pre vydanie (ktoré posúva aj `stable`) a
upravený názov vetvy pre ručný beh inde. Pull requesty nikdy
nepublikujú.

```sh
curl -fsSLO "https://proxy.losos.dasmat.us/updates/main/x86_64/{losos-registrar,SHA256SUMS}" && sha256sum -c --ignore-missing SHA256SUMS && chmod +x losos-registrar
```

Keď `proxy.losos.dasmat.us` neodpovedá, proxy je aj na
`https://losos-cache-proxy.vercel.app`. Keď neodpovedá ani jedna, tie isté
dva súbory z najnovšieho zostavenia main sú na
`https://losos.dasmat.us/proxy/updates/main/x86_64/`.

Súhrn krokov jobu obsahuje referenciu na artefakt, URL a kontrolný súčet.
`oras pull ghcr.io/dasmatus/losos/images:main-x86_64` stiahne tie isté súbory
bez proxy.
