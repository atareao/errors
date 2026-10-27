//! Construcción del cuerpo y de las cabeceras de una página de error.
//!
//! ---------------------------------------------------------------------------
//! API pública del módulo:
//!
//! ```ignore
//! #[derive(Debug)]
//! pub enum RenderError {
//!     /// Error de E/S: la página del tema existe y no se puede leer, o la
//!     /// serialización del cuerpo generado falló.
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
//! /// Cuerpo por formato. HTML es **la página del tema** elegido, en un solo
//! /// paso; JSON, XML y texto se generan a partir de `code`,
//! /// `canonical_reason(code)` y `description(code)` —y **ignoran** el tema—.
//! pub fn render_body(
//!     code: u16,
//!     format: Format,
//!     content_dir: &Path,
//!     theme: &Theme,
//! ) -> Result<Vec<u8>, RenderError>;
//!
//! /// `render_body` más las cabeceras y el estado.
//! pub fn render_page(
//!     config: &Config,
//!     code: u16,
//!     format: Format,
//!     theme: &Theme,
//! ) -> Result<RenderedPage, RenderError>;
//!
//! /// Respaldo de texto, infalible (no lee ficheros), con el estado y las
//! /// cabeceras de `code`: lo que se sirve cuando la página del tema existe y no
//! /// se puede leer.
//! pub fn render_text_fallback(code: u16, retry_after: u32) -> RenderedPage;
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
//!   `Retry-After` con el valor de `RETRY_AFTER` (`120` por defecto) solo en los
//!   códigos que lo llevan (`429` y `503`).
//!
//! # La cascada del cuerpo HTML: **un solo paso**
//!
//! El cuerpo HTML es **la página del tema** elegido ([`crate::theme::Theme::page`]):
//! la del operador en `CONTENT_DIR/<nombre>.html` si se puede leer y, si no, su
//! copia embebida. **No** hay capa por código: `CONTENT_DIR/<código>.html` y
//! `CONTENT_DIR/<código por defecto>.html` **no** se consultan. El motivo, escrito:
//! el repositorio entrega las páginas de los siete temas, así que una capa por
//! código del operador quedaría **siempre sombreada** —y una capa que nunca se lee
//! es una mentira del contrato—; el diseño por código tiene un sitio mejor, la
//! clase `c<código>` del `body`, que no puede discrepar del estado.
//!
//! Sea cual sea la página, el resultado pasa por [`plantilla`] —los marcadores
//! `{{code}}`/`{{reason}}`/`{{description}}`—: la sustitución se hace al servir,
//! no en el fichero. Un fallo de **E/S** al leer la página del operador sube como
//! [`RenderError::Io`] y **no** se tapa con la copia embebida: la página es
//! **contenido**, y el respaldo de texto —con el estado intacto— es la respuesta.

use crate::config::Config;
use crate::resolve::{self, Format};
use crate::theme::Theme;
use axum::http::StatusCode;
use serde::Serialize;
use std::fmt;
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
    /// Error de E/S al leer la página del tema del operador, o al serializar el
    /// cuerpo generado. Es el **único** fallo posible: la página embebida del
    /// tema cubre cualquier código de `400..=599`, así que la cascada solo puede
    /// quedarse sin página por una página del operador que exista y no se pueda
    /// leer.
    Io(std::io::Error),
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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

/// Cuerpo por formato. HTML es **la página del tema** ([`Theme::page`]) rellenada;
/// JSON, XML y texto se generan a partir de `code`, [`resolve::canonical_reason`]
/// y [`resolve::description`].
///
/// El `theme` solo se usa en [`Format::Html`]: los otros tres formatos no tienen
/// página y **lo ignoran**.
///
/// # Errors
///
/// [`RenderError::Io`] si la página del tema existe y no se puede leer (solo en
/// [`Format::Html`]), o si la serialización del cuerpo generado falla.
pub fn render_body(
    code: u16,
    format: Format,
    content_dir: &Path,
    theme: &Theme,
) -> Result<Vec<u8>, RenderError> {
    match format {
        Format::Html => html_body(code, content_dir, theme),
        Format::Json => json_body(code),
        Format::Xml => Ok(xml_body(code).into_bytes()),
        Format::Text => Ok(text_body(code).into_bytes()),
    }
}

/// [`render_body`] más las cabeceras de acompañamiento y el estado.
///
/// El valor de `Retry-After` sale de la [`Config`] que ya recibe: la firma no
/// cambia porque la configuración ya estaba aquí. El `theme` es el ya resuelto
/// para **esta** petición (`X-Theme` gana a `THEME`, ver [`crate::theme::resolve`]).
///
/// # Errors
///
/// Las mismas que [`render_body`].
pub fn render_page(
    config: &Config,
    code: u16,
    format: Format,
    theme: &Theme,
) -> Result<RenderedPage, RenderError> {
    let body = render_body(code, format, &config.content_dir, theme)?;

    Ok(RenderedPage {
        status: status_of(code),
        content_type: content_type_for(format),
        headers: companion_headers(code, config.retry_after),
        body,
    })
}

/// Respaldo de texto de una página de error: el cuerpo de [`Format::Text`] con
/// el estado y las cabeceras de `code`.
///
/// Es **infalible**: genera el cuerpo, así que no lee ningún fichero. Se sirve
/// cuando la página del tema **existe y no se puede leer** (`RenderError::Io`),
/// para no responder `500` por esa causa: el estado sigue siendo el código
/// resuelto (spec `error-page`, "Un fallo al leer el HTML no cambia el estado").
/// La **ausencia** de fichero ya no llega aquí: la página embebida del tema cubre
/// cualquier código, así que el respaldo queda reservado al fallo de E/S.
///
/// `retry_after` es el valor configurado ([`Config::retry_after`]): el respaldo
/// lleva las mismas cabeceras que [`render_page`], así que un `503` sin página
/// legible no pierde su `Retry-After` ni lo sirve con un valor distinto al
/// configurado.
pub fn render_text_fallback(code: u16, retry_after: u32) -> RenderedPage {
    RenderedPage {
        status: status_of(code),
        content_type: content_type_for(Format::Text),
        headers: companion_headers(code, retry_after),
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
/// los códigos que lo llevan, `Retry-After` con el valor configurado.
///
/// Qué códigos lo llevan lo decide [`resolve::retry_after_secs`]; aquí solo se
/// aporta `retry_after` ([`Config::retry_after`]) como valor.
fn companion_headers(code: u16, retry_after: u32) -> Vec<(&'static str, String)> {
    let mut headers = vec![(X_ROBOTS_TAG_NAME, X_ROBOTS_TAG_VALUE.to_string())];

    if let Some(secs) = resolve::retry_after_secs(code, retry_after) {
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

/// Cuerpo HTML: **la página del tema**, en un solo paso, rellenada.
///
/// El único paso es [`Theme::page`]: la página del operador en
/// `CONTENT_DIR/<nombre>.html` si se puede leer y, si no, su copia embebida. Una
/// página que existe y no se puede leer sale como [`RenderError::Io`] y **no**
/// cae a la copia embebida (Opción A de `design.md`): la página es **contenido**,
/// y tapar en silencio un directorio del operador roto ocultaría el fallo. El
/// registro lo delata y el estado no cambia.
///
/// El resultado pasa por [`plantilla`] —los marcadores `{{code}}`/`{{reason}}`/
/// `{{description}}`—: la sustitución se hace al servir, no en el fichero.
fn html_body(code: u16, content_dir: &Path, theme: &Theme) -> Result<Vec<u8>, RenderError> {
    let pagina = theme.page(content_dir).map_err(RenderError::Io)?;

    Ok(plantilla(&pagina, code))
}

/// La página con sus tres marcadores —`{{code}}`, `{{reason}}` y
/// `{{description}}`— rellenos con el código resuelto y con la razón canónica y
/// la descripción que devuelve [`resolve`].
///
/// Es la costura cómoda para producción: junta los datos que salen de las tablas
/// de [`resolve`] con la sustitución que hace [`rellena`]. Las reglas —y, sobre
/// todo, el **escape**— viven en [`rellena`]; esta función solo aporta los
/// valores crudos.
///
/// Es `pub(crate)` para que las pruebas del router puedan calcular el cuerpo
/// esperado de una página del operador con la **misma** lógica que producción, en
/// vez de reimplementarla.
pub(crate) fn plantilla(body: &[u8], code: u16) -> Vec<u8> {
    rellena(
        body,
        code,
        resolve::canonical_reason(code),
        resolve::description(code),
    )
}

/// Rellena los tres marcadores con el código y con esos valores, **escapados**
/// para HTML: los escribe el servidor, no un humano dentro del fichero.
///
/// Una página **sin** marcadores se sirve **byte a byte** —sin tocar el disco—,
/// se sustituye **cada** aparición y no solo la primera, y el escaneo es **por
/// bytes**, para no romper una página del operador que no esté en UTF-8. Lo que
/// se inserta se escapa con [`escape_html`]: la razón y la descripción las
/// escribe el servidor, no un humano dentro del fichero.
///
/// Cada reemplazo avanza por su **entrada** (ver [`reemplaza_todos`]), así que el
/// valor recién insertado no se vuelve a escanear; los valores son prosa del
/// proyecto y no pueden traer el marcador de otro —es una **suposición
/// declarada**, fijada en `un_valor_con_otro_marcador_sale_literal`—.
pub(crate) fn rellena(body: &[u8], code: u16, razon: &str, descripcion: &str) -> Vec<u8> {
    if !lleva_marcadores(body) {
        return body.to_vec();
    }

    let codigo = code.to_string();
    let razon = escape_html(razon);
    let descripcion = escape_html(descripcion);

    let cuerpo = reemplaza_todos(body, b"{{code}}", codigo.as_bytes());
    let cuerpo = reemplaza_todos(&cuerpo, b"{{reason}}", razon.as_bytes());

    reemplaza_todos(&cuerpo, b"{{description}}", descripcion.as_bytes())
}

/// ¿Lleva la página alguno de los tres marcadores de la plantilla?
fn lleva_marcadores(body: &[u8]) -> bool {
    [
        b"{{code}}".as_slice(),
        b"{{reason}}".as_slice(),
        b"{{description}}".as_slice(),
    ]
    .iter()
    .any(|marcador| busca(body, marcador).is_some())
}

/// Sustituye **todas** las apariciones de `aguja` por `valor`, por bytes.
///
/// Se avanza por el **documento de entrada** —nunca por la salida—: así el valor
/// recién insertado no se vuelve a escanear, aunque contuviera la aguja. El
/// escaneo es por bytes (ver [`plantilla`]) y no toca el disco.
fn reemplaza_todos(pajar: &[u8], aguja: &[u8], valor: &[u8]) -> Vec<u8> {
    debug_assert!(
        !aguja.is_empty(),
        "la aguja no puede estar vacía: el bucle no terminaría"
    );

    let mut salida = Vec::with_capacity(pajar.len());
    let mut resto = pajar;

    while let Some(inicio) = busca(resto, aguja) {
        salida.extend_from_slice(&resto[..inicio]);
        salida.extend_from_slice(valor);
        resto = &resto[inicio + aguja.len()..];
    }

    salida.extend_from_slice(resto);

    salida
}

/// Busca `aguja` en `pajar` y devuelve la posición de su primera aparición.
///
/// Es `pub(crate)` para que [`crate::theme`] use **esta** implementación —con
/// sus guardas para la aguja vacía y más larga que el pajar— en vez de mantener
/// una copia de prueba que podía divergir de la de producción.
pub(crate) fn busca(pajar: &[u8], aguja: &[u8]) -> Option<usize> {
    if aguja.is_empty() || aguja.len() > pajar.len() {
        return None;
    }

    pajar
        .windows(aguja.len())
        .position(|ventana| ventana == aguja)
}

/// Escapa `&`, `<` y `>` para insertar texto en HTML.
///
/// Son **esas tres** y no cinco. La decisión de **no** escapar la comilla sigue
/// siendo correcta: el único valor que se inserta **dentro de un atributo** es el
/// **código** (`content/default.html` pone `{{code}}` en `class="c{{code}}"`), y
/// el código es un `u16` re-serializado a decimal, así que solo puede producir
/// dígitos.
///
/// Un marcador **no numérico** dentro de un atributo sí exigiría escapar también
/// `"` y `'`: sin eso, una comilla del valor ahogaría el atributo y podría
/// cerrarlo antes de tiempo. Y escapar de más tampoco es gratis en contenido de
/// texto: convertiría en `&quot;` una comilla que el usuario debería leer tal
/// cual.
fn escape_html(texto: &str) -> String {
    let mut escapado = String::with_capacity(texto.len());

    for caracter in texto.chars() {
        match caracter {
            '&' => escapado.push_str("&amp;"),
            '<' => escapado.push_str("&lt;"),
            '>' => escapado.push_str("&gt;"),
            _ => escapado.push(caracter),
        }
    }

    escapado
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

    /// Fichero de fixture que representa el `default.html` del **operador**: la
    /// página propia de `tests/fixtures/content/`, **sin** ningún `<style>`, que se
    /// sirve rellenada.
    const FIXTURE_DEFAULT_HTML: &str = include_str!("../tests/fixtures/content/default.html");

    /// La página **embebida** del tema `default`: es `content/default.html`, con
    /// los marcadores `{{code}}`/`{{reason}}`/`{{description}}`. **Difiere** del
    /// fixture de arriba a propósito, y esa diferencia es la que impide invertir
    /// la precedencia: el del operador gana cuando existe.
    fn pagina_embebida(nombre: &str) -> &'static str {
        crate::embedded::EMBEBIDOS
            .iter()
            .find(|(n, _)| *n == nombre)
            .map(|(_, p)| *p)
            .unwrap_or_else(|| panic!("el tema embebido `{nombre}` no existe"))
    }

    /// La página del operador de `tests/fixtures/content/default.html` tal como se
    /// **sirve** para `code`: su plantilla rellenada.
    fn servida_del_fixture(code: u16) -> String {
        let rellena = plantilla(FIXTURE_DEFAULT_HTML.as_bytes(), code);
        let rellena =
            String::from_utf8(rellena).expect("la plantilla del fixture, rellenada, es UTF-8");

        // Red de la sustitución: si `plantilla` dejara de sustituir —o de usar el
        // escape—, **todas** las pruebas que pasan por este ayudante caerían de
        // golpe, en vez de quedarse verdes comparando la salida consigo misma.
        assert!(
            rellena.contains(&code.to_string()),
            "red de la sustitución: el cuerpo servido del fixture tiene que \
             **nombrar** su código (`{code}`), fue: {rellena}"
        );
        assert!(
            !rellena.contains("{{"),
            "red de la sustitución: no puede quedar ningún marcador sin rellenar, fue: {rellena}"
        );

        rellena
    }

    /// El tema por defecto, para las llamadas que no se ocupan del tema.
    fn tema_default() -> &'static Theme {
        use std::sync::OnceLock;
        static DEFAULT: OnceLock<Theme> = OnceLock::new();
        DEFAULT.get_or_init(|| {
            crate::theme::Themes::incorporados()
                .by_name(crate::theme::DEFAULT_THEME_NAME)
                .cloned()
                .expect("el tema por defecto tiene que estar en el registro")
        })
    }

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

    fn header<'a>(page: &'a RenderedPage, name: &str) -> Option<&'a str> {
        page.headers
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value.as_str())
    }

    // --- Cuerpo JSON, XML y texto ------------------------------------------

    #[test]
    fn el_cuerpo_json_lleva_las_cuatro_claves() {
        let body = render_body(404, Format::Json, &fixtures_content_dir(), tema_default()).unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["error"], Value::Bool(true));
        assert_eq!(json["code"], Value::from(404));
        assert_eq!(json["message"], Value::from("Not Found"));
        assert!(json["description"].is_string());
        assert!(!json["description"].as_str().unwrap().is_empty());
    }

    #[test]
    fn el_cuerpo_xml_lleva_el_elemento_code() {
        let body = render_body(503, Format::Xml, &fixtures_content_dir(), tema_default()).unwrap();
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
        let body = render_body(503, Format::Text, &fixtures_content_dir(), tema_default()).unwrap();
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

    // --- Cuerpo HTML: la página del tema, en un solo paso ------------------

    #[test]
    fn el_cuerpo_html_sale_de_la_pagina_del_tema_del_operador() {
        // El fichero del operador (`tests/fixtures/content/default.html`) es una
        // plantilla: su cuerpo servido son sus marcadores rellenados con el
        // código. La prueba fija las dos cosas: que sale **de ese fichero** —y no
        // de la página embebida— y que la plantilla lo rellena.
        let body = render_body(404, Format::Html, &fixtures_content_dir(), tema_default()).unwrap();
        let html = String::from_utf8(body).unwrap();

        assert_eq!(html.trim(), servida_del_fixture(404).trim());
        assert!(
            html.contains("404"),
            "el cuerpo tiene que nombrar su propio código (`404`): {html}"
        );
        assert!(
            !html.contains("{{"),
            "no puede quedar ningún marcador sin rellenar en el cuerpo: {html}"
        );
    }

    #[test]
    fn un_codigo_distinto_del_defecto_usa_la_pagina_del_tema() {
        // No hay capa por código: el código `418` también se sirve con la página
        // del tema `default` (`default.html`), rellenada con **418**.
        let body = render_body(418, Format::Html, &fixtures_content_dir(), tema_default()).unwrap();
        let html = String::from_utf8(body).unwrap();

        assert_eq!(html.trim(), servida_del_fixture(418).trim());
        assert!(
            html.contains("418"),
            "la página del operador se rellena con el código pedido (`418`): {html}"
        );
    }

    #[test]
    fn el_cuerpo_no_depende_del_formato_para_el_codigo() {
        // Ortogonalidad: el mismo `code` produce los cuatro cuerpos.
        for format in [Format::Html, Format::Json, Format::Xml, Format::Text] {
            let body = render_body(503, format, &fixtures_content_dir(), tema_default()).unwrap();
            assert!(!body.is_empty(), "cuerpo vacío para {format:?}");
        }
    }

    // --- Cabeceras de acompañamiento ---------------------------------------

    #[test]
    fn toda_pagina_de_error_lleva_x_robots_tag() {
        for code in [404, 500, 503] {
            let page = render_page(&fixtures_config(), code, Format::Json, tema_default()).unwrap();
            assert_eq!(
                header(&page, "X-Robots-Tag"),
                Some("noindex, nofollow, nosnippet, noarchive"),
                "X-Robots-Tag ausente o incorrecta para {code}"
            );
        }
    }

    #[test]
    fn el_503_lleva_retry_after_120() {
        let page = render_page(&fixtures_config(), 503, Format::Text, tema_default()).unwrap();
        assert_eq!(header(&page, "Retry-After"), Some("120"));
    }

    #[test]
    fn el_429_lleva_retry_after_120() {
        let page = render_page(&fixtures_config(), 429, Format::Text, tema_default()).unwrap();
        assert_eq!(header(&page, "Retry-After"), Some("120"));
    }

    #[test]
    fn el_404_no_lleva_retry_after() {
        let page = render_page(&fixtures_config(), 404, Format::Text, tema_default()).unwrap();
        assert_eq!(header(&page, "Retry-After"), None);
    }

    #[test]
    fn el_estado_es_el_codigo_resuelto_y_nunca_200() {
        let page = render_page(&fixtures_config(), 404, Format::Html, tema_default()).unwrap();
        assert_eq!(page.status, StatusCode::NOT_FOUND);
        assert_ne!(page.status, StatusCode::OK);

        let page = render_page(&fixtures_config(), 503, Format::Html, tema_default()).unwrap();
        assert_eq!(page.status, StatusCode::SERVICE_UNAVAILABLE);
    }

    #[test]
    fn el_content_type_sigue_al_formato() {
        let content_type = |format| {
            render_page(&fixtures_config(), 404, format, tema_default())
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
        // Auditoría I-2: el respaldo de texto es lo que se sirve cuando la página
        // del tema **existe y no se puede leer** (fallo de E/S); la **falta** de
        // fichero ya no llega aquí —la cubre la página embebida—. Aquí se fija lo
        // suyo: el estado sigue siendo el código resuelto (nunca `500`) y la
        // página no pierde `X-Robots-Tag`.
        let page = render_text_fallback(418, 120);

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
            header(&render_text_fallback(503, 120), "Retry-After"),
            Some("120")
        );
        assert_eq!(header(&render_text_fallback(404, 120), "Retry-After"), None);
    }

    // --- `RETRY_AFTER` configurado -----------------------------------------

    /// [`fixtures_config`] con `RETRY_AFTER` fijado: misma forma, sin tocar el
    /// ayudante que usan las pruebas del valor por defecto.
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

    #[test]
    fn el_503_lleva_el_retry_after_configurado() {
        let config = fixtures_config_con_retry_after("30");
        let page = render_page(&config, 503, Format::Text, tema_default()).unwrap();

        assert_eq!(
            header(&page, "Retry-After"),
            Some("30"),
            "el `503` debe llevar el valor configurado, no el 120 del defecto"
        );
    }

    #[test]
    fn el_429_lleva_el_retry_after_configurado() {
        let config = fixtures_config_con_retry_after("45");
        let page = render_page(&config, 429, Format::Text, tema_default()).unwrap();

        assert_eq!(
            header(&page, "Retry-After"),
            Some("45"),
            "el `429` debe llevar el valor configurado, no el 120 del defecto"
        );
    }

    #[test]
    fn el_retry_after_cero_sale_en_la_cabecera() {
        // `0` es un `delay-seconds` legal de RFC 9110 («reintenta ya») y tiene que
        // salir tal cual. Con un `if secs > 0` en `companion_headers`, el árbol
        // seguiría verde y la cabecera desaparecería en silencio.
        let config = fixtures_config_con_retry_after("0");
        let page = render_page(&config, 503, Format::Text, tema_default()).unwrap();

        assert_eq!(
            header(&page, "Retry-After"),
            Some("0"),
            "un `RETRY_AFTER=0` debe emitir `Retry-After: 0`, no suprimir la cabecera"
        );
    }

    #[test]
    fn el_404_no_lleva_retry_after_ni_configurado() {
        let config = fixtures_config_con_retry_after("30");
        let page = render_page(&config, 404, Format::Text, tema_default()).unwrap();

        assert_eq!(
            header(&page, "Retry-After"),
            None,
            "un `404` no lleva `Retry-After` por mucho que haya `RETRY_AFTER` configurado"
        );
    }

    #[test]
    fn el_respaldo_de_texto_lleva_el_retry_after_configurado() {
        let page = render_text_fallback(503, 30);

        assert_eq!(
            header(&page, "Retry-After"),
            Some("30"),
            "el respaldo del `503` debe llevar el valor configurado"
        );
        assert_eq!(page.status, StatusCode::SERVICE_UNAVAILABLE);
    }

    // --- Página embebida sin ficheros de datos -----------------------------
    //
    // La página del tema va embebida con `include_str!`, así que un `CONTENT_DIR`
    // que no existe ya no deja la cascada sin página: el disco no aporta nada y
    // entra la página del proyecto, rellenada con el código resuelto. El respaldo
    // de texto queda reservado al fallo de E/S.

    #[test]
    fn sin_directorio_el_cuerpo_html_sale_de_la_pagina_embebida_del_tema() {
        let body = render_body(503, Format::Html, &sin_contenido_dir(), tema_default())
            .expect("la página embebida está siempre");

        let rellenada = plantilla(pagina_embebida("default").as_bytes(), 503);

        assert_eq!(
            body, rellenada,
            "sin directorio, el cuerpo tiene que ser la página embebida del tema rellenada con `503`"
        );
        assert!(
            String::from_utf8(body).unwrap().contains("503"),
            "la página rellenada tiene que nombrar su código"
        );
    }

    #[test]
    fn sin_directorio_un_codigo_sin_pagina_propia_recibe_la_pagina_con_su_codigo() {
        // No hay página propia del `418`, así que el cuerpo es la página embebida
        // del tema rellenada con **418** —no con el defecto—, y `render_page`
        // conserva el estado `418`.
        let body = render_body(418, Format::Html, &sin_contenido_dir(), tema_default())
            .expect("la página embebida está siempre");

        let rellenada = plantilla(pagina_embebida("default").as_bytes(), 418);

        // Anclajes **independientes** de `plantilla`: el cuerpo servido nombra su
        // propio código y no deja ningún marcador.
        let texto = String::from_utf8(body.clone()).expect("el cuerpo servido es UTF-8");
        assert!(
            texto.contains("418"),
            "el cuerpo tiene que nombrar su propio código (`418`), no el defecto: {texto}"
        );
        assert!(
            !texto.contains("{{"),
            "no puede quedar ningún marcador sin rellenar en el cuerpo servido: {texto}"
        );

        assert_eq!(
            body, rellenada,
            "un código sin fichero propio recibe la página rellenada con **su** código (`418`)"
        );

        let page = render_page(&sin_contenido_config(), 418, Format::Html, tema_default())
            .expect("la página embebida está siempre");
        assert_eq!(
            page.status,
            StatusCode::IM_A_TEAPOT,
            "el estado sigue siendo el código resuelto (`418`)"
        );
        assert_eq!(page.body, rellenada);
    }

    #[test]
    fn la_cascada_es_un_solo_paso_y_el_unico_fallo_es_la_ilegibilidad() {
        // Ausencia de fichero: nunca falta página —la página embebida cubre
        // cualquier código—. Con un código que no tiene fichero (`599`), el
        // resultado es la página embebida rellenada con `599`, no un error.
        let body = render_body(599, Format::Html, &sin_contenido_dir(), tema_default())
            .expect("la página embebida cubre cualquier código: nunca falta la página");
        let texto = String::from_utf8(body).expect("la página servida es UTF-8");
        assert!(
            texto.contains("599"),
            "un `599` sin fichero propio recibe la página rellenada con `599`: {texto}"
        );

        // El único fallo posible es de **E/S**: la página del tema que existe y no
        // se puede leer (`tests/fixtures/pagina-ilegible/default.html` es un
        // directorio). Sigue siendo `Io`, y el respaldo de texto lo cubre.
        let ilegible = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pagina-ilegible");
        match render_body(404, Format::Html, &ilegible, tema_default()) {
            Err(RenderError::Io(_)) => {}
            other => panic!("una página ilegible tiene que ser `Io`: {other:?}"),
        }
    }

    // --- Plantilla del cuerpo HTML -----------------------------------------

    /// Una plantilla mínima con los tres marcadores, uno de ellos repetido: el
    /// `{{code}}` del título y el del `h1` son **dos apariciones** del mismo
    /// marcador.
    const PLANTILLA: &str =
        "<title>{{code}}</title><h1>{{code}}</h1><h2>{{reason}}</h2><p>{{description}}</p>";

    #[test]
    fn la_plantilla_rellena_el_codigo_la_razon_y_la_descripcion() {
        let rellena = plantilla(PLANTILLA.as_bytes(), 404);
        let rellena = String::from_utf8(rellena).expect("la plantilla de prueba es UTF-8");

        assert!(
            !rellena.contains("{{"),
            "no puede quedar ningún marcador sin rellenar: {rellena}"
        );
        assert!(
            rellena.contains(resolve::canonical_reason(404)),
            "la razón canónica tiene que estar en el cuerpo: {rellena}"
        );
        assert!(
            rellena.contains(resolve::description(404)),
            "la descripción tiene que estar en el cuerpo: {rellena}"
        );
    }

    #[test]
    fn la_plantilla_rellena_cada_aparicion() {
        let rellena = plantilla(PLANTILLA.as_bytes(), 503);
        let rellena = String::from_utf8(rellena).expect("la plantilla de prueba es UTF-8");

        assert_eq!(
            rellena.matches("503").count(),
            2,
            "las **dos** apariciones de `{{code}}` tienen que rellenarse, no solo la primera: \
             {rellena}"
        );
    }

    #[test]
    fn una_pagina_sin_marcadores_se_sirve_byte_a_byte() {
        let original = b"<p>esta pagina es mia y no lleva marcadores</p>";

        assert_eq!(
            plantilla(original, 404),
            original.to_vec(),
            "una pagina sin marcadores no se toca"
        );
    }

    #[test]
    fn la_sustitucion_es_por_bytes_y_respeta_lo_no_utf8() {
        // El escaneo es **por bytes**, porque una página del operador puede no
        // estar en UTF-8 (spec `error-page`, «El cuerpo HTML es la página del
        // tema»). Aquí los bytes `\xE9` y `\xF1` —latin-1, **no** UTF-8 válido—
        // rodean un `{{code}}` literal; la sustitución tiene que ocurrir igual y
        // los bytes de fuera del marcador tienen que salir **idénticos**, sin
        // pasar por `&str` en ningún punto. Si `rellena` decodificara con
        // `String::from_utf8_lossy`, `\xE9` se volvería `\u{FFFD}` y esta prueba
        // caería.
        let original: &[u8] = b"<p>caf\xE9 \xF1 {{code}}</p>";
        let rellena = plantilla(original, 404);

        assert!(
            rellena.contains(&0xE9),
            "el byte `\\xE9` de la página tiene que sobrevivir intacto: {rellena:?}"
        );
        assert!(
            rellena.contains(&0xF1),
            "el byte `\\xF1` de la página tiene que sobrevivir intacto: {rellena:?}"
        );
        assert!(
            !rellena.windows(3).any(|ventana| ventana == b"\xEF\xBF\xBD"),
            "no puede aparecer el carácter de reemplazo U+FFFD: la sustitución es por bytes, no \
             por `str`: {rellena:?}"
        );
        assert!(
            rellena.windows(3).any(|ventana| ventana == b"404"),
            "la sustitución tiene que ocurrir igual aunque la página no sea UTF-8: {rellena:?}"
        );
    }

    #[test]
    fn escape_html_escapa_lo_que_se_inserta() {
        assert_eq!(
            escape_html("a & b < c > d"),
            "a &amp; b &lt; c &gt; d",
            "el `&`, el `<` y el `>` tienen que salir escapados: los inserta el servidor"
        );
    }

    #[test]
    fn la_sustitucion_escapa_lo_que_inserta() {
        // La red que faltaba para el `SHALL` del escape: la tabla de `resolve` no
        // tiene hoy ningún `&`, `<` ni `>` —medido—, así que ninguna prueba con un
        // código real demuestra que `rellena` escape lo que inserta. La costura se
        // prueba con un valor **adversario**: es lo único que demuestra que la
        // sustitución **usa** el escape que tiene al lado.
        let rellena = rellena(
            b"<h2>{{reason}}</h2><p>{{description}}</p>",
            404,
            "a & b",
            "c < d > e",
        );
        let rellena = String::from_utf8(rellena).expect("la plantilla de prueba es UTF-8");

        assert!(
            rellena.contains("a &amp; b"),
            "la razón tiene que salir escapada: {rellena}"
        );
        assert!(
            rellena.contains("c &lt; d &gt; e"),
            "la descripción tiene que salir escapada: {rellena}"
        );
        assert!(
            !rellena.contains('&') || !rellena.contains("< d"),
            "no puede quedar ningún metacarácter sin escapar: {rellena}"
        );
    }

    #[test]
    fn un_valor_con_otro_marcador_sale_literal() {
        // Es una **suposición declarada**, no un deseo: `rellena` avanza por la
        // entrada y no reescanea lo insertado, así que un valor que contuviera
        // `{{code}}` sale **literal**. Hoy es inalcanzable —el código es un `u16`
        // re-serializado y las tablas de `resolve` son literales fijos sin
        // llaves—, pero la prueba lo deja escrito: cambiar esa garantía
        // (reescanear la salida) sería una decisión consciente, no un efecto
        // colateral.
        let rellena = rellena(b"<p>{{reason}}</p>", 404, "x {{code}} y", "z");
        let rellena = String::from_utf8(rellena).expect("la plantilla de prueba es UTF-8");

        assert!(
            rellena.contains("x {{code}} y"),
            "el valor insertado no se reescanea: `{{code}}` tiene que quedar literal, \
             fue: {rellena}"
        );
    }

    #[test]
    fn un_codigo_sin_fichero_propio_no_recibe_el_cuerpo_de_otro() {
        // El caso que da nombre al change `error-page-template`: el `451` recibe
        // **su** cuerpo, rellenado con `451`, y no el de otro código.
        let pagina = render_page(&fixtures_config(), 451, Format::Html, tema_default())
            .expect("la página del tema siempre está");
        let cuerpo = String::from_utf8(pagina.body).expect("el HTML es UTF-8");

        assert!(
            cuerpo.contains("451"),
            "el cuerpo tiene que nombrar su propio código: {cuerpo}"
        );
        assert!(
            !cuerpo.contains("404"),
            "el cuerpo no puede nombrar el `404`: {cuerpo}"
        );
    }
}
