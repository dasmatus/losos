---
title: Technická dokumentácia (KOP)
sidebar_position: 0
sidebar_label: Úvodná strana
slug: /project/kop
description: Komplexná odborná práca k LosOS v slovenčine, s anglickým zhrnutím a odkazom na PDF na odovzdanie.
---

# LosOS: technická dokumentácia projektu

**Komplexná odborná práca (KOP)** k praktickej časti odbornej zložky maturitnej
skúšky. Stredná priemyselná škola elektrotechnická, Hálova 16, Bratislava,
školský rok 2026/2027. Autor: Matúš Maštena. Konzultant: *(doplní autor)*.

:::info[English summary]
This chapter is the Slovak technical documentation handed in for the author's
matriculation project (the KOP). It follows the four official requirements:
(1) describe the system's main functions and options, (2) choose a kernel,
install the system, configure it and verify it, (3) prepare a price offer for
the hardware and software, (4) write the technical documentation. The English
handbook around it is the source for most of its content; the chapter adds
the kernel rationale, the verification procedure and the price offer. A PDF
of the whole chapter is built from these pages by the handbook's CI job and
published at [losos.dasmat.us/kop/LosOS-KOP.pdf](https://losos.dasmat.us/kop/LosOS-KOP.pdf).
:::

## Abstrakt

LosOS je bezstavový operačný systém postavený na NixOS, ktorý z vyradeného
mini-PC urobí domáci alebo firemný cloud, ktorý sa sám aktualizuje a spravuje
sa výlučne cez webové rozhranie: bez SSH a bez prihlasovania do systému.
Koreňový súborový systém je pri každom štarte postavený nanovo z jedného
deklaratívneho popisu, trvalé dáta ležia na šifrovanom zväzku s kľúčom v čipe
TPM. Na boxe beží Nextcloud (LosOS cloud) a Forgejo (LosOS Git) ako záťaž
v lokálnom Kubernetes clustri a voliteľne sa box pripája k sieti ďalších
boxov (mesh), ktorej požičiava voľný disk a procesor. Táto práca opisuje
funkcie a možnosti systému, zdôvodňuje výber jadra, dokumentuje inštaláciu,
základnú konfiguráciu a jej overenie, uvádza cenovú ponuku hardvéru a softvéru
a opisuje, ako je projekt zdokumentovaný a testovaný.

## Abstract

LosOS is a stateless operating system built on NixOS that turns a repurposed
mini-PC into a home or company cloud which updates itself and is administered
only through a web page: no SSH, no shell logins. The root filesystem is
rebuilt on every boot from one declarative description; durable data lives on
an encrypted volume whose key is kept in the TPM chip. The box runs Nextcloud
(LosOS cloud) and Forgejo (LosOS Git) as workloads in a local Kubernetes
cluster and can optionally join a mesh of other boxes, lending them its spare
disk and CPU. This document describes the system's functions and options,
justifies the kernel choice, documents the installation, the basic
configuration and its verification, gives a price offer for the hardware and
software, and describes how the project is documented and tested.

## Zadanie a kde je zodpovedané

| Bod zadania                                                                          | Kapitola                                                                 |
| ------------------------------------------------------------------------------------ | ------------------------------------------------------------------------ |
| 1. Preskúmajte a opíšte hlavné funkcionality a možnosti vybraného operačného systému | [2 Funkcionality a možnosti](./funkcionality.md), [3 Architektúra](./architektura.md) |
| 2a. Vyberte vhodný kernel                                                            | [4 Výber kernelu](./kernel.md)                                           |
| 2b. Nainštalujte operačný systém                                                     | [5 Inštalácia](./instalacia.md)                                          |
| 2c. Vykonajte základnú konfiguráciu                                                  | [6 Základná konfigurácia](./konfiguracia.md)                             |
| 2d. Overte správnosť konfigurácie a funkčnosť                                        | [7 Overenie](./overenie.md): mesh dvoch boxov s edge aj bez neho, testy, kontrolné zoznamy |
| 3. Spracujte cenovú ponuku hardvérového a softvérového vybavenia (navrhnutý dovetok: *v prípade reálneho nasadenia*, čaká na potvrdenie) | [8 Cenová ponuka](./cenova-ponuka.md): dve konfigurácie reálneho nasadenia |
| 4. Vypracujte technickú dokumentáciu                                                 | tento dokument a [9 Dokumentácia projektu](./dokumentacia.md)            |

## Ako čítať

Dokument je písaný pre komisiu a konzultanta, teda pre technicky zdatného
čitateľa, ktorý projekt nepozná. Každá kapitola začína tým, čo systém robí,
a až potom vysvetľuje, prečo je to tak navrhnuté. Odkazy na súbory v
repozitári (`modules/boot.nix` a podobne) smerujú do
[github.com/dasmatus/losos](https://github.com/dasmatus/losos); každý
významný súbor má v hlavičke komentár so zdôvodnením, prečo vyzerá tak, ako
vyzerá. Obrázky sú snímky skutočného systému z ukážok vo virtuálnom stroji.

Zdrojom tohto dokumentu sú stránky Markdown v priečinku
`handbook/docs/project/kop/` repozitára; z nich sa pri každej zmene zostaví aj
PDF na odovzdanie (kapitola 9 hovorí ako).
