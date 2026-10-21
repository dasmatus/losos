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

Beží to na vašom počítači, v terminálovom klientovi. Správcovské stránky
do vášho počítača nevidia, preto iba vytvoria token, ktorý pomocník
používa.

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

Nástroj sa pozná podľa programu vo vašej premennej `PATH` alebo podľa
priečinka s nastaveniami. `losos agents` vypíše, čo našiel:

```sh
losos agents
```

## Skôr než začnete

Terminálový klient už musí byť pripojený k zariadeniu (pozrite
[Terminálový klient](terminal)). Potom vytvorte ešte jeden token, iba pre
pomocníka: v **Nastavenia → Prístupové tokeny** zvoľte pod **Prístup**
možnosť **Agenti AI**. Taký token vie čítať súbory a repozitáre, nič iné,
a funguje iba pre pomocníka.

## Pridanie pomocníka

```sh
losos agents install
```

Postupne sa opýta na každý nájdený nástroj. V nástroji, ku ktorému poviete
nie, sa nič nezmení. Potom si raz vypýta token pre agentov AI.

Ak vymenujete nástroje, vezme iba tie, a s `--yes` sa nebude pýtať:

```sh
losos agents install claude-code zed --yes
```

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
- povedať, ako sa zariadeniu darí a pod ktorým účtom pomocník číta.

Pomocník číta to, čo môžete čítať vy. Človek s vlastným účtom na zariadení
dostane svoje súbory, nie vaše.

## Prečo je to bezpečné

Pomocník má vlastný kľúč GPG, vytvorený na vašom počítači. Váš vlastný
kľúč raz, pri nastavení, podpíše, že tento kľúč smie čítať toto jedno
zariadenie s týmto jedným tokenom. Zariadenie prijme kľúč pomocníka iba
spolu s týmto tokenom pre agentov AI a token pre agentov AI iba na čítanie
súborov, repozitárov a stavu zariadenia. Nástroj AI teda nikdy nedrží váš
vlastný kľúč a dostane sa k oveľa menej než vy.

Kľúč aj token sú v kľúčenke vášho pracovného prostredia, tam, kde
terminálový klient drží všetko ostatné.

## Odstránenie

```sh
losos agents remove
```

Opýta sa na každý nástroj, v ktorom pomocník je, odstráni ho a každý súbor
s nastaveniami vráti do pôvodného stavu. Keď už pomocník nie je v žiadnom
nástroji, klient zabudne jeho kľúč aj token. Token pre agentov AI zrušte aj
v **Nastavenia → Prístupové tokeny**.
