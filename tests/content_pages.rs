//! Pruebas de integración del **contenido** de la página de error del proyecto
//! (change `error-page-template`, sobre los requisitos «Los códigos que un proxy
//! emite tienen su propia página», «Las páginas no referencian ficheros que no
//! existen» y «Cada página se identifica con la clase de su código»).
//!
//! Estas pruebas no dependen del router: leen el directorio `content/` del
//! repositorio, el mismo que `CONTENT_DIR` apunta por defecto (`content`). Lo
//! que cierran no es un caso suelto sino **la clase entera de fallo**: si
//! mañana el proyecto entrega otra página, la página deja de llevar los
//! marcadores de la plantilla, un `href` local apunta a un fichero inexistente,
//! la página se cuelga de la hoja de un código en lugar de la hoja única
//! `css/error.css`, o el `body` deja de llevar el marcador de la clase, estas
//! pruebas vuelven a fallar.
//!
//! La página es **una plantilla** —no once ficheros por código—: lleva el código,
//! la razón y la descripción como marcadores que el servidor rellena al servir, y
//! conserva el nombre del código por defecto (`404.html`). La hoja de estilos es
//! **una sola** (`css/error.css`): la diferencia de aspecto por código se expresa
//! con la clase `c<código>` del `body` —el marcador `c{{code}}`—, no duplicando
//! fichero; por eso la prueba de hojas comprueba a la vez la hoja y la clase.
//!
//! Decisiones:
//!
//! - Ruta por `env!("CARGO_MANIFEST_DIR")`, no por directorio de trabajo: las
//!   pruebas de integración se ejecutan con el cwd del crate, pero fijar la
//!   ruta en tiempo de compilación evita depender de eso.
//! - Sin dependencias nuevas: los `href` se extraen con una búsqueda literal de
//!   `href="` … `"`. Es suficiente para el HTML que genera el proyecto y no
//!   arrastra un parser completo.
//! - Los fallos se **acumulan** en vez de abortar en el primero: cada aserción
//!   **nombra el fichero** que falla, y un solo `cargo test` deja la lista
//!   completa de lo que incumple el contrato.

use std::fs;
use std::path::{Path, PathBuf};

/// Directorio de contenido del repositorio.
fn content_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("content")
}

/// Lee un fichero de texto, con un fallo que dice **qué** fichero no se pudo
/// leer en vez del `io::Error` pelado.
fn leer(ruta: &Path) -> String {
    fs::read_to_string(ruta)
        .unwrap_or_else(|error| panic!("no se pudo leer {}: {error}", ruta.display()))
}

/// Lista los `*.html` de `dir`, ordenados para que el fallo sea reproducible.
fn paginas_html(dir: &Path) -> Vec<PathBuf> {
    let entradas = fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("no se pudo listar {}: {error}", dir.display()));

    let mut paginas: Vec<PathBuf> = entradas
        .filter_map(Result::ok)
        .map(|entrada| entrada.path())
        .filter(|ruta| ruta.extension().is_some_and(|ext| ext == "html"))
        .collect();
    paginas.sort();
    paginas
}

/// Extrae el valor de todos los `href="…"` de un documento.
fn hrefs(html: &str) -> Vec<String> {
    const PREFIJO: &str = "href=\"";

    let mut encontrados = Vec::new();
    let mut resto = html;

    while let Some(inicio) = resto.find(PREFIJO) {
        let tras_comilla = &resto[inicio + PREFIJO.len()..];
        let Some(fin) = tras_comilla.find('"') else {
            break;
        };
        encontrados.push(tras_comilla[..fin].to_string());
        resto = &tras_comilla[fin + 1..];
    }

    encontrados
}

/// Los `href` que no salen a otro host: los que empiezan por `/`.
fn hrefs_locales(html: &str) -> Vec<String> {
    hrefs(html)
        .into_iter()
        .filter(|href| href.starts_with('/'))
        .collect()
}

/// Quita la consulta (`?…`) y el fragmento (`#…`) de una ruta: lo que se sirve
/// es el path, no lo que va detrás.
fn ruta_sin_sufijo(href: &str) -> &str {
    href.split(['?', '#']).next().unwrap_or(href)
}

/// Convierte un `href` local en la ruta que debería existir dentro de
/// `CONTENT_DIR` (`/css/error.css` → `css/error.css`).
fn relativa_de_href(href: &str) -> &str {
    ruta_sin_sufijo(href).trim_start_matches('/')
}

/// Las hojas de estilos referenciadas por un documento, como rutas relativas
/// al directorio de contenido (`css/error.css`).
fn hojas_de_estilos(html: &str) -> Vec<String> {
    hrefs_locales(html)
        .into_iter()
        .map(|href| relativa_de_href(&href).to_string())
        .filter(|ruta| ruta.ends_with(".css"))
        .collect()
}

/// ¿La ruta es la hoja de un código concreto (`css/404.css`, `css/504.css`)?
/// Es justo lo que ninguna página debe referenciar: la hoja es única y neutra.
fn es_hoja_por_codigo(ruta: &str) -> bool {
    ruta.strip_prefix("css/")
        .and_then(|resto| resto.strip_suffix(".css"))
        .is_some_and(|cuerpo| !cuerpo.is_empty() && cuerpo.bytes().all(|b| b.is_ascii_digit()))
}

/// El valor del atributo `class` del elemento `body`, si lo lleva.
fn clase_del_body(html: &str) -> Option<String> {
    const PREFIJO: &str = "class=\"";

    let etiqueta = &html[html.find("<body")?..];
    let etiqueta = &etiqueta[..etiqueta.find('>')?];
    let tras_prefijo = &etiqueta[etiqueta.find(PREFIJO)? + PREFIJO.len()..];

    Some(tras_prefijo[..tras_prefijo.find('"')?].to_string())
}

/// Falla con la lista completa de incumplimientos, uno por línea y precedido
/// del motivo, en vez de con el primero encontrado.
fn exigir_sin_fallos(motivo: &str, fallos: &[String]) {
    assert!(
        fallos.is_empty(),
        "{motivo} ({} incumplimiento(s)):\n- {}",
        fallos.len(),
        fallos.join("\n- ")
    );
}

/// La única página del proyecto lleva los tres marcadores de la plantilla
/// (`{{code}}`, `{{reason}}`, `{{description}}`): su código, su razón y su
/// descripción los rellena el servidor al servir, no el fichero a mano.
#[test]
fn la_pagina_lleva_los_marcadores_de_la_plantilla() {
    // El proyecto entrega **una** página, y su código, su razón y su descripción
    // son marcadores que el servidor rellena al servir.
    let pagina = leer(&content_dir().join("404.html"));

    for marcador in ["{{code}}", "{{reason}}", "{{description}}"] {
        assert!(
            pagina.contains(marcador),
            "la página del proyecto tiene que llevar `{marcador}`: {pagina}"
        );
    }
}

/// El proyecto entrega **una sola** página —la plantilla—, y conserva el nombre
/// del código por defecto (`404.html`), no una por código.
#[test]
fn el_proyecto_entrega_una_sola_pagina() {
    let paginas = paginas_html(&content_dir());

    assert_eq!(
        paginas.len(),
        1,
        "el proyecto entrega **una** página —la plantilla—, no una por código: {paginas:?}"
    );
    assert_eq!(
        paginas[0].file_name().unwrap(),
        "404.html",
        "la plantilla conserva el nombre del código por defecto"
    );
}

/// Ningún `href` local de ninguna página apunta a un fichero que no exista.
#[test]
fn cada_href_local_de_cada_pagina_resuelve() {
    let dir = content_dir();
    let paginas = paginas_html(&dir);
    assert!(
        !paginas.is_empty(),
        "no hay páginas HTML en {}: el directorio de contenido debería tenerlas",
        dir.display()
    );

    let mut fallos = Vec::new();

    for pagina in paginas {
        let html = leer(&pagina);

        for href in hrefs_locales(&html) {
            let destino = dir.join(relativa_de_href(&href));
            if !destino.is_file() {
                fallos.push(format!(
                    "{} referencia {href}, que no existe como fichero: {}",
                    pagina.display(),
                    destino.display()
                ));
            }
        }
    }

    exigir_sin_fallos(
        "toda página debe referenciar solo ficheros presentes en el directorio de contenido",
        &fallos,
    );
}

/// La página única de `content/` comparte la **única** hoja `css/error.css` —no
/// cuelga de la hoja de un código—, esa hoja existe, y el `body` lleva la clase
/// de su código como **marcador** (`c{{code}}`), que el servidor rellena con el
/// código resuelto **al servir**: sobre una sola hoja, la clase es lo que
/// diferencia la apariencia, y quien la garantiza es la plantilla, no el fichero
/// escrito a mano.
#[test]
fn cada_pagina_comparte_la_hoja_unica_y_se_identifica_con_su_clase() {
    const HOJA_COMPARTIDA: &str = "css/error.css";
    /// El marcador del código en la clase del `body`: el servidor lo sustituye
    /// por el código resuelto, así que la clase y el estado no pueden discrepar.
    const CLASE_DE_PLANTILLA: &str = "c{{code}}";

    let dir = content_dir();
    let paginas = paginas_html(&dir);
    assert!(
        !paginas.is_empty(),
        "no hay páginas HTML en {}: el directorio de contenido debería tenerlas",
        dir.display()
    );

    let mut fallos = Vec::new();

    let hoja = dir.join(HOJA_COMPARTIDA);
    if !hoja.is_file() {
        fallos.push(format!(
            "falta la hoja compartida {} que debe referenciar la página",
            hoja.display()
        ));
    }

    for pagina in paginas {
        let html = leer(&pagina);

        // Hojas: todas `css/error.css`; ninguna la hoja de un código.
        let hojas = hojas_de_estilos(&html);
        if hojas.is_empty() {
            fallos.push(format!(
                "{} no referencia ninguna hoja de estilos: debería referenciar {HOJA_COMPARTIDA}",
                pagina.display()
            ));
        }
        for hoja in hojas {
            if hoja == HOJA_COMPARTIDA {
                continue;
            }
            if es_hoja_por_codigo(&hoja) {
                fallos.push(format!(
                    "{} referencia {hoja}, la hoja de un código; la hoja es única y se llama {HOJA_COMPARTIDA}",
                    pagina.display()
                ));
            } else {
                fallos.push(format!(
                    "{} referencia {hoja}; la página debe usar la hoja compartida {HOJA_COMPARTIDA}",
                    pagina.display()
                ));
            }
        }

        // La clase del `body` es el marcador que el servidor rellena con el
        // código resuelto.
        match clase_del_body(&html) {
            Some(clase) if clase.split_whitespace().any(|c| c == CLASE_DE_PLANTILLA) => {}
            Some(clase) => fallos.push(format!(
                "{} tiene el body con clase {clase:?}; debería llevar {CLASE_DE_PLANTILLA:?}, el \
                 marcador que el servidor rellena con el código resuelto",
                pagina.display()
            )),
            None => fallos.push(format!(
                "{} no tiene un body con atributo class; debería llevar {CLASE_DE_PLANTILLA:?}",
                pagina.display()
            )),
        }
    }

    exigir_sin_fallos(
        "la página debe usar la hoja única `css/error.css` y llevar en el body el marcador de \
         su código",
        &fallos,
    );
}
