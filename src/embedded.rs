//! Copia **embebida** de la página de error del proyecto y de su hoja de estilos.
//!
//! Un binario instalado no lleva ficheros de datos: `cargo install errserve`
//! deja un ejecutable y nada más, y `CONTENT_DIR` apunta por defecto a un
//! `content` que no existe. Este módulo guarda los bytes de `content/` **dentro
//! del binario**, con `include_str!`/`include_bytes!` (decisión D3), de modo que
//! un binario instalado sirva la página de verdad y su hoja.
//!
//! Es **una** página —una plantilla con los marcadores `{{code}}`, `{{reason}}`
//! y `{{description}}`—, no once ficheros por código: la misma página sirve para
//! cualquier código de `400..=599`, rellenada al servir (`src/render.rs`).
//!
//! ---------------------------------------------------------------------------
//! API de la fase GREEN:
//!
//! ```ignore
//! /// La página HTML embebida del proyecto: la plantilla de `content/404.html`.
//! pub static PAGE: &'static str;
//!
//! /// Asset embebido por path **exacto** (`/css/error.css`), con su `Content-Type`.
//! pub fn asset(path: &str) -> Option<(&'static [u8], &'static str)>;
//! ```
//!
//! La consulta del asset compara el path **de forma exacta** —sin prefijos, sin
//! normalizar barras finales—, porque el respaldo de `ServeDir` recibe **todo
//! lo que no está en el disco** —`serve_dir/future.rs`, línea 108, manda al
//! respaldo lo mismo un `FileNotFound` que un `InvalidFilename`— y un `200` de
//! más ya no lo corrige nadie: con `fallback` nadie fuerza el estado.
//!
//! La prueba de paridad (decisión D5) vive **aquí**, en los `#[cfg(test)]` del
//! módulo que guarda la página, y **no** en `tests/`: el crate no tiene *lib
//! target*, así que una prueba de integración no ve las constantes del binario
//! —para verlas habría que añadir un `lib` que cambiaría la forma del crate para
//! todos—, y leer `content/` por `CARGO_MANIFEST_DIR` es lo que ya hace
//! `tests/content_pages.rs`.

/// La página HTML embebida del proyecto: los bytes de `content/404.html`.
///
/// Se llama `404.html` porque es la página del **código por defecto**
/// (`DEFAULT_ERROR_PAGE`, de fábrica `404`) y **conserva ese nombre**. El motivo
/// está **medido**: un fichero con nombre de código lo intercepta la cascada del
/// código **antes** de servirse, así que `GET /404.html` responde `404` con la
/// página y nunca `200`, mientras que un nombre neutro —`error.html`— se serviría
/// como asset **con los marcadores a la vista**. El precio, escrito: el fichero
/// se llama `404.html` y sirve también el `451`.
///
/// Es la **tercera** capa de la cascada de [`crate::render`]: se consulta solo
/// cuando el directorio del operador no tenía ni el fichero del código ni el del
/// código por defecto. Como cubre cualquier código de `400..=599`, la cascada no
/// puede quedarse sin página.
///
/// `include_str!` exige que el fichero exista al compilar (D6), de modo que
/// borrar o renombrar `content/404.html` deja de degradarse en silencio al
/// respaldo de texto y pasa a ser un error de compilación; el reverso —una página
/// nueva del directorio que nadie embebe— lo vigila la prueba de paridad (D5).
pub static PAGE: &str = include_str!("../content/404.html");

/// Los bytes de `content/css/error.css`, como `static` **de tipo array**.
///
/// Un `static` tiene **una sola dirección**, garantizada por el lenguaje, y de
/// este se toman las dos referencias —la del asset y la del tema `default`— al
/// **mismo** array, así que apuntan a la misma dirección. Es lo que hace que
/// `std::ptr::eq` sobre [`DEFAULT_SHEET`] sea una comprobación **estable**.
///
/// El array intermedio **no es decorativo**: `static X: &[u8] =
/// include_bytes!(…)` no basta, porque al leer esa `&[u8]` en el inicializador
/// de otro `static` el compilador **inlinea** el `include_bytes!` de nuevo y
/// cada expansión crea un `static` anónimo propio. Con varias *codegen units*,
/// el enlazador no los funde y las direcciones difieren aunque los bytes sean
/// iguales (medido con `cargo test`, que compila el binario en varias CGU). Un
/// `static` de tipo array sí tiene una única dirección y `&DEFAULT_SHEET_BYTES`
/// la referencia en todos los sitios.
///
/// El fichero tiene que existir al compilar (`include_bytes!` lo exige, D6), y
/// `content/` viaja dentro del `.crate`, así que un binario instalado la sirve.
pub static DEFAULT_SHEET_BYTES: [u8; include_bytes!("../content/css/error.css").len()] =
    *include_bytes!("../content/css/error.css");

/// La hoja de estilos de `content/css/error.css`, embebida **una sola vez**.
///
/// Es la **única** copia de esos bytes en el binario, y la comparten dos
/// consumidores que no pueden divergir: el asset `/css/error.css`
/// ([`ASSETS`]) y el tema `default` ([`crate::theme::THEMES`]). Si cada uno
/// llevara su propio `include_bytes!`, el binario guardaría los mismos bytes
/// dos veces y, peor, podrían separarse sin que nadie lo notase.
///
/// Apunta a [`DEFAULT_SHEET_BYTES`], un `static` de tipo array con **una sola
/// dirección**: las dos lecturas de este valor de puntero —la de [`ASSETS`] y la
/// de [`crate::theme::THEMES`]— dan la **misma** dirección, y la identidad se
/// comprueba con `std::ptr::eq` (ver la documentación de
/// [`DEFAULT_SHEET_BYTES`]). Con una `const`, o incluso con un `static` de tipo
/// `&[u8]` que lleve el `include_bytes!` directamente, la identidad **no** es
/// estable.
pub static DEFAULT_SHEET: &[u8] = &DEFAULT_SHEET_BYTES;

/// Assets embebidos, por su path **exacto**, con su `Content-Type`.
///
/// El path se compara tal cual —sin prefijos, sin normalizar barras finales—
/// porque el respaldo de `ServeDir` recibe **todo lo que no está en el disco**,
/// no solo un `FileNotFound`: `serve_dir/future.rs`, línea 108, hace caer al
/// respaldo lo mismo `FileNotFound` que `InvalidFilename` —que no es «un
/// directorio» sino el byte NUL, `serve_dir/open_file.rs`, líneas 117-118 y
/// 153-154, y un directorio sale como `FileNotFound`, `open_file.rs`, líneas
/// 290-291, con `append_index_html_on_directories(false)`— y, como el `fallback`
/// no fuerza el estado de la respuesta, un `200` de más
/// no lo corrige nadie (D2).
///
/// La hoja es [`DEFAULT_SHEET`]: el asset y el tema `default` son la **misma**
/// constante, no dos copias de los mismos bytes —y como [`DEFAULT_SHEET`] es un
/// `static`, comparten literalmente el mismo puntero (ver su documentación)—.
static ASSETS: [(&str, &[u8], &str); 1] = [("/css/error.css", DEFAULT_SHEET, "text/css")];

/// Asset embebido por path **exacto** (`/css/error.css`), con su `Content-Type`.
pub fn asset(path: &str) -> Option<(&'static [u8], &'static str)> {
    ASSETS
        .iter()
        .find(|(candidate, _, _)| *candidate == path)
        .map(|(_, bytes, content_type)| (*bytes, *content_type))
}

#[cfg(test)]
mod tests {
    use super::{PAGE, asset};
    use std::path::PathBuf;

    /// Directorio de contenido del repositorio: la copia que la página embebida
    /// tiene que igualar byte a byte.
    fn content_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("content")
    }

    /// Los `*.html` de `content/`, ordenados (no recursivo: la hoja vive en
    /// `css/` y no cuenta).
    fn paginas_del_directorio() -> Vec<PathBuf> {
        let entradas = std::fs::read_dir(content_dir())
            .expect("el directorio `content/` tiene que poder listarse");

        let mut paginas: Vec<PathBuf> = entradas
            .filter_map(Result::ok)
            .map(|entrada| entrada.path())
            .filter(|ruta| ruta.extension().is_some_and(|ext| ext == "html"))
            .collect();
        paginas.sort();

        paginas
    }

    #[test]
    fn la_pagina_embebida_es_byte_a_byte_la_de_content() {
        // D5, sentido 1: los bytes embebidos igualan la lectura de
        // `content/404.html`. Detecta una copia embebida obsoleta o distinta
        // —alguien retoca la página y quien instaló el crate sigue viendo la
        // vieja—.
        let ruta = content_dir().join("404.html");
        let del_disco = std::fs::read(&ruta)
            .unwrap_or_else(|error| panic!("no se pudo leer {}: {error}", ruta.display()));

        assert_eq!(
            PAGE.as_bytes(),
            del_disco.as_slice(),
            "la página embebida no coincide byte a byte con {}",
            ruta.display()
        );
    }

    #[test]
    fn la_hoja_embebida_es_byte_a_byte_la_de_content() {
        // D5/D2: la hoja también se embebe, y con su `Content-Type`.
        let (bytes, content_type) = asset("/css/error.css")
            .expect("la hoja `/css/error.css` tiene que estar en la tabla de assets");
        assert_eq!(content_type, "text/css", "Content-Type de la hoja embebida");

        let ruta = content_dir().join("css/error.css");
        let del_disco = std::fs::read(&ruta)
            .unwrap_or_else(|error| panic!("no se pudo leer {}: {error}", ruta.display()));

        assert_eq!(
            bytes,
            del_disco.as_slice(),
            "la hoja embebida no coincide byte a byte con {}",
            ruta.display()
        );
    }

    #[test]
    fn la_plantilla_embebida_lleva_los_tres_marcadores() {
        // La página embebida es la **plantilla** del proyecto: tiene que llevar
        // los tres marcadores que el render rellena al servir. Sin ellos, la
        // página embebida diría un código fijo —el `404`— para cualquier código.
        for marcador in ["{{code}}", "{{reason}}", "{{description}}"] {
            assert!(
                PAGE.contains(marcador),
                "la plantilla embebida tiene que llevar `{marcador}`: {PAGE}"
            );
        }
    }

    #[test]
    fn el_directorio_entrega_una_sola_pagina_y_es_la_embebida() {
        // D5, en los **dos** sentidos: `content/` tiene que entregar exactamente
        // el fichero que el binario embebe (`404.html`), y ningún otro `*.html`.
        // Una página nueva del directorio que nadie embebe —o el fichero
        // embebido renombrado— rompe esta prueba.
        assert_eq!(
            paginas_del_directorio(),
            vec![content_dir().join("404.html")],
            "`content/` tiene que entregar **una** página, la que el binario embebe (`404.html`)"
        );
    }
}
