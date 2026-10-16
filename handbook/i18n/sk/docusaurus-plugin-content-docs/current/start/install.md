---
title: Inštalácia
sidebar_position: 2
---

# Inštalácia

Na inštaláciu potrebujete jeden USB kľúč, jeden sieťový kábel a asi pol
hodiny, z ktorej väčšinu len čakáte. Inštalátor sa opýta jednu otázku a inak
beží sám. **Vymaže všetky pevné disky v počítači.**

## Čo potrebujete \{#what-you-need}

- Počítač so 64-bitovým procesorom Intel alebo AMD (x86_64) a firmvérom UEFI
  alebo BIOS, aspoň 4 GiB pamäte a jeden disk s kapacitou 40 GiB alebo viac.
  Tvorba widgetov pomocou AI chce 8 GiB pamäte, pozrite si
  [Tvorcu widgetov](/in-depth/widget-builder.md). Dobre sa hodia mini-PC
  určené do kancelárií. Väčšina z nich má vo firmvéri čip TPM 2.0, nazývaný
  Intel PTT alebo AMD fTPM, a inštalátor v ňom uloží kľúč disku. LosOS beží
  iba na x86_64. Na počítač s ARM ho nenainštalujete, a najmä
  **nie na Mac s Apple Silicon (rad M)**.
- USB kľúč s kapacitou 2 GiB alebo viac.
- Káblové pripojenie do siete s prístupom na internet. Inštalátor si systém,
  ktorý inštaluje, stiahne.
- Obrazovku a klávesnicu k počítaču, iba na inštaláciu. Potom obrazovka
  ukazuje banner s adresou zariadenia a zariadenie nepotrebuje ani jedno.

:::tip[O terminálovom klientovi rozhodnite teraz]
Ak chcete zariadenie neskôr spravovať z terminálu cez `losos`, rozhodnite
sa teraz. Zariadenie dôveruje iba GPG kľúču, ktorý bol na USB kľúči, z ktorého
sa inštalovalo, a USB kľúč z vydania žiadny nemá.
[Terminálový klient](/manual/terminal.md) ukazuje, ako pripraviť USB kľúč
s vaším kľúčom.
:::

## Postup \{#steps}

1. Stiahnite inštalačné ISO a jeho `.sha256` z
   [najnovšieho vydania](https://github.com/dasmatus/losos/releases)
   a zapíšte ho na USB kľúč pomocou balenaEtcher, Rufus v režime DD alebo `dd`.
2. Vo firmvéri počítača buď **vypnite Secure Boot**, alebo pridajte
   certifikát LosOS vedľa certifikátov Microsoftu, aby firmvér mohol USB kľúč
   overiť. Certifikáty Microsoftu kvôli tomu nemažte.
   [Secure Boot a podpísané médiá](secure-boot) ukazujú ako. Nový počítač
   dôveruje iba kľúčom Microsoftu, takže so zapnutým Secure Boot a bez
   zapísaného certifikátu firmvér USB kľúč odmietne, niekedy bez akejkoľvek
   správy. Nainštalované zariadenie potrebuje Secure Boot vypnutý v každom
   prípade, pretože jeho vlastný zavádzač podpísaný nie je.
3. Naštartujte z USB kľúča. Počas štartu ukazuje logo LosOS, na počítači
   s BIOS nad bootovacím menu a na počítači s UEFI samostatne. Potom menu pod
   logom ponúkne **BIOS**, **UEFI** alebo **autodetect** na klávesoch 1, 2
   a 3 a po 30 sekundách zvolí autodetect, takže inštalácia prebehne aj bez
   obsluhy. Autodetect zvolí režim, v ktorom naštartoval samotný USB kľúč,
   a to je správne takmer pre každého. [Varianty inštalácie](../types/install-variants)
   túto voľbu vysvetľujú.
4. Čakajte. Inštalátor nájde všetky pevné disky, spojí ich do jedného
   šifrovaného zväzku, stiahne systém a nainštaluje ho a na obrazovke
   označuje každý krok, ktorý práve beží. Na počítači s čipom TPM jeden krok
   do neho zapečatí kľúč disku. Bez neho obrazovka upozorní, že kľúč je na
   bootovacom oddiele.
   [Varianty inštalácie](../types/install-variants#how-the-disk-is-unlocked)
   vysvetľujú, čo to znamená.
5. Keď obrazovka oznámi, že LosOS je nainštalovaný, vyberte USB kľúč
   a stlačením ľubovoľného klávesu reštartujte. Po reštarte obrazovka
   ukazuje lososa a pod ním banner s adresou zariadenia. Otvorte túto adresu
   v prehliadači na ľubovoľnom počítači v tej istej sieti a pokračujte podľa
   [Prvého spustenia](first-run).

![Menu inštalátora pod lososom: stlačte 1 pre BIOS, 2 pre UEFI alebo 3 pre autodetect, ktorý sa po 30 sekundách spustí sám.](@site/docs/img/installer-screen-menu.png)

![Kroky inštalátora pod lososom, bežiaci krok je označený. Tento počítač ešte nemá sieť, takže obrazovka hovorí, aby ste zapojili kábel, a nemá čip TPM, takže hovorí, kam ide kľúč disku.](@site/docs/img/installer-screen-steps.png)

![Koniec inštalácie: LosOS je nainštalovaný, vyberte USB kľúč a stlačením ľubovoľného klávesu reštartujte.](@site/docs/img/installer-screen-done.png)

![Obrazovka zariadenia po prvom štarte: losos a pod ním LosOS v0.4.0 je pripravený, s adresami zariadenia a jeho názvom .local.](@site/docs/img/tty1-banner.png)

## Vyskúšanie vo virtuálnom stroji \{#trying-it-in-a-virtual-machine}

Zariadenie beží v QEMU, virt-manageri aj VirtualBoxe. Treba vedieť dve veci:

- Ak chcete odomykanie cez TPM, dajte VM emulovaný TPM, `swtpm`. Bez neho sa
  zariadenie nainštaluje v režime s keyfile, ktorý tiež funguje.
- Hostiteľ VM zvyčajne nepreloží názov zariadenia `.local`. Použite IP
  adresu z bannera.
  [Inštalácia podrobne](/in-depth/install.md#try-it-in-a-vm-bios) obsahuje
  presné riadky pre QEMU. Na predvádzanie zariadenia je k dispozícii aj
  hotový demo obraz disku, `losos-disk-qcow2`, bez šifrovania a bez
  inštalátora.

### S quickemu \{#with-quickemu}

Repozitár obsahuje konfiguráciu pre
[quickemu](https://github.com/quickemu-project/quickemu), `docs/vm/losos.conf`.
Dá VM 4 jadrá, 8 GiB pamäte a 64 GiB disk, firmvér UEFI s vypnutým Secure
Boot a emulovaný TPM, takže inštalátor zapečatí kľúč disku rovnako ako na
mini-PC. Čísla sú nad minimom, pretože zariadenie spúšťa LosOS cloud a LosOS
Git vo vlastnom clustri a každú noc sa nanovo zostavuje na ten istý disk.

1. Nainštalujte quickemu a `swtpm`.
2. Stiahnite inštalačné ISO z vydania a uložte ho ako
   `docs/vm/losos/losos-installer.iso`.
3. Spustite `quickemu --vm docs/vm/losos.conf`. Disk, premenné firmvéru a stav TPM
   sa uchovávajú vo `docs/vm/losos/`, takže ďalšie štarty zapečatený kľúč nájdu.
4. Keď inštalátor oznámi, že LosOS je nainštalovaný, stlačením klávesu
   reštartujte. Inštalátor dá zavádzač zariadenia na prvé miesto v poradí
   štartu firmvéru, takže VM naštartuje zo svojho disku. Ďalšie spustenia
   quickemu ISO vynechajú, pretože disk už nie je prázdny.
5. Na hostiteľovi otvorte `http://localhost:8080`. quickemu presmeruje port
   8080 na port 80 zariadenia a 8443 na 443.

:::note[quickemu potrebuje KVM]
quickemu potrebuje hardvérovú virtualizáciu, na Linuxe je to KVM, a na
hostiteľovi bez nej sa odmietne spustiť.
:::
