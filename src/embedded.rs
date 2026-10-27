//! Copia **embebida** de las páginas de tema del proyecto.
//!
//! Un binario instalado no lleva ficheros de datos: `cargo install errserve`
//! deja un ejecutable y nada más, y `CONTENT_DIR` apunta por defecto a un
//! `content` que no existe. Este módulo guarda los bytes de `content/` **dentro
//! del binario**, con `include_str!`.
//!
//! # El conjunto se descubre al compilar
//!
//! El modelo anterior hardcodeaba cada tema con una constante. Ahora `build.rs`
//! lee el directorio `content/`, genera un array `EMBEBIDOS` con todos los
//! `*.html` que encuentre —ordenados por nombre— y este módulo lo incluye con
//! `include!`. Añadir un tema es dejar un `*.html` en `content/` y reconstruir.
//!
//! El array generado tiene la forma:
//!
//! ```ignore
//! pub static EMBEBIDOS: [(&str, &str); N] = [
//!     ("amber", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/content/amber.html"))),
//!     // ...
//! ];
//! ```
//!
//! Un tema es una página y el nombre del tema es la ruta del fichero: cada entrada
//! del array es la copia embebida de `content/<nombre>.html`, la que sirve
//! [`crate::theme::Theme::page`] cuando el directorio del operador no tiene el
//! fichero. El código, la razón y la descripción son **marcadores de plantilla**
//! (`{{code}}`, `{{reason}}`, `{{description}}`) que [`crate::render`] rellena al
//! servir, así que una sola página por tema cubre cualquier código de `400..=599`.
//!
//! Si `content/` no existe o no contiene ningún `*.html`, la compilación aborta con
//! un mensaje claro: el crate siempre lleva sus temas.

include!(concat!(env!("OUT_DIR"), "/temas_embebidos.rs"));
