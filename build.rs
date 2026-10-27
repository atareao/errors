//! Descubre las páginas de tema al compilar y genera `temas_embebidos.rs`.
//!
//! Lee el directorio `content/` del crate (ruta absoluta desde
//! `CARGO_MANIFEST_DIR`), filtra los `*.html` cuyo tallo sea UTF-8 no vacío, los
//! ordena por nombre para que el build sea determinista y emite en `$OUT_DIR` un
//! array `EMBEBIDOS` con una entrada por página:
//!
//! ```ignore
//! pub static EMBEBIDOS: [(&str, &str); N] = [
//!     ("amber", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/content/amber.html"))),
//!     // ...
//! ];
//! ```
//!
//! Si `content/` no existe o no contiene ningún `*.html`, aborta la compilación
//! con un mensaje claro: el crate siempre lleva sus temas. Emite
//! `cargo:rerun-if-changed=content` para que añadir o quitar un fichero dispare
//! la recompilación.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::exit;

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR no está definida");
    let content_dir = PathBuf::from(&manifest_dir).join("content");

    if !content_dir.is_dir() {
        eprintln!(
            "error: el directorio 'content/' no existe en {}.\n\
             El crate siempre lleva sus temas: abortando la compilación.",
            content_dir.display()
        );
        exit(1);
    }

    // Un fichero nuevo o borrado en `content/` tiene que disparar la recompilación.
    println!("cargo:rerun-if-changed=content");

    let mut ficheros: Vec<String> = Vec::new();
    for entrada in fs::read_dir(&content_dir).expect("no se puede leer el directorio 'content'") {
        let entrada = entrada.expect("error al iterar el directorio 'content'");
        let path = entrada.path();

        if !path.is_file() {
            continue;
        }

        // La extensión tiene que ser exactamente `html`.
        if path.extension().and_then(|ext| ext.to_str()) != Some("html") {
            continue;
        }

        // El nombre es el tallo: UTF-8 y no vacío.
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        if stem.is_empty() {
            continue;
        }

        ficheros.push(stem.to_string());
    }

    if ficheros.is_empty() {
        eprintln!(
            "error: el directorio 'content/' no contiene ningún fichero .html.\n\
             El crate siempre lleva sus temas: abortando la compilación."
        );
        exit(1);
    }

    // Orden determinista por nombre (tallo).
    ficheros.sort();

    let mut codigo = String::new();
    codigo.push_str("// Generado automáticamente por build.rs. NO EDITAR A MANO.\n");
    codigo.push_str(&format!(
        "pub static EMBEBIDOS: [(&str, &str); {}] = [\n",
        ficheros.len()
    ));

    for nombre in &ficheros {
        // `{:?}` escapa correctamente el nombre como literal de Rust. La ruta se
        // construye con `concat!` y `env!`, que resuelven al compilar.
        codigo.push_str(&format!(
            "    ({:?}, include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), {:?}))),\n",
            nombre,
            format!("/content/{}.html", nombre)
        ));
    }

    codigo.push_str("];\n");

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR no está definida");
    let destino = PathBuf::from(out_dir).join("temas_embebidos.rs");
    fs::write(&destino, codigo).expect("no se puede escribir el fichero generado");
}
