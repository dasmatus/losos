---
title: Sicherheitsmodell
sidebar_position: 4
---

# Sicherheitsmodell

Diese Seite ist eine Zusammenfassung für Besitzer. Das vollständige Dokument
ist
[`docs/security-model.md`](https://github.com/dasmatus/losos/blob/main/docs/security-model.md)
im Repository. Es wird zusammen mit dem Code versioniert und ist so
geschrieben, dass du ihm widersprechen kannst.

## Vertrauensgrenzen

1. **Internet → Edge.** Nur die Edge ist dem Internet zugewandt. Eine Box
   öffnet einen Tunnel nach außen und lauscht von außen auf nichts.
2. **Edge → Box.** Noise verschlüsselt den Tunnel durchgehend. Die Box merkt
   sich den Schlüssel der Edge beim ersten Kontakt und lehnt einen
   geänderten ab.
3. **LAN → Box.** Die Box liefert HTTPS mit ihrem eigenen Zertifikat aus,
   dem du einmal pro Rechner vertraust. Bis dahin könnte jemand im selben
   WLAN, der Verkehr abfangen kann, die erste Nutzung abfangen.
4. **Deine Dateien ↔ die Kopien des Mesh.** Die Box hat zwei Konten mit
   getrennten Gruppen und Home-Verzeichnissen im Modus 700. Das Verzeichnis
   des Mesh ist mit einem eigenen Schlüssel verschlüsselt, den die Box nur
   lädt, solange das Teilen an ist.
5. **Eine offizielle Edge ↔ irgendeine Edge.** Die Box trägt einen
   öffentlichen Root-Schlüssel, mit dem sie die LosOS-Edge von der eigenen
   Edge einer Firma unterscheidet. Der Handel braucht die LosOS-Edge, alles
   andere funktioniert mit beiden. Die private Hälfte wird nur auf dem
   eigenen Rechner des Projektinhabers erzeugt und benutzt. Das Werkzeug, das
   sie benutzt, meldet die Person über GitHub an und prüft das Konto gegen
   eine Liste im Repository. Keine Box und keine Edge hält je den privaten
   Schlüssel. Das signierte Zertifikat erreicht die Edge über das Web, und
   die Edge prüft das GitHub-Konto des Absenders gegen dieselbe Liste, bevor
   sie es installiert.

![Die Grenzen der Box: eine Eingangstür, eine Steuer-API nur auf Loopback, der nach außen zur Edge geöffnete Tunnel und ein verschlüsseltes Volume.](@site/docs/img/box-architecture.svg)

## Was der Admin-Schlüssel ist

Der Ersatzschlüssel mit 64 Zeichen ist so mächtig wie root: Er kann jede
Einstellung schreiben und die Box neu bauen. Die Box erzeugt ihn und zeigt
ihn einmal auf einer Seite des Assistenten. Danach gibt sie ihn nur einem
Browser, der das Passwort des Besitzers nachweist. Die Steuer-API lauscht nur
auf dem Loopback der Box, hinter dem Nur-LAN-Schutz. Zehn Fehlversuche
bremsen eine Adresse. Die Box führt ein Audit-Log über Änderungen an
Einstellungen, Passwort-Rücksetzungen und Resets.

## Härtung

Standardmäßig an: Härtungsparameter und sysctls für den Kernel, eine Sperrliste
für Kernelmodule, ein tmpfs-`/tmp` und systemd-Sandboxing für den Webserver,
mDNS und den Steuerdienst. Vier Schutzmaßnahmen, die etwas kaputt machen
können, schaltest du im Bereich Sicherheit ein: AppArmor, ein gehärteter
Speicherallokator, kein SMT, USBGuard. [Härtung](/in-depth/hardening.md)
beschreibt jeden Schalter und was er kostet.

![Einstellungen, Sicherheit: die vier zusätzlichen Schutzmaßnahmen, die standardmäßig aus sind, jede mit dem, was sie kostet.](@site/docs/img/settings-security.png)

## Bekannte Grenzen, offen gesagt

- **Das Installationsmedium ist signiert, das installierte System nicht.**
  Eine Firmware mit eingetragenem LosOS-Zertifikat prüft den Bootloader des
  Sticks, ein signiertes Image mit Kernel, initrd und Kommandozeile. Dieser
  prüft den Hash des System-Images, bevor er es einhängt, sodass ein
  veränderter Stick nicht startet. Der eigene Bootloader der Box und ihre
  nächtlichen Kernel sind nicht signiert. Die Box läuft deshalb mit
  ausgeschaltetem Secure Boot, und ihre Bootkette schützt nur der physische
  Besitz. Die Details stehen unter [Secure Boot und signierte
  Medien](../start/secure-boot).

- **Ein Origin.** Die Admin-Seiten, LosOS cloud und LosOS Git teilen sich
  eine Adresse. Eine Cross-Site-Scripting-Lücke in einer der Apps könnte den
  Admin-Schlüssel aus einem Tab lesen, der ihn hat. Die Abwehr sind die
  strenge Content-Security-Policy der Admin-Seiten, der Nur-LAN-Schutz und
  aktuell gehaltene Apps. Das Projekt hat einen eigenen Namen für die
  Admin-Seiten erwogen und ist bei einer Adresse geblieben.
- **Diebstahl der ganzen Box.** Das TPM gibt den Festplattenschlüssel an
  jede Software heraus, die auf diesem Rechner gestartet wird, also kann ein
  Dieb mit der Box ihn lesen. Die Verschlüsselung schützt nur eine einzeln
  ausgebaute Festplatte. Auf einer Box ohne TPM ist sogar die Festplatte
  allein lesbar. [TPM und der Festplattenschlüssel](tpm) erklärt den
  Unterschied.
- **Das Audit-Log** rotiert nicht, und root kann es umschreiben.
- **Die Drosselung gilt pro Adresse**, sodass ein Angreifer im LAN, der
  Adressen fälscht, jedes Mal ein neues Kontingent bekommt.
- **Ein selbst geschriebenes Widget** läuft in jedem Browser, der die Box
  öffnet. Es ist in einer Sandbox vom Admin-Schlüssel und der API getrennt,
  kann aber alles anzeigen, was sein Autor geschrieben hat, und alles aus
  dem Internet laden.
