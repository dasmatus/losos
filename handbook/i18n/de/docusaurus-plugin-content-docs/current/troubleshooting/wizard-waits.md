---
title: Der Assistent wartet und wartet
sidebar_position: 7
slug: /troubleshooting/wizard-waits
---

# Der Assistent wartet und wartet

<div className="losos-symptom">

**Was du siehst:** Schritt 2 der ersten Einrichtung sagt „Warten, bis diese
Box fertig gestartet ist“ und bleibt dabei.

</div>

<img src={require("@site/docs/img/wizard-step2-waiting.png").default} alt="Schritt 2 des Assistenten wartet auf LosOS cloud und zeigt, worauf genau." width="520" />

## Warum \{#why}

Das Passwort liegt in LosOS cloud, deshalb kann der Assistent es erst
setzen, wenn LosOS cloud sich installiert hat. Das tut LosOS cloud beim
ersten Start der Box. Auf einem Mini-PC dauert das zwei bis fünf Minuten,
in einer emulierten VM ohne Hardwarebeschleunigung können es fünfzehn sein.
Die Seite prüft alle paar Sekunden und macht von selbst weiter.

## Was du tun kannst \{#what-to-do}

1. **Warte zehn Minuten**, bevor du irgendetwas anderes tust. Öffne
   `http://<address>/api/setup/claim` in einem anderen Tab. Wenn dort
   `"ready": false` steht, nennt das Feld `waitingFor`, worauf die Box
   wartet.
2. **Nach fünfzehn Minuten** ziehst du den Stecker, um die Box neu zu
   starten, und öffnest den Assistenten wieder. Der erste Start von LosOS
   cloud lässt sich neu anstoßen.
3. **Eine Box, die auch nach einem Neustart nie so weit kommt**, hat selten
   ein Netzwerkproblem, denn sie lädt nichts herunter und alles liegt auf
   der Box. Meist ist die Festplatte voll oder die Installation
   fehlgeschlagen. Prüf den Bereich Speicher, wenn die Admin-Seiten
   aufgehen, sonst installier neu.

## Ein bekannter Fehler \{#a-known-bug}

Builds von vor dem 6. Oktober 2026 konnten das Feld auf „starting“ stehen
lassen, nachdem LosOS cloud schon bereit war. Pull Request #62 hat das
behoben. Wenn du die Seite neu lädst und es direkt weitergeht, warst du
davon betroffen.
