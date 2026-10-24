//! Normalización del path de la petición **antes** de clasificarla.
//!
//! ---------------------------------------------------------------------------
//! Por qué existe este módulo (y no se clasifica con `request.uri().path()`):
//!
//! El servidor de ficheros (`ServeDir`) percent-decodifica el path y pliega los
//! componentes `.` —y rechaza los `..`— antes de tocar el disco. Si el
//! clasificador mira el path **crudo**, ambos ven cosas distintas y una página
//! de error se sirve como si fuera un asset, con `200`:
//!
//! ```text
//! GET /./404.html    el crudo no aporta código -> ServeDir ve `404.html` -> 200
//! GET /404%2Ehtml    el crudo no aporta código -> ServeDir ve `404.html` -> 200
//! GET /40%34.html    el crudo no aporta código -> ServeDir ve `404.html` -> 200
//! ```
//!
//! Es exactamente el fallo que este proyecto corrige, así que el path se
//! normaliza **una sola vez**, al entrar al middleware: la misma cadena
//! normalizada clasifica (`is_error_page_path`) y se reescribe en la URI que
//! recibe `ServeDir`. Así clasificador y servidor de ficheros no pueden
//! discrepar.
//!
//! El módulo usa `percent-encoding` (ya presente en el árbol de forma
//! transitiva vía `tower-http`) en lugar de un decodificador propio: la
//! diferencia entre decodificar bien y decodificar casi bien es una bypass de
//! página de error, y la caja está probada y es diminuta.

use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, percent_decode, utf8_percent_encode};

/// Caracteres que se percent-encodan al reescribir la URI: todo menos los
/// caracteres sin reserva (`A-Z a-z 0-9 - . _ ~`) y el separador `/`.
///
/// Es deliberadamente conservador. Lo que importa es que **`%` entra en el
/// conjunto**: el path reescrito no vuelve a contener ningún percent-encoding,
/// así que el único decode que queda es el de `ServeDir`, que recupera
/// exactamente [`NormalizedPath::as_str`]. Sin esto, un `%25` (el `%` literal)
/// se decodificaría dos veces y volvería a abrirse la bypass
/// (`/404%252Ehtml` -> `/404%2Ehtml` -> `/404.html`).
///
/// De regalo, encoding todo lo demás (incluidos `?`, `#`, espacio y los bytes
/// no ASCII) garantiza que el path resultante es un `PathAndQuery` válido.
const URI_PATH_ENCODE_SET: AsciiSet = NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~')
    .remove(b'/');

/// Path de la petición ya normalizado: percent-decodificado y con los
/// componentes `.` y `..` plegados.
///
/// Se construye con [`NormalizedPath::new`] y es la **única** representación
/// del path que ven el clasificador de páginas de error y el servidor de
/// ficheros.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedPath {
    text: String,
}

impl NormalizedPath {
    /// Normaliza el path crudo de la petición.
    ///
    /// 1. Percent-decode (`%2E` -> `.`, `%2F` -> `/`, `%34` -> `4`).
    /// 2. Pliega los componentes: `.` y los segmentos vacíos se descartan, `..`
    ///    hace *pop* y **nunca** sale de la raíz (`/../../etc/passwd` ->
    ///    `/etc/passwd`).
    ///
    /// El resultado siempre empieza por `/` y nunca contiene `.` ni `..`, así
    /// que es seguro entregárselo a un servidor de ficheros que lo resuelva
    /// contra `CONTENT_DIR`.
    ///
    /// Bytes que no sean UTF-8 válido se sustituyen por `U+FFFD`. No hay riesgo
    /// de clasificar de menos lo que se sirve de más: los patrones que clasifican
    /// una página de error (`404`, `.html`…) son ASCII puro, y un byte sustituido
    /// nunca es ASCII, así que ningún path con pérdida puede coincidir con ellos
    /// (`ServeDir`, además, rechaza esos paths con un `404`).
    pub fn new(raw: &str) -> Self {
        let decoded = percent_decode(raw.as_bytes()).decode_utf8_lossy();

        Self {
            text: fold_components(&decoded),
        }
    }

    /// Path normalizado en texto. Es el que usan
    /// [`crate::resolve::resolve_code_with_source`] y
    /// [`crate::resolve::resolve_format`] para clasificar.
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// Path normalizado listo para la URI: percent-encoded, de modo que el
    /// *único* decode que quede (el de `ServeDir`) devuelva [`Self::as_str`]
    /// exactamente.
    pub fn to_uri_path(&self) -> String {
        utf8_percent_encode(&self.text, &URI_PATH_ENCODE_SET).to_string()
    }
}

/// Pliega los componentes de un path ya percent-decodificado.
///
/// El criterio imita a `std::path::Path::components` sobre un path absoluto:
/// los separadores repetidos y los segmentos vacíos no aportan nada, `.` se
/// descarta y `..` deshace el componente anterior —sin salir nunca de la raíz,
/// porque un `..` sobre la raíz se ignora—.
fn fold_components(decoded: &str) -> String {
    let mut segments: Vec<&str> = Vec::new();

    for segment in decoded.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                segments.pop();
            }
            segment => segments.push(segment),
        }
    }

    let mut path = String::with_capacity(decoded.len() + 1);
    for segment in segments {
        path.push('/');
        path.push_str(segment);
    }

    if path.is_empty() {
        path.push('/');
    }

    path
}

#[cfg(test)]
mod tests {
    use super::*;

    fn normalized(raw: &str) -> String {
        NormalizedPath::new(raw).as_str().to_string()
    }

    /// Decodifica una sola vez, como hace `ServeDir` antes de tocar el disco.
    fn decode_once(encoded: &str) -> String {
        percent_decode(encoded.as_bytes())
            .decode_utf8_lossy()
            .into_owned()
    }

    // --- Percent-decode (auditoría B-1) -----------------------------------

    #[test]
    fn un_path_limpio_no_cambia() {
        assert_eq!(normalized("/"), "/");
        assert_eq!(normalized("/404"), "/404");
        assert_eq!(normalized("/404.html"), "/404.html");
        assert_eq!(normalized("/css/404.css"), "/css/404.css");
    }

    #[test]
    fn el_percent_encoding_se_decodifica() {
        assert_eq!(normalized("/404%2Ehtml"), "/404.html");
        assert_eq!(normalized("/40%34.html"), "/404.html");
        assert_eq!(normalized("/404%2eJSON"), "/404.JSON");
    }

    #[test]
    fn un_slash_codificado_es_un_separador() {
        assert_eq!(normalized("/css%2f404.css"), "/css/404.css");
    }

    #[test]
    fn un_porcentaje_literal_no_se_decodifica_dos_veces() {
        // `/404%252Ehtml` decodifica a `/404%2Ehtml` (un `%` literal), que
        // **no** es `/404.html`: la segunda decodificación la haría `ServeDir`,
        // y por eso el path reescrito vuelve a escapar el `%`.
        let path = NormalizedPath::new("/404%252Ehtml");
        assert_eq!(path.as_str(), "/404%2Ehtml");
        assert!(
            path.to_uri_path().contains("%25"),
            "el `%` literal debe ir escapado en la URI: {}",
            path.to_uri_path()
        );
    }

    #[test]
    fn un_percent_encoding_incompleto_o_invalido_se_deja_tal_cual() {
        assert_eq!(normalized("/404%"), "/404%");
        assert_eq!(normalized("/404%zz"), "/404%zz");
    }

    // --- Plegado de componentes (auditoría B-1) ---------------------------

    #[test]
    fn un_componente_curdir_se_pliega() {
        assert_eq!(normalized("/./404.html"), "/404.html");
        assert_eq!(normalized("/css/./404.css"), "/css/404.css");
        assert_eq!(normalized("/././"), "/");
    }

    #[test]
    fn un_componente_parentdir_hace_pop_y_nunca_sale_de_la_raiz() {
        assert_eq!(normalized("/css/../404.html"), "/404.html");
        assert_eq!(normalized("/css/../../Cargo.toml"), "/Cargo.toml");
        assert_eq!(normalized("/../Cargo.toml"), "/Cargo.toml");
        assert_eq!(normalized("/..%2f..%2fetc%2fpasswd"), "/etc/passwd");
        assert_eq!(normalized("/../.."), "/");
    }

    #[test]
    fn los_separadores_repetidos_se_pliegan() {
        assert_eq!(normalized("//css//404.css"), "/css/404.css");
        assert_eq!(normalized("/"), "/");
    }

    #[test]
    fn un_path_sin_barra_inicial_queda_absoluto() {
        assert_eq!(normalized("404.html"), "/404.html");
    }

    // --- Reescritura de la URI --------------------------------------------

    #[test]
    fn la_uri_reescrita_decodifica_al_path_normalizado() {
        for raw in [
            "/",
            "/404",
            "/404.html",
            "/404%2Ehtml",
            "/40%34.html",
            "/./404.html",
            "/css/../css/404.css",
            "/404%252Ehtml",
            "/css/n%C3%A9.html",
        ] {
            let path = NormalizedPath::new(raw);

            assert_eq!(
                decode_once(&path.to_uri_path()),
                path.as_str(),
                "la URI de `{raw}` no vuelve al path normalizado"
            );
        }
    }

    #[test]
    fn la_uri_reescrita_es_ascii_y_valida() {
        for raw in ["/404%2Ehtml", "/css/n%C3%A9.html", "/a b/c?d"] {
            let uri_path = NormalizedPath::new(raw).to_uri_path();

            assert!(
                uri_path.is_ascii() && uri_path.starts_with('/') && !uri_path.contains('?'),
                "path de URI inválido para `{raw}`: {uri_path}"
            );
        }
    }

    #[test]
    fn los_bytes_no_utf8_no_colisionan_con_un_patron_ascii() {
        // `%FF` no es UTF-8 válido: se sustituye, y el resultado no puede
        // parecerse a `/404.html`.
        assert_ne!(normalized("/%FF04.html"), "/404.html");
        assert_eq!(normalized("/%FF04.html"), "/\u{fffd}04.html");
    }
}
