//! Resolución del código de error y del formato de respuesta, en cascada.
//!
//! ---------------------------------------------------------------------------
//! API que asume la fase RED (la fase GREEN debe implementarla tal cual):
//!
//! ```ignore
//! #[derive(Debug, Clone, Copy, PartialEq, Eq)]
//! pub enum Format { Html, Json, Xml, Text }
//!
//! /// Cascada del código: path -> `X-Code` -> `default`, con el origen
//! /// (path, header o default) incluido.
//! pub fn resolve_code_with_source(
//!     path: &str,
//!     x_code: Option<&str>,
//!     default: u16,
//! ) -> (u16, CodeSource);
//!
//! /// Cascada del formato: extensión -> `Content-Type` -> `X-Format` -> `Accept`
//! /// -> `Format::Text`.
//! pub fn resolve_format(
//!     path: &str,
//!     content_type: Option<&str>,
//!     x_format: Option<&str>,
//!     accept: Option<&str>,
//! ) -> Format;
//!
//! /// Razón canónica del código, en inglés (`404` -> `"Not Found"`), como en
//! /// `tarampampam/error-pages`. Código desconocido -> razón genérica.
//! pub fn canonical_reason(code: u16) -> &'static str;
//!
//! /// Descripción larga del código; genérica para los códigos no tabulados.
//! pub fn description(code: u16) -> &'static str;
//!
//! /// Cabecera `Retry-After`: `Some(configured)` solo para `429` y `503`,
//! /// `None` para el resto (spec `error-page`, "Las cabeceras de acompañamiento").
//! /// Decide **qué códigos** la llevan; el valor lo aporta la configuración.
//! pub fn retry_after_secs(code: u16, configured: u32) -> Option<u32>;
//! ```
//!
//! Detalles que la fase GREEN debe respetar:
//!
//! - La cascada del código extrae el entero del **primer segmento** del path,
//!   ignorando la extensión (`/404.json` -> `404`). Un primer segmento no
//!   numérico (`/`, `/foo`) no resuelve por esa vía.
//! - La cascada del código **no** depende del formato: `/503.xml` es un `503`.
//! - Para `Content-Type`, `X-Format` y `Accept` el match es de **tipo MIME**
//!   (sufijo `; charset=...` admitido y listas de `Accept` separadas por comas
//!   recorridas en orden, ganando el primer tipo soportado que aparezca —
//!   así un navegador, que manda `text/html,application/xhtml+xml,...`, cae en
//!   HTML y no en texto plano).
//! - Para la extensión del path: `.html`/`.htm` -> `Html`, `.json` -> `Json`,
//!   `.xml` -> `Xml`, `.txt` -> `Text`.
//! - Solo se resuelven **códigos de error utilizables** (`400..=599`): una
//!   fuente que aporte otro código se considera **vacía** y se continúa con la
//!   siguiente. Ver [`is_usable_code`].
//! - `canonical_reason` y `description` son la **fuente única** del texto de los
//!   cuatro formatos —HTML, JSON, XML y texto—: la razón y la descripción se
//!   escriben una sola vez, aquí, y el HTML las incrusta al servir.

use axum::http::StatusCode;

/// Código utilizable más bajo: el rango que Traefik intercepta
/// (`errors.status: 400-599`).
pub const MIN_ERROR_CODE: u16 = 400;

/// Código utilizable más alto.
pub const MAX_ERROR_CODE: u16 = 599;

/// Razón canónica que se usa cuando el código no está tabulado.
const UNKNOWN_REASON: &str = "Unknown Error";

/// Descripción que se usa cuando el código no está en la tabla.
const UNKNOWN_DESCRIPTION: &str = "An unexpected error occurred while processing the request.";

/// Formatos de cuerpo soportados, en paridad con `tarampampam/error-pages`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// Página HTML estática de `CONTENT_DIR`.
    Html,
    /// Objeto JSON generado.
    Json,
    /// Documento XML generado.
    Xml,
    /// Texto plano generado (el formato por defecto).
    Text,
}

/// ¿El código es utilizable como estado de una página de error?
///
/// El rango utilizable es `400..=599`, el que Traefik intercepta
/// (`errors.status: 400-599`). Fuera de él, un código no es representable de
/// forma fiable como respuesta final: `1xx` no lo es en absoluto (hyper
/// sustituye la cabeza y borra las cabeceras), `204` y `304` no admiten cuerpo
/// y un `2xx` contradice la razón de existir del proyecto —una página de error
/// nunca se sirve con `200`—.
///
/// Lo usan la cascada del código y la validación de `DEFAULT_ERROR_PAGE`.
pub fn is_usable_code(code: u16) -> bool {
    (MIN_ERROR_CODE..=MAX_ERROR_CODE).contains(&code)
}

/// Fuente de la que salió el código resuelto.
///
/// Existe porque el contador de páginas servidas distingue de dónde vino el
/// código: sin esa distinción, una cabecera `X-Code` elegida por quien llama es
/// indistinguible de un `503` real y el contador —la única señal de
/// observabilidad del servicio— se puede inflar a voluntad justo durante una
/// incidencia (spec `observability`).
///
/// Son tres y solo tres porque son los tres orígenes de la cascada de
/// [`resolve_code_with_source`]; el estado que traiga una respuesta del servidor
/// de ficheros sustituida **no** es un origen del código resuelto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeSource {
    /// El código lo aportó el path (`/404`, `/503.xml`).
    Path,
    /// El código lo aportó la cabecera `X-Code`.
    Header,
    /// No lo aportó ninguna fuente: es el código por defecto.
    Default,
}

/// Cascada del código: path -> `X-Code` -> `default`, con el origen incluido.
///
/// El primer origen que hace match gana, así que el path siempre manda sobre
/// la cabecera. No depende del formato: `/503.xml` es un `503`.
///
/// Una fuente que aporte un código **no utilizable** (`/200`, `X-Code: 0`…)
/// se considera vacía y se continúa con la siguiente: `/200` con
/// `X-Code: 503` es un `503`. Si ninguna aporta un código utilizable, gana
/// `default`, que el llamante ([`Config`](crate::config::Config)) garantiza en
/// el rango `400..=599`: el defecto nunca podría resolverse desde el path ni
/// desde la cabecera, así que si llegara fuera de rango saldría una página de
/// error con un estado no utilizable. El `debug_assert` lo vigila en pruebas.
///
/// Es el **único** punto de entrada público de la cascada, y devuelve también
/// de qué fuente salió —para etiquetar el contador de páginas servidas, ver
/// [`CodeSource`]—, sin cambiar el código devuelto.
pub fn resolve_code_with_source(
    path: &str,
    x_code: Option<&str>,
    default: u16,
) -> (u16, CodeSource) {
    debug_assert!(
        is_usable_code(default),
        "el código por defecto debe estar en 400..=599: {default}"
    );

    if let Some(code) = code_from_path(path).filter(|code| is_usable_code(*code)) {
        return (code, CodeSource::Path);
    }

    if let Some(code) = x_code
        .and_then(parse_code)
        .filter(|code| is_usable_code(*code))
    {
        return (code, CodeSource::Header);
    }

    (default, CodeSource::Default)
}

/// Código que aporta el path por sí solo, si aporta alguno.
///
/// Se extrae el entero del **primer segmento**, ignorando la extensión
/// (`/404.json` -> `Some(404)`). Un primer segmento que no sea un entero
/// (`/`, `/foo`, `""`) devuelve `None`.
///
/// Solo mira la **forma** del path: `/200` devuelve `Some(200)` aunque `200` no
/// sea utilizable como código de error. Es a propósito, porque lo usa el router
/// para decidir si una petición *es* una página de error —`/200` lo es, y debe
/// responder el código por defecto, no el fichero `200.html`—; quien recorta el
/// rango es [`resolve_code_with_source`], con [`is_usable_code`].
pub fn code_from_path(path: &str) -> Option<u16> {
    let first_segment = path
        .trim_start_matches('/')
        .split('/')
        .next()
        .unwrap_or_default();

    let stem = match first_segment.rsplit_once('.') {
        Some((stem, extension)) if format_of_extension(extension).is_some() => stem,
        _ => first_segment,
    };

    parse_code(stem)
}

/// Cascada del formato: extensión -> `Content-Type` -> `X-Format` -> `Accept`
/// -> `Format::Text`.
///
/// Las cabeceras se comparan por tipo MIME (se admite el sufijo `; charset=...`
/// y, en `Accept`, listas separadas por comas recorridas en orden: gana el
/// primer tipo soportado que aparezca). Una pista no soportada no decide: cae
/// al siguiente origen de la cascada.
pub fn resolve_format(
    path: &str,
    content_type: Option<&str>,
    x_format: Option<&str>,
    accept: Option<&str>,
) -> Format {
    if let Some(format) = format_of_path(path) {
        return format;
    }

    if let Some(format) = content_type.and_then(format_of_mime) {
        return format;
    }

    if let Some(format) = x_format.and_then(format_of_mime) {
        return format;
    }

    if let Some(format) = accept.and_then(format_of_accept) {
        return format;
    }

    Format::Text
}

/// Razón canónica del código, en inglés (`404` -> `"Not Found"`), como en
/// `tarampampam/error-pages`. Un código desconocido —o fuera del rango de
/// `http::StatusCode`— usa una razón genérica, nunca vacía.
pub fn canonical_reason(code: u16) -> &'static str {
    StatusCode::from_u16(code)
        .ok()
        .and_then(|status| status.canonical_reason())
        .unwrap_or(UNKNOWN_REASON)
}

/// Descripción larga del código; genérica para los códigos no tabulados.
///
/// Es **la fuente única** del texto de los cuatro formatos: el HTML la incrusta
/// al servir (rellenando `{{description}}`), y JSON, XML y texto la llevan tal
/// cual. Por eso la redacción no puede tener dos versiones —antes el `<p>` de las
/// páginas HTML escribía la suya a mano y cinco códigos decían una cosa en HTML y
/// otra en JSON—.
pub fn description(code: u16) -> &'static str {
    match code {
        400 => "The server cannot process the request because the client sent something malformed.",
        401 => "Authentication is required, and has either failed or not been provided.",
        403 => "The server understood the request but refuses to authorise it.",
        404 => "The requested resource could not be found on this server.",
        405 => "The request method is not supported by the resource you asked for.",
        408 => "The server timed out waiting for the client to finish sending the request.",
        410 => "The resource requested is no longer available and will not be available again.",
        418 => "The server is a teapot: it refuses to brew coffee.",
        422 => "The request was well-formed but could not be processed due to semantic errors.",
        429 => {
            "Too many requests have been sent in a given amount of time. Please try again later."
        }
        500 => "The server encountered an unexpected condition and could not complete the request.",
        501 => "The server does not support the functionality required to fulfil the request.",
        502 => "The server received an invalid response from an upstream server.",
        503 => {
            "The service is temporarily unavailable, usually because of maintenance or overload. \
             Please try again later."
        }
        504 => "An upstream server did not send a response in time.",
        _ => UNKNOWN_DESCRIPTION,
    }
}

/// Cabecera `Retry-After`: `Some(configured)` solo para `429` y `503`, `None`
/// para el resto (spec `error-page`, "Las cabeceras de acompañamiento").
///
/// Decide **qué códigos** llevan la cabecera —y eso no se configura: lo decide
/// el código— y deja el **valor** a la configuración que lo acompaña
/// (`RETRY_AFTER`, `120` por defecto). La frontera se mantiene así: `resolve`
/// decide, `render` compone.
pub fn retry_after_secs(code: u16, configured: u32) -> Option<u32> {
    match code {
        429 | 503 => Some(configured),
        _ => None,
    }
}

/// Parsea un código, rechazando lo que no sea un entero sin signo de 16 bits
/// (la cadena vacía incluida).
fn parse_code(raw: &str) -> Option<u16> {
    raw.parse::<u16>().ok()
}

/// Formato que declara la extensión del path, si declara alguno.
fn format_of_path(path: &str) -> Option<Format> {
    let last_segment = path
        .trim_start_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or_default();

    let (_, extension) = last_segment.rsplit_once('.')?;

    format_of_extension(extension)
}

/// Formato que declara una extensión de fichero (`json` -> `Json`).
fn format_of_extension(extension: &str) -> Option<Format> {
    match extension.to_ascii_lowercase().as_str() {
        "html" | "htm" => Some(Format::Html),
        "json" => Some(Format::Json),
        "xml" => Some(Format::Xml),
        "txt" => Some(Format::Text),
        _ => None,
    }
}

/// Formato que declara un tipo MIME, ignorando sus parámetros (`; charset=...`).
fn format_of_mime(value: &str) -> Option<Format> {
    let mime = match value.split(';').next() {
        Some(mime) => mime.trim(),
        None => "",
    };

    match mime.to_ascii_lowercase().as_str() {
        "text/html" | "application/xhtml+xml" => Some(Format::Html),
        "application/json" => Some(Format::Json),
        "application/xml" | "text/xml" => Some(Format::Xml),
        "text/plain" => Some(Format::Text),
        _ => None,
    }
}

/// Formato que declara una cabecera `Accept`: el primer tipo soportado de la
/// lista (las pistas no soportadas, como `image/png` o `*/*`, no deciden).
fn format_of_accept(value: &str) -> Option<Format> {
    value.split(',').find_map(format_of_mime)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// El código resuelto sin el origen: la cascada con la etiqueta descartada.
    fn resolve_code(path: &str, x_code: Option<&str>, default: u16) -> u16 {
        resolve_code_with_source(path, x_code, default).0
    }

    // --- Cascada del código (tasks.md 1.2) ---------------------------------

    #[test]
    fn el_codigo_del_path_se_resuelve() {
        assert_eq!(resolve_code("/404", None, 404), 404);
        assert_eq!(resolve_code("/503", None, 404), 503);
    }

    #[test]
    fn el_path_gana_a_la_cabecera_x_code() {
        assert_eq!(resolve_code("/404", Some("500"), 404), 404);
    }

    #[test]
    fn sin_codigo_en_el_path_gana_la_cabecera() {
        assert_eq!(resolve_code("/", Some("503"), 404), 503);
    }

    #[test]
    fn sin_pistas_gana_el_defecto() {
        assert_eq!(resolve_code("/", None, 404), 404);
        assert_eq!(resolve_code("/", None, 418), 418);
    }

    #[test]
    fn un_path_no_numerico_no_resuelve_por_esa_via() {
        assert_eq!(resolve_code("/foo", Some("500"), 404), 500);
        assert_eq!(resolve_code("/foo", None, 404), 404);
        assert_eq!(resolve_code("/", Some("no-es-un-numero"), 404), 404);
        assert_eq!(resolve_code("", None, 404), 404);
    }

    #[test]
    fn la_extension_no_confunde_al_codigo() {
        assert_eq!(resolve_code("/404.html", None, 404), 404);
        assert_eq!(resolve_code("/404.json", None, 404), 404);
        assert_eq!(resolve_code("/503.xml", None, 404), 503);
        assert_eq!(resolve_code("/404.txt", None, 404), 404);
        assert_eq!(resolve_code("/404.htm", None, 404), 404);
    }

    // --- Solo códigos utilizables (auditoría B-2) -------------------------

    #[test]
    fn el_rango_utilizable_es_400_599() {
        assert!(!is_usable_code(0));
        assert!(!is_usable_code(100));
        assert!(!is_usable_code(200));
        assert!(!is_usable_code(204));
        assert!(!is_usable_code(304));
        assert!(!is_usable_code(399));
        assert!(is_usable_code(400));
        assert!(is_usable_code(503));
        assert!(is_usable_code(599));
        assert!(!is_usable_code(600));
        assert!(!is_usable_code(1000));
    }

    #[test]
    fn un_codigo_no_utilizable_en_el_path_cae_al_defecto() {
        assert_eq!(resolve_code("/200", None, 404), 404);
        assert_eq!(resolve_code("/100", None, 404), 404);
        assert_eq!(resolve_code("/204", None, 404), 404);
        assert_eq!(resolve_code("/304", None, 404), 404);
        assert_eq!(resolve_code("/1000", None, 404), 404);
        assert_eq!(resolve_code("/200.html", None, 404), 404);
    }

    #[test]
    fn un_codigo_no_utilizable_en_el_path_deja_paso_a_la_cabecera() {
        assert_eq!(resolve_code("/200", Some("503"), 404), 503);
        assert_eq!(resolve_code("/100", Some("418"), 404), 418);
    }

    #[test]
    fn un_x_code_no_utilizable_cae_al_defecto() {
        assert_eq!(resolve_code("/", Some("200"), 404), 404);
        assert_eq!(resolve_code("/", Some("0"), 404), 404);
        assert_eq!(resolve_code("/", Some("100"), 404), 404);
        assert_eq!(resolve_code("/", Some("1000"), 404), 404);
        assert_eq!(resolve_code("/", Some("abc"), 404), 404);
        assert_eq!(resolve_code("/", Some(""), 404), 404);
    }

    #[test]
    fn un_path_con_forma_de_codigo_lo_aporta_aunque_no_sea_utilizable() {
        // La forma manda para clasificar; el rango se recorta al resolver.
        assert_eq!(code_from_path("/200"), Some(200));
        assert_eq!(code_from_path("/200.html"), Some(200));
        assert_eq!(code_from_path("/404.html"), Some(404));
    }

    // --- Cascada del formato (tasks.md 1.3) --------------------------------

    #[test]
    fn la_extension_manda() {
        assert_eq!(resolve_format("/404.json", None, None, None), Format::Json);
        assert_eq!(resolve_format("/404.xml", None, None, None), Format::Xml);
        assert_eq!(resolve_format("/404.txt", None, None, None), Format::Text);
        assert_eq!(resolve_format("/404.html", None, None, None), Format::Html);
        assert_eq!(resolve_format("/404.htm", None, None, None), Format::Html);
    }

    #[test]
    fn el_content_type_de_la_peticion_decide() {
        assert_eq!(
            resolve_format("/404", Some("application/json"), None, None),
            Format::Json
        );
    }

    #[test]
    fn x_format_decide_sin_extension() {
        assert_eq!(
            resolve_format("/404", None, Some("application/xml"), None),
            Format::Xml
        );
    }

    #[test]
    fn accept_decide_faltando_lo_demas() {
        assert_eq!(
            resolve_format("/404", None, None, Some("text/html")),
            Format::Html
        );
        assert_eq!(
            resolve_format("/404", None, None, Some("application/json")),
            Format::Json
        );
    }

    #[test]
    fn un_accept_de_navegador_real_cae_en_html() {
        let accept =
            "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8";
        assert_eq!(
            resolve_format("/404", None, None, Some(accept)),
            Format::Html
        );
    }

    #[test]
    fn un_content_type_con_charset_decide() {
        assert_eq!(
            resolve_format("/404", Some("application/json; charset=utf-8"), None, None),
            Format::Json
        );
    }

    #[test]
    fn sin_pistas_el_defecto_es_texto_plano() {
        assert_eq!(resolve_format("/404", None, None, None), Format::Text);
        assert_eq!(resolve_format("/", None, None, None), Format::Text);
    }

    #[test]
    fn el_orden_de_la_cascada_se_respeta() {
        // La extensión gana a todo lo demás.
        assert_eq!(
            resolve_format(
                "/404.json",
                Some("text/html"),
                Some("application/xml"),
                Some("text/html"),
            ),
            Format::Json
        );
        // `Content-Type` gana a `X-Format` y a `Accept`.
        assert_eq!(
            resolve_format(
                "/404",
                Some("application/json"),
                Some("application/xml"),
                Some("text/html"),
            ),
            Format::Json
        );
        // `X-Format` gana a `Accept`.
        assert_eq!(
            resolve_format("/404", None, Some("application/xml"), Some("text/html")),
            Format::Xml
        );
    }

    #[test]
    fn una_pista_no_soportada_cae_al_siguiente_origen() {
        assert_eq!(
            resolve_format("/404", Some("image/png"), None, Some("text/html")),
            Format::Html
        );
        assert_eq!(
            resolve_format("/404", None, Some("image/png"), Some("text/html")),
            Format::Html
        );
        assert_eq!(
            resolve_format("/404", None, None, Some("image/png")),
            Format::Text
        );
    }

    // --- Ortogonalidad código/formato (tasks.md 1.4) -----------------------

    #[test]
    fn codigo_y_formato_son_ortogonales_en_503_xml() {
        assert_eq!(resolve_code("/503.xml", None, 404), 503);
        assert_eq!(resolve_format("/503.xml", None, None, None), Format::Xml);
    }

    // --- Tablas de mensajes que consume `render` ---------------------------

    #[test]
    fn la_razon_canonica_de_los_codigos_comunes() {
        assert_eq!(canonical_reason(400), "Bad Request");
        assert_eq!(canonical_reason(404), "Not Found");
        assert_eq!(canonical_reason(418), "I'm a teapot");
        assert_eq!(canonical_reason(500), "Internal Server Error");
        assert_eq!(canonical_reason(503), "Service Unavailable");
        // Un código no tabulado usa una razón genérica, nunca vacía.
        assert!(!canonical_reason(599).is_empty());
    }

    #[test]
    fn la_descripcion_nunca_esta_vacia() {
        assert!(!description(404).is_empty());
        assert!(!description(503).is_empty());
        assert!(!description(599).is_empty());
    }

    #[test]
    fn retry_after_solo_para_429_y_503() {
        // Qué códigos llevan la cabecera lo sigue decidiendo esta función; el
        // valor lo aporta la configuración (aquí, el defecto `120`) y se
        // devuelve tal cual, sin retocarlo.
        assert_eq!(retry_after_secs(429, 120), Some(120));
        assert_eq!(retry_after_secs(503, 120), Some(120));
        assert_eq!(retry_after_secs(404, 120), None);
        assert_eq!(retry_after_secs(500, 120), None);

        // Con el valor esperado igual al del defecto, una implementación que
        // ignorase el parámetro y devolviese `120` fijo —la de antes de este
        // change— también pasaría las cuatro aserciones de arriba. Estos pares
        // usan un valor distinto del esperado para que el parámetro discrimine.
        assert_eq!(retry_after_secs(429, 30), Some(30));
        assert_eq!(retry_after_secs(503, 45), Some(45));

        // Caza la implementación que devolviese la cabecera para cualquier
        // código: un `404` con un valor distinto del defecto tiene que seguir
        // siendo `None`.
        assert_eq!(retry_after_secs(404, 30), None);
        // Caza cualquier recorte o saturación del valor: el extremo sale tal
        // cual, sin retocarlo.
        assert_eq!(retry_after_secs(503, u32::MAX), Some(u32::MAX));
    }
}
