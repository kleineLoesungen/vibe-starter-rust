# Änderungen

Neueste Version oben. Eine Überschrift je Version im Format `JJJJ.MM.TT`,
bei mehreren Veröffentlichungen am selben Tag mit angehängter Zählung (`.2`).
Die oberste Überschrift ist die Version, die die Anwendung von sich meldet.

Einträge sind reiner Text — Markdown darin wird nicht ausgewertet.
Nach der Version darf Zusatztext stehen (`## 2026.10.09 — Herbstausgabe`).
Zeilen, die mit `Migration:` beginnen, werden getrennt hervorgehoben — sie
müssen vor dem Deployment eingespielt werden (das erledigt der Start selbst).

## 2026.10.09
- Version und Änderungen unter System sichtbar, Version zusätzlich in der Fußzeile
- Hinweis auf neuere Versionen, wenn eine Prüfadresse hinterlegt ist
- Migration: 0005_settings.sql
