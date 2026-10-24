//! Construcción del cuerpo y de las cabeceras de una página de error.
//!
//! ---------------------------------------------------------------------------
//! API que asume la fase RED (la fase GREEN debe implementarla tal cual):
//!
//! ```ignore
//! #[derive(Debug)]
//! pub enum RenderError {
//!     /// El fichero HTML ni el del código pedido ni el del código por defecto.
//!     MissingHtml { code: u16, default_code: u16 },
//!     Io(std::io::Error),
//! }
//! impl std::fmt::Display for RenderError { /* ... */ }
//! impl std::error::Error for RenderError {}
//!
//! /// Respuesta completa de una página de error, lista para servir.
//! #[derive(Debug)]
//! pub struct RenderedPage {
//!     /// Estado de la respuesta: **siempre** el código resuelto, nunca `200`.
//!     pub status: StatusCode,
//!     /// `Content-Type` según el formato.
//!     pub content_type: &'static str,
//!     /// Cabeceras de acompañamiento: `X-Robots-Tag` y, para `429`/`503`,
//!     /// `Retry-After`. `Content-Type` y `Content-Length` los pone el servidor.
//!     pub headers: Vec<(&'static str, String)>,
//!     pub body: Vec<u8>,
//! }
//!
//! /// Cuerpo por formato. HTML se lee de `content_dir/<code>.html` con caída a
//! /// `content_dir/<default_code>.html`; JSON, XML y texto se generan a partir
//! /// de `code`, `canonical_reason(code)` y `description(code)`.
//! pub fn render_body(
//!     code: u16,
//!     default_code: u16,
//!     format: Format,
//!     content_dir: &Path,
//! ) -> Result<Vec<u8>, RenderError>;
//!
//! /// `render_body` más las cabeceras y el estado.
//! pub fn render_page(
//!     config: &Config,
//!     code: u16,
//!     format: Format,
//! ) -> Result<RenderedPage, RenderError>;
//!
//! /// Respaldo de texto, infalible (no lee ficheros), con el estado y las
//! /// cabeceras de `code`: lo que se sirve cuando falta el HTML.
//! pub fn render_text_fallback(code: u16) -> RenderedPage;
//! ```
//!
//! Contrato de la spec (`error-page`):
//!
//! - JSON: objeto con las claves `error` (`true`), `code` (número), `message`
//!   (razón canónica) y `description`.
//! - XML: declaración `<?xml version="1.0" encoding="utf-8"?>` y un elemento
//!   `code` con el número, más `message` y `description`.
//! - Texto: código, razón y descripción en texto plano.
//! - `Content-Type`: `text/html; charset=utf-8`, `application/json`,
//!   `application/xml` y `text/plain; charset=utf-8`.
//! - `X-Robots-Tag: noindex, nofollow, nosnippet, noarchive` en toda página;
//!   `Retry-After: 120` solo en los códigos que lo llevan.

use crate::config::Config;
use crate::resolve::{self, Format};
use axum::http::StatusCode;
use serde::Serialize;
use std::fmt;
use std::io::ErrorKind;
use std::path::Path;

/// Cabecera que se añade a **toda** página de error.
const X_ROBOTS_TAG_NAME: &str = "X-Robots-Tag";
/// Valor de [`X_ROBOTS_TAG_NAME`], en paridad con `error-pages`.
pub const X_ROBOTS_TAG_VALUE: &str = "noindex, nofollow, nosnippet, noarchive";
/// Cabecera con los segundos de espera antes de reintentar.
const RETRY_AFTER_NAME: &str = "Retry-After";

/// Fallo al construir el cuerpo de una página de error.
#[derive(Debug)]
pub enum RenderError {
    /// El fichero HTML ni el del código pedido ni el del código por defecto.
    MissingHtml { code: u16, default_code: u16 },
    /// Error de E/S al leer el HTML, o al serializar el cuerpo generado.
    Io(std::io::Error),
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RenderError::MissingHtml { code, default_code } => write!(
                f,
                "no hay página HTML para el error: ni `{code}.html` ni `{default_code}.html` \
                 existen en el directorio de contenido"
            ),
            RenderError::Io(err) => {
                write!(
                    f,
                    "no se pudo construir el cuerpo de la página de error: {err}"
                )
            }
        }
    }
}

impl std::error::Error for RenderError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            RenderError::Io(err) => Some(err),
            RenderError::MissingHtml { .. } => None,
        }
    }
}

/// Respuesta completa de una página de error, lista para servir.
#[derive(Debug)]
pub struct RenderedPage {
    /// Estado de la respuesta: **siempre** el código resuelto, nunca `200`.
    pub status: StatusCode,
    /// `Content-Type` según el formato.
    pub content_type: &'static str,
    /// Cabeceras de acompañamiento: `X-Robots-Tag` y, para `429`/`503`,
    /// `Retry-After`. `Content-Type` y `Content-Length` los pone el servidor.
    pub headers: Vec<(&'static str, String)>,
    /// Cuerpo de la respuesta.
    pub body: Vec<u8>,
}

/// Cuerpo por formato. HTML se lee de `content_dir/<code>.html` con caída a
/// `content_dir/<default_code>.html`; JSON, XML y texto se generan a partir de
/// `code`, [`resolve::canonical_reason`] y [`resolve::description`].
///
/// # Errors
///
/// [`RenderError::MissingHtml`] si el fichero del código pedido y el del código
/// por defecto faltan (solo en [`Format::Html`]), y [`RenderError::Io`] si la
/// lectura o la serialización fallan.
pub fn render_body(
    code: u16,
    default_code: u16,
    format: Format,
    content_dir: &Path,
) -> Result<Vec<u8>, RenderError> {
    match format {
        Format::Html => html_body(code, default_code, content_dir),
        Format::Json => json_body(code),
        Format::Xml => Ok(xml_body(code).into_bytes()),
        Format::Text => Ok(text_body(code).into_bytes()),
    }
}

/// [`render_body`] más las cabeceras de acompañamiento y el estado.
///
/// # Errors
///
/// Las mismas que [`render_body`].
pub fn render_page(
    config: &Config,
    code: u16,
    format: Format,
) -> Result<RenderedPage, RenderError> {
    let body = render_body(code, config.default_error_page, format, &config.content_dir)?;

    Ok(RenderedPage {
        status: status_of(code),
        content_type: content_type_for(format),
        headers: companion_headers(code),
        body,
    })
}

/// Respaldo de texto de una página de error: el cuerpo de [`Format::Text`] con
/// el estado y las cabeceras de `code`.
///
/// Es **infalible**: genera el cuerpo, así que no lee ningún fichero. Se sirve
/// cuando el HTML del código —y el del código por defecto— no existe, para no
/// responder `500` por esa causa: el estado sigue siendo el código resuelto
/// (spec `error-page`, "Un fallo al leer el HTML no cambia el estado").
pub fn render_text_fallback(code: u16) -> RenderedPage {
    RenderedPage {
        status: status_of(code),
        content_type: content_type_for(Format::Text),
        headers: companion_headers(code),
        body: text_body(code).into_bytes(),
    }
}

/// Estado de la respuesta a partir del código resuelto.
///
/// La resolución ([`crate::resolve::resolve_code_with_source`]) solo entrega
/// códigos de `400` a `599` y `Config` rechaza un `DEFAULT_ERROR_PAGE` fuera de
/// ese rango, así que la degradación a `500` es inalcanzable por esa vía: queda
/// como red de seguridad y **nunca** podría producir un `200`.
fn status_of(code: u16) -> StatusCode {
    StatusCode::from_u16(code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
}

/// Cabeceras de acompañamiento de toda página de error: `X-Robots-Tag` y, para
/// los códigos que lo llevan, `Retry-After`.
fn companion_headers(code: u16) -> Vec<(&'static str, String)> {
    let mut headers = vec![(X_ROBOTS_TAG_NAME, X_ROBOTS_TAG_VALUE.to_string())];

    if let Some(secs) = resolve::retry_after_secs(code) {
        headers.push((RETRY_AFTER_NAME, secs.to_string()));
    }

    headers
}

/// `Content-Type` que corresponde a cada formato.
fn content_type_for(format: Format) -> &'static str {
    match format {
        Format::Html => "text/html; charset=utf-8",
        Format::Json => "application/json",
        Format::Xml => "application/xml",
        Format::Text => "text/plain; charset=utf-8",
    }
}

/// Cuerpo HTML: el fichero del código pedido y, si no existe, el del código por
/// defecto.
fn html_body(code: u16, default_code: u16, content_dir: &Path) -> Result<Vec<u8>, RenderError> {
    if let Some(body) = read_html(content_dir, code).map_err(RenderError::Io)? {
        return Ok(body);
    }

    if code != default_code
        && let Some(body) = read_html(content_dir, default_code).map_err(RenderError::Io)?
    {
        return Ok(body);
    }

    Err(RenderError::MissingHtml { code, default_code })
}

/// Lee `content_dir/<code>.html`; `Ok(None)` si el fichero no existe.
fn read_html(content_dir: &Path, code: u16) -> Result<Option<Vec<u8>>, std::io::Error> {
    match std::fs::read(content_dir.join(format!("{code}.html"))) {
        Ok(body) => Ok(Some(body)),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err),
    }
}

/// Cuerpo JSON: `{"error":true,"code":...,"message":...,"description":...}`.
///
/// Se serializa desde un `struct` (y no desde un mapa) para que el orden de las
/// claves sea el documentado por `error-pages`.
fn json_body(code: u16) -> Result<Vec<u8>, RenderError> {
    let body = JsonBody {
        error: true,
        code,
        message: resolve::canonical_reason(code),
        description: resolve::description(code),
    };

    serde_json::to_vec(&body).map_err(|err| RenderError::Io(std::io::Error::other(err)))
}

/// Forma del cuerpo JSON.
#[derive(Debug, Serialize)]
struct JsonBody<'a> {
    error: bool,
    code: u16,
    message: &'a str,
    description: &'a str,
}

/// Cuerpo XML: declaración y un elemento `error` con `code`, `message` y
/// `description`.
fn xml_body(code: u16) -> String {
    let message = escape_xml(resolve::canonical_reason(code));
    let description = escape_xml(resolve::description(code));

    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n\
         <error><code>{code}</code><message>{message}</message><description>{description}</description></error>"
    )
}

/// Cuerpo de texto plano: el código, su razón canónica y la descripción.
fn text_body(code: u16) -> String {
    format!(
        "{code} {}\n{}\n",
        resolve::canonical_reason(code),
        resolve::description(code)
    )
}

/// Escapa el texto para que sea XML válido.
fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::resolve::Format;
    use axum::http::StatusCode;
    use serde_json::Value;
    use std::path::{Path, PathBuf};

    /// Fichero de fixture que representa `content/404.html`.
    const FIXTURE_404_HTML: &str = include_str!("../tests/fixtures/content/404.html");

    fn fixtures_content_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/content")
    }

    fn fixtures_config() -> Config {
        Config::from_env_iter([(
            "CONTENT_DIR".to_string(),
            fixtures_content_dir().to_string_lossy().into_owned(),
        )])
        .unwrap()
    }

    fn header<'a>(page: &'a RenderedPage, name: &str) -> Option<&'a str> {
        page.headers
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value.as_str())
    }

    // --- Cuerpo JSON y XML (tasks.md 1.5) ----------------------------------

    #[test]
    fn el_cuerpo_json_lleva_las_cuatro_claves() {
        let body = render_body(404, 404, Format::Json, &fixtures_content_dir()).unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["error"], Value::Bool(true));
        assert_eq!(json["code"], Value::from(404));
        assert_eq!(json["message"], Value::from("Not Found"));
        assert!(json["description"].is_string());
        assert!(!json["description"].as_str().unwrap().is_empty());
    }

    #[test]
    fn el_cuerpo_xml_lleva_el_elemento_code() {
        let body = render_body(503, 404, Format::Xml, &fixtures_content_dir()).unwrap();
        let xml = String::from_utf8(body).unwrap();

        assert!(
            xml.contains("<code>503</code>"),
            "el XML debe contener <code>503</code>, fue: {xml}"
        );
        assert!(xml.contains("<message>"), "el XML debe llevar <message>");
        assert!(
            xml.contains("<description>"),
            "el XML debe llevar <description>"
        );
    }

    #[test]
    fn el_cuerpo_de_texto_lleva_codigo_y_razon() {
        let body = render_body(503, 404, Format::Text, &fixtures_content_dir()).unwrap();
        let text = String::from_utf8(body).unwrap();

        assert!(
            text.contains("503"),
            "el texto debe llevar el código: {text}"
        );
        assert!(
            text.contains("Service Unavailable"),
            "el texto debe llevar la razón canónica: {text}"
        );
    }

    #[test]
    fn el_cuerpo_html_sale_del_fichero_del_codigo() {
        let body = render_body(404, 404, Format::Html, &fixtures_content_dir()).unwrap();
        let html = String::from_utf8(body).unwrap();

        assert_eq!(html.trim(), FIXTURE_404_HTML.trim());
    }

    #[test]
    fn el_cuerpo_html_cae_al_fichero_del_codigo_por_defecto() {
        // No hay `content/418.html`; el cuerpo debe ser el de `content/404.html`.
        let body = render_body(418, 404, Format::Html, &fixtures_content_dir()).unwrap();
        let html = String::from_utf8(body).unwrap();

        assert_eq!(html.trim(), FIXTURE_404_HTML.trim());
    }

    #[test]
    fn el_cuerpo_no_depende_del_formato_para_el_codigo() {
        // Ortogonalidad: el mismo `code` produce los cuatro cuerpos.
        for format in [Format::Html, Format::Json, Format::Xml, Format::Text] {
            let body = render_body(503, 404, format, &fixtures_content_dir()).unwrap();
            assert!(!body.is_empty(), "cuerpo vacío para {format:?}");
        }
    }

    // --- Cabeceras de acompañamiento (tasks.md 1.7) ------------------------

    #[test]
    fn toda_pagina_de_error_lleva_x_robots_tag() {
        for code in [404, 500, 503] {
            let page = render_page(&fixtures_config(), code, Format::Json).unwrap();
            assert_eq!(
                header(&page, "X-Robots-Tag"),
                Some("noindex, nofollow, nosnippet, noarchive"),
                "X-Robots-Tag ausente o incorrecta para {code}"
            );
        }
    }

    #[test]
    fn el_503_lleva_retry_after_120() {
        let page = render_page(&fixtures_config(), 503, Format::Text).unwrap();
        assert_eq!(header(&page, "Retry-After"), Some("120"));
    }

    #[test]
    fn el_429_lleva_retry_after_120() {
        let page = render_page(&fixtures_config(), 429, Format::Text).unwrap();
        assert_eq!(header(&page, "Retry-After"), Some("120"));
    }

    #[test]
    fn el_404_no_lleva_retry_after() {
        let page = render_page(&fixtures_config(), 404, Format::Text).unwrap();
        assert_eq!(header(&page, "Retry-After"), None);
    }

    #[test]
    fn el_estado_es_el_codigo_resuelto_y_nunca_200() {
        let page = render_page(&fixtures_config(), 404, Format::Html).unwrap();
        assert_eq!(page.status, StatusCode::NOT_FOUND);
        assert_ne!(page.status, StatusCode::OK);

        let page = render_page(&fixtures_config(), 503, Format::Html).unwrap();
        assert_eq!(page.status, StatusCode::SERVICE_UNAVAILABLE);
    }

    #[test]
    fn el_content_type_sigue_al_formato() {
        let content_type = |format| {
            render_page(&fixtures_config(), 404, format)
                .unwrap()
                .content_type
        };

        assert!(content_type(Format::Html).starts_with("text/html"));
        assert!(content_type(Format::Json).starts_with("application/json"));
        assert!(content_type(Format::Xml).starts_with("application/xml"));
        assert!(content_type(Format::Text).starts_with("text/plain"));
    }

    #[test]
    fn el_respaldo_de_texto_conserva_el_codigo_y_sus_cabeceras() {
        // Auditoría I-2: sin fichero HTML, el estado sigue siendo el código
        // resuelto (nunca `500`) y la página no pierde `X-Robots-Tag`.
        let page = render_text_fallback(418);

        assert_eq!(page.status, StatusCode::IM_A_TEAPOT);
        assert_ne!(page.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_ne!(page.status, StatusCode::OK);
        assert!(page.content_type.starts_with("text/plain"));
        assert_eq!(
            header(&page, "X-Robots-Tag"),
            Some("noindex, nofollow, nosnippet, noarchive")
        );

        let body = String::from_utf8(page.body).unwrap();
        assert!(
            body.contains("418"),
            "el cuerpo debe llevar el código: {body}"
        );
    }

    #[test]
    fn el_respaldo_de_texto_mantiene_retry_after() {
        assert_eq!(
            header(&render_text_fallback(503), "Retry-After"),
            Some("120")
        );
        assert_eq!(header(&render_text_fallback(404), "Retry-After"), None);
    }
}
