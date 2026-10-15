---
title: Zdieľanie je odmietnuté
sidebar_position: 12
slug: /troubleshooting/sharing-refused
---

# Zdieľanie je odmietnuté

<div className="losos-symptom">

**Čo vidíte:** prepínač na zdieľanie disku, pripojenie k mesh alebo
požičiavanie voľného času sa nedá zapnúť, je sivý, alebo **Použiť** zmenu
odmietne s udaním dôvodu.

</div>

## Dôvody a ktorý je ten váš \{#the-reasons-and-which-one-you-have}

![Panel Mesh bez dostupného edge: prepínače zdieľania sú sivé s textom Vyžaduje dostupný okrajový proxy server.](@site/docs/img/mesh-none.png)

| Čo vidíte                                              | Dôvod                                                                                       | Čo robiť                                                                   |
| ------------------------------------------------------ | ------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------- |
| Prepínač je sivý, „Vyžaduje dostupný okrajový proxy server.“; panel Mesh hlási **Žiadny okrajový proxy server sa nenašiel** | Zariadenie nenašlo žiadny edge. Zdieľanie úložiska a výpočtového výkonu potrebuje miesto, *kam* zdieľať. | [Nenašiel sa žiadny edge](edge-not-found).                                  |
| Použitie zmien odmietnuté: „no edge proxy is reachable from this box, so `losos.sharingMyStorage` cannot be turned on …“ | Edge odpovedal, keď ste prepínač prepli, ale kým ste zmeny použili, zmizol. Zariadenie odmieta sľúbiť miesto spoločnému priestoru, na ktorý nedosiahne. | Počkajte, kým panel Mesh edge znova ukáže (kontroluje to každých 20 sekúnd), a zmeny použite znova. Nič sa nezmenilo. |
| Riadok edge má varovnú značku; Trh hlási „Okrajový proxy server, ktorý toto zariadenie našlo, neprevádzkuje LosOS“ | Edge je vlastný edge nejakej firmy, alebo koreňový kľúč LosOS ešte nie je zverejnený. Zdieľanie cez neho funguje. Obchodovanie cez neho zariadenie zámerne odmieta. | Na obchodovanie použite edge LosOS. Pozrite [oficiálny alebo nie](../types/official-edge#official-or-not). |
| „Najprv sa pripojte k sieti mesh.“                     | Požičiavanie voľného času potrebuje, aby bolo zariadenie v klastri edge.                    | Zapnite **Pripojiť sa k sieti mesh** a potom nastavte hodiny; obe zmeny idú von jedným použitím. |
| Hodiny sú nastavené, no nič sa nikdy nepožičia         | Okno začína a končí v tej istej minúte, alebo zariadenie v ňom nikdy nie je nečinné.         | Nastavte skutočné okno. Popis pod hodinami uvádza, aké je dlhé.            |
| Použitie zmien v mesh zlyhá                            | Zlyhala samotná prestavba, po tom, čo zariadenie zmenu prijalo.                             | [Použitie zmien zlyhá](apply-fails), potom to skúste znova, keď edge beží. |

Zariadenie vždy odmieta iba *zapínanie*. Zariadenie, ktorému edge zmizol, si
zdieľanie, ktoré už malo, ponechá, môže ďalej meniť všetky ostatné
nastavenia a zdieľanie môže kedykoľvek vypnúť.

## Čo zdieľanie nikdy nerobí \{#what-sharing-never-does}

Nikdy nezdieľa vaše vlastné súbory. Zdieľané miesto patrí samostatnému účtu
na zariadení s vlastným šifrovaným adresárom. Vaše súbory a kópie mesh
nemôžu čítať jedny druhé. Vypnutie zdieľania urobí tento adresár znova
nečitateľným, aj pre samotné zariadenie.
