---
name: spec
description: Erst Spezifikation, dann Code. Klärt offene Entscheidungen, schreibt eine Spezifikation nach docs/specs/ und wartet auf Freigabe, bevor Code entsteht. Für neue Funktionen und Umbauten, die mehrere Dateien oder das Datenmodell berühren — nicht für kleine Korrekturen.
argument-hint: [kurze Beschreibung der Funktion]
---

Du planst eine Funktion für dieses Projekt: **$ARGUMENTS**

Schreibe in diesem Durchgang **keinen Produktionscode**. Das Ergebnis ist eine
Spezifikation, über die entschieden wird.

## 1. Erst lesen, dann fragen

Verschaffe dir Klarheit, bevor du fragst:

- `CLAUDE.md` für die geltenden Regeln
- `docs/RECIPES.md` für das passende Muster
- den Code der Stellen, die betroffen sind

Fragen, deren Antwort im Projekt steht, nicht stellen.

## 2. Offene Entscheidungen klären

Stelle **höchstens fünf** Fragen, und nur solche, bei denen verschiedene
Antworten zu ernsthaft verschiedener Arbeit führen. Typisch sind:

- Wer darf das? Rolle, Scope, nur eigene Daten oder systemweit?
- Was passiert mit vorhandenen Daten? Braucht es eine Migration?
- Oberfläche, Schnittstelle oder beides?
- Was ist ausdrücklich **nicht** Teil davon?

Für alles andere triffst du eine begründete Annahme und schreibst sie in die
Spezifikation. Keine Fragen zu Dingen, die eine naheliegende Vorgabe haben.

## 3. Spezifikation schreiben

Lege `docs/specs/<kurzname>.md` an, in dieser Gliederung:

```markdown
# <Titel>

## Ziel
Ein bis drei Sätze: Was soll danach möglich sein, das es jetzt nicht ist?

## Nicht dabei
Was bewusst draußen bleibt. Verhindert späteres Ausufern.

## Verhalten
Die Abläufe aus Sicht der Benutzerin, inklusive Fehlerfällen.

## Berechtigungen
Welche Rolle, welcher Scope. Eigene Daten oder systemweit?

## Datenmodell
Neue oder geänderte Tabellen und Spalten, mit der Nummer der neuen Migration
(die nächste freie — `ls migrations/`).

## Betroffene Stellen
Dateien je Schicht: domain/, web/, api/, templates/, Konfiguration.

## Tests
Welche Fälle abgesichert werden, besonders die Grenzfälle.

## Annahmen und Risiken
Getroffene Annahmen, offene Punkte, mögliche Nebenwirkungen.
```

Die Spezifikation beschreibt **Verhalten**, nicht Zeile für Zeile den Code.
Wenn eine Entscheidung gegen eine Regel aus `CLAUDE.md` verstößt, benenne das
ausdrücklich und begründe es.

## 4. Anhalten

Fasse die Spezifikation in wenigen Sätzen zusammen, nenne die offenen
Annahmen und **warte auf Freigabe**. Nicht ungefragt mit der Umsetzung
weitermachen.

## 5. Nach der Freigabe

Erst dann umsetzen: den passenden Rezepten aus `docs/RECIPES.md` folgen, Tests
schreiben, `just check` laufen lassen und das Ergebnis nennen. Weicht die
Umsetzung von der Spezifikation ab, die Datei nachziehen — sie soll
beschreiben, was tatsächlich gebaut wurde.
