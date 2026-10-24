//! Servidor de páginas de error para Traefik.
//!
//! Clon supersencillo de `tarampampam/error-pages` con **una diferencia
//! deliberada**: la respuesta devuelve el **código de error real**, nunca `200`.
//!
//! ---------------------------------------------------------------------------
//! API que asume la fase RED (la fase GREEN debe implementarla tal cual):
//!
//! ```ignore
//! /// Construye el `Router` de axum: endpoints de servicio (`/healthz`,
//! /// `/health`, `/health/live`, `/live`, `/version`), assets estáticos de
//! /// `config.content_dir` y *fallback* de páginas de error (resolución de
//! /// código y de formato + `render_page`).
//! fn build_router(config: &Config) -> axum::Router;
//!
//! /// Arranque real: `Config::from_env()?`, `tracing`, `axum::serve` en
//! /// `0.0.0.0:{config.port}`.
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>>;
//! ```
//!
//! Contrato de la spec (`http-server`):
//!
//! - Los endpoints de servicio responden **siempre `200`** y no pasan por la
//!   resolución de páginas de error.
//! - `GET /version` devuelve `{"version":"..."}` con `Content-Type` JSON.
//! - `GET /css/404.css` sirve el fichero de `CONTENT_DIR` con `Content-Type`
//!   `text/css`.
//! - El *fallback* responde con el estado igual al código resuelto: `GET /404`
//!   es un `404`, y `GET /` con `X-Code: 503` es un `503`.
//!
//! El path se normaliza (percent-decode + plegado de `.` y `..`, ver
//! [`path`]) **antes** de clasificarlo, y la misma cadena normalizada se
//! reescribe en la URI que recibe `ServeDir`: si el clasificador mirara el path
//! crudo, `/./404.html` o `/404%2Ehtml` se servirían como asset con `200`, que
//! es justo lo que este proyecto corrige.

mod config;
mod path;
mod render;
mod resolve;

use crate::config::Config;
use crate::path::NormalizedPath;
use crate::render::{RenderedPage, render_page, render_text_fallback};
use crate::resolve::{code_from_path, resolve_code, resolve_format};
use axum::Router;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::uri::{PathAndQuery, Uri};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::get;
use axum::{Json, serve};
use std::net::{Ipv4Addr, SocketAddr};
use tower_http::services::ServeDir;
use tower_http::trace::{DefaultOnFailure, TraceLayer};
use tracing_subscriber::EnvFilter;

/// Dirección de escucha: detrás de Traefik, en todas las interfaces.
const BIND_ADDRESS: Ipv4Addr = Ipv4Addr::UNSPECIFIED;

/// Filtro de registro de reserva, solo si `LOG_LEVEL` no es un filtro de
/// `tracing` válido.
const FALLBACK_LOG_LEVEL: &str = "info";

/// Construye el `Router` de axum: endpoints de servicio (`/healthz`,
/// `/health`, `/health/live`, `/live`, `/version`), assets estáticos de
/// `config.content_dir` y *fallback* de páginas de error (resolución de
/// código y de formato + `render_page`).
fn build_router(config: &Config) -> Router {
    Router::new()
        .route("/healthz", get(alive))
        .route("/health", get(alive))
        .route("/health/live", get(alive))
        .route("/live", get(alive))
        .route("/version", get(version))
        // Los assets se sirven desde la **única** ruta de estáticos, la misma
        // que usa el render del HTML (`CONTENT_DIR`).
        .fallback_service(
            ServeDir::new(&config.content_dir).append_index_html_on_directories(false),
        )
        // El middleware se añade después de las rutas: axum solo envuelve lo
        // que ya existe, y el *fallback* es una de esas cosas.
        .layer(middleware::from_fn_with_state(config.clone(), error_pages))
}

/// Arranque real: `Config::from_env()?`, `tracing`, `axum::serve` en
/// `0.0.0.0:{config.port}`.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env()?;
    init_tracing(&config.log_level);

    let address = SocketAddr::from((BIND_ADDRESS, config.port));
    let listener = tokio::net::TcpListener::bind(address).await?;

    tracing::info!(
        %address,
        content_dir = %config.content_dir.display(),
        default_error_page = config.default_error_page,
        "servidor de páginas de error para Traefik"
    );

    // El clasificador por defecto (`ServerErrorsAsFailures`) marca **todo** 5xx
    // como fallo y lo registra en `ERROR`. Como este servicio *sirve* páginas
    // 5xx, cada `503` legítima inundaba el log con un `ERROR` que no es un
    // fallo real. El `on_failure` baja a `DEBUG`: el suceso sigue ahí para
    // quien depure, pero deja de fingir un error en el nivel por defecto
    // (`LOG_LEVEL=info`). La traza de la petición la dan `on_request` y
    // `on_response`, que en tower-http 0.6 ya emiten a `DEBUG`.
    let trace_layer =
        TraceLayer::new_for_http().on_failure(DefaultOnFailure::new().level(tracing::Level::DEBUG));

    serve(listener, build_router(&config).layer(trace_layer)).await?;

    Ok(())
}

/// Instala el registro con el filtro de la configuración.
///
/// Un `LOG_LEVEL` que no sea un filtro de `tracing` válido no aborta el
/// arranque: se avisa por `stderr` y se usa [`FALLBACK_LOG_LEVEL`]. Un
/// `LOG_LEVEL` vacío **no** se sustituye por el defecto: se entrega tal cual al
/// filtro, que decidirá qué habilita.
fn init_tracing(log_level: &str) {
    let filter = match EnvFilter::try_new(log_level) {
        Ok(filter) => filter,
        Err(err) => {
            eprintln!("LOG_LEVEL no es un filtro válido ({err}); se usa `{FALLBACK_LOG_LEVEL}`");
            EnvFilter::new(FALLBACK_LOG_LEVEL)
        }
    };

    if let Err(err) = tracing_subscriber::fmt().with_env_filter(filter).try_init() {
        eprintln!("no se pudo instalar el registro: {err}");
    }
}

/// Endpoints de salud: siempre `200`, nunca pasan por la página de error.
async fn alive() -> StatusCode {
    StatusCode::OK
}

/// `GET /version` -> `{"version":"..."}`, como en `error-pages`.
async fn version() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "version": env!("CARGO_PKG_VERSION") }))
}

/// Cabeceras de la petición que alimentan las cascadas de código y de formato.
struct Hints {
    x_code: Option<String>,
    content_type: Option<String>,
    x_format: Option<String>,
    accept: Option<String>,
}

impl Hints {
    fn new(headers: &HeaderMap) -> Self {
        let read = |name: &str| {
            headers
                .get(name)
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned)
        };

        Self {
            x_code: read("x-code"),
            content_type: read(header::CONTENT_TYPE.as_str()),
            x_format: read("x-format"),
            accept: read(header::ACCEPT.as_str()),
        }
    }
}

/// Middleware de cierre: lo que no es un asset estático existente es una página
/// de error, con el estado igual al código resuelto (nunca `200`).
///
/// Va en un middleware y no en un `fallback` para que las páginas de error
/// manden sobre los ficheros: si no, `/404.html` se serviría desde
/// `CONTENT_DIR/404.html` con un `200`, justo lo que este proyecto corrige.
///
/// El path se normaliza **una sola vez** aquí (percent-decode + plegado de `.`
/// y `..`) y la URI se reescribe con esa misma cadena antes de entregársela a
/// `ServeDir`, que de todas formas decodifica y pliega por su cuenta: así el
/// clasificador y el servidor de ficheros no pueden ver paths distintos.
async fn error_pages(State(config): State<Config>, mut request: Request, next: Next) -> Response {
    let hints = Hints::new(request.headers());
    let normalized = NormalizedPath::new(request.uri().path());

    match normalized_uri(request.uri(), &normalized) {
        Some(uri) => *request.uri_mut() = uri,
        None => {
            // Inalcanzable con un path normalizado (siempre `/` + ASCII), pero
            // si pasara no se arriesga un asset servido con `200`: se responde
            // la página de error.
            tracing::warn!(path = normalized.as_str(), "no se pudo reescribir la URI");
            return error_response(&config, &normalized, &hints).await;
        }
    }

    if is_error_page_path(normalized.as_str()) {
        return error_response(&config, &normalized, &hints).await;
    }

    let response = next.run(request).await;

    // El asset no existe: en vez del `404` vacío de `ServeDir`, la página de
    // error del código resuelto.
    if response.status() == StatusCode::NOT_FOUND {
        return error_response(&config, &normalized, &hints).await;
    }

    response
}

/// Reescribe la URI con el path normalizado, conservando el *query string*.
///
/// Devuelve `None` si el resultado no es una URI válida. No puede pasar con un
/// [`NormalizedPath`] (empieza por `/` y solo contiene ASCII sin reservados tras
/// el re-encoding), pero el fallo se trata en vez de ignorarse: clasificar sobre
/// un path y servir sobre otro es la vulnerabilidad que este proyecto corrige.
fn normalized_uri(original: &Uri, normalized: &NormalizedPath) -> Option<Uri> {
    let path = normalized.to_uri_path();
    let path_and_query = match original.query() {
        Some(query) => format!("{path}?{query}"),
        None => path,
    };

    let mut parts = original.clone().into_parts();
    parts.path_and_query = Some(PathAndQuery::from_maybe_shared(path_and_query).ok()?);

    Uri::from_parts(parts).ok()
}

/// ¿La petición es una página de error (o la raíz) y no un asset estático?
///
/// Lo es si el path —ya normalizado— aporta un código por su primer segmento
/// —`/404`, `/404.html`, `/503.xml`— o si es la raíz, que siempre es una página
/// de error.
fn is_error_page_path(path: &str) -> bool {
    let first_segment = path
        .trim_start_matches('/')
        .split('/')
        .next()
        .unwrap_or_default();

    first_segment.is_empty() || code_from_path(path).is_some()
}

/// Resuelve código y formato y renderiza la página de error.
///
/// El render lee del disco, así que va en `spawn_blocking`: el hilo del
/// ejecutor async no se bloquea con E/S síncrona (auditoría I-1).
async fn error_response(config: &Config, path: &NormalizedPath, hints: &Hints) -> Response {
    let code = resolve_code(
        path.as_str(),
        hints.x_code.as_deref(),
        config.default_error_page,
    );
    let format = resolve_format(
        path.as_str(),
        hints.content_type.as_deref(),
        hints.x_format.as_deref(),
        hints.accept.as_deref(),
    );

    let rendered = tokio::task::spawn_blocking({
        let config = config.clone();
        move || render_page(&config, code, format)
    })
    .await;

    match rendered {
        Ok(Ok(page)) => page_response(page),
        Ok(Err(err)) => {
            // Falta el HTML del código y el del código por defecto (o no se
            // pudo leer). El estado sigue siendo el código resuelto y nunca
            // `500`: se responde el cuerpo de texto (auditoría I-2).
            tracing::warn!(code, error = %err, "sin HTML de la página de error: se responde texto");
            page_response(render_text_fallback(code))
        }
        Err(err) => {
            tracing::error!(code, error = %err, "la tarea de render falló: se responde texto");
            page_response(render_text_fallback(code))
        }
    }
}

/// Convierte una página renderizada en la respuesta de axum. `Content-Length`
/// lo calcula hyper a partir del cuerpo; el resto de cabeceras van aquí.
fn page_response(page: RenderedPage) -> Response {
    let mut response = Response::new(Body::from(page.body));
    *response.status_mut() = page.status;

    insert_header(
        response.headers_mut(),
        header::CONTENT_TYPE.as_str(),
        page.content_type,
    );

    for (name, value) in page.headers {
        insert_header(response.headers_mut(), name, &value);
    }

    response
}

/// Inserta una cabecera, ignorándola si el nombre o el valor no son válidos.
fn insert_header(headers: &mut HeaderMap, name: &str, value: &str) {
    let Ok(name) = HeaderName::from_bytes(name.as_bytes()) else {
        return;
    };
    let Ok(value) = HeaderValue::from_str(value) else {
        return;
    };

    headers.insert(name, value);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use axum::body::Body;
    use axum::http::{Request, StatusCode, header};
    use http_body_util::BodyExt;
    use std::path::{Path, PathBuf};
    use tower::ServiceExt;

    fn fixtures_content_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/content")
    }

    /// Configuración de prueba: el directorio de fixtures como `CONTENT_DIR`,
    /// para no depender de los ficheros reales de `content/`.
    fn fixtures_config() -> Config {
        Config::from_env_iter([(
            "CONTENT_DIR".to_string(),
            fixtures_content_dir().to_string_lossy().into_owned(),
        )])
        .unwrap()
    }

    /// Directorio de contenido **sin ningún HTML**: sirve para comprobar que un
    /// fallo al leer la página no cambia el estado (auditoría I-2).
    fn no_html_config() -> Config {
        let content_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/no-html");

        Config::from_env_iter([(
            "CONTENT_DIR".to_string(),
            content_dir.to_string_lossy().into_owned(),
        )])
        .unwrap()
    }

    /// Fichero de fixture que representa `content/404.html`.
    fn fixture_404_html() -> &'static str {
        include_str!("../tests/fixtures/content/404.html")
    }

    fn get(uri: &str) -> Request<Body> {
        Request::builder().uri(uri).body(Body::empty()).unwrap()
    }

    async fn send(request: Request<Body>) -> axum::response::Response {
        build_router(&fixtures_config())
            .oneshot(request)
            .await
            .unwrap()
    }

    fn content_type(response: &axum::response::Response) -> String {
        response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_string()
    }

    /// Valor de una cabecera de la respuesta, si está presente.
    fn header_value(response: &axum::response::Response, name: &str) -> Option<String> {
        response
            .headers()
            .get(name)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned)
    }

    async fn body_bytes(response: axum::response::Response) -> Vec<u8> {
        response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec()
    }

    // --- Código devuelto (tasks.md 1.6) -----------------------------------

    #[tokio::test]
    async fn una_pagina_404_responde_404_y_no_200() {
        let response = send(get("/404")).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_ne!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn x_code_determina_el_estado_en_la_raiz() {
        let request = Request::builder()
            .uri("/")
            .header("X-Code", "503")
            .body(Body::empty())
            .unwrap();

        let response = send(request).await;
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_ne!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn la_raiz_sin_pistas_devuelve_el_codigo_por_defecto() {
        let response = send(get("/")).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn el_estado_no_depende_del_formato_pedido() {
        for uri in ["/503", "/503.json", "/503.xml", "/503.txt"] {
            let response = send(get(uri)).await;
            assert_eq!(
                response.status(),
                StatusCode::SERVICE_UNAVAILABLE,
                "estado incorrecto en {uri}"
            );
        }
    }

    // --- Endpoints de servicio (tasks.md 1.6) -----------------------------

    #[tokio::test]
    async fn los_endpoints_de_salud_responden_200() {
        for uri in ["/healthz", "/health", "/health/live", "/live"] {
            let response = send(get(uri)).await;
            assert_eq!(response.status(), StatusCode::OK, "fallo en {uri}");
            assert!(
                body_bytes(response).await.is_empty(),
                "`{uri}` no debe devolver una página de error"
            );
        }
    }

    #[tokio::test]
    async fn version_responde_json_con_la_clave_version() {
        let response = send(get("/version")).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert!(
            content_type(&response).starts_with("application/json"),
            "Content-Type inesperado: {}",
            content_type(&response)
        );

        let body = body_bytes(response).await;
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(
            json.get("version").is_some_and(|value| value.is_string()),
            "el JSON debe llevar la clave `version`: {json}"
        );
    }

    // --- Assets estáticos (tasks.md 1.8) ----------------------------------

    #[tokio::test]
    async fn los_css_se_sirven_desde_content_dir() {
        let response = send(get("/css/404.css")).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert!(
            content_type(&response).starts_with("text/css"),
            "Content-Type inesperado: {}",
            content_type(&response)
        );

        let body = body_bytes(response).await;
        assert!(!body.is_empty(), "el CSS servido no puede estar vacío");
    }

    // --- Normalización del path (auditoría B-1) ---------------------------

    #[tokio::test]
    async fn un_componente_curdir_en_el_path_no_sirve_la_pagina_como_asset() {
        let response = send(get("/./404.html")).await;

        assert_ne!(
            response.status(),
            StatusCode::OK,
            "`/./404.html` no puede servirse como asset con 200"
        );
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let body = String::from_utf8(body_bytes(response).await).unwrap();
        assert_eq!(body.trim(), fixture_404_html().trim());
    }

    #[tokio::test]
    async fn un_path_percent_codificado_no_sirve_la_pagina_como_asset() {
        for uri in ["/404%2Ehtml", "/40%34.html", "/404%2eHTML", "/%2E/404.html"] {
            let response = send(get(uri)).await;

            assert_eq!(
                response.status(),
                StatusCode::NOT_FOUND,
                "`{uri}` debe responder 404, no {:?}",
                response.status()
            );
        }
    }

    #[tokio::test]
    async fn el_html_de_una_pagina_de_error_no_se_sirve_nunca_con_200() {
        // Es la razón de ser del middleware: `content/404.html` existe, y aun
        // así `/404.html` es una página de error (404), no un asset (200).
        let response = send(get("/404.html")).await;

        assert_ne!(response.status(), StatusCode::OK);
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert!(
            content_type(&response).starts_with("text/html"),
            "Content-Type inesperado: {}",
            content_type(&response)
        );

        let body = String::from_utf8(body_bytes(response).await).unwrap();
        assert_eq!(body.trim(), fixture_404_html().trim());
    }

    #[tokio::test]
    async fn un_doble_percent_encoding_tampoco_esquiva_la_pagina_de_error() {
        // `/404%252Ehtml` decodifica a `/404%2Ehtml`, que **no** es
        // `/404.html`: no puede acabar sirviendo el fichero como asset.
        let response = send(get("/404%252Ehtml")).await;

        assert_ne!(response.status(), StatusCode::OK);
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn no_se_puede_salir_del_directorio_de_contenido() {
        for uri in [
            "/../Cargo.toml",
            "/..%2f..%2fetc%2fpasswd",
            "/css/../../Cargo.toml",
        ] {
            let response = send(get(uri)).await;
            let status = response.status();

            assert!(
                (400..=599).contains(&status.as_u16()),
                "`{uri}` debe estar en 400..=599, fue {status}"
            );

            let body = String::from_utf8_lossy(&body_bytes(response).await).to_string();
            assert!(
                !body.contains("[package]") && !body.contains("root:"),
                "`{uri}` no puede filtrar el contenido de un fichero: {body}"
            );
        }
    }

    // --- Solo códigos utilizables (auditoría B-2) -------------------------

    #[tokio::test]
    async fn un_codigo_no_utilizable_en_el_path_responde_el_defecto() {
        for uri in ["/200", "/100", "/204", "/304", "/1000", "/200.html"] {
            let response = send(get(uri)).await;

            assert_eq!(
                response.status(),
                StatusCode::NOT_FOUND,
                "`{uri}` debe responder el código por defecto (404), no {:?}",
                response.status()
            );
        }
    }

    #[tokio::test]
    async fn un_codigo_no_utilizable_deja_paso_a_la_cabecera() {
        let request = Request::builder()
            .uri("/200")
            .header("X-Code", "503")
            .body(Body::empty())
            .unwrap();

        assert_eq!(
            send(request).await.status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
    }

    #[tokio::test]
    async fn un_x_code_no_utilizable_cae_al_defecto() {
        for value in ["200", "100", "204", "0", "1000", "abc", ""] {
            let request = Request::builder()
                .uri("/")
                .header("X-Code", value)
                .body(Body::empty())
                .unwrap();

            let response = send(request).await;

            assert_eq!(
                response.status(),
                StatusCode::NOT_FOUND,
                "`X-Code: {value}` debe caer al defecto (404), no {:?}",
                response.status()
            );
        }
    }

    // --- Cuerpo HTML con caída (tasks.md 1.5) -----------------------------

    #[tokio::test]
    async fn el_html_cae_al_defecto_conservando_el_codigo() {
        // No hay `418.html`: el cuerpo es el de `404.html` y el estado sigue
        // siendo 418.
        let response = send(get("/418.html")).await;

        assert_eq!(response.status(), StatusCode::IM_A_TEAPOT);

        let body = String::from_utf8(body_bytes(response).await).unwrap();
        assert_eq!(body.trim(), fixture_404_html().trim());
    }

    // --- Cabeceras a nivel de router (tasks.md 1.7) -----------------------

    #[tokio::test]
    async fn el_404_lleva_x_robots_tag_y_no_lleva_retry_after() {
        let response = send(get("/404")).await;

        assert_eq!(
            header_value(&response, "X-Robots-Tag").as_deref(),
            Some("noindex, nofollow, nosnippet, noarchive")
        );
        assert_eq!(header_value(&response, "Retry-After"), None);
    }

    #[tokio::test]
    async fn el_503_lleva_retry_after_120() {
        let response = send(get("/503")).await;

        assert_eq!(
            header_value(&response, "Retry-After").as_deref(),
            Some("120")
        );
    }

    #[tokio::test]
    async fn el_json_se_anuncia_como_json() {
        let response = send(get("/404.json")).await;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert!(
            content_type(&response).starts_with("application/json"),
            "Content-Type inesperado: {}",
            content_type(&response)
        );
    }

    #[tokio::test]
    async fn sin_accept_el_formato_por_defecto_es_texto_plano() {
        let response = send(get("/404")).await;

        assert!(
            content_type(&response).starts_with("text/plain"),
            "Content-Type inesperado: {}",
            content_type(&response)
        );
    }

    // --- Un fallo al leer el HTML conserva el código (auditoría I-2) ------

    #[tokio::test]
    async fn sin_fichero_html_el_estado_sigue_siendo_el_codigo_resuelto() {
        // `CONTENT_DIR` no tiene `418.html` **ni** `404.html`.
        let response = build_router(&no_html_config())
            .oneshot(get("/418.html"))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::IM_A_TEAPOT);
        assert_ne!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            header_value(&response, "X-Robots-Tag").as_deref(),
            Some("noindex, nofollow, nosnippet, noarchive")
        );

        let body = String::from_utf8(body_bytes(response).await).unwrap();
        assert!(
            body.contains("418"),
            "el respaldo de texto debe llevar el código: {body}"
        );
    }

    // --- Endpoints de servicio y assets que no son páginas (tasks.md 1.6) -

    #[tokio::test]
    async fn un_asset_inexistente_es_una_pagina_de_error() {
        let response = send(get("/nada.css")).await;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            header_value(&response, "X-Robots-Tag").as_deref(),
            Some("noindex, nofollow, nosnippet, noarchive")
        );
    }

    #[tokio::test]
    async fn un_asset_existente_gana_a_la_cabecera_x_code() {
        // Decisión del spec ("Un asset existente gana a la página de error"):
        // `X-Code` no convierte un asset existente en una página de error.
        let request = Request::builder()
            .uri("/css/404.css")
            .header("X-Code", "503")
            .body(Body::empty())
            .unwrap();

        let response = send(request).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert!(
            content_type(&response).starts_with("text/css"),
            "Content-Type inesperado: {}",
            content_type(&response)
        );
    }
}
