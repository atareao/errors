//! Copia **embebida** de las páginas de error del proyecto.
//!
//! Un binario instalado no lleva ficheros de datos: `cargo install errserve`
//! deja un ejecutable y nada más, y `CONTENT_DIR` apunta por defecto a un
//! `content` que no existe. Este módulo guarda los bytes de `content/` **dentro
//! del binario**, con `include_str!` (change `themes-as-pages`), de modo que un
//! binario instalado sirva las **siete** páginas de tema.
//!
//! Un **tema es una página** y el nombre del tema **es** la ruta del fichero: cada
//! constante de aquí es la copia embebida de `content/<nombre>.html`, la que sirve
//! [`crate::theme::Theme::page`] cuando el directorio del operador no tiene el
//! fichero. Son **siete** constantes, una por tema, y **no** once por código: el
//! código, la razón y la descripción son **marcadores de plantilla**
//! (`{{code}}`, `{{reason}}`, `{{description}}`) que [`crate::render`] rellena al
//! servir, así que una sola página por tema cubre cualquier código de `400..=599`.
//!
//! # Por qué el patrón de array intermedio desapareció
//!
//! El modelo anterior embebía la **hoja** `default` a través de un `static` de
//! tipo **array** ([`include_bytes!`] con un `&[u8]` intermedio) porque el asset
//! `/css/error.css` y el tema `default` tenían que apuntar a la **misma**
//! dirección y `std::ptr::eq` se usaba como comprobación. Con un `static X: &[u8]
//! = include_bytes!(…)` el compilador re-inlinea la macro en cada uso y, con
//! varias *codegen units*, las direcciones difieren aunque los bytes sean iguales.
//!
//! Aquí **no hace falta**: las páginas se comparan por **bytes** —la prueba de
//! paridad compara el contenido embebido con el del repositorio, no punteros—, así
//! que cada página es un `&'static str` directo con su `include_str!`. El array
//! intermedio era andamiaje de una comprobación de identidad que este change
//! retira.
//!
//! # La prueba de paridad
//!
//! Vive **aquí**, en los `#[cfg(test)]` del módulo que guarda las páginas, y **no**
//! en `tests/`: el crate no tiene *lib target*, así que una prueba de integración
//! no ve estas constantes —para verlas habría que añadir un `lib` que cambiaría la
//! forma del crate para todos—. Compara **página a página** —siete comparaciones,
//! no una— los bytes embebidos con el fichero del repositorio del que se embeben,
//! de modo que las dos copias no puedan divergir en silencio.

/// La página del tema `default`: los bytes de `content/default.html`.
///
/// Es la página del **tema por defecto** y el punto de personalización del
/// operador (`CONTENT_DIR/default.html`). `include_str!` exige que el fichero
/// exista al compilar: borrar o renombrar una página del proyecto es un **error
/// de compilación**, no un respaldo silencioso (decisión D6 de
/// `error-page-template`, conservada).
pub static DEFAULT_PAGE: &str = include_str!("../content/default.html");

/// La página del tema `minimal`: los bytes de `content/minimal.html`.
pub static MINIMAL_PAGE: &str = include_str!("../content/minimal.html");

/// La página del tema `terminal`: los bytes de `content/terminal.html`.
pub static TERMINAL_PAGE: &str = include_str!("../content/terminal.html");

/// La página del tema `paper`: los bytes de `content/paper.html`.
pub static PAPER_PAGE: &str = include_str!("../content/paper.html");

/// La página del tema `amber`: los bytes de `content/amber.html`.
pub static AMBER_PAGE: &str = include_str!("../content/amber.html");

/// La página del tema `compact`: los bytes de `content/compact.html`.
pub static COMPACT_PAGE: &str = include_str!("../content/compact.html");

/// La página del tema `matrix`: los bytes de `content/matrix.html`.
///
/// Es un **documento propio** —canvas de lluvia, glitch, terminal, minijuego y
/// toast—, no una piel de las seis sobrias: su marcado puede diferir y no tiene
/// por qué llevar `h2` ni `hr`.
pub static MATRIX_PAGE: &str = include_str!("../content/matrix.html");

#[cfg(test)]
mod tests {
    use super::{
        AMBER_PAGE, COMPACT_PAGE, DEFAULT_PAGE, MATRIX_PAGE, MINIMAL_PAGE, PAPER_PAGE,
        TERMINAL_PAGE,
    };
    use std::path::PathBuf;

    /// Las siete páginas embebidas con el nombre de su tema, en el orden del
    /// proyecto. El tallo del fichero de cada una es su nombre.
    const PAGINAS: [(&str, &str); 7] = [
        ("default", DEFAULT_PAGE),
        ("minimal", MINIMAL_PAGE),
        ("terminal", TERMINAL_PAGE),
        ("paper", PAPER_PAGE),
        ("amber", AMBER_PAGE),
        ("compact", COMPACT_PAGE),
        ("matrix", MATRIX_PAGE),
    ];

    /// Directorio de contenido del repositorio: la copia que cada página embebida
    /// tiene que igualar byte a byte.
    fn content_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("content")
    }

    /// Los `*.html` de `content/`, ordenados.
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
    fn las_paginas_embebidas_son_byte_a_byte_las_de_content() {
        // D5, sentido 1 y **página a página** (siete comparaciones, no una):
        // los bytes embebidos de cada tema igualan la lectura de
        // `content/<nombre>.html`. Detecta una copia embebida obsoleta o distinta
        // —alguien retoca una página de `content/` y quien instaló el crate sigue
        // viendo la vieja—.
        for (nombre, embebida) in PAGINAS {
            let ruta = content_dir().join(format!("{nombre}.html"));
            let del_disco = std::fs::read(&ruta)
                .unwrap_or_else(|error| panic!("no se pudo leer {}: {error}", ruta.display()));

            assert_eq!(
                embebida.as_bytes(),
                del_disco.as_slice(),
                "la página embebida del tema `{nombre}` no coincide byte a byte con {}",
                ruta.display()
            );
        }
    }

    #[test]
    fn cada_pagina_embebida_lleva_los_tres_marcadores() {
        // Cada página embebida es una **plantilla**: tiene que llevar los tres
        // marcadores que el render rellena al servir. Sin ellos, la página diría
        // un código fijo para cualquier código.
        for (nombre, embebida) in PAGINAS {
            for marcador in ["{{code}}", "{{reason}}", "{{description}}"] {
                assert!(
                    embebida.contains(marcador),
                    "la página embebida del tema `{nombre}` tiene que llevar `{marcador}`"
                );
            }
        }
    }

    #[test]
    fn el_directorio_entrega_exactamente_las_siete_paginas_de_tema() {
        // D5, en los **dos** sentidos: `content/` tiene que entregar exactamente
        // las siete páginas de tema (`default.html`, …, `matrix.html`), y ningún
        // otro `*.html`. Una página de más —el `404.html` del código por defecto,
        // que este change retira— o de menos rompe esta prueba.
        let mut esperadas: Vec<PathBuf> = PAGINAS
            .iter()
            .map(|(nombre, _)| content_dir().join(format!("{nombre}.html")))
            .collect();
        esperadas.sort();

        assert_eq!(
            paginas_del_directorio(),
            esperadas,
            "`content/` tiene que entregar las siete páginas de tema, ni una más ni una menos"
        );
    }
}
