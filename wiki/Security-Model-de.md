[English](Security-Model) · [Slovenčina](Security-Model-sk) · **Deutsch**

# Sicherheitsmodell

Das vollständige Dokument ist
[`docs/security-model.md`](https://github.com/dasmatus/losos/blob/main/docs/security-model.md)
im Repository, zusammen mit dem Code versioniert. Die Kurzfassung:

## Vertrauensgrenzen

1. **Internet zur Edge.** Nur der [Master-Proxy](Master-Proxy-de) auf dem
   VPS ist dem Internet zugewandt.
2. **Edge zur Appliance.** Ein rathole-Tunnel, mit Noise verschlüsselt.
3. **LAN zur Appliance.** HTTPS mit einem selbstsignierten Zertifikat, dem
   du einmal vertraust. Bis dahin kann ein Angreifer im LAN die erste
   Nutzung abfangen.
4. **`notshared` und `shared`.** Getrennte Benutzer und Gruppen,
   Home-Verzeichnisse mit Modus 700.

## Bekannte Einschränkungen

- **Gemeinsamer Origin.** Admin-UI, Nextcloud und Forgejo teilen sich einen
  Origin. Eine XSS-Lücke in Nextcloud oder Forgejo kann das Admin-Token
  lesen, und das Token ist root. Abhilfe wäre ein eigener Hostname für die
  Admin-UI.
- **Gegen den Diebstahl der ganzen Box gibt es keinen Schutz.** Der
  Festplattenschlüssel ist ohne PCR-Bindung im TPM versiegelt, der Chip gibt
  ihn also an jede Software heraus, die auf diesem Rechner läuft. Nur die
  Festplatte allein ist unlesbar. Auf einem Rechner ohne TPM liegt das
  Keyfile auf einer unverschlüsselten ESP, und selbst die Festplatte allein
  ist lesbar. [TPM und Entsperren der Festplatte](TPM-de) enthält die
  Details.
- **Das Audit-Log ist nicht manipulationssicher** und wird nicht rotiert.
- **Die Drosselung erfolgt pro Adresse**, sodass gefälschte Absenderadressen
  im LAN sie umgehen.
