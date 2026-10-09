//! Update-Prüfung gegen einen **lokalen** Testserver.
//!
//! Bewusst nie gegen GitHub: `just check` darf nicht vom Internet abhängen und
//! soll nicht rot werden, weil jemand gerade kein Netz hat.

use axum::Router;
use axum::routing::get;
use sqlx::PgPool;
use starter::changelog::check::status;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Ein Testserver, der einen Changelog ausliefert und mitzählt, wie oft er
/// abgerufen wurde. Der Zähler ist der eigentliche Prüfpunkt beim
/// Zwischenspeicher.
struct Testserver {
    url: String,
    abrufe: Arc<AtomicUsize>,
}

/// Antwortet immer gleich.
async fn testserver(antwort: &str, status_code: u16) -> Testserver {
    starten(antwort.to_string(), move |_| status_code).await
}

/// Antwortet beim **ersten** Abruf mit 200 und danach mit 404. Damit lässt
/// sich der Fall „gleiche Adresse, Abruf schlägt jetzt fehl" prüfen — der, bei
/// dem der zuletzt bekannte Stand stehen bleiben soll.
async fn testserver_erst_gut_dann_kaputt(antwort: &str) -> Testserver {
    starten(
        antwort.to_string(),
        |abrufe| if abrufe == 0 { 200 } else { 404 },
    )
    .await
}

async fn starten(
    antwort: String,
    status_code: impl Fn(usize) -> u16 + Send + Sync + 'static,
) -> Testserver {
    let abrufe = Arc::new(AtomicUsize::new(0));
    let zaehler = abrufe.clone();
    let koerper = Arc::new(antwort);
    let status_code = Arc::new(status_code);

    let app = Router::new().route(
        "/CHANGELOG.md",
        get(move || {
            let zaehler = zaehler.clone();
            let koerper = koerper.clone();
            let status_code = status_code.clone();
            async move {
                let vorher = zaehler.fetch_add(1, Ordering::SeqCst);
                (
                    axum::http::StatusCode::from_u16(status_code(vorher)).unwrap(),
                    koerper.to_string(),
                )
            }
        }),
    );

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    Testserver {
        url: format!("http://{addr}/CHANGELOG.md"),
        abrufe,
    }
}

const FERNER_CHANGELOG: &str = "\
## 2099.01.02
- Etwas Neues
- Migration: 9999_beispiel.sql

## 2099.01.01
- Etwas Älteres
";

#[sqlx::test]
async fn ohne_adresse_wird_nichts_abgerufen(db: PgPool) {
    let s = status(&db, None, false).await.unwrap();
    assert!(!s.enabled);
    assert!(!s.update_available());
    assert!(s.checked_at.is_none());
    // Die eigene Version kennt die Anwendung trotzdem.
    assert!(s.installed.is_some());
}

#[sqlx::test]
async fn neuere_versionen_werden_gemeldet(db: PgPool) {
    let server = testserver(FERNER_CHANGELOG, 200).await;

    let s = status(&db, Some(&server.url), false).await.unwrap();
    assert!(s.enabled);
    assert!(s.ok);
    assert_eq!(s.latest.as_deref(), Some("2099.01.02"));
    assert!(
        s.update_available(),
        "2099 muss neuer sein als die eigene Version"
    );
    assert_eq!(s.newer.len(), 2);
    assert_eq!(s.newer[0].migrations, vec!["9999_beispiel.sql"]);
    assert!(s.installed_label().is_some());
}

#[sqlx::test]
async fn innerhalb_der_frist_wird_nicht_erneut_abgerufen(db: PgPool) {
    let server = testserver(FERNER_CHANGELOG, 200).await;

    status(&db, Some(&server.url), false).await.unwrap();
    status(&db, Some(&server.url), false).await.unwrap();
    status(&db, Some(&server.url), false).await.unwrap();

    assert_eq!(
        server.abrufe.load(Ordering::SeqCst),
        1,
        "der Zwischenspeicher muss den zweiten und dritten Aufruf abfangen"
    );
}

#[sqlx::test]
async fn jetzt_pruefen_uebergeht_den_zwischenspeicher(db: PgPool) {
    let server = testserver(FERNER_CHANGELOG, 200).await;

    status(&db, Some(&server.url), false).await.unwrap();
    status(&db, Some(&server.url), true).await.unwrap();

    assert_eq!(server.abrufe.load(Ordering::SeqCst), 2);
}

#[sqlx::test]
async fn fehlgeschlagener_abruf_behaelt_den_letzten_stand(db: PgPool) {
    // Dieselbe Adresse: erst erreichbar, beim zweiten Mal 404. Dann soll die
    // Systemseite den zuletzt bekannten Stand weiter anzeigen und ihn nur als
    // veraltet kennzeichnen.
    let server = testserver_erst_gut_dann_kaputt(FERNER_CHANGELOG).await;

    let s = status(&db, Some(&server.url), false).await.unwrap();
    assert!(s.ok);

    let s = status(&db, Some(&server.url), true).await.unwrap();
    assert!(!s.ok, "ein 404 darf nicht als Erfolg gelten");
    assert_eq!(
        s.latest.as_deref(),
        Some("2099.01.02"),
        "der letzte bekannte Stand muss stehen bleiben"
    );
    assert_eq!(s.newer.len(), 2);
    assert_eq!(server.abrufe.load(Ordering::SeqCst), 2);
}

#[sqlx::test]
async fn unerreichbare_adresse_gilt_als_fehlschlag(db: PgPool) {
    // Eine Adresse ohne Zuhörer: die Verbindung scheitert sofort.
    let tot = {
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let a = l.local_addr().unwrap();
        drop(l);
        format!("http://{a}/CHANGELOG.md")
    };

    let s = status(&db, Some(&tot), false).await.unwrap();
    assert!(!s.ok);
    assert!(s.newer.is_empty());
    // Trotz Fehlschlag keine Fehlerseite: die eigene Version steht da.
    assert!(s.installed.is_some());
}

#[sqlx::test]
async fn adresswechsel_verwirft_den_alten_stand(db: PgPool) {
    let alt = testserver(FERNER_CHANGELOG, 200).await;
    status(&db, Some(&alt.url), false).await.unwrap();

    let neu = testserver("## 2099.05.05\n- Anderes Repository\n", 200).await;
    let s = status(&db, Some(&neu.url), false).await.unwrap();

    assert_eq!(s.latest.as_deref(), Some("2099.05.05"));
    assert_eq!(s.newer.len(), 1);
    assert_eq!(
        neu.abrufe.load(Ordering::SeqCst),
        1,
        "eine neue Adresse muss sofort abgerufen werden, nicht erst nach Ablauf der Frist"
    );
}

#[sqlx::test]
async fn aeltere_eintraege_gelten_nicht_als_update(db: PgPool) {
    let server = testserver("## 1999.01.01\n- Uralt\n", 200).await;
    let s = status(&db, Some(&server.url), false).await.unwrap();

    assert!(s.ok);
    assert!(!s.update_available(), "ältere Versionen sind kein Update");
}

#[sqlx::test]
async fn sehr_viele_eintraege_werden_gekappt(db: PgPool) {
    // 120 Versionen, alle neuer als die eigene. Gespeichert werden höchstens
    // 100 — sonst wächst die Zeile in `settings` mit dem fremden Changelog.
    let mut changelog = String::new();
    for i in (0..120).rev() {
        let monat = 1 + i / 28;
        let tag = 1 + i % 28;
        changelog.push_str(&format!("## 2099.{monat:02}.{tag:02}\n- Punkt {i}\n\n"));
    }

    let server = testserver(&changelog, 200).await;
    let s = status(&db, Some(&server.url), false).await.unwrap();

    assert!(s.ok);
    assert_eq!(s.newer.len(), 100, "MAX_ENTRIES muss greifen");
}

#[sqlx::test]
async fn zu_grosse_antwort_wird_verworfen(db: PgPool) {
    // Über 256 KiB: der Körper darf nicht vollständig in den Speicher gelesen
    // werden, und der Abruf gilt als fehlgeschlagen.
    let mut changelog = String::from("## 2099.01.01\n- Beginn\n");
    changelog.push_str(&"- Füllung ohne Ende\n".repeat(20_000));

    let server = testserver(&changelog, 200).await;
    let s = status(&db, Some(&server.url), false).await.unwrap();

    assert!(!s.ok, "eine zu große Antwort darf nicht als Erfolg gelten");
    assert!(s.newer.is_empty());
}
