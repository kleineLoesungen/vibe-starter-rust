//! Changelog und Version der laufenden Anwendung.
//!
//! Die Version wird nirgends gepflegt — sie **ist** die oberste Überschrift in
//! `CHANGELOG.md`:
//!
//! ```markdown
//! ## 2026.10.09
//! - Suche in der Benutzerliste
//! - Migration: 0006_projects.sql
//! ```
//!
//! Die Datei wird beim Kompilieren mit `include_str!` eingebettet. Damit ist
//! die Version auch im Container richtig, ohne dass die Datei dorthin kopiert
//! werden muss. Ein Release ist ohnehin ein Neubau.
//!
//! **Darum muss `CHANGELOG.md` beim Build vorhanden sein** — auch im
//! Docker-Build (siehe `Dockerfile` und `.dockerignore`).

pub mod check;

/// Der eingebettete Changelog dieser Installation.
const CHANGELOG: &str = include_str!("../../CHANGELOG.md");

/// Ein Eintrag: eine Version mit ihren Änderungen.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Entry {
    /// Rohfassung, z. B. "2026.10.09" oder "2026.10.09.2".
    pub version: String,
    /// Änderungen ohne den führenden Strich.
    pub items: Vec<String>,
    /// Einträge, die mit "Migration:" beginnen — getrennt, weil sie beim
    /// Deployment eine eigene Rolle spielen.
    pub migrations: Vec<String>,
}

impl Entry {
    /// Version in deutscher Schreibweise: "09.10.2026", mit Zählung "09.10.2026 (2)".
    pub fn label(&self) -> String {
        version_label(&self.version)
    }
}

/// Zerlegt einen Changelog in Einträge, neueste zuerst.
///
/// Zeilen, die in kein Muster passen, werden übergangen. Eine formlose Zeile
/// darf die Anwendung nie zum Absturz bringen — sie steht in einer Datei, die
/// Menschen von Hand pflegen.
pub fn parse(markdown: &str) -> Vec<Entry> {
    let mut entries: Vec<Entry> = Vec::new();

    for line in markdown.lines() {
        let trimmed = line.trim();

        if let Some(version) = version_heading(trimmed) {
            entries.push(Entry {
                version,
                items: Vec::new(),
                migrations: Vec::new(),
            });
            continue;
        }

        // Stichpunkte gehören zur zuletzt gesehenen Version. Steht einer vor
        // der ersten Überschrift, hat er keine Version und entfällt.
        let Some(entry) = entries.last_mut() else {
            continue;
        };
        let Some(text) = bullet(trimmed) else {
            continue;
        };

        match text.strip_prefix_ignore_case("Migration:") {
            Some(rest) => entry.migrations.push(rest.trim().to_string()),
            None => entry.items.push(text.to_string()),
        }
    }

    entries
}

/// "## 2026.10.09" oder "## 2026.10.09.2" — sonst `None`.
///
/// Zusatztext nach der Version ist erlaubt ("## 2026.10.09 — Herbstausgabe").
/// Sonst würde so eine Zeile stillschweigend übergangen, ihre Stichpunkte
/// landeten beim älteren Eintrag und die Anwendung meldete weiter die alte
/// Version — ein Fehler, den niemand bemerkt.
fn version_heading(line: &str) -> Option<String> {
    let rest = line.strip_prefix("##")?.trim();
    // Keine tiefere Ebene: "### …" beginnt nach dem Abschneiden mit "#".
    if rest.starts_with('#') {
        return None;
    }
    let erstes_wort = rest.split_whitespace().next()?;
    is_version(erstes_wort).then(|| erstes_wort.to_string())
}

fn is_version(text: &str) -> bool {
    let teile: Vec<&str> = text.split('.').collect();
    if !(3..=4).contains(&teile.len()) {
        return false;
    }
    let laengen = [4, 2, 2];
    teile.iter().enumerate().all(|(i, teil)| {
        !teil.is_empty()
            && teil.chars().all(|c| c.is_ascii_digit())
            && laengen
                .get(i)
                .is_none_or(|erwartet| teil.len() == *erwartet)
    })
}

/// "- Text" oder "* Text" — sonst `None`.
fn bullet(line: &str) -> Option<&str> {
    let rest = line
        .strip_prefix("- ")
        .or_else(|| line.strip_prefix("* "))?
        .trim();
    (!rest.is_empty()).then_some(rest)
}

/// Kleine Hilfe, damit "Migration:" und "migration:" gleich behandelt werden.
trait StripPrefixIgnoreCase {
    fn strip_prefix_ignore_case(&self, prefix: &str) -> Option<&str>;
}

impl StripPrefixIgnoreCase for str {
    fn strip_prefix_ignore_case(&self, prefix: &str) -> Option<&str> {
        self.get(..prefix.len())
            .filter(|anfang| anfang.eq_ignore_ascii_case(prefix))
            .map(|_| &self[prefix.len()..])
    }
}

/// Vergleicht zwei Datums-Versionen: 2026.10.09 < 2026.10.09.2 < 2026.10.10.
pub fn version_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    parts(a).cmp(&parts(b))
}

fn parts(version: &str) -> [u32; 4] {
    let mut werte = [0u32; 4];
    for (i, teil) in version.split('.').take(4).enumerate() {
        werte[i] = teil.parse().unwrap_or(0);
    }
    werte
}

/// "2026.10.09" → "09.10.2026", "2026.10.09.2" → "09.10.2026 (2)".
pub fn version_label(version: &str) -> String {
    let teile: Vec<&str> = version.split('.').collect();
    if teile.len() < 3 {
        return version.to_string();
    }
    let datum = format!("{}.{}.{}", teile[2], teile[1], teile[0]);
    match teile.get(3) {
        Some(zaehlung) => format!("{datum} ({zaehlung})"),
        None => datum,
    }
}

/// Alle Einträge dieser Installation, neueste zuerst.
pub fn entries() -> Vec<Entry> {
    parse(CHANGELOG)
}

/// Version dieser Installation, oder `None` bei leerem Changelog.
pub fn installed_version() -> Option<String> {
    entries().into_iter().next().map(|e| e.version)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BEISPIEL: &str = "\
# Änderungen

Ein Vorspann, der übergangen werden muss.

## 2026.10.09.2
- Zweite Veröffentlichung am selben Tag

## 2026.10.09
- Suche in der Benutzerliste
* Auch Sternchen zählen als Stichpunkt
- Migration: 0005_settings.sql
- migration: klein geschrieben zählt auch

### Unterabschnitt wird übergangen
- Dieser Punkt gehört noch zu 2026.10.09
";

    #[test]
    fn eintraege_werden_in_reihenfolge_gelesen() {
        let e = parse(BEISPIEL);
        assert_eq!(e.len(), 2);
        assert_eq!(e[0].version, "2026.10.09.2");
        assert_eq!(e[1].version, "2026.10.09");
    }

    #[test]
    fn migrationen_stehen_getrennt() {
        let e = parse(BEISPIEL);
        assert_eq!(
            e[1].migrations,
            vec!["0005_settings.sql", "klein geschrieben zählt auch"]
        );
        assert!(e[1].items.iter().all(|i| !i.starts_with("Migration")));
    }

    #[test]
    fn stichpunkte_mit_strich_und_sternchen() {
        let e = parse(BEISPIEL);
        assert!(
            e[1].items
                .contains(&"Suche in der Benutzerliste".to_string())
        );
        assert!(
            e[1].items
                .contains(&"Auch Sternchen zählen als Stichpunkt".to_string())
        );
    }

    #[test]
    fn tiefere_ueberschriften_eroeffnen_keine_version() {
        let e = parse(BEISPIEL);
        // Der Punkt nach "### Unterabschnitt" gehört weiter zur letzten Version.
        assert!(
            e[1].items
                .contains(&"Dieser Punkt gehört noch zu 2026.10.09".to_string())
        );
    }

    #[test]
    fn formlose_eingaben_ergeben_keine_eintraege() {
        for text in [
            "",
            "nur Fließtext",
            "## ohne Datum\n- Punkt",
            "## 2026.13\n- zu kurz",
            "- Punkt ohne Überschrift",
        ] {
            assert!(parse(text).is_empty(), "unerwartete Einträge in {text:?}");
        }
    }

    #[test]
    fn ueberschrift_mit_zusatztext_zaehlt_trotzdem() {
        let e = parse("## 2026.10.09 — Herbstausgabe\n- Etwas\n");
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].version, "2026.10.09");
        assert_eq!(e[0].items, vec!["Etwas"]);
    }

    #[test]
    fn versionen_werden_richtig_sortiert() {
        use std::cmp::Ordering::*;
        assert_eq!(version_cmp("2026.10.09", "2026.10.10"), Less);
        assert_eq!(version_cmp("2026.10.09", "2026.10.09.2"), Less);
        assert_eq!(version_cmp("2026.10.09.2", "2026.10.09.3"), Less);
        assert_eq!(version_cmp("2026.10.09", "2026.10.09"), Equal);
        assert_eq!(version_cmp("2027.01.01", "2026.12.31"), Greater);
    }

    #[test]
    fn beschriftung_fuer_menschen() {
        assert_eq!(version_label("2026.10.09"), "09.10.2026");
        assert_eq!(version_label("2026.10.09.2"), "09.10.2026 (2)");
        assert_eq!(version_label("kaputt"), "kaputt");
    }

    #[test]
    fn die_eigene_datei_hat_eine_version() {
        // Schlägt fehl, wenn jemand CHANGELOG.md leert oder das Format bricht.
        assert!(
            installed_version().is_some(),
            "CHANGELOG.md hat keinen gültigen Versionseintrag"
        );
    }
}
