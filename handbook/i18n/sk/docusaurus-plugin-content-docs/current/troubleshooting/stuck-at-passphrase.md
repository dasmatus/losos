---
title: Zaseknuté pri výzve na heslo disku
sidebar_position: 6
slug: /troubleshooting/stuck-at-passphrase
---

# Zaseknuté pri výzve na heslo disku

<div className="losos-symptom">

**Čo vidíte:** namiesto adresy zariadenia panel pod lososom hlási
`Please enter passphrase for disk persist` (alebo niečo podobné) a čaká. Na
počítači, ktorý štartuje v textovom režime, je ten istý riadok na čiernej
obrazovke. Pri bežnom štarte zariadenie heslo disku nikdy nepýta.

</div>

![Zariadenie zastavené pri štarte: panel pod lososom žiada Please enter passphrase for disk persist.](@site/docs/img/passphrase-prompt.png)

## Prečo sa to stane \{#why-it-happens}

Zariadenie nedokázalo odomknúť šifrovaný zväzok spôsobom, akým ho podľa
inštalácie odomknúť malo:

| Inštalácia           | Čo sa pokazilo                                                                                                            |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| **TPM**              | Čip už kľúč nedrží alebo ho už nevydá. TPM vo firmvéri bol vymazaný alebo resetovaný, čip bol vymenený, alebo bol disk presunutý do iného počítača. Vo VM emulovaný TPM naštartoval s iným adresárom stavu ako pri inštalácii. |
| **Súbor s kľúčom**   | Bootovací oddiel súbor s kľúčom stratil alebo poškodil, alebo bol disk naklonovaný iba čiastočne.                          |
| **Akákoľvek, pred 2. októbrom 2026** | Chyba v starších inštalátoroch naformátovala zväzok jedným kľúčom a pokúšala sa ho odomknúť iným, pretože jeden zatúlaný bajt sa cestou stratil. Každé také zariadenie sa pri prvom štarte zastavilo pri tejto výzve. Opravil to pull request #47. Inštalujte z aktuálneho ISO. |

## Čo môžete urobiť \{#what-you-can-do}

:::note[Žiadne heslo na zadanie]
Nie je žiadne heslo, ktoré by sa dalo napísať. Kľúč je náhodný reťazec so
4096 znakmi, ktorý nikto nikdy nevidel. Záložná kópia je *vnútri*
šifrovaného zväzku, takže ho otvoriť nedokáže.
:::

1. **Raz vypnite a zapnite.** TPM, ktorý počas uponáhľaného štartu
   neodpovedal, niekedy odpovie nabudúce.
2. **Vo VM** pred spustením VM spustite emulovaný TPM s tým istým adresárom
   stavu, aký sa použil pri inštalácii.
3. **Na skutočnom hardvéri** skontrolujte, či je TPM vo firmvéri (Intel PTT
   alebo AMD fTPM) stále zapnutý a či ho nevymazala aktualizácia firmvéru
   alebo „obnovenie predvolených hodnôt“.
4. Inak **preinštalujte**. Bez kľúča sú dáta na zväzku nečitateľné, presne
   ako šifrovanie sľubovalo. Vaše súbory sú v kópii, ktorú si uchovávate
   inde.

## Ako tomu nabudúce predísť \{#preventing-the-next-time}

Majte ďalšiu kópiu svojich dát. Nevymazávajte TPM vo firmvéri. Vo VM
uchovávajte adresár stavu TPM spolu s obrazom disku. Stránka
[Varianty inštalácie](../types/install-variants#how-the-disk-is-unlocked)
vysvetľuje kompromis, ktorý režim TPM robí.
