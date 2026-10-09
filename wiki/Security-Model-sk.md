[English](Security-Model) · **Slovenčina** · [Deutsch](Security-Model-de)

# Bezpečnostný model

Úplný dokument je
[`docs/security-model.md`](https://github.com/dasmatus/losos/blob/main/docs/security-model.md)
v repozitári, verzovaný spolu s kódom. Skrátená verzia:

## Hranice dôvery

1. **Internet k edge.** Do internetu je obrátený iba [master proxy](Master-Proxy-sk)
   na VPS.
2. **Edge k zariadeniu.** Tunel rathole, šifrovaný cez Noise.
3. **LAN k zariadeniu.** HTTPS so self-signed certifikátom, ktorému raz
   dôverujete. Kým to neurobíte, útočník v LAN môže zachytiť prvé použitie.
4. **`notshared` a `shared`.** Oddelení používatelia a skupiny, domovské
   adresáre s režimom 700.

## Známe obmedzenia

- **Spoločný origin.** Admin UI, Nextcloud a Forgejo zdieľajú jeden origin.
  XSS v Nextcloude alebo Forgejo môže prečítať admin token a token znamená
  root. Riešením by bol samostatný hostname pre admin UI.
- **Proti krádeži celého zariadenia ochrana nie je.** Diskový kľúč je
  zapečatený do TPM bez naviazania na PCR, takže ho čip vydá akémukoľvek
  softvéru, ktorý na tom počítači beží. Nečitateľný je iba samotný disk. Na
  počítači bez TPM leží keyfile na nešifrovanom ESP a čitateľný je dokonca
  aj samotný disk. Podrobnosti sú v [TPM a odomykanie disku](TPM-sk).
- **Auditný log nie je odolný proti manipulácii** a nerotuje sa.
- **Obmedzovanie (throttling) je podľa adresy**, takže podvrhnutie adresy
  v LAN ho obíde.
