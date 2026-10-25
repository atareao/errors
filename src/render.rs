//! Construcción del cuerpo y de las cabeceras de una página de error.
//!
//! ---------------------------------------------------------------------------
//! API pública del módulo:
//!
//! ```ignore
//! #[derive(Debug)]
//! pub enum RenderError {
//!     /// Error de E/S: la página del operador existe y no se puede leer, o la
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
//! /// Cuerpo por formato. HTML se lee de `content_dir/<code>.html` con caída a
//! /// `content_dir/<default_code>.html`; agotadas esas dos, entra la **plantilla
//! /// embebida**, rellenada con el código resuelto. El HTML se sirve
//! /// **autocontenido**: el marcador `<link rel="stylesheet" href="/css/error.css" />`
//! /// se sustituye por un `<style>` con la hoja del `theme`. JSON, XML y texto se
//! /// generan a partir de `code`, `canonical_reason(code)` y `description(code)`
//! /// —y **ignoran** el tema—.
//! pub fn render_body(
//!     code: u16,
//!     default_code: u16,
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
//! /// cabeceras de `code`: lo que se sirve cuando la página del operador existe
//! /// y no se puede leer.
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
//! Contrato de la spec `error-page` (change `self-contained-themes`, "El HTML
//! servido es autocontenido"): la respuesta HTML no pide ningún fichero para
//! verse —el marcador se sustituye por un `<style>` con la hoja del tema
//! elegido—, una página **sin** marcador se sirve **byte a byte** sin leer
//! ninguna hoja, y un `<link>` que **no** sea el marcador se deja intacto.

use crate::config::Config;
use crate::embedded;
use crate::resolve::{self, Format};
use crate::theme::Theme;
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
    /// Error de E/S al leer el HTML del operador, o al serializar el cuerpo
    /// generado. Es el **único** fallo posible: la plantilla embebida cubre
    /// cualquier código de `400..=599`, así que la cascada ya no puede quedarse
    /// sin página por **ausencia** de fichero —solo por una página del operador
    /// que exista y no se pueda leer—.
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

/// Cuerpo por formato. HTML se lee de `content_dir/<code>.html` con caída a
/// `content_dir/<default_code>.html`; agotadas esas dos, la **plantilla
/// embebida** rellenada con el código. JSON, XML y texto se generan a partir de
/// `code`, [`resolve::canonical_reason`] y [`resolve::description`].
///
/// El `theme` solo se usa en [`Format::Html`] —para la hoja que se incrusta—:
/// los otros tres formatos no tienen hoja y **lo ignoran**.
///
/// # Errors
///
/// [`RenderError::Io`] si la página del operador existe y no se puede leer
/// (solo en [`Format::Html`]), o si la serialización del cuerpo generado falla.
pub fn render_body(
    code: u16,
    default_code: u16,
    format: Format,
    content_dir: &Path,
    theme: &Theme,
) -> Result<Vec<u8>, RenderError> {
    match format {
        Format::Html => html_body(code, default_code, content_dir, theme),
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
    let body = render_body(
        code,
        config.default_error_page,
        format,
        &config.content_dir,
        theme,
    )?;

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
/// cuando una página del operador **existe y no se puede leer** (`RenderError::Io`),
/// para no responder `500` por esa causa: el estado sigue siendo el código
/// resuelto (spec `error-page`, "Un fallo al leer el HTML no cambia el estado").
/// La **ausencia** de fichero ya no llega aquí: la plantilla embebida cubre
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

/// Cuerpo HTML: la capa del operador **entera** y, solo si no da nada, la
/// **plantilla embebida**.
///
/// Los **dos primeros** pasos de la cascada son del **operador** —el fichero del
/// código pedido y, si no existe, el del código por defecto, en `CONTENT_DIR`—;
/// el **tercero** —la plantilla embebida— es del **proyecto**, y cubre cualquier
/// código de `400..=599` rellenada con el código resuelto. Una página que existe
/// y no se puede leer sale como [`RenderError::Io`] y **no** cae a la plantilla
/// (Opción A de `design.md`): es un error de configuración que el registro
/// delata, y la plantilla lo taparía en silencio.
///
/// La asimetría con la hoja sigue siendo deliberada: **la hoja hace lo
/// contrario** —un `warn!` y la copia embebida, nunca un error— porque la página
/// es **contenido** y la hoja **adorno**; degradar un `503` a texto plano porque
/// un CSS no se pudo leer empeoraría el síntoma que el servicio existe para
/// aliviar. El detalle está en [`Theme::sheet`].
///
/// Sea cual sea la capa que aporte la página, el resultado pasa por
/// [`plantilla`] —los marcadores `{{code}}`/`{{reason}}`/`{{description}}`— y por
/// [`autocontenido`] —el marcador de la hoja—: las dos sustituciones se hacen al
/// servir, no en el fichero.
fn html_body(
    code: u16,
    default_code: u16,
    content_dir: &Path,
    theme: &Theme,
) -> Result<Vec<u8>, RenderError> {
    let body = raw_html(code, default_code, content_dir)?;
    let body = plantilla(&body, code);

    Ok(autocontenido(&body, content_dir, theme))
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
/// Son **tres** agujeros, y las reglas son las del marcador de la hoja
/// ([`autocontenido`]): una página **sin** marcadores se sirve **byte a byte**
/// —sin tocar el disco—, se sustituye **cada** aparición y no solo la primera, y
/// el escaneo es **por bytes**, para no romper una página del operador que no
/// esté en UTF-8. Lo que se inserta se escapa con [`escape_html`]: la razón y la
/// descripción las escribe el servidor, no un humano dentro del fichero.
///
/// Cada reemplazo avanza por su **entrada** (ver [`reemplaza_todos`]), así que el
/// valor recién insertado no se vuelve a escanear; los valores son prosa del
/// proyecto y no pueden traer el marcador de otro —es una **suposición
/// declarada**, fijada en `un_valor_con_otro_marcador_sale_literal`—.
fn rellena(body: &[u8], code: u16, razon: &str, descripcion: &str) -> Vec<u8> {
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
/// siendo correcta, pero por otro motivo del que decía antes este comentario: el
/// único valor que se inserta **dentro de un atributo** es el **código**
/// (`content/404.html` pone `{{code}}` en `class="c{{code}}"`), y el código es un
/// `u16` re-serializado a decimal, así que solo puede producir dígitos.
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

/// La cascada del HTML: `CONTENT_DIR/<código>.html` → `CONTENT_DIR/<código por
/// defecto>.html` → la **plantilla embebida**.
///
/// Los dos primeros pasos son del **operador** (su `CONTENT_DIR`); el tercero es
/// del **proyecto** y no puede fallar: la plantilla está en el binario y cubre
/// cualquier código, así que la cascada nunca se queda sin página. Por eso el
/// único error posible es de E/S —una página del operador que existe y no se
/// puede leer—.
fn raw_html(code: u16, default_code: u16, content_dir: &Path) -> Result<Vec<u8>, RenderError> {
    if let Some(body) = read_html(content_dir, code).map_err(RenderError::Io)? {
        return Ok(body);
    }

    if code != default_code
        && let Some(body) = read_html(content_dir, default_code).map_err(RenderError::Io)?
    {
        return Ok(body);
    }

    // Agotado el disco, la plantilla del proyecto. Un `&'static str` embebido no
    // puede fallar al leerse: los bytes ya están en el binario, así que esta capa
    // no aporta una variante nueva a `RenderError`.
    Ok(embedded::PAGE.as_bytes().to_vec())
}

/// Sirve la página **autocontenida**: el marcador `<link rel="stylesheet"
/// href="/css/error.css" />` se sustituye por un `<style>` con la hoja del tema.
///
/// La razón del cambio: con el middleware `errors` de Traefik, la página se
/// sirve **en el host de la aplicación**, y allí `/css/error.css` lo contesta el
/// upstream, no este servicio. El navegador recibe su `404` (con un
/// `Content-Type` que no es CSS) y rechaza la hoja, así que la página llega sin
/// estilos. Incrustando la hoja, la página deja de pedir nada para verse.
///
/// Tres reglas, y las tres importan:
///
/// - **Sin marcador no se lee ninguna hoja** y el HTML se devuelve byte a byte.
///   No es un atajo: es lo que garantiza que un CSS roto no pueda afectar a una
///   página que no lo necesita, y lo que mantiene válidos los fixtures —una
///   página propia del operador, ya autocontenida, se sirve tal cual—.
/// - Un `<link>` que **no** sea el marcador se deja intacto: es del operador.
/// - Se sustituye **cada** aparición del marcador, no solo la primera. Un
///   `<link>` sin escapar solo puede aparecer de dos formas: como etiqueta —y
///   entonces hay que sustituirlo— o dentro de un comentario —para mostrarlo
///   como texto hay que escaparlo (`&lt;link`) y entonces ya no casa con
///   `<link`—. En el segundo caso el `<style>` resultante queda dentro del
///   comentario, invisible. Lo que **no** puede quedar en ningún caso es un
///   enlace vivo a `css/error.css`. El escaneo es textual y **no** respeta
///   comentarios: respetarlos exigiría llevar su estado (o un parser) para un
///   caso que no cambia nada de lo que el usuario ve.
///
/// El escaneo es **por bytes**, no por `str`: el marcador (`<link`, `rel`,
/// `href`, `/css/error.css`) es ASCII puro, así que una página del operador en
/// otra codificación —ISO-8859-1, Shift_JIS— tiene que incrustar la hoja igual.
/// Decodificar a `str` y rendirse ante bytes no-UTF-8 dejaría el enlace roto y
/// sin estilos **en silencio** en esa página —el `warn!` no saltaría porque no
/// hay error—, que es justo el fallo que este change arregla.
fn autocontenido(body: &[u8], content_dir: &Path, theme: &Theme) -> Vec<u8> {
    // La comprobación va **antes** de leer la hoja: sin marcador no se lee, y con
    // esto el camino rápido no toca el disco.
    if marcador(body).is_none() {
        return body.to_vec();
    }

    let hoja = theme.sheet(content_dir);

    // Se compone por bytes: la hoja del operador no tiene por qué ser UTF-8
    // válido, y sustituirla con pérdida cambiaría los bytes servidos.
    let mut salida = Vec::with_capacity(body.len() + hoja.len() + "<style>\n\n</style>".len());
    let mut resto = body;

    // Se avanza por el **documento de entrada**, nunca por la salida: así el
    // `<style>` recién insertado no se vuelve a escanear.
    while let Some((inicio, fin)) = marcador(resto) {
        salida.extend_from_slice(&resto[..inicio]);
        salida.extend_from_slice(b"<style>\n");
        salida.extend_from_slice(&hoja);
        salida.extend_from_slice(b"\n</style>");
        resto = &resto[fin..];
    }

    salida.extend_from_slice(resto);

    salida
}

/// El nombre de etiqueta del marcador, en bytes.
const MARCADOR_ETIQUETA: &[u8] = b"link";
/// El valor de `href` que identifica al marcador. Comparación **exacta**.
const MARCADOR_HREF: &str = "/css/error.css";

/// Localiza el elemento marcador y devuelve el rango de bytes que ocupa, desde
/// el `<` hasta el `>` que cierra la etiqueta.
///
/// El marcador es un `<link>` —nombre de etiqueta comparado **sin distinguir
/// mayúsculas**— que lleve `rel` con el valor `stylesheet` (también sin
/// distinguir mayúsculas) y `href` con el valor **exacto** `/css/error.css`. Da
/// igual el orden de los atributos y las dos comillas valen.
fn marcador(html: &[u8]) -> Option<(usize, usize)> {
    let mut cursor = 0;

    while let Some(relativo) = html[cursor..].iter().position(|&byte| byte == b'<') {
        let inicio = cursor + relativo;
        let nombre = inicio + 1;

        let es_link = html
            .get(nombre..nombre + MARCADOR_ETIQUETA.len())
            .is_some_and(|candidato| candidato.eq_ignore_ascii_case(MARCADOR_ETIQUETA))
            && html
                .get(nombre + MARCADOR_ETIQUETA.len())
                .copied()
                .is_some_and(es_frontera_de_etiqueta);

        if es_link
            && let Some((fin, atributos)) =
                atributos_de_etiqueta(html, nombre + MARCADOR_ETIQUETA.len())
            && es_el_marcador(&atributos)
        {
            return Some((inicio, fin));
        }

        cursor = inicio + 1;
    }

    None
}

/// ¿El byte cierra el nombre de la etiqueta? Espacio, tabulador, salto, `/` o
/// `>`. Sin esta frontera, `<linking …>` se tomaría por un `<link>`.
fn es_frontera_de_etiqueta(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | b'/' | b'>')
}

fn es_espacio(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r')
}

/// Los atributos de una etiqueta como pares `(nombre, valor)`, cada uno un tramo
/// del propio documento. Son **bytes** y no `str`: el escaneo es por bytes, para
/// que una página en otra codificación también case con el marcador (ver
/// [`autocontenido`]). El alias evita repetir el tipo anidado en la firma.
type Atributos<'a> = Vec<(&'a [u8], &'a [u8])>;

/// Los atributos de una etiqueta, desde `cursor` —el byte siguiente al nombre—
/// hasta el `>` que la cierra. Devuelve el índice **después** del `>` y la lista
/// de pares `(nombre, valor)`.
///
/// Sin dependencias: no hay `regex` ni un parser de HTML —el árbol compila sin
/// red—, y un `<link>` no tiene contenido anidado, así que basta con recorrer
/// hasta el `>` que no esté entre comillas. Devuelve `None` si la etiqueta no
/// cierra antes de terminar el documento.
fn atributos_de_etiqueta(html: &[u8], mut cursor: usize) -> Option<(usize, Atributos<'_>)> {
    let mut atributos = Atributos::new();

    loop {
        while html.get(cursor).copied().is_some_and(es_espacio) {
            cursor += 1;
        }

        match html.get(cursor).copied()? {
            b'>' => return Some((cursor + 1, atributos)),
            // El `/` de una etiqueta vacía (`<link … />`); la iteración
            // siguiente encuentra el `>`.
            b'/' => cursor += 1,
            _ => {
                let inicio_nombre = cursor;
                while let Some(byte) = html.get(cursor).copied() {
                    if es_espacio(byte) || matches!(byte, b'=' | b'>' | b'/') {
                        break;
                    }
                    cursor += 1;
                }
                let nombre = &html[inicio_nombre..cursor];

                while html.get(cursor).copied().is_some_and(es_espacio) {
                    cursor += 1;
                }

                let mut valor: &[u8] = b"";
                if html.get(cursor) == Some(&b'=') {
                    cursor += 1;
                    while html.get(cursor).copied().is_some_and(es_espacio) {
                        cursor += 1;
                    }

                    match html.get(cursor).copied()? {
                        comilla @ (b'"' | b'\'') => {
                            cursor += 1;
                            let inicio_valor = cursor;
                            while html
                                .get(cursor)
                                .copied()
                                .is_some_and(|byte| byte != comilla)
                            {
                                cursor += 1;
                            }
                            valor = &html[inicio_valor..cursor];
                            if html.get(cursor).copied() == Some(comilla) {
                                cursor += 1;
                            }
                        }
                        _ => {
                            let inicio_valor = cursor;
                            while let Some(byte) = html.get(cursor).copied() {
                                if es_espacio(byte) || byte == b'>' {
                                    break;
                                }
                                cursor += 1;
                            }
                            valor = &html[inicio_valor..cursor];
                        }
                    }
                }

                atributos.push((nombre, valor));
            }
        }
    }
}

/// ¿Los atributos describen el marcador? `rel` vale `stylesheet` (sin distinguir
/// mayúsculas) y `href` vale **exactamente** `/css/error.css`.
fn es_el_marcador(atributos: &[(&[u8], &[u8])]) -> bool {
    let rel = atributos
        .iter()
        .find(|(nombre, _)| nombre.eq_ignore_ascii_case(b"rel"));
    let href = atributos
        .iter()
        .find(|(nombre, _)| nombre.eq_ignore_ascii_case(b"href"));

    match (rel, href) {
        (Some((_, rel)), Some((_, href))) => {
            rel.eq_ignore_ascii_case(b"stylesheet") && *href == MARCADOR_HREF.as_bytes()
        }
        _ => false,
    }
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

    /// Fichero de fixture que representa `content/404.html` del **operador**: la
    /// página propia de `tests/fixtures/content/`, sin marcadores de plantilla ni
    /// de hoja, que se sirve **byte a byte**.
    const FIXTURE_404_HTML: &str = include_str!("../tests/fixtures/content/404.html");

    /// La **plantilla** embebida del proyecto: es `content/404.html` del
    /// repositorio, con los marcadores `{{code}}`/`{{reason}}`/`{{description}}`,
    /// que la fase GREEN embebe. **Difiere** del fixture de arriba a propósito, y
    /// esa diferencia es la que impide invertir la precedencia: el del operador
    /// gana cuando existe.
    const EMBEDDED_404_HTML: &str = include_str!("../content/404.html");

    /// Hoja de estilos **embebida** del tema `default`: es
    /// `content/css/error.css`, la misma constante que alimenta el asset
    /// `/css/error.css`. Las páginas de `content/` llevan el marcador, así que
    /// al servirse sale esta hoja incrustada en un `<style>`.
    const EMBEDDED_CSS: &str = include_str!("../content/css/error.css");

    /// El marcador, tal cual está en las páginas de `content/`: es lo que el
    /// render sustituye al servir (`tasks.md` 1.8, change
    /// `self-contained-themes`).
    const MARCADOR: &str = r#"<link rel="stylesheet" href="/css/error.css" />"#;

    /// La página tal como se **sirve**: el marcador sustituido por un `<style>`
    /// con la hoja. Es el contrato del HTML autocontenido, y estas pruebas lo
    /// fijan sin tocar la implementación.
    fn servida_autocontenida(pagina: &str, hoja: &str) -> Vec<u8> {
        pagina
            .replace(MARCADOR, &format!("<style>\n{hoja}\n</style>"))
            .into_bytes()
    }

    /// ¿`pajar` contiene `aguja` como subsecuencia de bytes contigua? Hace falta
    /// para comprobar la salida cuando esta lleva bytes que **no** son UTF-8
    /// válido, donde `String::from_utf8` no sirve.
    fn contiene(pajar: &[u8], aguja: &[u8]) -> bool {
        pajar.windows(aguja.len()).any(|ventana| ventana == aguja)
    }

    /// La página del operador de `tests/fixtures/content/404.html` tal como se
    /// **sirve** para `code`: su plantilla rellenada. El fixture no lleva el
    /// marcador de la hoja, así que [`autocontenido`] no añade nada.
    fn servida_del_fixture(code: u16) -> String {
        let rellena = plantilla(FIXTURE_404_HTML.as_bytes(), code);
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
        crate::theme::default_theme()
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

    // --- Cuerpo JSON y XML (tasks.md 1.5) ----------------------------------

    #[test]
    fn el_cuerpo_json_lleva_las_cuatro_claves() {
        let body = render_body(
            404,
            404,
            Format::Json,
            &fixtures_content_dir(),
            tema_default(),
        )
        .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(json["error"], Value::Bool(true));
        assert_eq!(json["code"], Value::from(404));
        assert_eq!(json["message"], Value::from("Not Found"));
        assert!(json["description"].is_string());
        assert!(!json["description"].as_str().unwrap().is_empty());
    }

    #[test]
    fn el_cuerpo_xml_lleva_el_elemento_code() {
        let body = render_body(
            503,
            404,
            Format::Xml,
            &fixtures_content_dir(),
            tema_default(),
        )
        .unwrap();
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
        let body = render_body(
            503,
            404,
            Format::Text,
            &fixtures_content_dir(),
            tema_default(),
        )
        .unwrap();
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
        // El fichero del operador (`tests/fixtures/content/404.html`) es una
        // plantilla: su cuerpo servido son sus marcadores rellenados con el
        // código. La prueba fija las dos cosas: que sale **de ese fichero** —y no
        // de la plantilla embebida— y que la plantilla lo rellena.
        let body = render_body(
            404,
            404,
            Format::Html,
            &fixtures_content_dir(),
            tema_default(),
        )
        .unwrap();
        let html = String::from_utf8(body).unwrap();

        assert_eq!(html.trim(), servida_del_fixture(404).trim());
        // Anclajes **independientes**: el cuerpo servido nombra su propio código
        // y no deja ningún marcador. El `assert_ne!` que había aquí (comparar
        // contra la constante cruda `EMBEDDED_404_HTML`) se cumplía **siempre**
        // —el HTML servido va relleno y la constante lleva los marcadores—, así
        // que no distinguía que la página saliera del fixture y no de la
        // plantilla embebida.
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
    fn el_cuerpo_html_cae_al_fichero_del_codigo_por_defecto() {
        // No hay `418.html`; el cuerpo sale del `404.html` del operador —su
        // plantilla—, rellenado con **418**.
        let body = render_body(
            418,
            404,
            Format::Html,
            &fixtures_content_dir(),
            tema_default(),
        )
        .unwrap();
        let html = String::from_utf8(body).unwrap();

        assert_eq!(html.trim(), servida_del_fixture(418).trim());
        assert!(
            html.contains("418"),
            "la plantilla del operador se rellena con el código pedido (`418`): {html}"
        );
    }

    #[test]
    fn el_cuerpo_no_depende_del_formato_para_el_codigo() {
        // Ortogonalidad: el mismo `code` produce los cuatro cuerpos.
        for format in [Format::Html, Format::Json, Format::Xml, Format::Text] {
            let body =
                render_body(503, 404, format, &fixtures_content_dir(), tema_default()).unwrap();
            assert!(!body.is_empty(), "cuerpo vacío para {format:?}");
        }
    }

    // --- Cabeceras de acompañamiento (tasks.md 1.7) ------------------------

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
        // Auditoría I-2: el respaldo de texto es lo que se sirve cuando la
        // página del operador **existe y no se puede leer** (fallo de E/S); la
        // **falta** de fichero ya no llega aquí —la cubre la plantilla embebida—.
        // Aquí se fija lo suyo: el estado sigue siendo el código resuelto (nunca
        // `500`) y la página no pierde `X-Robots-Tag`.
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
        // Con el valor por defecto (`120`, el que `render_page` recibiría de una
        // `Config` sin `RETRY_AFTER`), el respaldo conserva la misma cabecera.
        assert_eq!(
            header(&render_text_fallback(503, 120), "Retry-After"),
            Some("120")
        );
        assert_eq!(header(&render_text_fallback(404, 120), "Retry-After"), None);
    }

    // --- `RETRY_AFTER` configurado (change `retry-after-configurable`,
    //     tasks.md 1.5-1.7) ------------------------------------------------
    //
    // `render_page` recibe ya la `Config`, así que el valor entra por ahí sin
    // cambiar su firma. La prueba lo fija por la vía inyectable
    // (`Config::from_env_iter`).
    //
    // El respaldo de texto (`render_text_fallback`) no pudo probarse en RED: su
    // firma ganó un `retry_after: u32` en GREEN, así que entonces no compilaba
    // con la nueva ni probaba nada con la vieja. Se cubre aquí abajo, con el
    // parámetro ya en su sitio.

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
        // Los dos códigos que llevan la cabecera comparten el valor: la variable
        // es una sola, así que dos respuestas no pueden desincronizarse.
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
        // Decisión D2 de `design.md`: `0` es un `delay-seconds` legal de
        // RFC 9110 («reintenta ya») y tiene que salir tal cual. Con un
        // `if secs > 0` en `companion_headers`, el árbol seguiría verde y la
        // cabecera desaparecería en silencio.
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
        // Guarda, no RED: hoy ya pasa, porque `404` no es un código que lleve la
        // cabecera. Fija que `RETRY_AFTER` configura **el valor** de los códigos
        // que la llevan, no **qué** códigos la llevan.
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
        // La mitad que el RED no pudo fijar (tasks.md 1.5): cuando falta el HTML
        // del `503` —y el del código por defecto— se responde el respaldo de
        // texto, y ese camino también tiene que llevar el valor configurado, no
        // el `120` del defecto. Aquí el `503` sale del mismo valor que `render`
        // recibe de la `Config` en producción (`config.retry_after`).
        let page = render_text_fallback(503, 30);

        assert_eq!(
            header(&page, "Retry-After"),
            Some("30"),
            "el respaldo del `503` debe llevar el valor configurado"
        );
        assert_eq!(page.status, StatusCode::SERVICE_UNAVAILABLE);
    }

    // --- Plantilla embebida sin ficheros de datos (change `error-page-template`,
    //     tasks.md 2.3-2.5) --------------------------------------------------
    //
    // La fase GREEN embebe **una** plantilla (y su hoja) con
    // `include_str!`/`include_bytes!`, así que un `CONTENT_DIR` que no existe ya
    // no deja la cascada sin página: la del disco se agota **entera**
    // (`CONTENT_DIR/<código>.html` y `CONTENT_DIR/<defecto>.html`) y entonces
    // entra la plantilla del proyecto, rellenada con el código resuelto. Estas
    // pruebas fijan el orden y el caso límite que el respaldo de texto conserva
    // (ahora reservado al fallo de E/S).

    #[test]
    fn sin_directorio_el_cuerpo_html_sale_de_la_plantilla_embebida() {
        // Sin directorio de contenido, el cuerpo sale de la plantilla embebida
        // rellenada con `503`. Desde `self-contained-themes` la página se sirve
        // **autocontenida**: lleva la hoja del tema en un `<style>` donde estaba
        // el marcador, así que el cuerpo es la plantilla con el marcador
        // sustituido.
        let body = render_body(503, 404, Format::Html, &sin_contenido_dir(), tema_default())
            .expect("la plantilla embebida está siempre");

        let rellenada = plantilla(EMBEDDED_404_HTML.as_bytes(), 503);
        let rellenada = String::from_utf8(rellenada).expect("la plantilla rellenada es UTF-8");

        assert_eq!(
            body,
            servida_autocontenida(&rellenada, EMBEDDED_CSS),
            "sin directorio, el cuerpo tiene que ser la plantilla embebida rellenada con `503`, \
             autocontenida con la hoja `default`"
        );
        assert!(
            String::from_utf8(body).unwrap().contains("503"),
            "la plantilla rellenada tiene que nombrar su código"
        );
    }

    #[test]
    fn sin_directorio_un_codigo_sin_pagina_propia_recibe_la_plantilla_con_su_codigo() {
        // No hay página propia del `418` en ninguna capa, así que el cuerpo es
        // la plantilla embebida rellenada con **418** —no con el defecto—, y
        // `render_page` conserva el estado `418`.
        let body = render_body(418, 404, Format::Html, &sin_contenido_dir(), tema_default())
            .expect("la plantilla embebida está siempre");

        let rellenada = plantilla(EMBEDDED_404_HTML.as_bytes(), 418);
        let rellenada = String::from_utf8(rellenada).expect("la plantilla rellenada es UTF-8");

        // Anclajes **independientes** de `plantilla`: el cuerpo servido nombra su
        // propio código y no deja ningún marcador. Sin ellos esta prueba era
        // vacua —comparaba el cuerpo con una salida calculada por la misma
        // `plantilla` de producción, así que una sustitución que dejara de
        // sustituir la habría dejado verde—.
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
            body,
            servida_autocontenida(&rellenada, EMBEDDED_CSS),
            "un código sin fichero propio recibe la plantilla rellenada con **su** código (`418`)"
        );

        let page = render_page(&sin_contenido_config(), 418, Format::Html, tema_default())
            .expect("la plantilla embebida está siempre");
        assert_eq!(
            page.status,
            StatusCode::IM_A_TEAPOT,
            "el estado sigue siendo el código resuelto (`418`)"
        );
        assert_eq!(
            page.body,
            servida_autocontenida(&rellenada, EMBEDDED_CSS),
            "el cuerpo es la plantilla rellenada con `418`, autocontenida"
        );
    }

    #[test]
    fn la_cascada_ya_no_se_queda_sin_pagina_y_el_unico_fallo_es_la_ilegibilidad() {
        // Ausencia de fichero: nunca falta página —la plantilla cubre cualquier
        // código—. Con un código que no tiene fichero en ninguna capa (`599` con
        // defecto `418`), el resultado es la plantilla rellenada con `599`, no un
        // error.
        let body = render_body(599, 418, Format::Html, &sin_contenido_dir(), tema_default())
            .expect("la plantilla cubre cualquier código: nunca falta la página");
        let texto = String::from_utf8(body).expect("la página servida es UTF-8");
        assert!(
            texto.contains("599"),
            "un `599` sin fichero propio recibe la plantilla rellenada con `599`: {texto}"
        );

        // El único fallo posible es de **E/S**: una página del operador que existe
        // y no se puede leer (`tests/fixtures/pagina-ilegible/404.html` es un
        // directorio). Sigue siendo `Io`, y el respaldo de texto lo cubre.
        let ilegible = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pagina-ilegible");
        match render_body(404, 404, Format::Html, &ilegible, tema_default()) {
            Err(RenderError::Io(_)) => {}
            other => panic!("una página ilegible tiene que ser `Io`: {other:?}"),
        }
    }

    // --- Sustitución del marcador (change `self-contained-themes`,
    //     tasks.md 1.1) ------------------------------------------------------
    //
    // El marcador se sustituye **al servir**, no en el fichero: las páginas de
    // `content/` lo conservan y el render lo cambia por un `<style>` con la hoja
    // del tema. Estas pruebas fijan las tres reglas en aislado —así el fallo
    // señala la regla, no la cascada—, y el recorrido completo desde el router
    // vive en `src/main.rs`.

    /// La página servida a partir de un cuerpo dado, con el tema `default` y un
    /// `CONTENT_DIR` inexistente (la hoja sale de la copia embebida).
    fn sirve(html: &str) -> String {
        let salida = autocontenido(html.as_bytes(), &sin_contenido_dir(), tema_default());

        String::from_utf8(salida).expect("la página servida de una entrada UTF-8 es UTF-8")
    }

    #[test]
    fn el_marcador_se_sustituye_por_un_style_con_la_hoja() {
        let salida = sirve(&format!(
            "<html><head>{MARCADOR}</head><body>c404</body></html>"
        ));

        assert!(
            salida.contains("<style>"),
            "el marcador tiene que salir como un bloque `<style>`, fue: {salida}"
        );
        assert!(
            salida.contains(EMBEDDED_CSS),
            "el bloque `<style>` tiene que llevar la hoja del tema, fue: {salida}"
        );
        assert!(
            !salida.contains("rel="),
            "el marcador no puede quedar en el cuerpo servido, fue: {salida}"
        );
        assert!(
            salida.contains("<body>c404</body>"),
            "solo se sustituye el marcador: el resto de la página queda igual, fue: {salida}"
        );
    }

    #[test]
    fn la_sustitucion_acepta_cualquier_orden_de_atributos_y_las_dos_comillas() {
        // Nombre de etiqueta y de atributo sin distinguir mayúsculas; `rel` sin
        // distinguir mayúsculas; `href` comparado **exacto**. Y valen las dos
        // comillas y cualquier orden.
        let casos = [
            r#"<link href="/css/error.css" rel="stylesheet" />"#,
            r#"<link REL="StyleSheet" HREF="/css/error.css">"#,
            r#"<link rel='stylesheet' href='/css/error.css'>"#,
            r#"<link rel = "stylesheet" href = "/css/error.css" />"#,
        ];

        for caso in casos {
            let salida = sirve(&format!("<head>{caso}</head>"));

            assert!(
                salida.contains("<style>") && salida.contains(EMBEDDED_CSS),
                "`{caso}` tiene que sustituirse por el `<style>` con la hoja, fue: {salida}"
            );
            assert!(
                !salida.contains("href=\"/css/error.css\"") && !salida.contains("HREF="),
                "`{caso}` no puede quedar en el cuerpo servido, fue: {salida}"
            );
        }
    }

    #[test]
    fn sin_marcador_la_pagina_sale_byte_a_byte() {
        // Una página propia del operador, ya autocontenida, se sirve **tal
        // cual**: sin marcador la sustitución no lee ninguna hoja ni toca un
        // byte. Es lo que mantiene válidos los fixtures.
        let pagina = "<!doctype html>\n<html><head><title>x</title></head><body><h1>404</h1></body></html>\n";

        assert_eq!(
            sirve(pagina).as_bytes(),
            pagina.as_bytes(),
            "sin marcador el HTML se devuelve byte a byte"
        );
    }

    #[test]
    fn un_enlace_que_no_es_el_marcador_se_deja_en_paz() {
        let ajeno = r#"<link rel="stylesheet" href="/css/otra.css">"#;
        let sin_rel = r#"<link href="/css/error.css">"#;
        let salida = sirve(&format!("<head>{ajeno}{sin_rel}{MARCADOR}</head>"));

        assert!(
            salida.contains(ajeno),
            "un `<link>` que no es el marcador tiene que seguir en el cuerpo, fue: {salida}"
        );
        assert!(
            salida.contains(sin_rel),
            "un `<link>` sin `rel=\"stylesheet\"` tampoco es el marcador, fue: {salida}"
        );
        assert!(
            salida.contains("<style>") && !salida.contains(MARCADOR),
            "y el marcador, en medio de los dos, se sustituye, fue: {salida}"
        );
    }

    #[test]
    fn una_pagina_no_utf8_con_el_marcador_se_sirve_autocontenida() {
        // El escáner trabaja **por bytes**: una página del operador en otra
        // codificación con el marcador tiene que incrustar la hoja igual. Antes
        // se decodificaba a `str` y, ante bytes no-UTF-8, se devolvía el cuerpo
        // tal cual **en silencio** —con el enlace roto y sin estilos, justo el
        // fallo que el change arregla—. Los bytes `\xE9\xF1` no son UTF-8 válido
        // y tienen que sobrevivir intactos en la salida.
        let pagina: &[u8] =
            b"<!doctype html>\n<!-- \xE9\xF1 -->\n<html><head><link rel=\"stylesheet\" href=\"/css/error.css\" /></head></html>";

        let salida = autocontenido(pagina, &sin_contenido_dir(), tema_default());

        assert!(
            contiene(&salida, b"<style>"),
            "el marcador tiene que salir como un bloque `<style>`"
        );
        assert!(
            contiene(&salida, EMBEDDED_CSS.as_bytes()),
            "el bloque `<style>` tiene que llevar la hoja del tema"
        );
        assert!(
            !contiene(&salida, MARCADOR.as_bytes()),
            "el marcador no puede quedar en el cuerpo servido"
        );
        assert!(
            contiene(&salida, b"\xE9\xF1"),
            "los bytes no-UTF-8 (`\\xE9\\xF1`) tienen que seguir intactos en la salida"
        );
    }

    // --- Plantilla del cuerpo HTML (change `error-page-template`,
    //     tasks.md de la fase RED) -------------------------------------------
    //
    // El cuerpo HTML deja de ser once ficheros con los valores a mano y pasa a
    // ser **una** plantilla rellenada al servir: `{{code}}`, `{{reason}}` y
    // `{{description}}` los sustituye el render por el código resuelto y por la
    // razón canónica y la descripción. Estas pruebas fijan la costura en
    // aislado; en RED fallan por aserción (la costura todavía devuelve el cuerpo
    // tal cual), y la fase GREEN es la que rellena.

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
    fn escape_html_escapa_lo_que_se_inserta() {
        assert_eq!(
            escape_html("a & b < c > d"),
            "a &amp; b &lt; c &gt; d",
            "el `&`, el `<` y el `>` tienen que salir escapados: los inserta el servidor"
        );
    }

    #[test]
    fn la_sustitucion_escapa_lo_que_inserta() {
        // La red que faltaba para el `SHALL` del escape (auditoría I2): la tabla
        // de `resolve` no tiene hoy ningún `&`, `<` ni `>` —medido—, así que
        // ninguna prueba con un código real demuestra que `rellena` escape lo que
        // inserta. La costura se prueba con un valor **adversario**: es lo único
        // que demuestra que la sustitución **usa** el escape que tiene al lado.
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
        // El caso que da nombre al change: hoy el `451` recibe el fichero del
        // `404`, y el cuerpo miente sobre el estado.
        let pagina = render_page(&fixtures_config(), 451, Format::Html, tema_default())
            .expect("la plantilla siempre está");
        let cuerpo = String::from_utf8(pagina.body).expect("el HTML es UTF-8");

        assert!(
            cuerpo.contains("451"),
            "el cuerpo tiene que nombrar su propio código: {cuerpo}"
        );
        assert!(
            !cuerpo.contains("404"),
            "el cuerpo no puede nombrar el `404`: es el defecto que este change corrige: {cuerpo}"
        );
    }
}
