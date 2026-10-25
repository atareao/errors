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
//! Contrato de la spec `error-page` (change `self-contained-themes`): el HTML
//! servido es **autocontenido** —el marcador `<link rel="stylesheet"
//! href="/css/error.css" />` se sustituye al servir por un `<style>` con la hoja
//! del tema elegido (ver [`theme`])—, el tema del servicio lo elige `THEME` y,
//! por petición, `X-Theme`, que además se declara en `Vary`.
//!
//! El path se normaliza (percent-decode + plegado de `.` y `..`, ver
//! [`path`]) **antes** de clasificarlo, y la misma cadena normalizada se
//! reescribe en la URI que recibe `ServeDir`: si el clasificador mirara el path
//! crudo, `/./404.html` o `/404%2Ehtml` se servirían como asset con `200`, que
//! es justo lo que este proyecto corrige.

mod config;
mod embedded;
mod metrics;
mod path;
mod render;
mod resolve;
mod theme;

use crate::config::{Config, LogFormat};
use crate::metrics::PageCounters;
use crate::path::NormalizedPath;
use crate::render::{RenderedPage, render_page, render_text_fallback};
use crate::resolve::{code_from_path, resolve_code_with_source, resolve_format};
use axum::Router;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::uri::{PathAndQuery, Uri};
use axum::http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, header};
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

/// Paths de los **endpoints de servicio** que declara [`build_router`].
///
/// Están aquí para que el middleware [`error_pages`] los deje fuera de la
/// sustitución: un endpoint de servicio **no es una página de error**. Un método
/// que el endpoint no admite es un hecho sobre *esa ruta* —axum lo dice con un
/// `405` y su `Allow`—, y convertirlo en la página de error da un `404` que a la
/// vez declara qué métodos admite la ruta: incoherente, y una mentira sobre qué
/// es esa ruta.
///
/// **Esta lista tiene que seguir a la de [`build_router`]**: es exactamente la
/// de las rutas que se registran allí. Si mañana se añade un endpoint de
/// servicio en `build_router` y no aquí, sus estados de error volverán a salir
/// como páginas de error.
const SERVICE_ENDPOINTS: [&str; 6] = [
    "/healthz",
    "/health",
    "/health/live",
    "/live",
    "/version",
    "/metrics",
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
///
/// Los paths de esas seis rutas de servicio son los de [`SERVICE_ENDPOINTS`]:
/// las dos listas van juntas, y añadir una ruta aquí sin añadirla allí deja sus
/// estados de error a merced de [`error_pages`].
fn build_router(config: &Config) -> Router {
    let state = AppState {
        config: config.clone(),
        counters: PageCounters::new(),
    };

    Router::new()
        // Estas seis rutas son las de [`SERVICE_ENDPOINTS`], la lista que usa
        // [`error_pages`] para no sustituir sus estados de error: las dos van
        // juntas.
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
        // que usa el render del HTML (`CONTENT_DIR`). El respaldo del
        // `ServeDir` sirve la copia **embebida** cuando no hay fichero: así un
        // binario instalado sin ficheros de datos sirve su hoja igual (D2).
        .fallback_service(
            ServeDir::new(&config.content_dir)
                .append_index_html_on_directories(false)
                .fallback(embedded_assets()),
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

/// Servicio de respaldo de `ServeDir`: sirve los assets **embebidos** cuando no
/// hay ningún fichero en `CONTENT_DIR`.
///
/// Es el segundo escalón de D2. `ServeDir` sigue intentando el disco primero
/// —así el fichero de un operador gana cuando se puede leer (D1)— y solo cuando
/// no encuentra el fichero cede el paso aquí. El `fallback` **no** altera el
/// estado que devolvemos, así que un path que no esté en la tabla tiene que
/// responder `404` —el middleware lo sustituye por la página de error— y no un
/// `200` de más: lo fijan las pruebas 1.9 y 1.9c.
///
/// El camino corre en el hilo *async* —a diferencia del render, que va en
/// `spawn_blocking` por la auditoría I-1—, y por eso aquí **no** hay E/S de
/// disco: los bytes son la tabla en memoria de [`embedded`]. La forma es un
/// `Router` de axum porque el `Service` que `ServeDir::fallback` espera es
/// `tower::Service`, que este crate solo tiene como dependencia de desarrollo y
/// no puede nombrar en producción; el `Router` **es** ese `Service` de verdad
/// —`Clone + Send`, con un `poll_ready` que devuelve `Ready` sin bloquear— y no
/// un cierre suelto.
fn embedded_assets() -> Router {
    Router::new().fallback(embedded_asset)
}

/// Respaldo de [`embedded_assets`]: el asset embebido del path, o `404`.
///
/// El path ya viene **normalizado** —el middleware [`error_pages`] reescribió la
/// URI antes de ceder el paso, y `fold_components` pliega la barra final—, así
/// que no hay una segunda forma de llegar al asset (tarea 3.3). La igualdad es
/// **exacta**, sin prefijos ni subcadenas: lo que la discrimina es un path que
/// **extiende** al del asset (`/css/error.css/extra`), que no tiene segmentos
/// vacíos que plegar y llega entero al respaldo; una consulta con `starts_with`
/// lo serviría con `200`.
async fn embedded_asset(request: Request) -> Response {
    let Some((bytes, content_type)) = embedded::asset(request.uri().path()) else {
        return not_found_response();
    };

    let is_head = request.method() == Method::HEAD;

    // A `HEAD` se le responde como al `GET` —estado y `Content-Type`— pero sin
    // cuerpo; el `Content-Length` del asset se deja, que es lo que haría el
    // `GET`.
    let mut response = Response::new(if is_head {
        Body::empty()
    } else {
        Body::from(bytes)
    });

    insert_header(
        response.headers_mut(),
        header::CONTENT_TYPE.as_str(),
        content_type,
    );

    if is_head {
        insert_header(
            response.headers_mut(),
            header::CONTENT_LENGTH.as_str(),
            &bytes.len().to_string(),
        );
    }

    response
}

/// `404` sin cuerpo: lo que un path fuera de la tabla de assets responde para
/// que el middleware lo sustituya por la página de error.
///
/// Con `fallback` nadie corrige el estado de esta respuesta, así que devolver
/// aquí un `200` de más saldría tal cual (D2).
fn not_found_response() -> Response {
    let mut response = Response::new(Body::empty());
    *response.status_mut() = StatusCode::NOT_FOUND;
    response
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
///
/// Como la respuesta depende de esas cabeceras, aquí mismo se declaran en
/// `Vary`: ver [`declare_vary`].
async fn correlation(request: Request, next: Next) -> Response {
    let carried = correlation_headers(request.headers());

    let mut response = next.run(request).await;

    for (name, value) in carried {
        response.headers_mut().insert(name, value);
    }

    declare_vary(response.headers_mut());

    response
}

/// Declara en `Vary` que la respuesta depende de las cabeceras de correlación y
/// de `X-Theme` (spec `response-extras`, "Las cabeceras de correlación se
/// declaran en Vary").
///
/// Su contenido cambia según esas cabeceras: las de correlación se copian de la
/// petición —sin declararlas, una caché intermedia podría servirle a un usuario
/// el identificador de otro— y `X-Theme` cambia **el cuerpo** entero del HTML,
/// porque elige la hoja que va incrustada (sin declararlo, a un cliente le
/// llegaría el tema que pidió otro, o el cuerpo de otro).
///
/// Se añade **siempre**, traiga o no la petición esas cabeceras —una lista que
/// cambiase según la petición no le serviría a una caché—, y con `append` porque
/// `Vary` es una cabecera de lista: si alguna capa ya declaró algo, se une en vez
/// de pisarse.
///
/// La lista declara también `Accept-Encoding` —que es lo que hace que la capa de
/// compresión, al encontrarlo ya declarado, no añada un segundo `Vary` con lo
/// mismo—, y va en un **único** valor: una caché tiene que leer las seis
/// declaraciones juntas, y repartirlas en dos valores del mismo campo no aporta
/// nada.
fn declare_vary(headers: &mut HeaderMap) {
    let value = format!(
        "Accept-Encoding, {}, X-Theme",
        CORRELATION_HEADERS.join(", ")
    );

    append_header(headers, header::VARY.as_str(), &value);
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

/// Cabeceras de la petición que alimentan las cascadas de código y de formato,
/// y la elección de tema.
struct Hints {
    x_code: Option<String>,
    content_type: Option<String>,
    x_format: Option<String>,
    accept: Option<String>,
    /// `X-Theme`: el tema que pide **esta** petición. Es una pista, no un error
    /// si viene mal: un valor desconocido cae al tema configurado (ver
    /// [`theme::resolve`]).
    x_theme: Option<String>,
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
            x_theme: read("x-theme"),
        }
    }
}

/// Middleware de cierre: lo que no es un asset estático existente —ni un
/// endpoint de servicio— es una página de error, con el estado igual al código
/// resuelto (nunca `200`).
///
/// Va en un middleware y no en un `fallback` para que las páginas de error
/// manden sobre los ficheros: si no, `/404.html` se serviría desde
/// `CONTENT_DIR/404.html` con un `200`, justo lo que este proyecto corrige.
///
/// Toda respuesta del servidor de ficheros **con estado de error** se sustituye
/// por la página de error —no solo su `404`—: también el `405` de un método que
/// no sirve estáticos, el `416` de un `Range` múltiple y el `500` de un
/// segmento de path que no cabe como nombre de fichero, que salían con el
/// cuerpo vacío y sin las cabeceras de la página (spec `error-page`, "Ningún
/// estado de error del servidor de ficheros se escapa").
///
/// Los **endpoints de servicio** quedan fuera: no son el servidor de ficheros y
/// no son páginas de error. Ver [`SERVICE_ENDPOINTS`].
///
/// El estado de la página es **el código resuelto**, nunca el suyo: el estado
/// del servidor de ficheros no es una fuente de la cascada. Conservarlo —lo que
/// hacía la primera versión de este change— salía peor que el defecto, medido
/// con el binario delante: obligaba a propagar cabeceras que describen *su*
/// respuesta (`Allow` en el `405`, `Content-Range` en el `416`) so pena de
/// incumplir un `MUST` de RFC 9110; dejaba el estado y el cuerpo en desacuerdo
/// (`GET /nada.png` con `X-Code: 503` salía `404` con el cuerpo del `503` y su
/// `Retry-After: 120`); y con `DEFAULT_ERROR_PAGE=503` convertía cualquier
/// asset inexistente en un `404`, dejando sin efecto a `X-Code`.
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

    // Los endpoints de servicio —`SERVICE_ENDPOINTS`, los que declara
    // `build_router`— quedan fuera de la sustitución: no son páginas de
    // error. Su `405` —el método que no admiten— es un estado honesto del
    // endpoint y lo dice con su `Allow`; convertir esa respuesta en una página
    // nuestra produce un `404` que a la vez declara qué métodos admite la ruta,
    // que es tanto como mentir sobre qué es esa ruta. Es el estado que daba
    // axum aquí antes de este change.
    //
    // El límite, dicho: hoy un endpoint de servicio solo devuelve `200` o el
    // `405` del propio router, así que esta salida temprana no se está saltando
    // ningún `5xx` suyo. Si algún día uno devolviera un `5xx`, ese `5xx` **no**
    // pasaría por esta vía: saldría tal cual, sin página de error. Es el lado
    // correcto por el que caer —un fallo interno de un endpoint de servicio no
    // es una petición de página de error—, pero conviene saberlo.
    if is_service_endpoint(normalized.as_str()) {
        return response;
    }

    let status = response.status();

    // Cualquier estado de error del servidor de ficheros se sustituye por la
    // página de error: es la única forma de que ninguno salga sin cuerpo ni sin
    // nuestras cabeceras. Lo que **no** se toca:
    //
    // - El `200` de un asset existente: se sirve tal cual, y convertirlo en
    //   página de error es el fallo que este proyecto corrige.
    // - El `304` de una petición condicional (`If-Modified-Since`), que es el
    //   único `3xx` que `ServeDir` emite aquí. Se deja pasar **a propósito**: no
    //   es un estado de error, no admite cuerpo y es la respuesta que el cliente
    //   pidió. (Este es el `3xx` al que se refería el comentario viejo, que
    //   decía que `append_index_html_on_directories(false)` los suprimía: eso es
    //   falso —esa opción suprime la redirección de directorio, no los `3xx`—,
    //   así que no lo «endurezcas» mañana leyendo aquella frase.)
    //
    // La condición de abajo es la que distingue: un `304` no es error de cliente
    // ni de servidor, así que no entra.
    //
    // El estado del servidor de ficheros **no** se pasa a `error_response`: no
    // es una fuente de la cascada, así que el estado de la página es el código
    // resuelto. Conservarlo obligaba a propagar el `Allow` del `405` o el
    // `Content-Range` del `416` so pena de incumplir un `MUST` de RFC 9110,
    // dejaba el estado y el cuerpo en desacuerdo y anulaba `X-Code`.
    if status.is_client_error() || status.is_server_error() {
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

/// ¿El path —ya normalizado— es uno de los endpoints de servicio
/// ([`SERVICE_ENDPOINTS`])?
///
/// Compara sobre el path **normalizado** —el mismo que usa el resto del
/// middleware, no el crudo de la petición— y exige **igualdad exacta**: otro
/// path que no sea uno de los seis registrados en [`build_router`] no es un
/// endpoint de servicio, así que sigue su camino como cualquier otro (y, al no
/// existir como asset, acaba en la página de error). `/healthz/` o `/metrics/x`
/// no son esos endpoints.
fn is_service_endpoint(path: &str) -> bool {
    SERVICE_ENDPOINTS.contains(&path)
}

/// Resuelve código y formato y renderiza la página de error.
///
/// Es el **único** punto donde se sirve una página de error, así que es donde
/// se cuenta: cada página servida suma uno al contador del par `(código, origen
/// del código)`. El código ya está resuelto aquí y siempre es del rango
/// utilizable, que es lo que hace que el contador no pueda crecer sin límite.
///
/// El estado, el cuerpo y las cabeceras salen del **código resuelto** por la
/// cascada: el estado de la respuesta del servidor de ficheros que esta página
/// sustituye no es una de las fuentes de esa cascada. Conservarlo obligaba a
/// propagar cabeceras que describen *su* respuesta —el `Allow` del `405`, el
/// `Content-Range` del `416`— so pena de incumplir un `MUST` de RFC 9110,
/// dejaba el estado y el cuerpo en desacuerdo (un `404` con el `Retry-After` y
/// el cuerpo del `503`) y, con `DEFAULT_ERROR_PAGE=503`, convertía cualquier
/// asset inexistente en un `404`, dejando sin efecto a `X-Code`.
///
/// El render lee del disco, así que va en `spawn_blocking`: el hilo del
/// ejecutor async no se bloquea con E/S síncrona (auditoría I-1).
async fn error_response(state: &AppState, path: &NormalizedPath, hints: &Hints) -> Response {
    let config = &state.config;

    let (code, source) = resolve_code_with_source(
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

    // El tema se resuelve por petición: un `X-Theme` **conocido** gana al tema
    // configurado y un valor ausente o desconocido cae a él (ver
    // [`theme::resolve`]). Es lo mismo que hacen las cascadas de código y de
    // formato, y por eso el tema viaja hasta el render: sin `X-Theme` la hoja
    // incrustada es siempre la del servicio.
    let theme = theme::resolve(hints.x_theme.as_deref(), config.theme);

    state.counters.record(code, source);

    let rendered = tokio::task::spawn_blocking({
        let config = config.clone();
        move || render_page(&config, code, format, theme)
    })
    .await;

    match rendered {
        Ok(Ok(page)) => page_response(page),
        Ok(Err(err)) => {
            // Falta el HTML del código y el del código por defecto (o no se
            // pudo leer). El estado sigue siendo el código resuelto y nunca
            // `500`: se responde el cuerpo de texto (auditoría I-2). El mensaje
            // no habla de «existe o no» porque cubre las dos causas, y la
            // precisa —`MissingHtml` habla de las dos capas, `Io` de la lectura—
            // viaja en el campo `error`.
            tracing::warn!(
                code,
                error = %err,
                "no se pudo obtener el HTML de la página de error: se responde texto"
            );
            page_response(render_text_fallback(code, config.retry_after))
        }
        Err(err) => {
            tracing::error!(code, error = %err, "la tarea de render falló: se responde texto");
            page_response(render_text_fallback(code, config.retry_after))
        }
    }
}

/// Convierte una página renderizada en la respuesta de axum. `Content-Length`
/// lo calcula hyper a partir del cuerpo; el resto de cabeceras van aquí.
///
/// El estado es el del código resuelto ([`RenderedPage::status`]) y las
/// cabeceras de acompañamiento se calcularon a partir de él
/// ([`RenderedPage::headers`]), así que el estado, el cuerpo y el `Retry-After`
/// no pueden discrepar.
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
///
/// `insert` **sustituye** lo que hubiera con ese nombre: para las cabeceras de
/// la página de error, que no se repiten, es lo correcto.
fn insert_header(headers: &mut HeaderMap, name: &str, value: &str) {
    let Ok(name) = HeaderName::from_bytes(name.as_bytes()) else {
        return;
    };
    let Ok(value) = HeaderValue::from_str(value) else {
        return;
    };

    headers.insert(name, value);
}

/// Añade una cabecera a las que ya hubiera con ese nombre, en vez de pisarlas.
///
/// Es lo que necesita una cabecera de lista como `Vary`; con `insert` se
/// perdería lo que otra capa (la compresión) haya declarado.
fn append_header(headers: &mut HeaderMap, name: &str, value: &str) {
    let Ok(name) = HeaderName::from_bytes(name.as_bytes()) else {
        return;
    };
    let Ok(value) = HeaderValue::from_str(value) else {
        return;
    };

    headers.append(name, value);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::metrics::COUNTER_NAME;
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

    /// Configuración de prueba con `DEFAULT_ERROR_PAGE=503`: el defecto
    /// configurado tiene que mandar sobre el `404` del servidor de ficheros.
    fn default_503_config() -> Config {
        Config::from_env_iter([
            (
                "CONTENT_DIR".to_string(),
                fixtures_content_dir().to_string_lossy().into_owned(),
            ),
            ("DEFAULT_ERROR_PAGE".to_string(), "503".to_string()),
        ])
        .unwrap()
    }

    /// Configuración de prueba con `RETRY_AFTER` fijado: el valor configurado
    /// tiene que llegar también a una página de error cuyo código no lo aporta
    /// el path, sino la cabecera `X-Code`.
    fn fixtures_config_con_retry_after(value: &str) -> Config {
        Config::from_env_iter([
            (
                "CONTENT_DIR".to_string(),
                fixtures_content_dir().to_string_lossy().into_owned(),
            ),
            ("RETRY_AFTER".to_string(), value.to_string()),
        ])
        .expect("la configuración de fixtures debe construirse")
    }

    /// Fichero de fixture que representa `content/404.html`.
    fn fixture_404_html() -> &'static str {
        include_str!("../tests/fixtures/content/404.html")
    }

    /// Página **embebida** del `503`: el fichero del repositorio (`content/503.html`)
    /// que la fase GREEN embebe. Se fija aquí con `include_str!` —y no por la API
    /// del módulo `embedded`, que aún no existe en RED— para no mezclar las dos
    /// fases.
    const EMBEDDED_503_HTML: &str = include_str!("../content/503.html");
    /// Página **embebida** del `404`: el fichero del repositorio (`content/404.html`,
    /// 460 bytes), distinto del fixture de 271 bytes de `fixture_404_html`.
    const EMBEDDED_404_HTML: &str = include_str!("../content/404.html");
    /// Hoja de estilos **embebida**: `content/css/error.css`.
    const EMBEDDED_CSS: &str = include_str!("../content/css/error.css");
    /// Hoja de estilos del directorio de fixtures: **propia y distinguible** de
    /// la embebida (`tests/fixtures/content/css/error.css`).
    const FIXTURE_CSS: &str = include_str!("../tests/fixtures/content/css/error.css");

    /// El marcador del HTML autocontenido, tal cual está en las páginas de
    /// `content/`: el render lo sustituye al servir por un `<style>` con la hoja
    /// (change `self-contained-themes`).
    const MARCADOR: &str = r#"<link rel="stylesheet" href="/css/error.css" />"#;

    /// La página tal como se **sirve**: el marcador sustituido por un `<style>`
    /// con la hoja. Es el contrato del HTML autocontenido —una página con
    /// marcador ya no enlaza la hoja, la lleva dentro— y estas pruebas lo fijan
    /// sin depender de la implementación.
    fn pagina_autocontenida(pagina: &str, hoja: &str) -> Vec<u8> {
        pagina
            .replace(MARCADOR, &format!("<style>\n{hoja}\n</style>"))
            .into_bytes()
    }

    /// Directorio de contenido que **no existe**: el caso de un binario
    /// instalado sin ficheros de datos, con `CONTENT_DIR` apuntando a un
    /// `content` que no está.
    ///
    /// La aserción de abajo impide que alguien cree el directorio por accidente
    /// y convierta estas pruebas en otra cosa.
    fn sin_contenido_dir() -> PathBuf {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/inexistente-y-ademas-inexistente");

        assert!(
            !dir.exists(),
            "el fixture `sin contenido` no puede existir: {}",
            dir.display()
        );

        dir
    }

    /// `Config` con un `CONTENT_DIR` inexistente y el defecto por defecto
    /// (`404`): la capa del operador no aporta nada y tiene que entrar la
    /// embebida.
    fn sin_contenido_config() -> Config {
        Config::from_env_iter([(
            "CONTENT_DIR".to_string(),
            sin_contenido_dir().to_string_lossy().into_owned(),
        )])
        .expect("la configuración de fixtures debe construirse")
    }

    /// `Config` con un `CONTENT_DIR` **sin ningún HTML** (`tests/fixtures/no-html`)
    /// y `DEFAULT_ERROR_PAGE=418`: un defecto legal pero **fuera** de la tabla
    /// embebida (los once son `400, 401, 403, 404, 405, 408, 429, 500, 502, 503,
    /// 504`). Es el único camino que le queda al respaldo de texto una vez las
    /// páginas están embebidas (D4).
    ///
    /// Reutiliza [`no_html_config`] —el directorio sin HTML ya existente— y le
    /// cambia solo el defecto, para no dejar ese ayudante sin usar.
    fn default_418_sin_html_config() -> Config {
        let mut config = no_html_config();
        config.default_error_page = 418;

        config
    }

    /// Hoja de estilos de un tema **nuevo**: la fase GREEN la crea en
    /// `content/themes/<nombre>.css` (tasks.md 2.2).
    ///
    /// Se lee en **tiempo de ejecución** —y no con `include_str!`— a propósito:
    /// en RED el fichero todavía no existe, y un `include_str!` rompería la
    /// **compilación** de toda la suite. Leída así, la prueba compila y falla
    /// aquí, en el `expect`, que es como la fase RED debe fallar.
    fn hoja_de_tema(nombre: &str) -> String {
        let ruta = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("content/themes")
            .join(format!("{nombre}.css"));
        let mensaje = format!(
            "la fase GREEN tiene que crear la hoja del tema `{nombre}` en {}",
            ruta.display()
        );

        std::fs::read_to_string(&ruta).expect(&mensaje)
    }

    /// La configuración de las pruebas —la de las páginas **embebidas**, que son
    /// las que llevan el marcador— con `THEME` puesto.
    ///
    /// Hoy `THEME` es una clave desconocida para [`Config::from_env_iter`], así
    /// que en RED esta configuración sale con el tema por defecto y la prueba
    /// llega a su aserción de comportamiento; en GREEN `THEME` se parsea.
    fn config_con_theme(theme: &str) -> Config {
        Config::from_env_iter([
            (
                "CONTENT_DIR".to_string(),
                sin_contenido_dir().to_string_lossy().into_owned(),
            ),
            ("THEME".to_string(), theme.to_string()),
        ])
        .expect("la configuración de pruebas con `THEME` debe construirse")
    }

    /// Directorio de contenido con una hoja **propia del operador** para el tema
    /// `terminal` (`tests/fixtures/tema-del-operador/themes/terminal.css`) y
    /// **sin** `404.html`: la página sale de la capa embebida y lo que se prueba
    /// es que su hoja gana a la copia embebida del tema.
    fn tema_del_operador_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tema-del-operador")
    }

    /// `Config` con la hoja del operador de [`tema_del_operador_dir`] y
    /// `THEME=terminal`.
    fn config_tema_del_operador() -> Config {
        Config::from_env_iter([
            (
                "CONTENT_DIR".to_string(),
                tema_del_operador_dir().to_string_lossy().into_owned(),
            ),
            ("THEME".to_string(), "terminal".to_string()),
        ])
        .expect("la configuración del fixture `tema-del-operador` debe construirse")
    }

    /// Directorio de contenido donde el tema `terminal` espera su hoja en
    /// `themes/terminal.css`, pero ahí hay un **directorio**
    /// (`tests/fixtures/hoja-ilegible`): leerlo es un error real y la hoja no se
    /// puede obtener.
    fn hoja_ilegible_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/hoja-ilegible")
    }

    /// `Config` con la hoja ilegible de [`hoja_ilegible_dir`] y `THEME=terminal`.
    fn config_hoja_ilegible() -> Config {
        Config::from_env_iter([
            (
                "CONTENT_DIR".to_string(),
                hoja_ilegible_dir().to_string_lossy().into_owned(),
            ),
            ("THEME".to_string(), "terminal".to_string()),
        ])
        .expect("la configuración del fixture `hoja-ilegible` debe construirse")
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

    #[tokio::test]
    async fn un_metodo_no_admitido_sobre_un_endpoint_de_servicio_conserva_el_405_de_axum() {
        // Los endpoints de servicio no son páginas de error: su `405` —el
        // método que no admiten— es un estado honesto del endpoint y lo dice con
        // su `Allow`. Interceptarlo daba un `404` que a la vez declaraba qué
        // métodos admite la ruta, y eso no describe ninguna página de error.
        // Los seis endpoints de servicio, con su path literal —no leído de
        // `SERVICE_ENDPOINTS`—: la prueba comprueba así que la lista del código
        // sigue siendo la de `build_router`.
        for uri in [
            "/healthz",
            "/health",
            "/health/live",
            "/live",
            "/version",
            "/metrics",
        ] {
            for method in ["POST", "PUT"] {
                let request = Request::builder()
                    .method(method)
                    .uri(uri)
                    .body(Body::empty())
                    .unwrap();

                let response = send(request).await;

                assert_eq!(
                    response.status(),
                    StatusCode::METHOD_NOT_ALLOWED,
                    "`{method} {uri}` debe conservar el 405 de axum, no {:?}",
                    response.status()
                );
                assert_eq!(
                    header_value(&response, "Allow").as_deref(),
                    Some("GET,HEAD"),
                    "`{method} {uri}`: el 405 del endpoint describe la ruta con su `Allow`"
                );
                assert_eq!(
                    header_value(&response, "X-Robots-Tag"),
                    None,
                    "`{method} {uri}`: un endpoint de servicio no lleva las cabeceras \
                     de la página de error"
                );

                let body = body_bytes(response).await;
                assert!(
                    body.is_empty(),
                    "`{method} {uri}`: la respuesta no puede ser una página de error, \
                     y este cuerpo no está vacío: {}",
                    String::from_utf8_lossy(&body)
                );
            }
        }
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

    #[tokio::test]
    async fn un_asset_con_if_modified_since_en_el_futuro_responde_304_y_no_una_pagina() {
        // El único `3xx` que `ServeDir` emite aquí es el `304` de una petición
        // condicional, y pasa tal cual **a propósito**: no es un estado de
        // error, no admite cuerpo y es la respuesta que el cliente pidió. Ni se
        // intercepta ni se convierte en página de error.
        //
        // La fecha tiene que ser un IMF-fixdate válido **y con el día de la
        // semana correcto**: `httpdate` valida el día y descarta la cabecera si
        // no cuadra (`Wed, 21 Oct 2030` es un lunes, así que esa fecha se
        // ignora y la respuesta sería un `200`). Esta está bien formada, es
        // futura para cualquier copia del repositorio y el `Last-Modified` del
        // fixture nunca la alcanza.
        let response = send(get_with_headers(
            "/css/404.css",
            &[("If-Modified-Since", "Tue, 01 Jan 2999 00:00:00 GMT")],
        ))
        .await;

        assert_eq!(
            response.status(),
            StatusCode::NOT_MODIFIED,
            "`If-Modified-Since` posterior al `Last-Modified` del asset da `304`"
        );
        assert_eq!(
            header_value(&response, "X-Robots-Tag"),
            None,
            "un `304` no es una página de error: no puede llevar sus cabeceras"
        );
        assert!(
            body_bytes(response).await.is_empty(),
            "un `304` no admite cuerpo"
        );
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
        // tasks.md 1.12, re-apuntada en el change `embedded-error-pages`. Antes
        // arrancaba con `DEFAULT_ERROR_PAGE=404` y un `CONTENT_DIR` sin HTML; con
        // las páginas embebidas, el defecto `404` **sí** está en la tabla, así
        // que la prueba quedaría verde por la página embebida del `404` y
        // dejaría de fijar el respaldo de texto —su motivo—. Por eso arranca con
        // `DEFAULT_ERROR_PAGE=418`, legal pero fuera de la tabla, y sigue
        // exigiendo el respaldo de texto (`text/plain`) con el código resuelto y
        // nunca `500`.
        let response = build_router(&default_418_sin_html_config())
            .oneshot(get("/418.html"))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::IM_A_TEAPOT);
        assert_ne!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert!(
            content_type(&response).starts_with("text/plain"),
            "un `418` no está en la tabla embebida: el cuerpo tiene que ser el respaldo \
             de texto, fue: {}",
            content_type(&response)
        );
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
    // La prueba fija el contrato de la capa de compresión, que `build_router`
    // ya aplica (`CompressionLayer::new()`). Ojo al umbral de
    // `tower_http::compression::predicate`: su predicado por defecto
    // (`SizeAbove`, 32 bytes) no comprime respuestas más pequeñas, así que la
    // prueba usa `/404` (cuerpo de texto de ~70 bytes) y no un cuerpo mínimo.
    // Los cuerpos concretos no se fijan aquí: se comparan entre sí para que la
    // prueba no dependa del texto de las páginas.

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
            render_text_fallback(404, 120).body,
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

    /// Total de páginas servidas con un código, leído del texto Prometheus.
    ///
    /// Con la etiqueta de origen un código puede repartirse entre varias series
    /// (`{code="404",source="path"}`, `{code="404",source="default"}`…), así que
    /// buscar la línea entera ya no encontraría nada: se **suman** todas las
    /// series de ese código, sea cual sea su origen.
    ///
    /// Sumar no debilita lo que esta prueba comprobaba —el total de páginas
    /// servidas de cada código—, y se sigue exigiendo la etiqueta completa
    /// (`code="404",` y el `} ` de cierre) y un valor parseable: un `contains`
    /// sobre el cuerpo daría por bueno un `33`.
    fn contador_prometheus(cuerpo: &str, code: u16) -> Option<u64> {
        let prefijo = format!("{COUNTER_NAME}{{code=\"{code}\",");

        let mut total = None;
        for linea in cuerpo.lines() {
            let Some(resto) = linea.strip_prefix(&prefijo) else {
                continue;
            };
            let Some((_, valor)) = resto.split_once("} ") else {
                continue;
            };
            let Ok(valor) = valor.trim().parse::<u64>() else {
                continue;
            };

            total = Some(total.unwrap_or(0) + valor);
        }

        total
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

    #[tokio::test]
    async fn el_path_gana_a_la_cabecera_y_el_contador_lo_declara_con_origen_path() {
        // Guarda del contador (tasks.md 1.10): `/404` es una página de error por
        // su path, así que se sirve como `404` aunque venga `X-Code: 503`, y el
        // contador tiene que declararlo con origen `path`. Router propio de la
        // prueba, como en `el_contador_distingue_de_donde_salio_el_codigo`: las
        // cuentas deterministas necesitan un contador propio, porque las demás
        // pruebas corren en paralelo y también piden `/404`.
        let router = build_router(&fixtures_config());

        let request = Request::builder()
            .uri("/404")
            .header("X-Code", "503")
            .body(Body::empty())
            .unwrap();
        let response = router.clone().oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let response = router.oneshot(get("/metrics")).await.unwrap();
        let cuerpo = String::from_utf8(body_bytes(response).await).unwrap();

        assert!(
            cuerpo.contains(r#"errors_pages_total{code="404",source="path"} 1"#),
            "el path ganó: la página se cuenta con origen `path`:\n{cuerpo}"
        );
        assert!(
            !cuerpo.contains(r#"source="header""#),
            "el path ganó, así que ninguna serie puede declarar origen `header`:\n{cuerpo}"
        );
    }

    // --- Ningún estado de error del servidor de ficheros se escapa ---------
    //
    // Change `security-hardening`, tasks.md 1.1-1.8. Los tres defectos de la
    // auditoría, cada uno con la petición que lo reproduce: un segmento de path
    // que no cabe como nombre de fichero (`ENAMETOOLONG`), un método que el
    // servidor de ficheros no admite y un `Range` múltiple. La spec `error-page`
    // ("Ningún estado de error del servidor de ficheros se escapa") manda
    // sustituir **cualquier** respuesta suya con estado de error —`4xx` o `5xx`,
    // no solo el `404`— por la página de error, con el estado igual al **código
    // resuelto** por la cascada (path -> `X-Code` -> defecto).
    //
    // El estado del servidor de ficheros **no** es una de las fuentes de esa
    // cascada, así que no elige ni el estado ni el cuerpo ni las cabeceras.
    // Conservarlo se probó y se descartó, y estas pruebas fijan por qué, con las
    // tres mediciones hechas con el binario delante:
    //
    // - Obligaba a propagar cabeceras que describen *su* respuesta (`Allow` en
    //   el `405`, `Content-Range` en el `416`) so pena de incumplir un `MUST` de
    //   RFC 9110: el `PUT /css/404.css` salía `405` **sin** `Allow`.
    // - Dejaba el estado y el cuerpo en desacuerdo: `GET /nada.png` con
    //   `X-Code: 503` salía `404` con el cuerpo del `503` y su
    //   `Retry-After: 120`.
    // - Con `DEFAULT_ERROR_PAGE=503`, cualquier asset inexistente salía `404`,
    //   dejando sin efecto a `X-Code`, que existe justo para los despliegues que
    //   no reescriben el path.
    //
    // De ahí que `PUT /css/404.css` (un `405` del servidor de ficheros) lleve el
    // estado y el cuerpo del código resuelto (`404`, el defecto: `/css/404.css`
    // no aporta código), y que un segmento imposible (`500` suyo) lleve
    // también los del resuelto.

    /// Un segmento de path que no cabe como nombre de fichero: el servidor de
    /// ficheros falla al abrirlo (`ENAMETOOLONG`, os error 36) y responde un
    /// `500` vacío y sin cabeceras.
    fn uri_imposible() -> String {
        format!("/{}", "a".repeat(300))
    }

    /// Los nombres que declara `Vary`, en minúsculas, sin repetir y en orden.
    ///
    /// Junta **todas** las cabeceras `Vary` (varias capas pueden añadir la
    /// suya) y las parte por comas, que es como la spec compara —`Accept-Encoding`
    /// en minúsculas— con independencia de cómo las escriba cada capa.
    fn vary_names(response: &axum::response::Response) -> Vec<String> {
        let mut names: Vec<String> = response
            .headers()
            .get_all(header::VARY)
            .iter()
            .filter_map(|value| value.to_str().ok())
            .flat_map(|value| value.split(','))
            .map(|name| name.trim().to_ascii_lowercase())
            .filter(|name| !name.is_empty())
            .collect();

        names.sort();
        names.dedup();

        names
    }

    #[tokio::test]
    async fn un_segmento_de_path_imposible_da_la_pagina_de_error_y_no_un_500() {
        let response = send(get(&uri_imposible())).await;
        let status = response.status();

        assert_ne!(
            status,
            StatusCode::INTERNAL_SERVER_ERROR,
            "una URL larga no puede fabricar un 5xx del servicio"
        );
        assert!(
            (400..=599).contains(&status.as_u16()),
            "el estado debe estar en 400..=599, fue {status}"
        );
        // El segmento no aporta código (no es un número), así que el código
        // resuelto es el defecto de la configuración de prueba: `404`.
        assert_eq!(
            status,
            StatusCode::NOT_FOUND,
            "el 500 del servidor de ficheros debe caer al código resuelto, no {status}"
        );

        assert_eq!(
            header_value(&response, "X-Robots-Tag").as_deref(),
            Some("noindex, nofollow, nosnippet, noarchive"),
            "la página de error debe llevar sus cabeceras"
        );

        let body = body_bytes(response).await;
        assert!(
            !body.is_empty(),
            "el `500` del servidor de ficheros sale con el cuerpo vacío; la página de error no"
        );
        // Sin `Accept` el formato por defecto es texto, así que el cuerpo es el
        // de la página del código resuelto.
        assert_eq!(
            body,
            render_text_fallback(404, 120).body,
            "el cuerpo debe ser el de la página del código resuelto (texto)"
        );
    }

    #[tokio::test]
    async fn un_metodo_no_admitido_sobre_un_asset_da_la_pagina_del_codigo_resuelto() {
        let request = Request::builder()
            .method("PUT")
            .uri("/css/404.css")
            .body(Body::empty())
            .unwrap();

        let response = send(request).await;

        // El `405` del servidor de ficheros **no** se conserva. Conservarlo
        // obligaba a propagar el `Allow` que describe *su* respuesta —un `MUST`
        // de RFC 9110 §15.5.6— y ese `Allow` se perdía al construir la nuestra,
        // así que salía un `405` que mentía. `/css/404.css` no aporta código
        // (su primer segmento no es un número), así que el resuelto es el
        // defecto: `404`.
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_ne!(
            response.status(),
            StatusCode::METHOD_NOT_ALLOWED,
            "el `405` del servidor de ficheros no puede ser el estado de la página"
        );
        assert_eq!(
            header_value(&response, "X-Robots-Tag").as_deref(),
            Some("noindex, nofollow, nosnippet, noarchive"),
            "un `405` del servidor de ficheros se escapaba sin las cabeceras de la página"
        );

        let body = body_bytes(response).await;
        assert!(
            !body.is_empty(),
            "el `405` del servidor de ficheros sale con el cuerpo vacío: debe ser la página"
        );
        // El cuerpo es el de la cascada: `/css/404.css` no aporta código, así
        // que es el del defecto (`404`). Sin `Accept`, en texto.
        assert_eq!(
            body,
            render_text_fallback(404, 120).body,
            "el cuerpo debe ser el de la página del código resuelto (texto)"
        );
    }

    #[tokio::test]
    async fn un_asset_existente_sigue_sirviendose_con_200_y_no_como_pagina_de_error() {
        // Guarda del arreglo: interceptar los estados de error del servidor de
        // ficheros no puede tragarse el camino bueno.
        let response = send(get("/css/404.css")).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert!(
            content_type(&response).starts_with("text/css"),
            "Content-Type inesperado: {}",
            content_type(&response)
        );
        assert_eq!(
            header_value(&response, "X-Robots-Tag"),
            None,
            "un asset no es una página de error: no puede llevar sus cabeceras"
        );
        assert!(
            !body_bytes(response).await.is_empty(),
            "el CSS servido no puede estar vacío"
        );
    }

    #[tokio::test]
    async fn un_range_multiple_sobre_un_asset_da_la_pagina_del_codigo_resuelto() {
        // `tower-http` no soporta multi-rango, así que rechaza este `Range` con
        // un `416`: los dos rangos (`0-0` y `1-1`) son satisfacibles por
        // separado, no es un `Range` no satisfacible. Ese `416` tampoco se
        // conserva: la `Content-Range` que lo acompaña describe *su* respuesta,
        // y el estado del servidor de ficheros no es una fuente de la cascada.
        // Sale el código resuelto —el defecto, `404`— con la página.
        let response = send(get_with_headers(
            "/css/404.css",
            &[("Range", "bytes=0-0,1-1")],
        ))
        .await;

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_ne!(
            response.status(),
            StatusCode::RANGE_NOT_SATISFIABLE,
            "el `416` del servidor de ficheros no puede ser el estado de la página"
        );
        assert_eq!(
            header_value(&response, "X-Robots-Tag").as_deref(),
            Some("noindex, nofollow, nosnippet, noarchive"),
            "el `416` del servidor de ficheros salía sin las cabeceras de la página"
        );
        assert!(
            !body_bytes(response).await.is_empty(),
            "el `416` del servidor de ficheros sale con el cuerpo vacío: debe ser la página"
        );
    }

    #[tokio::test]
    async fn un_asset_inexistente_con_x_code_responde_el_codigo_resuelto() {
        // El `404` del servidor de ficheros no puede tapar la cabecera: el
        // estado sale de la cascada, así que `X-Code: 503` manda y el
        // `Retry-After` corresponde al estado que se sirve, no al código de un
        // cuerpo ajeno.
        let response = send(get_with_headers("/nada.png", &[("X-Code", "503")])).await;

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            header_value(&response, "Retry-After").as_deref(),
            Some("120"),
            "el `503` servido tiene que llevar su `Retry-After`"
        );

        let body = body_bytes(response).await;
        assert_eq!(
            body,
            render_text_fallback(503, 120).body,
            "el cuerpo debe ser el de la página del código resuelto (`503`)"
        );
    }

    #[tokio::test]
    async fn el_retry_after_configurado_no_depende_del_origen_del_codigo() {
        // Spec `error-page`, "el valor no depende del formato ni del origen del
        // código": el valor tiene que ser el mismo cuando el `503` sale del path
        // —en JSON, que es el otro eje de esa escena— y cuando sale de `X-Code`
        // sobre un asset inexistente. Y lo mismo para el `429`, el otro código
        // que lleva la cabecera.
        //
        // El camino de `X-Code` es el interesante: ahí el estado lo aportaba el
        // servidor de ficheros y el `Retry-After` salía pegado a un cuerpo ajeno
        // (ver `un_asset_inexistente_con_x_code_responde_el_codigo_resuelto`).
        let router = build_router(&fixtures_config_con_retry_after("30"));

        let casos = [
            // `/503.json` no existe como asset, así que es una página de error
            // cuyo código lo aporta el path; sin `Accept`, la extensión manda y
            // el formato es JSON.
            (
                "el `503` del path, en JSON",
                get("/503.json"),
                StatusCode::SERVICE_UNAVAILABLE,
                Some("application/json"),
            ),
            (
                "el `429` del path",
                get("/429"),
                StatusCode::TOO_MANY_REQUESTS,
                None,
            ),
            // Asset inexistente con `X-Code: 503`: el código lo aporta la
            // cabecera.
            (
                "el `503` de `X-Code` sobre un asset inexistente",
                get_with_headers("/nada.png", &[("X-Code", "503")]),
                StatusCode::SERVICE_UNAVAILABLE,
                None,
            ),
        ];

        for (etiqueta, request, estado, content_type_esperado) in casos {
            let response = router.clone().oneshot(request).await.unwrap();

            assert_eq!(response.status(), estado, "estado de {etiqueta}");
            assert_eq!(
                header_value(&response, "Retry-After").as_deref(),
                Some("30"),
                "el valor configurado debe llegar a {etiqueta}, sin depender del \
                 formato ni del origen del código"
            );
            // La etiqueta del caso JSON afirma que cubre también el eje del
            // formato: se comprueba el tipo MIME donde el caso lo declara, para
            // que una cascada de extensión rota (el `.json` servido como texto)
            // no siga pasando en verde.
            if let Some(content_type_esperado) = content_type_esperado {
                assert_eq!(
                    header_value(&response, "Content-Type").as_deref(),
                    Some(content_type_esperado),
                    "Content-Type de {etiqueta}"
                );
            }
        }
    }

    #[tokio::test]
    async fn el_defecto_configurado_manda_sobre_el_not_found_del_servidor_de_ficheros() {
        // Con `DEFAULT_ERROR_PAGE=503`, conservar el `404` del servidor de
        // ficheros contradecía el defecto configurado y dejaba sin efecto a
        // `X-Code`, que existe justo para los despliegues que no reescriben el
        // path.
        let response = build_router(&default_503_config())
            .oneshot(get("/nada.png"))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_ne!(response.status(), StatusCode::NOT_FOUND);

        let body = body_bytes(response).await;
        assert_eq!(
            body,
            render_text_fallback(503, 120).body,
            "el cuerpo debe ser el de la página del defecto configurado (`503`)"
        );
    }

    #[tokio::test]
    async fn el_retry_after_corresponde_al_estado_servido() {
        // `Retry-After` describe el estado que se sirve, no el código de un
        // cuerpo ajeno: sobre las respuestas interceptadas —donde el servidor de
        // ficheros aporta un estado— solo puede aparecer con un `429` o un
        // `503`. Hasta esta corrección, un asset inexistente con `X-Code: 503`
        // salía `404` **con** el `Retry-After: 120` del `503` resuelto.
        let interceptadas = [
            get_with_headers("/nada.png", &[("X-Code", "503")]),
            Request::builder()
                .method("PUT")
                .uri("/css/404.css")
                .body(Body::empty())
                .unwrap(),
            get(&uri_imposible()),
        ];

        for request in interceptadas {
            let response = send(request).await;
            let status = response.status();
            let retry_after = header_value(&response, "Retry-After");
            // `429` y `503` lo llevan; el resto no.
            let debe_llevarlo = matches!(
                status,
                StatusCode::TOO_MANY_REQUESTS | StatusCode::SERVICE_UNAVAILABLE
            );

            assert_eq!(
                retry_after.is_some(),
                debe_llevarlo,
                "`Retry-After` = {retry_after:?} con estado {status}: solo `429` y `503` lo llevan"
            );
        }
    }

    #[tokio::test]
    async fn una_respuesta_interceptada_sigue_la_cascada_de_formato() {
        // Guarda: la respuesta interceptada se construye como cualquier página
        // de error, así que el formato sale de la cascada (`Accept: text/html`
        // aquí) y el cuerpo es el HTML del código resuelto (`404`, el defecto).
        let request = Request::builder()
            .method("PUT")
            .uri("/css/404.css")
            .header("Accept", "text/html")
            .body(Body::empty())
            .unwrap();

        let response = send(request).await;

        assert!(
            content_type(&response).starts_with("text/html"),
            "Content-Type inesperado: {}",
            content_type(&response)
        );

        let body = body_bytes(response).await;
        assert_eq!(
            body,
            fixture_404_html().as_bytes(),
            "el cuerpo debe ser la página HTML del código resuelto (`404`)"
        );
    }

    #[tokio::test]
    async fn vary_declara_la_compresion_y_las_cuatro_cabeceras_de_correlacion() {
        // La respuesta copia las cabeceras de correlación de la petición, así
        // que su contenido depende de ellas: sin declararlo en `Vary`, una
        // caché intermedia podría servirle a un usuario el identificador de
        // otro. La lista tiene que ser la misma con y sin ellas en la petición
        // (una lista que cambiase no serviría a una caché), y la capa de
        // correlación envuelve a **todo** el router: la declaración va también
        // en los endpoints de servicio —`/healthz`, `/metrics`—, no solo en las
        // páginas de error.
        for uri in ["/404", "/healthz", "/metrics"] {
            let sin_correlacion = send(get(uri)).await;
            let con_correlacion = send(get_with_headers(
                uri,
                &[
                    ("X-Request-Id", "req-1"),
                    ("X-Trace-Id", "trace-2"),
                    ("X-Correlation-Id", "corr-3"),
                    ("X-Amzn-Trace-Id", "Root=1-abc"),
                ],
            ))
            .await;

            let sin = vary_names(&sin_correlacion);
            let con = vary_names(&con_correlacion);

            assert_eq!(
                con, sin,
                "`{uri}`: `Vary` no puede depender de la petición: sin cabeceras {sin:?}, \
                 con ellas {con:?}"
            );

            for nombre in [
                "accept-encoding",
                "x-request-id",
                "x-trace-id",
                "x-correlation-id",
                "x-amzn-trace-id",
            ] {
                assert!(
                    sin.contains(&nombre.to_string()),
                    "`{uri}`: falta `{nombre}` en `Vary` (Vary = {sin:?})"
                );
            }
        }
    }

    // --- Página embebida sin ficheros de datos (change `embedded-error-pages`,
    //     tasks.md 1.3-1.5, 1.9-1.11, 1.13) --------------------------------
    //
    // Con `CONTENT_DIR` inexistente, la fase GREEN sirve las páginas y la hoja
    // embebidas: el HTML por el render —la cascada del disco agotada **entera**
    // antes de la copia embebida, D1— y la hoja por el `fallback` de `ServeDir`
    // (D2). Las guardas de D1 (1.7, 1.8) y del respaldo (1.13) pasan ya en RED.

    #[tokio::test]
    async fn sin_directorio_una_pagina_html_sirve_la_copia_embebida_con_su_estado() {
        // tasks.md 1.3: hoy `/503.html` con `CONTENT_DIR` inexistente responde
        // un `503` con el respaldo de texto (`text/plain`); en GREEN responde la
        // página embebida del `503` (`text/html`). Y desde
        // `self-contained-themes` la sirve **autocontenida**: el marcador sale
        // sustituido por el `<style>` con la hoja del tema.
        let response = build_router(&sin_contenido_config())
            .oneshot(get("/503.html"))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert!(
            content_type(&response).starts_with("text/html"),
            "Content-Type inesperado: {}",
            content_type(&response)
        );

        let body = body_bytes(response).await;
        assert_eq!(
            body,
            pagina_autocontenida(EMBEDDED_503_HTML, EMBEDDED_CSS),
            "el cuerpo tiene que ser la página embebida de `content/503.html`, autocontenida"
        );
    }

    #[tokio::test]
    async fn sin_directorio_los_once_codigos_sirven_su_pagina_embebida() {
        // tasks.md 1.4. Se pide `GET /<código>` con `Accept: text/html` y **no**
        // `/<código>.html`: la extensión del path resolvería HTML por el primer
        // eslabón de la cascada de formato (la extensión) y no probaría que un
        // navegador —que manda `Accept: text/html`, no una extensión— también
        // recibe la página embebida. El `Accept` es el cuarto eslabón de esa
        // cascada, y es justo lo que un navegador detrás de Traefik envía.
        let router = build_router(&sin_contenido_config());

        for code in [400u16, 401, 403, 404, 405, 408, 429, 500, 502, 503, 504] {
            let request = get_with_headers(&format!("/{code}"), &[("Accept", "text/html")]);
            let response = router.clone().oneshot(request).await.unwrap();

            assert_eq!(
                response.status().as_u16(),
                code,
                "`/{code}` debe responder su propio estado"
            );
            assert!(
                content_type(&response).starts_with("text/html"),
                "`/{code}`: Content-Type inesperado: {}",
                content_type(&response)
            );

            let body = String::from_utf8(body_bytes(response).await).unwrap();
            assert!(
                body.contains(&format!("class=\"c{code}\"")),
                "`/{code}`: el cuerpo tiene que ser la página embebida de su código, fue: {body}"
            );
            assert_ne!(
                body.as_bytes(),
                render_text_fallback(code, 120).body.as_slice(),
                "`/{code}`: el cuerpo no puede ser el respaldo de texto"
            );
        }
    }

    #[tokio::test]
    async fn sin_directorio_la_pagina_embebida_no_se_sirve_como_asset_con_200() {
        // tasks.md 1.5: `/404.html` se clasifica como página de error **antes**
        // de llegar a `ServeDir`, así que la copia embebida nunca se sirve como
        // asset. La prueba fija el `404` —nunca `200`— y, para que sea roja en
        // RED (donde esta petición cae al respaldo de texto), que el cuerpo es
        // la página embebida (`text/html`), no el respaldo (`text/plain`).
        let response = build_router(&sin_contenido_config())
            .oneshot(get("/404.html"))
            .await
            .unwrap();

        assert_ne!(response.status(), StatusCode::OK);
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert!(
            content_type(&response).starts_with("text/html"),
            "Content-Type inesperado: {}",
            content_type(&response)
        );

        let body = body_bytes(response).await;
        assert_eq!(
            body,
            pagina_autocontenida(EMBEDDED_404_HTML, EMBEDDED_CSS),
            "el cuerpo tiene que ser la página embebida del `404`, autocontenida, no un asset \
             ni el respaldo"
        );
    }

    #[tokio::test]
    async fn el_directorio_gana_a_la_pagina_embebida_por_su_defecto() {
        // tasks.md 1.7 / D1 (guarda, verde en RED y en GREEN): con
        // `CONTENT_DIR=tests/fixtures/content` y el defecto `404`, `GET
        // /503.html` sirve el `404.html` del operador (271 bytes), **no** la
        // página embebida del `503`. La cascada del disco se agota entera antes
        // de entrar en la copia embebida.
        let response = build_router(&fixtures_config())
            .oneshot(get("/503.html"))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert!(content_type(&response).starts_with("text/html"));

        let body = body_bytes(response).await;
        assert_eq!(
            body,
            fixture_404_html().as_bytes(),
            "el cuerpo tiene que ser el `404.html` del operador"
        );
        assert_ne!(
            body,
            EMBEDDED_503_HTML.as_bytes(),
            "la página embebida del `503` no puede ganar al directorio del operador"
        );
    }

    #[tokio::test]
    async fn el_directorio_gana_a_la_pagina_embebida_del_mismo_codigo() {
        // tasks.md 1.8 / D1 (guarda): con `tests/fixtures/content`, `GET
        // /404.html` sirve su `404.html` (271 bytes) y no la copia embebida del
        // `404` (460 bytes).
        let response = build_router(&fixtures_config())
            .oneshot(get("/404.html"))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let body = body_bytes(response).await;
        assert_eq!(body, fixture_404_html().as_bytes());
        assert_ne!(
            body,
            EMBEDDED_404_HTML.as_bytes(),
            "la copia embebida del `404` no puede ganar al `404.html` del operador"
        );
    }

    #[tokio::test]
    async fn sin_directorio_la_hoja_embebida_se_sirve_y_lo_demas_es_pagina_de_error() {
        // tasks.md 1.9: la hoja embebida responde `200 text/css`; hoy responde
        // la página de error del defecto. Y un path que no esté en la tabla
        // sigue siendo página de error: el respaldo solo responde a sus paths.
        let router = build_router(&sin_contenido_config());

        let response = router.clone().oneshot(get("/css/error.css")).await.unwrap();
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "la hoja embebida tiene que servirse con 200, no como página de error"
        );
        assert!(
            content_type(&response).starts_with("text/css"),
            "Content-Type inesperado: {}",
            content_type(&response)
        );
        assert!(
            !body_bytes(response).await.is_empty(),
            "la hoja embebida no puede estar vacía"
        );

        let response = router.oneshot(get("/nada.css")).await.unwrap();
        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "un path que no está en la tabla de assets sigue siendo página de error"
        );
        assert_eq!(
            header_value(&response, "X-Robots-Tag").as_deref(),
            Some("noindex, nofollow, nosnippet, noarchive")
        );
    }

    #[tokio::test]
    async fn sin_directorio_head_de_la_hoja_embebida_responde_igual_sin_cuerpo() {
        // tasks.md 1.9b: `ServeDir` copia método, URI y cabeceras a la petición
        // del respaldo, así que `HEAD /css/error.css` pasa por la tabla y tiene
        // que responder como el `GET` —estado y `Content-Type`— pero **sin
        // cuerpo**. Ninguna prueba anterior montaba un `HEAD`.
        let request = Request::builder()
            .method("HEAD")
            .uri("/css/error.css")
            .body(Body::empty())
            .unwrap();

        let response = build_router(&sin_contenido_config())
            .oneshot(request)
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert!(
            content_type(&response).starts_with("text/css"),
            "Content-Type inesperado: {}",
            content_type(&response)
        );
        assert!(
            body_bytes(response).await.is_empty(),
            "una respuesta a `HEAD` no lleva cuerpo"
        );
    }

    #[tokio::test]
    async fn sin_directorio_un_path_que_extiende_al_asset_no_es_el_asset() {
        // tasks.md 1.9c (guarda): la tabla de assets compara el path **de forma
        // exacta**, no con prefijos ni subcadenas. El path que lo discrimina es
        // uno que **extiende** al del asset, `/css/error.css/extra`: no tiene
        // ningún segmento vacío que plegar (`src/path.rs::fold_components`,
        // líneas 106-130), así que llega entero al respaldo de `ServeDir` y una
        // consulta con `starts_with` o con `contains` devolvería la hoja con
        // `200` y esta prueba se caería.
        //
        // Y no la barra final: `fold_components` descarta los segmentos vacíos
        // (línea 111, `"" | "." => {}`) y reconstruye el path prefixando cada
        // segmento con `/`, así que `/css/error.css/` es **`/css/error.css`**
        // antes de llegar a `ServeDir` —`error_pages` reescribe la URI con esa
        // cadena—. Una petición con barra final es indistinguible de la misma sin
        // ella; no hay información que las separe, y por eso la prueba de abajo
        // fija que las dos dan la misma respuesta.
        let response = build_router(&sin_contenido_config())
            .oneshot(get("/css/error.css/extra"))
            .await
            .unwrap();

        assert_ne!(response.status(), StatusCode::OK);
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            header_value(&response, "X-Robots-Tag").as_deref(),
            Some("noindex, nofollow, nosnippet, noarchive")
        );
        assert!(
            !content_type(&response).starts_with("text/css"),
            "un path que solo extiende al asset no puede servirse como la hoja: {}",
            content_type(&response)
        );
    }

    #[tokio::test]
    async fn sin_directorio_la_barra_final_da_la_misma_respuesta_que_el_path_limpio() {
        // `fold_components` pliega la barra final antes de que el path llegue a
        // `ServeDir` (`src/path.rs`, línea 111: los segmentos vacíos se
        // descartan), así que `/css/error.css/` y `/css/error.css` son la **misma**
        // petición para el servidor de ficheros. La capa del disco responde a las
        // dos igual —lo mismo que aquí: sin directorio de contenido, las dos caen
        // al respaldo embebido—, así que un `200` con barra final no es una
        // incoherencia: esta prueba compara las dos respuestas de verdad y fija
        // que las dos capas coinciden, para que nadie «arregle» mañana una
        // asimetría que no existe.
        let router = build_router(&sin_contenido_config());

        let con_barra = router
            .clone()
            .oneshot(get("/css/error.css/"))
            .await
            .unwrap();
        let limpio = router.oneshot(get("/css/error.css")).await.unwrap();

        assert_eq!(con_barra.status(), StatusCode::OK);
        assert_eq!(limpio.status(), StatusCode::OK);
        assert!(
            content_type(&con_barra).starts_with("text/css"),
            "Content-Type inesperado con barra final: {}",
            content_type(&con_barra)
        );
        assert_eq!(
            content_type(&con_barra),
            content_type(&limpio),
            "las dos formas del path tienen que dar el mismo `Content-Type`"
        );

        let cuerpo_con_barra = body_bytes(con_barra).await;
        let cuerpo_limpio = body_bytes(limpio).await;
        assert_eq!(
            cuerpo_con_barra, cuerpo_limpio,
            "las dos formas del path tienen que dar el mismo cuerpo"
        );
        assert_eq!(
            cuerpo_con_barra,
            EMBEDDED_CSS.as_bytes(),
            "el cuerpo tiene que ser la hoja embebida"
        );
    }

    #[tokio::test]
    async fn sin_directorio_la_pagina_embebida_se_sirve_autocontenida() {
        // tasks.md 1.8 (change `self-contained-themes`): la página embebida del
        // `404` ya **no** enlaza a `css/error.css` —lleva la hoja incrustada en
        // un bloque `<style>`—, así que el navegador no pide ningún fichero para
        // verse. En RED esta primera mitad **falla**: hoy el cuerpo sigue siendo
        // el marcado con su `<link rel="stylesheet">`.
        let router = build_router(&sin_contenido_config());

        let response = router
            .clone()
            .oneshot(get_with_headers("/404.html", &[("Accept", "text/html")]))
            .await
            .unwrap();
        let body = String::from_utf8(body_bytes(response).await).unwrap();
        assert!(
            body.contains("<style"),
            "la página embebida tiene que servirse autocontenida, con un bloque `<style>`, \
             fue: {body}"
        );
        assert!(
            !body.contains("rel=\"stylesheet\""),
            "la página embebida servida no puede enlazar ninguna hoja, fue: {body}"
        );

        // La segunda mitad **no** cambia con el change: la hoja se sigue
        // sirviendo como asset —`200 text/css`— aunque la página ya no la
        // enlace. No es una página de error.
        let response = router.oneshot(get("/css/error.css")).await.unwrap();
        assert_eq!(
            response.status(),
            StatusCode::OK,
            "la hoja se sirve como asset aunque la página ya no la enlace, no como página de error"
        );
        assert!(
            content_type(&response).starts_with("text/css"),
            "Content-Type inesperado: {}",
            content_type(&response)
        );
    }

    #[tokio::test]
    async fn la_hoja_del_directorio_gana_a_la_hoja_embebida() {
        // tasks.md 1.11 / D1 (guarda): con `tests/fixtures/content`, la hoja del
        // operador se sirve tal cual —comparada con el `include_str!` del
        // fixture nuevo— y **no** la embebida.
        let response = build_router(&fixtures_config())
            .oneshot(get("/css/error.css"))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert!(
            content_type(&response).starts_with("text/css"),
            "Content-Type inesperado: {}",
            content_type(&response)
        );

        let body = body_bytes(response).await;
        assert_eq!(
            body,
            FIXTURE_CSS.as_bytes(),
            "el cuerpo tiene que ser la hoja del directorio de fixtures"
        );
        assert_ne!(
            body,
            EMBEDDED_CSS.as_bytes(),
            "la hoja embebida no puede ganar a la del directorio del operador"
        );
    }

    #[tokio::test]
    async fn sin_pagina_en_ninguna_capa_un_codigo_distinto_del_defecto_no_da_500() {
        // tasks.md 1.13 / D4 (guarda): con el defecto `418` —fuera de la tabla
        // embebida— y un `CONTENT_DIR` sin HTML, `GET /599.html` responde `599`
        // —el respaldo de texto— y nunca un `500`.
        let response = build_router(&default_418_sin_html_config())
            .oneshot(get("/599.html"))
            .await
            .unwrap();

        assert_eq!(response.status().as_u16(), 599);
        assert_ne!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert!(
            content_type(&response).starts_with("text/plain"),
            "sin página en ninguna capa el cuerpo es el respaldo de texto: {}",
            content_type(&response)
        );

        let body = String::from_utf8(body_bytes(response).await).unwrap();
        assert!(
            body.contains("599"),
            "el respaldo de texto debe llevar el código: {body}"
        );
    }

    // --- Temas autocontenidos (change `self-contained-themes`,
    //     tasks.md 1.1-1.6) ---------------------------------------------------
    //
    // El HTML servido pasa a ser **autocontenido**: al servir, el marcador
    // `<link rel="stylesheet" href="/css/error.css" />` se sustituye por un
    // bloque `<style>` con la hoja del tema elegido, así que la página deja de
    // pedir ningún fichero para verse. Es el arreglo del fallo medido en el
    // host de la aplicación, donde `/css/error.css` lo sirve el upstream y el
    // navegador rechaza la hoja por su `Content-Type`.
    //
    // Las pruebas que sirven una página **con** el marcador usan la copia
    // **embebida** (`sin_contenido_config()`), porque son las páginas de
    // `content/` —y su copia embebida— las que llevan el marcador; el
    // `404.html` de los fixtures **no** lo lleva a propósito (ver
    // `sin_marcador_la_pagina_se_sirve_byte_a_byte`).

    #[tokio::test]
    async fn el_html_servido_es_autocontenido() {
        // tasks.md 1.2: la página servida no pide ningún fichero para verse.
        // Para los dos códigos que cubren el ejemplo (`404` y `503`), el cuerpo
        // lleva la hoja del tema `default` incrustada y **sin**
        // `rel="stylesheet"`.
        let router = build_router(&sin_contenido_config());

        for code in [404u16, 503u16] {
            let request = get_with_headers(&format!("/{code}"), &[("Accept", "text/html")]);
            let response = router.clone().oneshot(request).await.unwrap();

            assert_eq!(response.status().as_u16(), code);

            let body = String::from_utf8(body_bytes(response).await).unwrap();
            assert!(
                body.contains(EMBEDDED_CSS),
                "`/{code}`: el cuerpo tiene que llevar la hoja `default` incrustada, fue: {body}"
            );
            assert!(
                !body.contains("rel=\"stylesheet\""),
                "`/{code}`: la página servida no puede enlazar ninguna hoja, fue: {body}"
            );
        }
    }

    #[tokio::test]
    async fn sin_theme_la_hoja_incrustada_es_la_de_siempre() {
        // tasks.md 1.3: sin `THEME` el aspecto no cambia. La hoja incrustada es
        // la de `content/css/error.css`, que es también el asset
        // `/css/error.css`.
        let response = build_router(&sin_contenido_config())
            .oneshot(get_with_headers("/404", &[("Accept", "text/html")]))
            .await
            .unwrap();

        let body = String::from_utf8(body_bytes(response).await).unwrap();
        assert!(
            body.contains(EMBEDDED_CSS),
            "sin `THEME` la hoja incrustada tiene que ser la `default`, fue: {body}"
        );
    }

    #[tokio::test]
    async fn theme_cambia_la_hoja_de_todo_el_servicio() {
        // tasks.md 1.3: `THEME` es del **servicio**, no de la petición.
        let response = build_router(&config_con_theme("terminal"))
            .oneshot(get_with_headers("/404", &[("Accept", "text/html")]))
            .await
            .unwrap();

        let body = String::from_utf8(body_bytes(response).await).unwrap();
        let terminal = hoja_de_tema("terminal");

        assert!(
            body.contains(&terminal),
            "con `THEME=terminal` el cuerpo tiene que llevar la hoja del tema `terminal`"
        );
        assert!(
            !body.contains(EMBEDDED_CSS),
            "con `THEME=terminal` el cuerpo NO puede llevar la hoja `default`"
        );
    }

    #[tokio::test]
    async fn la_hoja_de_un_tema_del_operador_gana_a_la_embebida() {
        // La delta exige que la hoja de un tema se resuelva **como un asset**
        // para *cualquier* tema, no solo para `default`. Aquí `THEME=terminal` y
        // el `CONTENT_DIR` trae su propio `themes/terminal.css`: la página sale
        // de la capa embebida (ese directorio no tiene `404.html` a propósito) y
        // lo que se prueba es que la hoja incrustada es la **del disco** y no la
        // embebida.
        let response = build_router(&config_tema_del_operador())
            .oneshot(get_with_headers("/404", &[("Accept", "text/html")]))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let body = String::from_utf8(body_bytes(response).await).unwrap();
        let del_disco = include_str!("../tests/fixtures/tema-del-operador/themes/terminal.css");

        assert!(
            body.contains(del_disco),
            "la hoja del operador (`themes/terminal.css`) tiene que ganar a la embebida, fue: {body}"
        );
        assert!(
            !body.contains(&hoja_de_tema("terminal")),
            "con su propia hoja, el tema no puede incrustar la copia embebida"
        );
    }

    #[tokio::test]
    async fn la_hoja_ilegible_cae_a_la_embebida_sin_tumbar_la_pagina() {
        // La delta exige el escenario «la hoja existe y no se puede leer»: un
        // `warn!` y la hoja embebida, **sin** que la página deje de servirse.
        // El fixture pone un **directorio** donde el tema espera su fichero
        // (`themes/terminal.css`): `std::fs::read` de un directorio falla en
        // Linux —un error de lectura real y reproducible—, a diferencia de un
        // fichero con permisos `000`, que en CI (corriendo como `root`) se lee
        // igual y probaría la rama equivocada.
        let response = build_router(&config_hoja_ilegible())
            .oneshot(get_with_headers("/404", &[("Accept", "text/html")]))
            .await
            .unwrap();

        // El estado sigue siendo el código resuelto (`404`), ni un `500` ni el
        // texto plano del respaldo: la hoja es adorno y no puede degradar la
        // página.
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert!(
            content_type(&response).starts_with("text/html"),
            "Content-Type inesperado: {}",
            content_type(&response)
        );

        let body = String::from_utf8(body_bytes(response).await).unwrap();
        assert!(
            body.contains(&hoja_de_tema("terminal")),
            "sin poder leer la hoja del operador, se incrusta la embebida, fue: {body}"
        );
    }

    #[tokio::test]
    async fn x_theme_elige_el_tema_de_esa_peticion() {
        // tasks.md 1.4/D6: `X-Theme` elige el tema **de esa petición**, con la
        // precedencia de siempre: gana al `THEME` configurado (aquí, el de por
        // defecto).
        let response = build_router(&config_con_theme("default"))
            .oneshot(get_with_headers(
                "/404",
                &[("Accept", "text/html"), ("X-Theme", "terminal")],
            ))
            .await
            .unwrap();

        let body = String::from_utf8(body_bytes(response).await).unwrap();
        assert!(
            body.contains(&hoja_de_tema("terminal")),
            "`X-Theme: terminal` tiene que elegir la hoja del tema `terminal`"
        );
    }

    #[tokio::test]
    async fn un_x_theme_desconocido_cae_al_tema_configurado() {
        // tasks.md 1.4/D6: `X-Theme` la elige quien llama, así que un valor
        // desconocido **no** es un error: cae al tema configurado, porque un
        // cliente no puede romper el servicio.
        let response = build_router(&config_con_theme("minimal"))
            .oneshot(get_with_headers(
                "/404",
                &[("Accept", "text/html"), ("X-Theme", "neon")],
            ))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let body = String::from_utf8(body_bytes(response).await).unwrap();
        assert!(
            body.contains(&hoja_de_tema("minimal")),
            "un `X-Theme` desconocido tiene que caer al tema configurado (`minimal`)"
        );
        assert!(
            !body.contains(&hoja_de_tema("terminal")),
            "un `X-Theme` desconocido no puede elegir la hoja de otro tema"
        );
    }

    #[tokio::test]
    async fn x_theme_no_afecta_al_json() {
        // tasks.md 1.4/D6: sin hoja que incrustar, el tema se ignora. El cuerpo
        // es el JSON de siempre, con su código.
        let response = build_router(&sin_contenido_config())
            .oneshot(get_with_headers(
                "/404",
                &[("Accept", "application/json"), ("X-Theme", "terminal")],
            ))
            .await
            .unwrap();

        assert!(
            content_type(&response).starts_with("application/json"),
            "Content-Type inesperado: {}",
            content_type(&response)
        );

        let body = body_bytes(response).await;
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            json.get("code").and_then(|value| value.as_u64()),
            Some(404),
            "el JSON no depende del tema: {json}"
        );
    }

    #[tokio::test]
    async fn x_theme_no_afecta_a_xml_ni_a_texto() {
        // La delta nombra los tres formatos que **no** son HTML; el JSON se
        // cubre arriba y aquí los otros dos. Para XML y texto, el cuerpo con
        // `X-Theme: terminal` tiene que ser **idéntico** al de la misma petición
        // sin la cabecera: sin hoja que incrustar, el tema se ignora.
        let router = build_router(&sin_contenido_config());

        for accept in ["application/xml", "text/plain"] {
            let con = router
                .clone()
                .oneshot(get_with_headers(
                    "/404",
                    &[("Accept", accept), ("X-Theme", "terminal")],
                ))
                .await
                .unwrap();
            let sin = router
                .clone()
                .oneshot(get_with_headers("/404", &[("Accept", accept)]))
                .await
                .unwrap();

            assert_eq!(con.status(), sin.status(), "el estado no depende del tema");
            assert!(
                content_type(&con).starts_with(accept),
                "Content-Type inesperado para `{accept}`: {}",
                content_type(&con)
            );

            assert_eq!(
                body_bytes(con).await,
                body_bytes(sin).await,
                "`X-Theme` no puede cambiar el cuerpo de `{accept}`"
            );
        }
    }

    #[tokio::test]
    async fn el_asset_de_la_hoja_no_sigue_al_tema_configurado() {
        // El tema y el asset son independientes: `THEME=terminal` cambia la hoja
        // que se **incrusta** en el HTML, no la que sirve `/css/error.css`. Ese
        // path es el asset público del tema `default` y sigue devolviendo esa
        // hoja.
        let response = build_router(&config_con_theme("terminal"))
            .oneshot(get("/css/error.css"))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert!(
            content_type(&response).starts_with("text/css"),
            "Content-Type inesperado: {}",
            content_type(&response)
        );

        let body = body_bytes(response).await;
        assert_eq!(
            body,
            EMBEDDED_CSS.as_bytes(),
            "`/css/error.css` es el asset del tema `default`: `THEME` no lo cambia"
        );
        assert_ne!(
            body,
            hoja_de_tema("terminal").as_bytes(),
            "el asset no puede seguir al tema configurado"
        );
    }

    #[tokio::test]
    async fn vary_declara_x_theme() {
        // tasks.md 1.4/D7: el cuerpo depende de `X-Theme`, así que una caché
        // intermedia tiene que saberlo. Se declara **siempre**, traiga o no la
        // petición la cabecera.
        let router = build_router(&sin_contenido_config());

        for peticion in [
            get("/404"),
            get_with_headers("/404", &[("X-Theme", "terminal")]),
        ] {
            let response = router.clone().oneshot(peticion).await.unwrap();
            let names = vary_names(&response);

            assert!(
                names.contains(&"x-theme".to_string()),
                "`Vary` tiene que declarar `X-Theme` (Vary = {names:?})"
            );
        }
    }

    #[tokio::test]
    async fn un_enlace_ajeno_se_deja_en_paz() {
        // tasks.md 1.1/D2: solo el marcador se sustituye. La página del fixture
        // `con-marcador` lleva el marcador **y** un `<link>` ajeno: el marcador
        // desaparece —su hoja se incrusta— y el enlace ajeno sigue en el cuerpo.
        let content_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/con-marcador");
        let config = Config::from_env_iter([(
            "CONTENT_DIR".to_string(),
            content_dir.to_string_lossy().into_owned(),
        )])
        .expect("la configuración del fixture `con-marcador` debe construirse");

        let response = build_router(&config)
            .oneshot(get_with_headers("/404", &[("Accept", "text/html")]))
            .await
            .unwrap();

        let body = String::from_utf8(body_bytes(response).await).unwrap();
        assert!(
            !body.contains("href=\"/css/error.css\""),
            "el marcador tiene que quedar sustituido por el bloque `<style>`, fue: {body}"
        );
        assert!(
            body.contains("href=\"/css/otra.css\""),
            "un `<link>` que no es el marcador tiene que seguir en el cuerpo, fue: {body}"
        );
    }

    #[tokio::test]
    async fn sin_marcador_la_pagina_se_sirve_byte_a_byte() {
        // tasks.md 1.1/D2 (guarda: hoy ya pasa). Una página sin el marcador se
        // sirve **tal cual**, sin leer ninguna hoja para servirla. El `404.html`
        // de los fixtures no lleva marcador.
        let response = build_router(&fixtures_config())
            .oneshot(get_with_headers("/404", &[("Accept", "text/html")]))
            .await
            .unwrap();

        let body = body_bytes(response).await;
        assert_eq!(
            body,
            include_str!("../tests/fixtures/content/404.html").as_bytes(),
            "sin marcador, la página se sirve byte a byte"
        );
    }
}
