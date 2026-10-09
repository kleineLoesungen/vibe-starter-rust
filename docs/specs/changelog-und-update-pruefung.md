# Changelog und Update-Prüfung

## Ziel

Jede Installation weiß, welche Version von ihr läuft, und zeigt Administratoren
unter **System**, was sich seit wann geändert hat. Ist eine Prüfadresse
eingetragen, meldet sie zusätzlich, dass im Repository eine neuere Version
liegt — mitsamt der Änderungen und einer Hervorhebung nötiger Migrationen.

Vorbild ist die Umsetzung in `kleineLoesungen/team-manager`; Format und Ablauf
sind absichtlich dieselben.

## Nicht dabei

- **Keine Seite „Was ist neu" für alle Benutzer** und keine Markierung
  „gelesen". Das wäre eine eigene Tabelle und ein eigener Menüpunkt.
- **Kein automatisches Aktualisieren.** Die Anwendung lädt nichts herunter und
  installiert nichts. Sie zeigt nur an, dass etwas Neueres existiert.
- **Keine privaten Repositories.** Geprüft wird eine öffentlich lesbare
  Rohdatei. Ein Zugangstoken kommt erst, wenn es gebraucht wird.
- **Kein Hinweis außerhalb der Systemseite.** Kein Abzeichen am Menü, keine
  Hinweisleiste.
- **Die Version wird nicht in `/api/v1/health` ausgegeben** — der Endpunkt ist
  ohne Anmeldung erreichbar.

## Verhalten

### Format von `CHANGELOG.md`

```markdown
# Änderungen

## 2026.10.09.2
- Version (Admin): Änderungen der installierten und früherer Versionen

## 2026.10.09
- Suche in der Benutzerliste
- Migration: 0006_projects.sql
```

- Eine Version ist eine Überschrift zweiter Ebene im Format `JJJJ.MM.TT`,
  bei mehreren Veröffentlichungen am selben Tag mit angehängter Zählung
  (`.2`, `.3`).
- Stichpunkte darunter gehören zu dieser Version.
- Ein Stichpunkt, der mit `Migration:` beginnt, ist eine Migration und wird
  getrennt und hervorgehoben angezeigt.
- Zeilen, die in kein Muster passen, werden stillschweigend übergangen. Die
  Anwendung darf an einer formlosen Zeile nicht scheitern.
- **Die installierte Version ist der oberste Eintrag.** Keine Versionsnummer an
  zweiter Stelle, kein `version`-Feld in `Cargo.toml`, das gepflegt werden muss.

Anzeige eines Datums für Menschen: `2026.10.09` → `09.10.2026`, mit Zählung
`2026.10.09.2` → `09.10.2026 (2)`.

### Systemseite (`/admin/system`)

Ein neuer Abschnitt **Version** oberhalb der bestehenden:

| Zeile | Inhalt |
|---|---|
| Installiert | Version der laufenden Instanz, sonst „unbekannt" |
| Neueste | oberste Version an der Prüfadresse, sonst „–" |
| Zuletzt geprüft | Zeitpunkt, bei Fehlschlag mit dem Zusatz „nicht erreichbar" |

Darunter:

- Liegt eine neuere Version vor: deren Einträge, neueste zuerst, Migrationen
  hervorgehoben.
- Immer: die Änderungen der installierten und der **drei vorherigen**
  Versionen.
- Ein Knopf **Jetzt prüfen**, der den Zwischenspeicher übergeht.

Ist keine Prüfadresse gesetzt, entfallen „Neueste" und „Zuletzt geprüft". Die
installierte Version und die Änderungen erscheinen weiterhin.

### Prüfung gegen das Repository

- Adresse aus `UPDATE_CHECK_URL`. **Leer oder nicht gesetzt bedeutet
  abgeschaltet** — das ist die Vorgabe im Kit, damit abgeleitete Projekte nicht
  die Versionen des Kits anzeigen und nicht ungefragt nach außen sprechen.
- Beispiel für ein eigenes Repository:
  `https://raw.githubusercontent.com/<konto>/<repo>/master/CHANGELOG.md`
- Abgerufen wird höchstens **alle 24 Stunden**, beim Aufruf der Systemseite.
  Der Knopf „Jetzt prüfen" erzwingt einen Abruf.
- **Zeitlimit 3 Sekunden.** Die Systemseite darf nie an einer langsamen
  Verbindung hängen.
- Schlägt der Abruf fehl, bleiben die zuletzt bekannten Einträge stehen; die
  Zeile „Zuletzt geprüft" weist den Fehlschlag aus. Es gibt **keine**
  Fehlerseite: Eine nicht erreichbare Prüfadresse ist kein Fehler der
  Anwendung.
- Abgerufen werden höchstens die 100 obersten Einträge.

## Berechtigungen

- Systemseite und Prüfung: **nur Administratoren**, über den vorhandenen
  `AdminUser`-Extractor. Der Knopf ist ein `POST` und läuft damit durch die
  Herkunftsprüfung.
- Die installierte Version steht zusätzlich klein in der **Fußzeile** und ist
  damit für alle angemeldeten Benutzer sichtbar. Bewusste Entscheidung: Sie
  hilft bei Rückfragen („welche Version hast du?") und ist kein Geheimnis. Ohne
  Anmeldung ist sie nirgends zu sehen.
- Keine Scopes, kein API-Zugang.

## Datenmodell

Neue Migration **`0005_settings.sql`** (nächste freie Nummer):

```sql
CREATE TABLE settings (
    key        TEXT        PRIMARY KEY,
    value      TEXT        NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

Ein allgemeiner Schlüssel-Wert-Speicher für Dinge, die zur Laufzeit entstehen
und keine eigene Tabelle rechtfertigen. Erster und vorerst einziger Eintrag:

- Schlüssel `update_check`, Wert ein JSON-Text mit `url`, `checked_at`, `ok`
  und `entries`.

`value` ist bewusst `TEXT` und nicht `JSONB`: Für `JSONB` bräuchte sqlx das
Feature `json`, und an den festgenagelten sqlx-Einstellungen soll ohne Not
nichts geändert werden. Das Parsen übernimmt `serde_json`.

Die Prüfadresse steht im Wert, damit ein Wechsel der Adresse den alten Stand
nicht als „neuere Version" ausgibt.

## Betroffene Stellen

**Neu**

| Datei | Inhalt |
|---|---|
| `CHANGELOG.md` | Erster Eintrag mit dem heutigen Datum |
| `src/changelog/mod.rs` | Eingebettetes `CHANGELOG.md`, Parser, Versionsvergleich, Beschriftung — reine Logik, gut testbar |
| `src/changelog/check.rs` | Abruf der Prüfadresse, Zwischenspeicher, Statusermittlung |
| `src/domain/settings.rs` | `get`/`set` auf der neuen Tabelle |
| `migrations/0005_settings.sql` | siehe oben |

**Geändert**

| Datei | Änderung |
|---|---|
| `src/config.rs` | `update_check_url: Option<String>` (leer = aus) |
| `src/web/system.rs` | Versionsabschnitt füllen, Handler für „Jetzt prüfen" |
| `src/web/mod.rs` | Route `POST /admin/system/update-check` |
| `src/templates.rs` | Felder der `SystemPage`, Zugriff auf die Version im `Layout` |
| `templates/pages/admin_system.html` | Abschnitt „Version" |
| `templates/layout/app.html` | Version in der Fußzeile |
| `Cargo.toml` | `reqwest`, `rustls` (siehe Risiken) |
| `Dockerfile`, `.dockerignore` | `CHANGELOG.md` in den Build (siehe Risiken) |
| `.env.example`, `docs/DEPLOY.md`, `README.md`, `CLAUDE.md` | `UPDATE_CHECK_URL`, Pflege des Changelogs |
| `scripts/new-project.sh` | `CHANGELOG.md` für das neue Projekt zurücksetzen |

Die Schichtregel bleibt gewahrt: Der Parser ist reine Logik, der Abruf ist
Infrastruktur und liegt wie `src/mail/` neben der Domäne, der Zugriff auf die
Tabelle liegt in `src/domain/`. Kein `Request`, kein `StatusCode`, kein
Template in der Domäne.

## Tests

**Parser und Versionen** (`src/changelog/mod.rs`)

- Mehrere Versionen mit Stichpunkten, neueste zuerst
- `Migration:`-Zeilen landen getrennt, Groß-/Kleinschreibung egal
- Formlose Zeilen, leere Datei, Datei ohne Überschrift — nie ein Absturz
- Vergleich: `2026.10.09` < `2026.10.09.2` < `2026.10.10`
- Beschriftung inklusive Zählung

**Zwischenspeicher und Status** (`src/changelog/check.rs`, gegen eine echte Datenbank)

- Erster Abruf schreibt den Eintrag, zweiter Aufruf innerhalb von 24 Stunden
  ruft nicht erneut ab
- „Jetzt prüfen" übergeht den Zwischenspeicher
- Fehlgeschlagener Abruf behält die alten Einträge und setzt `ok` auf falsch
- Gewechselte Prüfadresse verwirft die alten Einträge
- Keine Adresse gesetzt: Status „abgeschaltet", kein Netzzugriff

**HTTP** — gegen einen **lokalen Testserver** (ein kleiner Axum-Router im Test),
nie gegen GitHub. `just check` darf nicht vom Internet abhängen.

- Antwort 200 mit gültigem Changelog
- Antwort 404 und abgebrochene Verbindung
- Zeitlimit greift

**Oberfläche** (`tests/http.rs`)

- `/admin/system` zeigt die installierte Version
- `POST /admin/system/update-check` ist für Nicht-Administratoren gesperrt (403)
- Fußzeile enthält die Version

## Annahmen und Risiken

**Krypto-Anbieter von rustls — der heikelste Punkt.** Das Kit nutzt überall
`rustls` mit **ring** (sqlx, lettre). `reqwest` zieht mit dem Feature `rustls`
dagegen **aws-lc-rs** herein. Zwei Anbieter im selben Programm führen dazu,
dass rustls den Standardanbieter nicht mehr eindeutig bestimmen kann — das
fällt erst **zur Laufzeit** beim ersten TLS-Aufbau auf, nicht beim Kompilieren.
Deshalb:

- `reqwest` mit `default-features = false` und dem Feature
  `rustls-no-provider`,
- `rustls` als direkte Abhängigkeit mit dem Feature `ring`,
- einmalig beim Start den ring-Anbieter als Standard setzen.

Beim Umsetzen wird geprüft, dass `cargo tree -e features -i rustls` weiterhin
nur `ring` zeigt und kein `aws-lc-rs`.

**`CHANGELOG.md` muss beim Kompilieren vorliegen.** Mit `include_str!` landet
die Datei in der Binary; im Container braucht es sie dann zur Laufzeit nicht.
Aber: `.dockerignore` schließt derzeit `*.md` aus, und das `Dockerfile` kopiert
nur `src/`, `templates/` und `migrations/`. Ohne Anpassung **scheitert der
Container-Build** — die Tests würden das nicht bemerken. Deshalb gehört
`docker build` zur Abnahme dieser Funktion.

**Ausgehende Verbindung.** Eine Installation mit gesetzter Prüfadresse ruft
höchstens einmal am Tag GitHub auf und verrät dabei ihre IP-Adresse. Im Kit ist
das abgeschaltet; in `docs/DEPLOY.md` wird es benannt, damit es niemanden
überrascht.

**Abgeleitete Projekte.** Ein Projekt aus dem Kit erbt sonst den Changelog des
Kits und zeigte dessen Versionen an. `scripts/new-project.sh` ersetzt die Datei
deshalb durch einen frischen ersten Eintrag mit dem Datum des Tages.

**Pflegeaufwand.** Der Changelog wird von Hand geschrieben. Wird er vergessen,
bleibt die Versionsanzeige stehen — falsch wird sie nie, nur alt. Eine
Erzeugung aus Git-Commits ist bewusst nicht vorgesehen: Commit-Titel sind für
Entwickler geschrieben, Changelog-Einträge für Betreiber.

**Zeitzone.** „Zuletzt geprüft" wird wie alle Zeitstempel im Projekt über
`ui::datetime` ausgegeben und damit im Browser in Ortszeit umgerechnet.

**Offen gelassen:** private Repositories (Token), Signaturprüfung der
Prüfadresse, Benachrichtigung per E-Mail bei neuer Version.

---

## Stand der Umsetzung (2026-10-09)

Umgesetzt wie beschrieben. Abweichungen und Ergänzungen:

- Changelog-Einträge sind **reiner Text**; Markdown darin wird nicht
  ausgewertet. Das steht jetzt als Hinweis in `CHANGELOG.md`.
- `settings` bekam zusätzlich `updated_at`, das beim Schreiben mitgeführt wird.
- Geprüft: `cargo tree -e features -i rustls` zeigt ausschließlich `ring`.
- Geprüft: `docker build` übernimmt `CHANGELOG.md` (`.dockerignore` mit
  Ausnahme, zusätzliche `COPY`-Zeile).
- Umleitungen folgt der Abruf nur zu `https` und höchstens dreimal; der
  Antwortkörper wird in Stücken gelesen und bei `MAX_BYTES` verworfen. Beides
  kam aus der Sicherheitsprüfung: die Prüfadresse bestimmt der Betreiber, das
  Umleitungsziel und die Antwortgröße aber die Gegenstelle.
- Die Version steht nur für **Angemeldete** in der Fußzeile. Vorher war sie
  über `/styleguide` auch anonym lesbar — gegen die eigene Vorgabe.
- Überschriften mit Zusatztext ("## 2026.10.09 — Herbstausgabe") gelten als
  Version. Vorher wurden sie stillschweigend übergangen, und die Anwendung
  meldete weiter die alte Version.
- 112 Tests grün, davon 10 für die Update-Prüfung gegen einen lokalen Server.
  **Nicht** abgedeckt: das Zeitlimit von 3 s — ein Test dafür müsste drei
  Sekunden warten; das ist den Preis bei jedem `just check` nicht wert.
