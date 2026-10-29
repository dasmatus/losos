---
title: Nástroje AI na vašom počítači
sidebar_position: 10.5
slug: /manual/ai-agents
---

# Nástroje AI na vašom počítači

Terminálový klient nájde nástroje AI na vašom počítači a môže do nich
pridať **pomocníka LosOS**. S pomocníkom si nástroj prezrie vaše súbory v
LosOS Cloud a vaše repozitáre v LosOS Git, takže sa ho môžete pýtať na
vlastné poznámky a kód. Môže sa iba pozerať. Na zariadení nič nezmení,
nepresunie ani nezmaže.

Beží to na vašom počítači, v terminálovom klientovi, pretože správcovské
stránky do vášho počítača nevidia. Pomocník sa prihlasuje cez LosOS Login,
prihlásenie samotného zariadenia, vo vašom prehliadači.

## Ktoré nástroje

| Nástroj | Ako sa do neho pomocník dostane |
|---|---|
| Claude Code | ako doplnok, cez `claude plugin` |
| GitHub Copilot CLI | ako doplnok, cez `copilot plugin` |
| Codex | ako doplnok, cez `codex plugin` |
| Gemini CLI | ako rozšírenie, cez `gemini extensions` |
| Qwen Code | ako rozšírenie, cez `qwen extensions` |
| opencode | záznam v `~/.config/opencode/opencode.json` |
| Claude Desktop | záznam v `~/.config/Claude/claude_desktop_config.json` |
| Cursor | záznam v `~/.cursor/mcp.json` |
| VS Code a VSCodium | záznam v `mcp.json` v používateľských nastaveniach editora |
| Cline | záznam v jeho nastaveniach MCP vo VS Code |
| Windsurf | záznam v `~/.codeium/windsurf/mcp_config.json` |
| Zed | záznam pod `context_servers` v súbore `settings.json` editora Zed |
| LM Studio | záznam v `~/.lmstudio/mcp.json` |
| derisk Agent, v LosOS Desktop | záznam v `~/.config/derisk-agent/mcp.json` |

Nástroj sa pozná podľa programu vo vašej premennej `PATH` alebo podľa
priečinka s nastaveniami. `losos agents` vypíše, čo našiel:

```sh
losos agents
```

## Pridanie pomocníka

```sh
losos agents install --host https://mattbox.local
```

Použite adresu svojho zariadenia. Ak je terminálový klient k zariadeniu už
pripojený (pozrite [Terminálový klient](terminal)), `--host` môžete
vynechať.

Postupne sa opýta na každý nájdený nástroj. V nástroji, ku ktorému poviete
nie, sa nič nezmení. Ak vymenujete nástroje, vezme iba tie, a s `--yes` sa
nebude pýtať:

```sh
losos agents install claude-code zed --yes
```

Ak sa tento počítač k zariadeniu ešte nikdy nepripájal, klient ukáže
odtlačok certifikátu zariadenia. Porovnajte ho s tým, ktorý pre zariadenie
ukazuje váš prehliadač, a ak sa zhodujú, povedzte áno. Odvtedy sa pomocník
spojí iba so zariadením s týmto certifikátom.

Potom sa pomocník prihlási. Klient vypíše odkaz a kód:

```text
Pomocník sa do https://mattbox.local prihlasuje cez LosOS Login. Otvorte tento odkaz v prehliadači, prihláste sa a schváľte pomocníka LosOS:
  https://mattbox.local/auth/consent/openid/device-authorization?user_code=WDJBMJHT
Stránka ukazuje kód WDJBMJHT. Čaká sa na schválenie…
```

Otvorte odkaz, prihláste sa do LosOS Login ako vždy, aj s druhým krokom,
ak ho máte, skontrolujte, že stránka ukazuje ten istý kód, a zvoľte
**Accept**. Terminál potom pokračuje sám.

Skôr než klient zmení súbor s nastaveniami, skopíruje ho do
`~/.local/share/losos/companion/backups/`. Zmení iba vlastný záznam a vaše
komentáre aj ostatné servery nechá tak, ako boli. Nástroj, ktorý bol
otvorený, spustite znova, aby pomocníka načítal.

Gemini CLI sa pred inštaláciou pýta, či dôverujete priečinku, z ktorého
inštaluje. Vaše áno k Gemini CLI je aj odpoveďou na túto otázku: klient
pridá do zoznamu dôveryhodných priečinkov Gemini iba vlastný priečinok
pomocníka a pri odstránení ho zo zoznamu zase vyškrtne. Gemini CLI spúšťa
servery MCP, aj pomocníka, iba v priečinkoch, ktorým dôverujete.

## Čo pomocník vie

Keď je pridaný, nástroj od neho dostane tieto nástroje:

- vypísať priečinok v LosOS Cloud, nájsť súbory podľa mena a hľadať v ich
  texte;
- vypísať vaše repozitáre v LosOS Git a hľadať v kóde ich predvolených
  vetiev;
- vypísať vaše priečinky v LosOS Mail, hľadať v priečinku a prečítať
  správu, ktorá pre vás zostane neprečítaná;
- povedať, ako sa zariadeniu darí a pod ktorým účtom pomocník číta.

LosOS Mail je iba na zariadení, ktoré zdieľa svoje úložisko v sieti mesh.
Poznámky sú súbory v LosOS Cloud, takže ich pomocník nájde a prehľadá
nástrojmi pre Cloud.

Pomocník číta to, čo môžete čítať vy. Človek s vlastným účtom na zariadení
dostane svoje súbory, nie vaše.

## Prečo je to bezpečné

Pomocník vaše heslo nikdy nevidí. Zadáte ho do LosOS Login vo svojom
prehliadači a LosOS Login dá pomocníkovi dva tokeny: jeden otvára
zariadenie na hodinu a druhý po jej uplynutí získa nový pár. Ak pomocník
30 dní nikto nepoužije, musí sa prihlásiť znova.

Zariadenie týmto tokenom dovolí čítať súbory, repozitáre, poštu a stav
zariadenia, nič iné, ani keď ste správca. Pomocník posiela iba čítanie a
zariadenie všetko ostatné odmietne. Človek, ktorého účet čaká na druhý krok
alebo nové heslo, pomocníka neprihlási, kým to nedokončí.

Tokeny sú v kľúčenke vášho pracovného prostredia, tam, kde terminálový
klient drží všetko ostatné. Nikdy sa nezapíšu do nastavení nástroja AI:
každý nástroj spustí pomocníka a pomocník číta kľúčenku.

## Odstránenie

```sh
losos agents remove
```

Opýta sa na každý nástroj, v ktorom pomocník je, odstráni ho a každý súbor
s nastaveniami vráti do pôvodného stavu. Keď už pomocník nie je v žiadnom
nástroji, klient ho odhlási: oba tokeny vráti LosOS Login a zabudne ich.
Zariadenie ich prestane prijímať do minúty.
