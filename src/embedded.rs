//! Copia **embebida** de las páginas de error y de su hoja de estilos.
//!
//! Un binario instalado no lleva ficheros de datos: `cargo install errserve`
//! deja un ejecutable y nada más, y `CONTENT_DIR` apunta por defecto a un
//! `content` que no existe. Este módulo guarda los bytes de `content/` **dentro
//! del binario**, con `include_str!`/`include_bytes!` (decisión D3), de modo que
//! un binario instalado sirva las páginas de verdad y su hoja.
//!
//! ---------------------------------------------------------------------------
//! API que asume la fase RED (la fase GREEN debe implementarla tal cual):
//!
//! ```ignore
//! /// Los once códigos que tienen página embebida.
//! pub const PAGE_CODES: [u16; 11];
//!
//! /// Página HTML embebida de un código, si la hay.
//! pub fn page(code: u16) -> Option<&'static str>;
//!
//! /// Asset embebido por path **exacto** (`/css/error.css`), con su `Content-Type`.
//! pub fn asset(path: &str) -> Option<(&'static [u8], &'static str)>;
//! ```
//!
//! La fase GREEN implementa esa API tal cual, sin cambiar sus firmas. La
//! consulta del asset compara el path **de forma exacta** —sin prefijos, sin
//! normalizar barras finales—, porque el respaldo de `ServeDir` también recibe
//! un directorio (`InvalidFilename`) y un `200` de más ya no lo corrige nadie.
//!
//! La prueba de paridad (tasks.md 1.15, decisión D5) vive **aquí**, en los
//! `#[cfg(test)]` del módulo que guarda la tabla, y **no** en `tests/`: el crate
//! no tiene *lib target*, así que una prueba de integración no ve las constantes
//! del binario —para verlas habría que añadir un `lib` que cambiaría la forma
//! del crate para todos—, y leer `content/` por `CARGO_MANIFEST_DIR` es lo que ya
//! hace `tests/content_pages.rs`.

#[cfg(test)]
mod tests {
    use super::{PAGE_CODES, asset, page};
    use std::path::PathBuf;

    /// Directorio de contenido del repositorio: la copia que la tabla embebida
    /// tiene que igualar byte a byte.
    fn content_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("content")
    }

    /// Los once códigos, escritos **literalmente** para no depender de
    /// `PAGE_CODES` al comprobar que `PAGE_CODES` es el conjunto correcto.
    const CODIGOS: [u16; 11] = [400, 401, 403, 404, 405, 408, 429, 500, 502, 503, 504];

    #[test]
    fn la_tabla_declara_los_once_codigos_con_pagina() {
        assert_eq!(
            PAGE_CODES.len(),
            CODIGOS.len(),
            "`PAGE_CODES` tiene que tener los once códigos con página"
        );

        for code in CODIGOS {
            assert!(
                page(code).is_some(),
                "`page({code})` tiene que existir: es uno de los once códigos con página"
            );
        }
    }

    #[test]
    fn cada_pagina_embebida_es_byte_a_byte_la_de_content() {
        // D5, sentido 1: los bytes embebidos igualan la lectura de
        // `content/<código>.html`. Detecta una copia embebida obsoleta o
        // distinta —alguien retoca `content/503.html` y quien instaló el crate
        // sigue viendo la vieja—.
        for code in PAGE_CODES {
            let embebida = page(code)
                .unwrap_or_else(|| panic!("`page({code})` debe existir: está en `PAGE_CODES`"));
            let ruta = content_dir().join(format!("{code}.html"));
            let del_disco = std::fs::read(&ruta)
                .unwrap_or_else(|error| panic!("no se pudo leer {}: {error}", ruta.display()));

            assert_eq!(
                embebida.as_bytes(),
                del_disco.as_slice(),
                "la página embebida del `{code}` no coincide byte a byte con {}",
                ruta.display()
            );
        }
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
    fn la_lista_de_codigos_y_el_directorio_dicen_lo_mismo() {
        // D5, en los **dos** sentidos. Recorre `content/` con `read_dir` (no
        // recursivo: la hoja vive en `css/`, no cuenta) y lo compara con
        // `PAGE_CODES`, ordenados los dos.
        let entradas = std::fs::read_dir(content_dir())
            .expect("el directorio `content/` tiene que poder listarse");

        let mut codigos_del_directorio: Vec<u16> = entradas
            .filter_map(Result::ok)
            .map(|entrada| entrada.path())
            .filter(|ruta| ruta.extension().is_some_and(|ext| ext == "html"))
            .filter_map(|ruta| ruta.file_stem()?.to_str()?.parse::<u16>().ok())
            .collect();
        codigos_del_directorio.sort_unstable();

        let mut codigos_de_la_tabla: Vec<u16> = PAGE_CODES.to_vec();
        codigos_de_la_tabla.sort_unstable();

        assert_eq!(
            codigos_de_la_tabla, codigos_del_directorio,
            "todo `*.html` de `content/` tiene que tener entrada en la tabla, y toda entrada \
             de `PAGE_CODES` su fichero en `content/`"
        );

        // Sentido 2, explícito: cada entrada de `PAGE_CODES` tiene su fichero.
        for code in PAGE_CODES {
            let ruta = content_dir().join(format!("{code}.html"));
            assert!(
                ruta.is_file(),
                "`PAGE_CODES` incluye el `{code}`, pero no existe {}",
                ruta.display()
            );
        }
    }
}
