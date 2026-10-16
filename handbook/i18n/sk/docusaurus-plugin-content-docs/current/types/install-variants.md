---
title: Varianty inštalácie
sidebar_position: 4
---

# Varianty inštalácie

Dve rozhodnutia urobí inštalátor a jedno vy. Všetky tri zapíše do súboru na
zariadení, `modules/install-target.nix`. Preinštalovanie z toho istého média
ich zopakuje a nočná aktualizácia ich nikdy nezvráti.

## Ako sa odomyká disk \{#how-the-disk-is-unlocked}

Každé zariadenie má šifrovaný disk. Inštalátor ho naformátuje bez obsluhy
náhodným kľúčom, ktorý sám vytvorí. Varianty sa líšia v tom, kde je tento
kľúč pri štarte.

| Variant                    | Kedy                                                     | Čo to znamená                                                                                                                         |
| -------------------------- | -------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| **TPM** (predvolený)       | Inštalátor našiel čip TPM 2.0                            | Kľúč je zapečatený v čipe. Bootovací oddiel nenesie žiadne tajomstvo. Samotný disk, vybratý alebo naklonovaný, je nečitateľný. Zariadenie stále štartuje bez obsluhy. |
| **Súbor s kľúčom**         | Žiadny čip, alebo `losos-ctl install --no-tpm`            | Kľúč leží na nešifrovanom bootovacom oddiele, aby zariadenie mohlo štartovať bez obsluhy. Kto vezme disk, má aj dáta. Stačí pre VM alebo testovacie zariadenie. |

Inštalátor vypíše, ktorý variant zvolil: `unlock: TPM` alebo `unlock: keyfile
in the initrd`. Kľúč v TPM **nie je** zámerne viazaný na merania firmvéru.
Zariadenie si samo aktualizuje firmvér, zavádzač aj jadro a nemá shell, cez
ktorý by sa dalo zotaviť zo zablokovania. Cena za to je, že zlodej, ktorý
vezme celé zariadenie aj s čipom, ho stále dokáže naštartovať.
[Bezpečnostný model](../reference/security-model) to hovorí priamo.
[TPM a kľúč disku](../reference/tpm) vysvetľuje, čo stráca zariadenie bez
čipu a ako ho na zariadenie s čipom presunúť.

![Dôkaz z inštalácie vo virtuálnom stroji s emulovaným TPM: šifrovaný zväzok má v slote kľúča 1 token systemd-tpm2 a súbor s kľúčom zostáva iba na šifrovanom zväzku.](@site/docs/img/install-done-tpm.png)

:::note[Pri štarte žiadne heslo]
Zariadenie si pri štarte nikdy nepýta heslo. Ak sa na heslo pýta to vaše, niečo
nie je v poriadku. Pozrite si [Zaseknuté na výzve na heslo](../troubleshooting/stuck-at-passphrase).
:::

## UEFI alebo BIOS \{#uefi-or-bios}

Toto je jediná otázka, ktorú inštalátor kladie. **UEFI** dostane
systemd-boot. **BIOS** dostane GRUB a 1 MiB bootovací oddiel na prvom disku.
**Autodetect** zvolí režim, v ktorom bol naštartovaný USB kľúč, a ponuka ho
po 30 sekundách vyberie sama, takže inštalácia prebehne aj bez obsluhy. UEFI
vyžaduje, aby bol v režime UEFI naštartovaný aj samotný USB kľúč. Na kľúči
naštartovanom v režime BIOS inštalátor UEFI odmietne skôr, ako sa dotkne
akéhokoľvek disku. BIOS je hlavne pre virtuálne stroje. V režime UEFI je
USB kľúč podpísaný a firmvér so zapísaným certifikátom LosOS ho overí.
Vlastný zavádzač nainštalovaného zariadenia podpísaný nie je, takže zariadenie
potrebuje vypnutý Secure Boot. Pozrite si
[Secure Boot a podpísané médiá](../start/secure-boot).

![Ponuka firmvéru inštalátora pod lososom: 1 BIOS, 2 UEFI, 3 Autodetect, ktorá sa po 30 sekundách spustí sama.](@site/docs/img/installer-screen-menu.png)

## Tri médiá \{#the-three-media}

| Médium                       | Veľkosť   | Použitie                                                                                     |
| ---------------------------- | --------- | -------------------------------------------------------------------------------------------- |
| **Inštalačné ISO**           | malé      | Bežné médium, priložené ku každému vydaniu. Cieľový počítač si systém, ktorý inštaluje, stiahne. |
| **ISO s celým systémom**     | veľké     | Nesie zostavený systém, takže inštalácia ho kopíruje namiesto sťahovania. Zostavte si ho sami. |
| **Demo obraz disku** (QCOW2) | veľký     | Predinštalovaný virtuálny disk pre QEMU alebo virt-manager. Bez šifrovania, bez inštalátora. Na ukážky. |

## Rozšírenie do zvyšku disku \{#growing-into-the-rest-of-the-disk}

Inštalátor zámerne nechá 10 % disku nevyužitých. Zariadenie sa do tejto
rezervy môže neskôr rozšíriť bez toho, aby niekto otváral skrinku. Zaberie ju
tlačidlo **Použiť rezervu…** na paneli Úložisko. Pozrite si
[Zväčšenie disku](../manual/disk-growth).

![Disk zvonka dovnútra: jedna skupina zväzkov s logickým zväzkom, v ňom šifrovacia vrstva a v nej súborový systém, vedľa nich 10 % rezerva.](@site/docs/img/disk-growth-sk.svg)
