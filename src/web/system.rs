//! Systemseite unter /admin/system — nur Administratoren.
//!
//! Zeigt die WIRKSAME Konfiguration, aendert aber nichts daran. Die Quelle der
//! Wahrheit bleiben die Umgebungsvariablen. Grund: Name, Farben und Mailserver
//! sind Betriebsentscheidungen, die mit dem Deployment zusammengehoeren und
//! nachvollziehbar versioniert sein sollen — nicht still per Klick in der
//! Datenbank. Die Seite beantwortet die Frage „Was gilt gerade?" und laesst
//! den Mailversand ausprobieren.

use axum::extract::{Form, Query, State};
use axum::response::{IntoResponse, Redirect, Response};
use serde::Deserialize;

use crate::auth::extract::AdminUser;
use crate::changelog;
use crate::domain::user::User;
use crate::error::WebError;
use crate::mail::Mail;
use crate::mail::templates::TestMail;
use crate::state::AppState;
use crate::templates::{Layout, SystemPage, branding, render};

#[derive(Debug, Deserialize)]
pub struct SystemQuery {
    #[serde(default)]
    pub mail: Option<String>,
    /// Rückmeldung nach „Jetzt prüfen": "ok" oder "failed".
    #[serde(default)]
    pub checked: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TestMailForm {
    pub to: String,
}

pub async fn index(
    State(state): State<AppState>,
    AdminUser(admin): AdminUser,
    Query(query): Query<SystemQuery>,
) -> Result<Response, WebError> {
    let sent = query.mail.as_deref() == Some("sent");
    let checked = match query.checked.as_deref() {
        Some("ok") => Some("ok"),
        Some("failed") => Some("failed"),
        _ => None,
    };
    let to = admin.email.clone();
    render_page(&state, admin, &to, sent, None, checked).await
}

/// „Jetzt prüfen": übergeht den Zwischenspeicher.
pub async fn update_check(
    State(state): State<AppState>,
    AdminUser(admin): AdminUser,
) -> Result<Response, WebError> {
    let status =
        changelog::check::status(&state.db, state.config.update_check_url.as_deref(), true).await?;

    tracing::info!(by = %admin.id, erreichbar = status.ok, "Update-Prüfung ausgelöst");
    let ergebnis = if status.ok { "ok" } else { "failed" };
    Ok(Redirect::to(&format!("/admin/system?checked={ergebnis}")).into_response())
}

async fn render_page(
    state: &AppState,
    admin: User,
    to: &str,
    sent: bool,
    error: Option<String>,
    checked: Option<&'static str>,
) -> Result<Response, WebError> {
    // Ruft höchstens alle 24 Stunden wirklich ab; sonst aus dem Zwischenspeicher.
    let update =
        changelog::check::status(&state.db, state.config.update_check_url.as_deref(), false)
            .await?;

    // Installierte Version und die drei davor.
    let mut eintraege = changelog::entries();
    eintraege.truncate(4);

    render(SystemPage {
        layout: Layout::for_user("System", admin, "/admin/system"),
        brand: branding().clone(),
        public_url: state.config.public_url.clone(),
        cookie_secure: state.config.cookie_secure,
        trust_proxy: state.config.trust_proxy,
        transport: state.mailer.transport_info().clone(),
        from_address: state.mailer.from_address(),
        test_mail_to: to.to_string(),
        sent,
        error,
        update,
        changelog: eintraege,
        checked,
    })
}

pub async fn testmail(
    State(state): State<AppState>,
    AdminUser(admin): AdminUser,
    Form(form): Form<TestMailForm>,
) -> Result<Response, WebError> {
    let template = TestMail {
        brand: branding().clone(),
        recipient_name: form.to.clone(),
        triggered_by: admin.display_name.clone(),
        link: state.config.public_url.clone(),
    };

    let result = async {
        let mail = Mail::to(form.to.trim())
            .subject(format!("Testmail von {}", branding().name))
            .text(template.text())
            .template(&template)?;
        // Hier bewusst `send` und nicht `send_in_background`: Der Administrator
        // will wissen, ob der Mailserver die Nachricht angenommen hat.
        state.mailer.send(mail).await
    }
    .await;

    match result {
        Ok(()) => {
            tracing::info!(by = %admin.id, "Testmail verschickt");
            // Die Adresse nicht in die URL — sie landet sonst in Verlauf und Logs.
            Ok(Redirect::to("/admin/system?mail=sent").into_response())
        }
        Err(err) => {
            let status = err.status();
            // Hier ausnahmsweise die interne Meldung zeigen: Nur Administratoren
            // sehen die Seite, und "Verbindung abgelehnt" oder "Authentifizierung
            // fehlgeschlagen" ist genau die Auskunft, die sie brauchen.
            let text = match &err {
                crate::error::Error::Internal(e) => format!("{e:#}"),
                other => other.public_message(),
            };
            let to = form.to.clone();
            let body = render_page(&state, admin, &to, false, Some(text), None).await?;
            Ok((status, body).into_response())
        }
    }
}
