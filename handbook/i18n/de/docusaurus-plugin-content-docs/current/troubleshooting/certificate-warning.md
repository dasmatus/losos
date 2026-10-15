---
title: Zertifikatswarnung
sidebar_position: 4
slug: /troubleshooting/certificate-warning
---

# Zertifikatswarnung

<div className="losos-symptom">

**Was du siehst:** „Dies ist keine sichere Verbindung“, „Warnung: Potenzielles
Sicherheitsrisiko erkannt“, `NET::ERR_CERT_AUTHORITY_INVALID` oder ein
durchgestrichenes Schloss, auf einer `https://`-Adresse der Box.

</div>

## Warum \{#why}

Die Box signiert ihr Zertifikat selbst, weil keine öffentliche Stelle eines
für einen `.local`-Namen ausstellen kann. Bis du deinem Browser sagst, dass
er diesem Zertifikat vertrauen soll, warnt jede `https://`-Seite. Einfaches
`http://<address>` warnt nie und ist in deinem eigenen LAN in Ordnung. HTTPS
ist wichtig für Passkeys, und es verhindert, dass das Passwort im Klartext
durchs Netzwerk geht.

## Die Lösung: das Zertifikat einmal pro Rechner installieren \{#the-fix-install-the-certificate-once-per-computer}

Der erste Schritt des Assistenten zeigt den Einzeiler-Installer. Nach der
Einrichtung liefert die Box dieselben Dateien weiter aus, nur ins LAN:

<img src={require("@site/docs/img/wizard-step1-trust.png").default} alt="Der Zertifikatsschritt des Assistenten mit den Einzeiler-Installern und dem Fingerabdruck." width="520" />

```bash title="macOS und Linux"
curl -fsSL http://<address>/setup/trust.sh | sh
```

```powershell title="Windows (PowerShell)"
irm http://<address>/setup/trust.ps1 | iex
```

Das Skript fügt den Zertifikatsspeichern deines Benutzers für die Browser
ein Zertifikat hinzu und gibt dessen Fingerabdruck aus. Vergleich ihn mit
`http://<address>/setup/state.json`. Dann **starte den Browser neu** und
öffne `https://<name>.local`.

Von Hand lädst du `http://<address>/setup/losos-ca.crt` herunter und fügst
es als vertrauenswürdige Zertifizierungsstelle hinzu. Unter macOS geht das
in der Schlüsselbundverwaltung. Unter Windows ist es `certmgr.msc` →
Vertrauenswürdige Stammzertifizierungsstellen. Firefox hat eigene
Zertifikatseinstellungen, ein Handy hat seine Sicherheitseinstellungen.

## Warnt nach der Installation immer noch \{#still-warning-after-installing}

- **Du hast `https://<address>` geöffnet.** Das Zertifikat gilt nur für
  `<name>.local`, eine IP-Adresse warnt also immer. Nimm den Namen oder
  einfaches `http://` mit der Adresse.
- **Firefox unter Linux** hat für jedes Profil einen eigenen Speicher. Das
  Skript deckt jedes Profil ab, das es findet, aber für ein Profil, das du
  später anlegst, musst du es noch einmal ausführen.
- **Die Box wurde neu installiert** oder ihr Name hat sich geändert, also
  hat sie ein neues Zertifikat. Führ das Skript noch einmal aus. Der alte
  Eintrag schadet nicht.
- **Das Zertifikat ist abgelaufen.** Es gilt zwei Jahre ab der
  Installation. Die Box erzeugt ein neues, wenn es abläuft, und das Skript
  installiert das neue.
- **Chrome meldet, das Zertifikat sei „für diesen Namen nicht gültig“**
  nach einer Umbenennung: dieselbe Ursache, dieselbe Lösung.

## Soll ich die Warnung einfach wegklicken? \{#should-i-just-click-through}

In deinem eigenen LAN, bei einer Box, die du selbst installiert hast, ist
einmal Wegklicken nicht gefährlich. Aber sobald du einem Browser sagst,
dass er die Warnung ignorieren soll, ignoriert er auch einen echten
Angreifer im selben WLAN, und Passkeys bietet er dann nicht an. Installier
lieber das Zertifikat. Das ist eine Zeile.
