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
//! /// `/health`, `/health/live`, `/live`, `/version`, `/metrics`), assets
//! /// estáticos de `config.content_dir` y *fallback* de páginas de error
//! /// (resolución de código y de formato + `render_page`). Compresión gzip y
//! /// copia de las cabeceras de correlación van en el mismo router, para que
//! /// cubran por igual páginas, endpoints y assets.
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
//! Contrato de la spec `response-extras` (`release-hardening`): la respuesta se
//! comprime con gzip cuando el cliente la pide, y las cuatro cabeceras de
//! correlación se copian de la petición a la respuesta cuando venían. Contrato
//! de la spec `observability`: `GET /metrics` expone el contador de páginas
//! servidas por código en texto Prometheus.
//!
//! El path se normaliza (percent-decode + plegado de `.` y `..`, ver
//! [`path`]) **antes** de clasificarlo, y la misma cadena normalizada se
//! reescribe en la URI que recibe `ServeDir`: si el clasificador mirara el path
//! crudo, `/./404.html` o `/404%2Ehtml` se servirían como asset con `200`, que
//! es justo lo que este proyecto corrige.

mod config;
mod metrics;
mod path;
mod render;
mod resolve;

use crate::config::{Config, LogFormat};
use crate::metrics::PageCounters;
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
use tower_http::compression::CompressionLayer;
use tower_http::services::ServeDir;
use tower_http::trace::{DefaultOnFailure, TraceLayer};
use tracing_subscriber::EnvFilter;

/// Dirección de escucha: detrás de Traefik, en todas las interfaces.
const BIND_ADDRESS: Ipv4Addr = Ipv4Addr::UNSPECIFIED;

/// Filtro de registro de reserva, solo si `LOG_LEVEL` no es un filtro de
/// `tracing` válido.
const FALLBACK_LOG_LEVEL: &str = "info";

/// Cabeceras de correlación que se copian de la petición a la respuesta.
///
/// Son exactamente estas cuatro: la aplicación no inventa ninguna ni copia
/// ninguna otra (spec `response-extras`, "Las cabeceras de correlación se
/// copian"), que es lo que las hace fiables para quien las consume: si una
/// aparece en la respuesta, es porque venía en la petición.
const CORRELATION_HEADERS: [&str; 4] = [
    "X-Request-Id",
    "X-Trace-Id",
    "X-Correlation-Id",
    "X-Amzn-Trace-Id",
];

/// Estado compartido por el router: la configuración y el contador de páginas.
///
/// El contador va aquí (y no en un `static`) para que sea **del router**: se
/// comparte entre sus clones —un único contador para el servicio— y no entre
/// routers distintos, que es lo que mantiene deterministas las cuentas de las
/// pruebas cuando corren en paralelo.
#[derive(Debug, Clone)]
struct AppState {
    config: Config,
    counters: PageCounters,
}

/// Construye el `Router` de axum: endpoints de servicio (`/healthz`,
/// `/health`, `/health/live`, `/live`, `/version`, `/metrics`), assets
/// estáticos de `config.content_dir` y *fallback* de páginas de error
/// (resolución de código y de formato + `render_page`).
fn build_router(config: &Config) -> Router {
    let state = AppState {
        config: config.clone(),
        counters: PageCounters::new(),
    };

    Router::new()
        .route("/healthz", get(alive))
        .route("/health", get(alive))
        .route("/health/live", get(alive))
        .route("/live", get(alive))
        .route("/version", get(version))
        // `/metrics` es un endpoint de servicio, como `/version`: una ruta
        // explícita, así que no pasa por la resolución de páginas de error (ni
        // hereda su `X-Robots-Tag`).
        .route("/metrics", get(metrics))
        // Los assets se sirven desde la **única** ruta de estáticos, la misma
        // que usa el render del HTML (`CONTENT_DIR`).
        .fallback_service(
            ServeDir::new(&config.content_dir).append_index_html_on_directories(false),
        )
        // El middleware se añade después de las rutas: axum solo envuelve lo
        // que ya existe, y el *fallback* es una de esas cosas.
        .layer(middleware::from_fn_with_state(state.clone(), error_pages))
        .with_state(state)
        // La correlación envuelve a todo el router —páginas de error, endpoints
        // de servicio y assets—, así que va después del middleware de páginas.
        .layer(middleware::from_fn(correlation))
        // La compresión es la capa **más externa**: decide sobre el mensaje
        // final —el que sale por el socket—, incluidas las cabeceras que hayan
        // añadido las capas internas, y así ninguna capa interna puede
        // encontrarse un cuerpo ya comprimido ni escribir un `Content-Length`
        // que ya no corresponde a lo que se envía.
        .layer(CompressionLayer::new())
}

/// Arranque real: `Config::from_env()?`, `tracing`, `axum::serve` en
/// `0.0.0.0:{config.port}`.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env()?;
    init_tracing(&config.log_level, config.log_format);

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

/// Instala el registro con el filtro de la configuración y el formato pedido.
///
/// Un `LOG_LEVEL` que no sea un filtro de `tracing` válido no aborta el
/// arranque: se avisa por `stderr` y se usa [`FALLBACK_LOG_LEVEL`]. Un
/// `LOG_LEVEL` vacío **no** se sustituye por el defecto: se entrega tal cual al
/// filtro, que decidirá qué habilita.
///
/// El formato sí es estricto, pero **no aquí**: un `LOG_FORMAT` desconocido ya
/// lo rechazó [`Config::from_env`], de modo que esta función solo puede recibir
/// uno de los dos formatos válidos.
fn init_tracing(log_level: &str, log_format: LogFormat) {
    let filter = match EnvFilter::try_new(log_level) {
        Ok(filter) => filter,
        Err(err) => {
            eprintln!("LOG_LEVEL no es un filtro válido ({err}); se usa `{FALLBACK_LOG_LEVEL}`");
            EnvFilter::new(FALLBACK_LOG_LEVEL)
        }
    };

    let installed = match log_format {
        LogFormat::Text => tracing_subscriber::fmt().with_env_filter(filter).try_init(),
        // `json` serializa el registro de traza, uno por línea: lo que un
        // colector puede parsear sin expresiones regulares.
        LogFormat::Json => tracing_subscriber::fmt()
            .with_env_filter(filter)
            .json()
            .try_init(),
    };

    if let Err(err) = installed {
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

/// `GET /metrics` -> el contador de páginas servidas, en texto Prometheus.
async fn metrics(State(state): State<AppState>) -> Response {
    let mut response = Response::new(Body::from(state.counters.render()));
    insert_header(
        response.headers_mut(),
        header::CONTENT_TYPE.as_str(),
        metrics::CONTENT_TYPE,
    );

    response
}

/// Middleware que copia las cabeceras de correlación de la petición a la
/// respuesta (spec `response-extras`, "Las cabeceras de correlación se copian").
///
/// Va sobre **todo** el router para que las reciban por igual las páginas de
/// error, los endpoints de servicio y los assets: quien depura un `503` no
/// quiere que el identificador de la traza desaparezca por el camino que tomó
/// la petición. Los valores se guardan **antes** de ceder el paso a la capa
/// interna (la petición se consume ahí) y se reinyectan en la respuesta con el
/// mismo valor; lo que no venía, no se añade.
async fn correlation(request: Request, next: Next) -> Response {
    let carried = correlation_headers(request.headers());

    let mut response = next.run(request).await;

    for (name, value) in carried {
        response.headers_mut().insert(name, value);
    }

    response
}

/// Las cabeceras de [`CORRELATION_HEADERS`] presentes en la petición, en ese
/// orden.
fn correlation_headers(headers: &HeaderMap) -> Vec<(&'static str, HeaderValue)> {
    CORRELATION_HEADERS
        .iter()
        .filter_map(|name| {
            let value = headers.get(*name)?.clone();
            Some((*name, value))
        })
        .collect()
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
async fn error_pages(State(state): State<AppState>, mut request: Request, next: Next) -> Response {
    let hints = Hints::new(request.headers());
    let normalized = NormalizedPath::new(request.uri().path());

    match normalized_uri(request.uri(), &normalized) {
        Some(uri) => *request.uri_mut() = uri,
        None => {
            // Inalcanzable con un path normalizado (siempre `/` + ASCII), pero
            // si pasara no se arriesga un asset servido con `200`: se responde
            // la página de error.
            tracing::warn!(path = normalized.as_str(), "no se pudo reescribir la URI");
            return error_response(&state, &normalized, &hints).await;
        }
    }

    if is_error_page_path(normalized.as_str()) {
        return error_response(&state, &normalized, &hints).await;
    }

    let response = next.run(request).await;

    // El asset no existe: en vez del `404` vacío de `ServeDir`, la página de
    // error del código resuelto.
    if response.status() == StatusCode::NOT_FOUND {
        return error_response(&state, &normalized, &hints).await;
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
/// Es el **único** punto donde se sirve una página de error, así que es donde
/// se cuenta: cada página servida suma uno al contador de su código. El código
/// ya está resuelto aquí y siempre es del rango utilizable, que es lo que hace
/// que el contador no pueda crecer sin límite.
///
/// El render lee del disco, así que va en `spawn_blocking`: el hilo del
/// ejecutor async no se bloquea con E/S síncrona (auditoría I-1).
async fn error_response(state: &AppState, path: &NormalizedPath, hints: &Hints) -> Response {
    let config = &state.config;

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

    state.counters.record(code);

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
    use flate2::read::GzDecoder;
    use http_body_util::BodyExt;
    use std::io::Read;
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

    /// `GET` con cabeceras de la petición, para las pruebas de compresión y de
    /// correlación.
    fn get_with_headers(uri: &str, headers: &[(&str, &str)]) -> Request<Body> {
        let mut builder = Request::builder().uri(uri);
        for (name, value) in headers {
            builder = builder.header(*name, *value);
        }

        builder.body(Body::empty()).unwrap()
    }

    /// Descomprime un cuerpo gzip **de verdad**.
    ///
    /// Comprobar la cabecera `Content-Encoding` no demuestra que el cuerpo sea
    /// el comprimido: estas pruebas descomprimen y comparan el resultado con el
    /// cuerpo en claro.
    fn gunzip(comprimido: &[u8]) -> Vec<u8> {
        let mut descomprimido = Vec::new();
        GzDecoder::new(comprimido)
            .read_to_end(&mut descomprimido)
            .expect("el cuerpo debe ser gzip válido");

        descomprimido
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

    // --- Compresión gzip (change `release-hardening`, tasks.md 1.1-1.3) ---
    //
    // La capa de compresión la añade la fase GREEN; aquí solo se fija el
    // contrato. Ojo al umbral de `tower_http::compression::predicate`: su
    // predicado por defecto (`SizeAbove`, 32 bytes) no comprime respuestas más
    // pequeñas, así que la prueba usa `/404` (cuerpo de texto de ~70 bytes) y no
    // un cuerpo mínimo. Los cuerpos concretos no se fijan aquí: se comparan
    // entre sí para que la prueba no dependa del texto de las páginas.

    #[tokio::test]
    async fn con_accept_encoding_gzip_la_respuesta_va_comprimida_y_conserva_el_codigo() {
        let response = send(get_with_headers("/404", &[("Accept-Encoding", "gzip")])).await;

        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "el estado debe seguir siendo el código resuelto"
        );
        assert_eq!(
            header_value(&response, "Content-Encoding").as_deref(),
            Some("gzip"),
            "con `Accept-Encoding: gzip` la respuesta debe ir comprimida"
        );

        let comprimido = body_bytes(response).await;
        assert!(
            !comprimido.is_empty(),
            "el cuerpo comprimido no puede estar vacío"
        );

        // El cuerpo comprimido **descomprimido** debe ser el de siempre, no una
        // cabecera que promete gzip y un cuerpo que no lo es.
        let descomprimido = gunzip(&comprimido);
        let en_claro = body_bytes(send(get("/404")).await).await;
        assert_eq!(
            descomprimido, en_claro,
            "el cuerpo descomprimido debe ser idéntico al de la respuesta sin comprimir"
        );
    }

    #[tokio::test]
    async fn sin_accept_encoding_la_respuesta_no_va_comprimida() {
        let response = send(get("/404")).await;

        assert_eq!(
            header_value(&response, "Content-Encoding"),
            None,
            "sin `Accept-Encoding` la respuesta no puede ir comprimida"
        );

        let body = body_bytes(response).await;
        assert_eq!(
            body,
            render_text_fallback(404).body,
            "sin `Accept-Encoding` el cuerpo debe ser el de siempre"
        );
    }

    #[tokio::test]
    async fn toda_respuesta_comprimible_declara_vary_accept_encoding() {
        // `Vary` es lo que impide que una caché intermedia entregue a un cliente
        // un cuerpo comprimido que no pidió; va en toda respuesta comprimible,
        // se comprima o no en esta petición concreta.
        for peticion in [
            get("/404"),
            get_with_headers("/404", &[("Accept-Encoding", "gzip")]),
        ] {
            let response = send(peticion).await;
            let vary = header_value(&response, "Vary").unwrap_or_default();

            assert!(
                vary.to_ascii_lowercase().contains("accept-encoding"),
                "falta `Vary: Accept-Encoding` (Vary = {vary:?})"
            );
        }
    }

    // --- Cabeceras de correlación (tasks.md 1.4-1.5) ----------------------

    /// Las cuatro cabeceras que la spec manda copiar de la petición a la
    /// respuesta, y **solo** esas.
    const CABECERAS_DE_CORRELACION: [&str; 4] = [
        "X-Request-Id",
        "X-Trace-Id",
        "X-Correlation-Id",
        "X-Amzn-Trace-Id",
    ];

    #[tokio::test]
    async fn el_identificador_de_peticion_se_copia_a_la_respuesta() {
        let response = send(get_with_headers("/404", &[("X-Request-Id", "abc123")])).await;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            header_value(&response, "X-Request-Id").as_deref(),
            Some("abc123"),
            "la respuesta debe repetir el `X-Request-Id` de la petición"
        );
    }

    #[tokio::test]
    async fn las_cuatro_cabeceras_de_correlacion_se_copian_juntas() {
        let response = send(get_with_headers(
            "/503",
            &[
                ("X-Request-Id", "req-1"),
                ("X-Trace-Id", "trace-2"),
                ("X-Correlation-Id", "corr-3"),
                ("X-Amzn-Trace-Id", "Root=1-abc"),
            ],
        ))
        .await;

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

        let esperadas =
            CABECERAS_DE_CORRELACION
                .iter()
                .zip(["req-1", "trace-2", "corr-3", "Root=1-abc"]);

        for (nombre, valor) in esperadas {
            assert_eq!(
                header_value(&response, nombre).as_deref(),
                Some(valor),
                "`{nombre}` no se copió a la respuesta"
            );
        }
    }

    #[tokio::test]
    async fn una_cabecera_de_correlacion_ausente_no_se_inventa() {
        let response = send(get("/404")).await;

        for nombre in CABECERAS_DE_CORRELACION {
            assert_eq!(
                header_value(&response, nombre),
                None,
                "la respuesta no puede inventarse `{nombre}`"
            );
        }
    }

    #[tokio::test]
    async fn un_asset_existente_tambien_propaga_las_cabeceras_de_correlacion() {
        // El asset de los fixtures es `css/404.css`; el `content/` real sirve
        // `css/error.css`, pero las pruebas no dependen de él. Un asset
        // inexistente sería una página de error y no probaría este camino.
        let response = send(get_with_headers(
            "/css/404.css",
            &[("X-Request-Id", "asset-7")],
        ))
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            header_value(&response, "X-Request-Id").as_deref(),
            Some("asset-7"),
            "un asset también debe propagar las cabeceras de correlación"
        );
    }

    #[tokio::test]
    async fn un_endpoint_de_servicio_tambien_propaga_las_cabeceras() {
        let response = send(get_with_headers(
            "/version",
            &[("X-Correlation-Id", "svc-9")],
        ))
        .await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            header_value(&response, "X-Correlation-Id").as_deref(),
            Some("svc-9"),
            "un endpoint de servicio también debe propagar las cabeceras"
        );
    }

    // --- Métricas (tasks.md 1.6-1.7) --------------------------------------
    //
    // El contador es del **router**: se comparte entre sus clones y no entre
    // routers distintos. En producción hay un único router, así que es el
    // contador del servicio; en pruebas, un router propio es lo que hace
    // deterministas las cuentas, porque las demás pruebas corren en paralelo y
    // también piden `/404`.

    /// Valor del contador de un código, leído del texto Prometheus.
    ///
    /// Se busca la **línea entera** (`errors_pages_total{code="404"} 3`): un
    /// `contains` sobre el cuerpo daría por bueno un `33`.
    fn contador_prometheus(cuerpo: &str, code: u16) -> Option<u64> {
        let prefijo = format!("errors_pages_total{{code=\"{code}\"}} ");

        cuerpo
            .lines()
            .find_map(|linea| linea.strip_prefix(&prefijo))
            .and_then(|valor| valor.trim().parse().ok())
    }

    /// Router propio de la prueba: se le piden las páginas de error indicadas
    /// —`(código, veces)`— y después se pide `/metrics` a **ese mismo** router.
    async fn metrics_tras(peticiones: &[(u16, usize)]) -> axum::response::Response {
        let router = build_router(&fixtures_config());

        for (code, veces) in peticiones {
            for _ in 0..*veces {
                let uri = format!("/{code}");
                let response = router.clone().oneshot(get(&uri)).await.unwrap();

                assert_eq!(
                    response.status().as_u16(),
                    *code,
                    "la petición de preparación `{uri}` no devolvió {code}"
                );
            }
        }

        router.oneshot(get("/metrics")).await.unwrap()
    }

    #[tokio::test]
    async fn metrics_es_un_endpoint_de_servicio_y_no_una_pagina_de_error() {
        let response = build_router(&fixtures_config())
            .oneshot(get("/metrics"))
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            StatusCode::OK,
            "`/metrics` debe responder 200"
        );
        assert!(
            content_type(&response).starts_with("text/plain"),
            "Content-Type inesperado: {}",
            content_type(&response)
        );
        assert!(
            content_type(&response).contains("version=0.0.4"),
            "el `Content-Type` debe ser el de Prometheus, fue: {}",
            content_type(&response)
        );
        assert_eq!(
            header_value(&response, "X-Robots-Tag"),
            None,
            "`/metrics` no puede pasar por la resolución de páginas de error"
        );

        let cuerpo = String::from_utf8(body_bytes(response).await).unwrap();
        assert!(
            cuerpo.contains("errors_pages_total"),
            "`/metrics` debe declarar el contador:\n{cuerpo}"
        );
    }

    #[tokio::test]
    async fn el_contador_refleja_las_paginas_servidas_por_codigo() {
        let response = metrics_tras(&[(404, 3), (503, 1)]).await;

        assert_eq!(response.status(), StatusCode::OK);
        let cuerpo = String::from_utf8(body_bytes(response).await).unwrap();

        assert_eq!(
            contador_prometheus(&cuerpo, 404),
            Some(3),
            "tras 3 peticiones a `/404` el contador debe declarar 3:\n{cuerpo}"
        );
        assert_eq!(
            contador_prometheus(&cuerpo, 503),
            Some(1),
            "tras 1 petición a `/503` el contador debe declarar 1:\n{cuerpo}"
        );
    }

    #[tokio::test]
    async fn metrics_declara_la_ayuda_y_el_tipo_del_contador() {
        let response = metrics_tras(&[(404, 1)]).await;
        let cuerpo = String::from_utf8(body_bytes(response).await).unwrap();

        assert!(
            cuerpo.contains("# HELP errors_pages_total Páginas de error servidas, por código."),
            "falta la ayuda (`# HELP`) del contador:\n{cuerpo}"
        );
        assert!(
            cuerpo.contains("# TYPE errors_pages_total counter"),
            "falta el tipo (`# TYPE ... counter`) del contador:\n{cuerpo}"
        );
    }
}
