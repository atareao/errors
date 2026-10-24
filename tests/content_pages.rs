//! Pruebas de integración del **contenido** de páginas de error
//! (`error-page-coverage`, requisitos «Los códigos que un proxy emite tienen su
//! propia página», «Las páginas no referencian ficheros que no existen» y «Cada
//! página se identifica con la clase de su código»).
//!
//! Estas pruebas no dependen del router: leen el directorio `content/` del
//! repositorio, el mismo que `CONTENT_DIR` apunta por defecto (`content`). Lo
//! que cierran no es un caso suelto sino **la clase entera de fallo**: si
//! mañana falta una página, una página deja de nombrar su código, un `href`
//! local apunta a un fichero inexistente, una página se cuelga de la hoja de
//! otro código en lugar de la hoja única `css/error.css`, o el `body` deja de
//! llevar la clase de su propio código, estas pruebas vuelven a fallar.
//!
//! La hoja de estilos es **una sola** (`css/error.css`): once copias del mismo
//! contenido no son once hojas, son una mal repartida. La diferencia de aspecto
//! por código se expresa con la clase `c<código>` del `body`, no duplicando
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
//!   **nombra el código o el fichero** que falla, y un solo `cargo test` deja
//!   la lista completa de lo que incumple el contrato, no el primero de los
//!   once elementos.

use std::fs;
use std::path::{Path, PathBuf};

/// Códigos que un proxy emite con normalidad: los once que el change exige
/// cubiertos con página propia. Es la lista de la spec, en el mismo orden.
const CODIGOS_CUBIERTOS: [u16; 11] = [400, 401, 403, 404, 405, 408, 429, 500, 502, 503, 504];

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

/// El código que el nombre del fichero declara (`404.html` → `404`).
fn codigo_de_pagina(pagina: &Path) -> Option<u16> {
    pagina.file_stem()?.to_str()?.parse::<u16>().ok()
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

/// Cada código cubierto tiene su página `<código>.html` y esa página nombra su
/// propio código, de modo que el cuerpo nunca contradiga al estado.
#[test]
fn cada_codigo_cubierto_tiene_pagina_que_se_nombra() {
    let dir = content_dir();
    let mut fallos = Vec::new();

    for codigo in CODIGOS_CUBIERTOS {
        let pagina = dir.join(format!("{codigo}.html"));

        if !pagina.is_file() {
            fallos.push(format!(
                "falta la página {} del código cubierto {codigo}",
                pagina.display()
            ));
            continue;
        }

        let html = leer(&pagina);
        if !html.contains(&codigo.to_string()) {
            fallos.push(format!(
                "{} no nombra su propio código {codigo}: el cuerpo contradice al estado",
                pagina.display()
            ));
        }
    }

    exigir_sin_fallos(
        "cada código cubierto necesita su página, y la página debe nombrar ese código",
        &fallos,
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

/// Toda página de `content/` comparte la **única** hoja `css/error.css` —ninguna
/// cuelga de la hoja de un código, ni siquiera de la suya—, esa hoja existe, y
/// el `body` de cada página se identifica con la clase `c<código>` de su propio
/// código: sobre una sola hoja, la clase es lo que diferencia la apariencia.
#[test]
fn cada_pagina_comparte_la_hoja_unica_y_se_identifica_con_su_clase() {
    const HOJA_COMPARTIDA: &str = "css/error.css";

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
            "falta la hoja compartida {} que deben referenciar todas las páginas",
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
                    "{} referencia {hoja}; todas las páginas deben usar la hoja compartida {HOJA_COMPARTIDA}",
                    pagina.display()
                ));
            }
        }

        // Clase del `body` según el código del nombre del fichero.
        let Some(codigo) = codigo_de_pagina(&pagina) else {
            fallos.push(format!(
                "{} no declara un código en su nombre de fichero: no se puede saber su clase c<código>",
                pagina.display()
            ));
            continue;
        };
        let esperada = format!("c{codigo}");

        match clase_del_body(&html) {
            Some(clase) if clase.split_whitespace().any(|c| c == esperada) => {}
            Some(clase) => fallos.push(format!(
                "{} tiene el body con clase {clase:?}; debería llevar {esperada:?}",
                pagina.display()
            )),
            None => fallos.push(format!(
                "{} no tiene un body con atributo class; debería llevar {esperada:?}",
                pagina.display()
            )),
        }
    }

    exigir_sin_fallos(
        "toda página debe usar la hoja única `css/error.css` y llevar en el body la clase de su código",
        &fallos,
    );
}
