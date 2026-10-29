---
title: Zdieľanie a spoločné úpravy
sidebar_position: 4.45
slug: /manual/sharing-and-editing-together
---

# Zdieľanie a spoločné úpravy

![Súbory v LosOS Cloud.](@site/docs/img/cloud-files-sk.png)

V [LosOS Cloud](losos-cloud) sa textové súbory otvárajú v editore, do
ktorého môže písať viac ľudí naraz. Každý vidí hore tváre ostatných, ich
kurzory v ich vlastnej farbe s menom vedľa a pod textom, kto ho upravil
naposledy. Takto sa otvára obyčajný text: `.txt`, `.csv`, `.json`, `.yaml`
a podobné. Súbor Markdown sa otvorí v
[Poznámkach](notes-and-knowledge-bases) a ich **Upraviť** vás privedie sem.

Na jednom zariadení to funguje pre každého, kto môže súbor otvoriť. Keď je
zariadenie v sieti zariadení (mesh), môžete súbor alebo priečinok zdieľať aj
s človekom na inom zariadení a písať doň spolu na oboch zariadeniach.

## Otvorte súbor a upravujte ho \{#open-a-file-to-edit-it}

V časti **Súbory** kliknite na textový súbor. Editor ukazuje:

- ľudí, ktorí sú práve v súbore, ako okrúhle tváre;
- **Naživo**, kým je váš prehliadač pripojený k zariadeniu;
- kto súbor naposledy zmenil a kedy.

Súbor na zariadení sa uloží chvíľu potom, ako niekto prestane písať, takže
zmenu vidia aj aplikácie pre počítač a telefón. Každé uloženie je verzia, ku
ktorej sa môžete vrátiť. Ak súbor zmení aplikácia, kým ho majú ľudia
otvorený, zmena sa objaví v ich editoroch.

## Zdieľajte s niekým na inom zariadení \{#share-with-someone-on-another-box}

Každý má adresu v tvare `meno@zariadenie`, napríklad `eva@kitchenbox.local`.
Vašu nájdete dole v okne zdieľania.

1. V časti **Súbory** zaškrtnite súbor alebo priečinok a stlačte
   **Zdieľať**.
2. Napíšte adresu druhého človeka.
3. Vyberte **Môže upravovať** alebo **Môže zobraziť** a stlačte
   **Zdieľať**.

Zoznam v tom istom okne ukazuje každého, s kým ste zdieľali, a stav ponuky:
čaká, prijaté alebo odmietnuté. Tam aj zmeníte, čo môže robiť, alebo
krížikom zdieľanie zrušíte. Zrušenie súbor zoberie aj z jeho zariadenia.

## Čo dostanete \{#what-you-receive}

Súbory zdieľané s vami sú v časti **Zdieľané s vami**. Nový čaká, kým ho
**Prijmete** alebo **Odmietnete**. Potom:

- textový súbor sa otvorí v editore, spolu s jeho vlastníkom a každým, s kým
  je zdieľaný;
- priečinok sa otvorí ako zoznam, jeho súbory sa odtiaľ otvoria alebo
  stiahnu;
- každý iný súbor sa stiahne.

Súbor zostáva na zariadení vlastníka. Vaše zariadenie si drží kópiu
textového súboru, ktorý upravujete, takže môžete písať ďalej, aj keď je
druhé zariadenie mimo dosahu. Editor to vtedy povie a **Zdieľané s vami**
ukáže, že vaše zmeny čakajú. Keď sa zariadenia znova spoja, zmeny z oboch
strán sa spoja bez toho, aby sa stratil čo i len riadok.

## Ako sa zariadenia nájdu \{#how-boxes-find-each-other}

Zariadenie odpovedá iným zariadeniam na `https://<name>.local/mesh/v1/link`
alebo na verejnom mene, ktoré mu dá edge. Každé zariadenie má vlastný kľúč.
Keď sa dve zariadenia ozvú prvýkrát, každé si zapamätá kľúč druhého a odvtedy
odmietne zariadenie, ktoré pod tým istým menom príde s iným. Na tejto adrese
sa nikto neprihlasuje; o tom, kto hovorí, rozhodujú kľúče.

## Nastavenia \{#settings}

Sú na paneli **Rozšírené**:

- `losos.cloud.mesh.enable` zapína a vypína zdieľanie s inými zariadeniami.
  Je zapnuté, keď zariadenie zdieľa svoje úložisko alebo výpočtový výkon so
  sieťou. Spoločné úpravy na jednom zariadení fungujú v oboch prípadoch.
- `losos.cloud.mesh.name` je meno, na ktorom iné zariadenia toto zariadenie
  zastihnú, časť za `@` v adresách ľudí. V sieti zariadenia je to
  `<name>.local`, cez edge je to meno, ktoré zariadeniu dá edge. Iné
  zariadenia si kľúč tohto zariadenia pamätajú pod týmto menom, takže nové
  meno znamená zdieľať znova.
- `losos.cloud.mesh.selfSigned` dovolí zariadeniu prijať certifikát iného
  zariadenia, ktorý si podpísalo samo. Je zapnuté, pokiaľ zariadenie nie je
  dostupné cez edge.
