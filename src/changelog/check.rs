//! Abgleich mit dem Changelog im Repository.
//!
//! Die Anwendung lädt nichts herunter und installiert nichts. Sie liest eine
//! öffentlich lesbare Rohdatei und zeigt Administratoren, dass es eine neuere
//! Version gibt.
//!
//! Zwei Dinge halten das harmlos:
//!
//! * **Ohne `UPDATE_CHECK_URL` passiert nichts.** Das ist die Vorgabe im Kit —
//!   sonst würde jedes abgeleitete Projekt die Versionen des Kits anzeigen und
//!   ungefragt nach außen sprechen.
//! * **Ein Fehlschlag ist kein Fehler.** Keine Fehlerseite, kein Abbruch; der
//!   zuletzt bekannte Stand bleibt stehen und wird als veraltet gekennzeichnet.

use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::time::Duration;
use time::OffsetDateTime;

use super::{Entry, installed_version, version_cmp};
use crate::domain::settings;
use crate::error::Error;

/// Schlüssel in der Tabelle `settings`.
const CACHE_KEY: &str = "update_check";
/// So lange gilt ein Abruf als frisch.
const TTL: Duration = Duration::from_secs(24 * 60 * 60);
/// Die Systemseite darf nie an einer langsamen Verbindung hängen.
const TIMEOUT: Duration = Duration::from_secs(3);
/// Mehr Einträge zeigt ohnehin niemand an.
const MAX_ENTRIES: usize = 100;
/// Schutz vor einer Adresse, die etwas sehr Großes ausliefert.
const MAX_BYTES: usize = 256 * 1024;

/// Was die Systemseite anzeigt.
#[derive(Debug, Clone)]
pub struct UpdateStatus {
    /// `false`, wenn keine Prüfadresse gesetzt ist.
    pub enabled: bool,
    pub installed: Option<String>,
    pub latest: Option<String>,
    /// Einträge, die neuer sind als die installierte Version.
    pub newer: Vec<Entry>,
    /// `false`, wenn der letzte Abruf fehlschlug.
    pub ok: bool,
    pub checked_at: Option<OffsetDateTime>,
}

impl UpdateStatus {
    /// Liegt etwas Neueres vor?
    pub fn update_available(&self) -> bool {
        !self.newer.is_empty()
    }

    /// Installierte Version in deutscher Schreibweise.
    pub fn installed_label(&self) -> Option<String> {
        self.installed.as_deref().map(super::version_label)
    }

    /// Neueste bekannte Version in deutscher Schreibweise.
    pub fn latest_label(&self) -> Option<String> {
        self.latest.as_deref().map(super::version_label)
    }
}

/// Zwischengespeichertes Ergebnis. Die Adresse steht mit darin, damit ein
/// Wechsel den alten Stand nicht als „neuere Version" ausgibt.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Cache {
    url: String,
    /// Unix-Sekunden.
    checked_at: i64,
    ok: bool,
    entries: Vec<Entry>,
}

/// Status für die Systemseite. Ruft höchstens alle 24 Stunden ab —
/// `force` übergeht den Zwischenspeicher.
pub async fn status(db: &PgPool, url: Option<&str>, force: bool) -> Result<UpdateStatus, Error> {
    let installed = installed_version();

    let Some(url) = url.map(str::trim).filter(|u| !u.is_empty()) else {
        return Ok(UpdateStatus {
            enabled: false,
            installed,
            latest: None,
            newer: Vec::new(),
            ok: false,
            checked_at: None,
        });
    };

    let mut cache = cached_state(db).await;
    let frisch = cache.url == url && !is_expired(cache.checked_at);

    if force || !frisch {
        let geholt = fetch(url).await;
        let alte_eintraege = if cache.url == url {
            std::mem::take(&mut cache.entries)
        } else {
            Vec::new()
        };

        cache = Cache {
            url: url.to_string(),
            checked_at: OffsetDateTime::now_utc().unix_timestamp(),
            ok: geholt.is_some(),
            // Bei einem Fehlschlag den zuletzt bekannten Stand behalten.
            entries: geholt.unwrap_or(alte_eintraege),
        };

        match serde_json::to_string(&cache) {
            Ok(json) => settings::set(db, CACHE_KEY, &json).await?,
            Err(e) => tracing::warn!("Update-Stand nicht speicherbar: {e}"),
        }
    }

    let newer = match &installed {
        Some(v) => cache
            .entries
            .iter()
            .filter(|e| version_cmp(&e.version, v).is_gt())
            .cloned()
            .collect(),
        None => Vec::new(),
    };

    Ok(UpdateStatus {
        enabled: true,
        installed,
        latest: cache.entries.first().map(|e| e.version.clone()),
        newer,
        ok: cache.ok,
        checked_at: OffsetDateTime::from_unix_timestamp(cache.checked_at).ok(),
    })
}

async fn cached_state(db: &PgPool) -> Cache {
    match settings::get(db, CACHE_KEY).await {
        Ok(Some(json)) => serde_json::from_str(&json).unwrap_or_default(),
        Ok(None) => Cache::default(),
        Err(e) => {
            tracing::warn!("Update-Stand nicht lesbar: {e}");
            Cache::default()
        }
    }
}

fn is_expired(checked_at: i64) -> bool {
    let alter = OffsetDateTime::now_utc().unix_timestamp() - checked_at;
    alter < 0 || alter as u64 >= TTL.as_secs()
}

/// Holt die Rohdatei. `None` bei jedem Fehler — Netz weg, 404, Zeitlimit.
async fn fetch(url: &str) -> Option<Vec<Entry>> {
    ensure_crypto_provider();

    let client = reqwest::Client::builder()
        .timeout(TIMEOUT)
        .redirect(redirect_policy())
        .user_agent(concat!("starter-update-check/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| tracing::warn!("Update-Prüfung: Client nicht baubar: {e}"))
        .ok()?;

    let antwort = client
        .get(url)
        .send()
        .await
        .map_err(|e| tracing::info!("Update-Prüfung nicht möglich: {}", e.without_url()))
        .ok()?;

    if !antwort.status().is_success() {
        tracing::info!(
            "Update-Prüfung: {} antwortet mit {}",
            loggable(url),
            antwort.status()
        );
        return None;
    }

    let text = body(antwort).await?;
    let mut entries = super::parse(&text);
    entries.truncate(MAX_ENTRIES);
    Some(entries)
}

/// Liest den Antwortkörper in Stücken und bricht bei `MAX_BYTES` ab.
///
/// Nicht `.text()`: Das puffert erst alles und begrenzt danach — eine
/// Gegenstelle, die endlos liefert, füllt damit innerhalb des Zeitlimits den
/// Speicher, bis der Prozess abgeräumt wird.
async fn body(mut antwort: reqwest::Response) -> Option<String> {
    if antwort
        .content_length()
        .is_some_and(|n| n > MAX_BYTES as u64)
    {
        tracing::info!("Update-Prüfung: Antwort ist zu groß");
        return None;
    }

    let mut rohdaten: Vec<u8> = Vec::new();
    loop {
        match antwort.chunk().await {
            Ok(Some(stueck)) => {
                if rohdaten.len() + stueck.len() > MAX_BYTES {
                    tracing::info!("Update-Prüfung: Antwort ist zu groß");
                    return None;
                }
                rohdaten.extend_from_slice(&stueck);
            }
            Ok(None) => break,
            Err(e) => {
                tracing::info!("Update-Prüfung: Antwort nicht lesbar: {}", e.without_url());
                return None;
            }
        }
    }

    Some(String::from_utf8_lossy(&rohdaten).into_owned())
}

/// Umleitungen nur zu `https`, höchstens drei.
///
/// Die Prüfadresse bestimmt der Betreiber, das **Umleitungsziel** aber die
/// Gegenstelle. Ohne diese Regel führt ein `302 Location: http://169.254.169.254/…`
/// dazu, dass der Server eine Anfrage aus dem internen Netz heraus stellt.
/// Drei Sprünge, weil `github.com/…/raw/…` auf `raw.githubusercontent.com`
/// umleitet — das soll weiter funktionieren.
fn redirect_policy() -> reqwest::redirect::Policy {
    reqwest::redirect::Policy::custom(|versuch| {
        if versuch.previous().len() >= 3 {
            versuch.error("zu viele Umleitungen")
        } else if versuch.url().scheme() == "https" {
            versuch.follow()
        } else {
            versuch.error("Umleitung nur zu https")
        }
    })
}

/// Adresse ohne Anmeldedaten und Query, für Logs.
///
/// Wer trotz der Dokumentation ein Token in die Adresse schreibt, soll es
/// nicht im Betriebslog wiederfinden.
fn loggable(url: &str) -> String {
    match url::Url::parse(url) {
        Ok(mut u) => {
            u.set_query(None);
            u.set_fragment(None);
            let _ = u.set_password(None);
            let _ = u.set_username("");
            u.to_string()
        }
        Err(_) => "<unlesbare Adresse>".to_string(),
    }
}

/// Setzt ring als Krypto-Anbieter von rustls.
///
/// Nötig, weil `reqwest` mit `rustls-no-provider` eingebunden ist: Ohne diese
/// Zeile bestimmt rustls den Anbieter aus seinen Features — und sobald im
/// Abhängigkeitsbaum zwei Anbieter liegen, bricht das zur Laufzeit ab. Das Kit
/// nutzt überall ring (sqlx, lettre), also auch hier.
fn ensure_crypto_provider() {
    static EINMAL: std::sync::Once = std::sync::Once::new();
    EINMAL.call_once(|| {
        // Fehlschlag heisst: Es ist bereits ein Anbieter gesetzt. Das ist in Ordnung.
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}
