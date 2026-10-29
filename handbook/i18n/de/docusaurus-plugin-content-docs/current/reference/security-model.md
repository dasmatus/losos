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

## Vertrauensgrenzen \{#trust-boundaries}

1. **Internet → Edge.** Nur die Edge ist dem Internet zugewandt. Eine Box
   öffnet einen Tunnel nach außen und lauscht von außen auf nichts.
2. **Edge → Box.** TLS 1.3 verschlüsselt den Tunnel durchgehend, über
   QUIC oder, wo UDP gesperrt ist, über TCP. Die Box merkt sich das
   Zertifikat der Edge beim ersten Kontakt und lehnt ein geändertes ab.
   Im Tunnel bleiben die Verbindungen der Besucher bis zum eigenen
   Zertifikat der Box verschlüsselt: Die Edge leitet sie nach dem Namen
   weiter und kann sie nicht lesen. Sie sieht aber, welcher Name angefragt
   wurde, wann, wie viele Daten flossen und die Adresse der Box zu Hause.
   Siehe [Die Edge kann den Verkehr nicht lesen](../in-depth/master-proxy.md#the-edge-cannot-read-the-traffic).
3. **LAN → Box.** Die Box liefert HTTPS mit ihrem eigenen Zertifikat aus,
   dem du einmal pro Rechner vertraust. Bis dahin könnte jemand im selben
   WLAN, der Verkehr abfangen kann, die erste Nutzung abfangen.
4. **Deine Dateien ↔ die Kopien des Mesh.** Die Box hat zwei Konten mit
   getrennten Gruppen und Home-Verzeichnissen im Modus 700. Das Verzeichnis
   des Mesh ist mit einem eigenen Schlüssel verschlüsselt, den die Box nur
   lädt, solange das Teilen an ist. Dieser Schlüssel liegt auf der Box, die
   den Platz verleiht. Willst du eigene Daten im Mesh haben, sichere sie auf
   Speicher, den du dort gekauft hast: Deine Box verschlüsselt sie mit deinem
   Wiederherstellungscode, bevor sie hinausgehen, und die Verleiher und die
   Edge haben nur Chiffretext.
5. **Eine offizielle Edge ↔ irgendeine Edge.** Die Box prüft jede Edge
   selbst, bevor sie über sie handelt: den OpenPGP-Pass ihres Betreibers,
   ein TPM, das zeigt, dass ein veröffentlichtes LosOS-Gateway-Image
   unverändert gestartet ist, und Einstellungen, die den Anteil von LosOS an
   die Adressen zahlen, die die Box mitbringt. Der Handel braucht eine Edge,
   die besteht, alles andere funktioniert mit jeder Edge. Kein
   LosOS-Schlüssel macht eine Edge offiziell, also gibt es keinen privaten Schlüssel,
   den man stehlen könnte. Eine Edge, die bei einer Manipulation ertappt
   wurde, nutzt die Box, die sie ertappt hat, nie wieder.

![Die Grenzen der Box: eine Eingangstür, eine Steuer-API nur auf Loopback, der nach außen zur Edge geöffnete Tunnel und ein verschlüsseltes Volume.](@site/docs/img/box-architecture-de.svg)

## Was der Admin-Schlüssel ist \{#what-the-admin-key-is}

Der Ersatzschlüssel mit 64 Zeichen ist so mächtig wie root: Er kann jede
Einstellung schreiben und die Box neu bauen. Die Box erzeugt ihn und zeigt
ihn einmal auf einer Seite des Assistenten. Danach gibt sie ihn nur einem
Browser, der das Passwort des Besitzers nachweist. Die Steuer-API lauscht nur
auf dem Loopback der Box, hinter dem Nur-LAN-Schutz. Zehn Fehlversuche
bremsen eine Adresse. Die Box führt ein Audit-Log über Änderungen an
Einstellungen, Passwort-Rücksetzungen und Resets.

## Härtung \{#hardening}

Standardmäßig an: Härtungsparameter und sysctls für den Kernel, eine Sperrliste
für Kernelmodule, ein tmpfs-`/tmp` und systemd-Sandboxing für den Webserver,
mDNS und den Steuerdienst. Vier Schutzmaßnahmen, die etwas kaputt machen
können, schaltest du im Bereich Sicherheit ein: AppArmor, ein gehärteter
Speicherallokator, kein SMT, USBGuard. [Härtung](/in-depth/hardening.md)
beschreibt jeden Schalter und was er kostet.

![Einstellungen, Sicherheit: die vier zusätzlichen Schutzmaßnahmen, die standardmäßig aus sind, jede mit dem, was sie kostet.](@site/docs/img/settings-security-de.png)

## Bekannte Grenzen, offen gesagt \{#known-limitations-stated-plainly}

- **Das Installationsmedium ist signiert, das installierte System nicht.**
  Eine Firmware mit eingetragenem LosOS-Zertifikat prüft den Bootloader des
  Sticks, ein signiertes Image mit Kernel, initrd und Kommandozeile. Dieser
  prüft den Hash des System-Images, bevor er es einhängt, sodass ein
  veränderter Stick nicht startet. Der eigene Bootloader der Box und ihre
  nächtlichen Kernel sind nicht signiert. Die Box läuft deshalb mit
  ausgeschaltetem Secure Boot, und ihre Bootkette schützt nur der physische
  Besitz. Die Details stehen unter [Secure Boot und signierte
  Medien](../start/secure-boot).

- **Ein Origin.** Die Admin-Seiten, LosOS Cloud und LosOS Git teilen sich
  eine Adresse. Eine Cross-Site-Scripting-Lücke in einer der Apps könnte den
  Admin-Schlüssel aus einem Tab lesen, der ihn hat. Die Abwehr sind die
  strenge Content-Security-Policy der Admin-Seiten, der Nur-LAN-Schutz und
  aktuell gehaltene Apps. Das Projekt hat einen eigenen Namen für die
  Admin-Seiten erwogen und ist bei einer Adresse geblieben.
- **Diebstahl der ganzen Box.** Das TPM gibt den Festplattenschlüssel an
  jede Software heraus, die auf diesem Rechner gestartet wird, also kann ein
  Dieb mit der Box ihn lesen. Die Verschlüsselung schützt nur eine einzeln
  ausgebaute Festplatte. Auf einer Box ohne TPM ist sogar die Festplatte
  allein lesbar. Eine mit `--tang` installierte Box öffnet sich nur mit einem
  Schlüsselserver in ihrem eigenen Netz und fragt anderswo nach dem
  Wiederherstellungscode. Eine mit `--tang-edge` installierte Box braucht
  die Antwort ihrer Edge, und der Besitzer kann der Edge sagen, einer
  gestohlenen Box nicht mehr zu antworten. [TPM und der
  Festplattenschlüssel](tpm) erklärt den Unterschied.
- **Das Audit-Log** rotiert nicht, und root kann es umschreiben.
- **Die Drosselung gilt pro Adresse**, sodass ein Angreifer im LAN, der
  Adressen fälscht, jedes Mal ein neues Kontingent bekommt.
- **Ein selbst geschriebenes Widget** läuft in jedem Browser, der die Box
  öffnet. Es ist in einer Sandbox vom Admin-Schlüssel und der API getrennt,
  kann aber alles anzeigen, was sein Autor geschrieben hat, und alles aus
  dem Internet laden.
