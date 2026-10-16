---
title: Hängt an der Passphrase-Abfrage
sidebar_position: 6
slug: /troubleshooting/stuck-at-passphrase
---

# Hängt an der Passphrase-Abfrage

<div className="losos-symptom">

**Was du siehst:** Statt der Adresse der Box steht im Feld unter dem Lachs
`Please enter passphrase for disk persist` (oder etwas Ähnliches), und die
Box wartet. Auf einem Rechner, der im Textmodus startet, steht dieselbe
Zeile auf einem schwarzen Bildschirm. Bei einem normalen Start fragt die Box
nie nach einer Passphrase.

</div>

![Eine Box, die beim Start angehalten hat: Das Feld unter dem Lachs fragt Please enter passphrase for disk persist.](@site/docs/img/passphrase-prompt.png)

## Warum das passiert \{#why-it-happens}

Die Box konnte das verschlüsselte Volume nicht auf die Art entsperren, für
die sie installiert wurde:

| Installation         | Was schiefgegangen ist                                                                                                    |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| **TPM**              | Der Chip hat den Schlüssel nicht mehr oder gibt ihn nicht mehr heraus. Das TPM der Firmware wurde gelöscht oder zurückgesetzt, der Chip wurde ersetzt, oder die Festplatte kam in einen anderen Rechner. In einer VM startete das emulierte TPM mit einem anderen Zustandsverzeichnis als bei der Installation. |
| **Schlüsseldatei**   | Auf der Boot-Partition ist die Schlüsseldatei verloren gegangen oder beschädigt, oder die Festplatte wurde nur teilweise geklont. |
| **Beliebig, vor dem 2. Oktober 2026** | Ein Fehler in älteren Installern formatierte das Volume mit einem Schlüssel und versuchte, es mit einem anderen zu entsperren, weil unterwegs ein einzelnes Byte verloren ging. Jede solche Box blieb beim ersten Start an dieser Abfrage hängen. Pull Request #47 hat das behoben. Installiere von einem aktuellen ISO. |

## Was du tun kannst \{#what-you-can-do}

:::note[Keine Passphrase zum Eintippen]
Es gibt keine Passphrase, die du eintippen könntest. Der Schlüssel ist eine
zufällige Zeichenkette mit 4096 Zeichen, die nie jemand gesehen hat. Die
Wiederherstellungskopie liegt *im* verschlüsselten Volume, also kann sie
dieses Volume nicht öffnen.
:::

1. **Einmal aus- und wieder einschalten.** Ein TPM, das bei einem eiligen
   Start nicht geantwortet hat, antwortet manchmal beim nächsten Mal.
2. **In einer VM** startest du das emulierte TPM mit demselben
   Zustandsverzeichnis wie bei der Installation, bevor du die VM startest.
3. **Auf echter Hardware** prüfst du, ob das TPM der Firmware, Intel PTT
   oder AMD fTPM, noch eingeschaltet ist und ob kein Firmware-Update und
   kein „Auf Standardwerte zurücksetzen“ es gelöscht hat.
4. Sonst **installierst du neu**. Ohne den Schlüssel sind die Daten auf
   dem Volume unlesbar, und genau das hat die Verschlüsselung versprochen.
   Deine Dateien sind in der Kopie, die du anderswo aufbewahrst.

## Damit es nicht wieder passiert \{#preventing-the-next-time}

Bewahr eine weitere Kopie deiner Daten auf. Lösch das TPM nicht in der
Firmware. In einer VM bewahrst du das TPM-Zustandsverzeichnis zusammen mit
dem Festplatten-Image auf. Die Seite
[Installationsvarianten](../types/install-variants#how-the-disk-is-unlocked)
erklärt, welchen Kompromiss der TPM-Modus eingeht.
