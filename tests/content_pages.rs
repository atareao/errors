//! Pruebas de integración del **contenido** de las páginas de error del proyecto
//! (change `themes-as-pages`, sobre los requisitos «Las siete páginas del
//! proyecto», «Ninguna página pide nada por la red» y «Cada página se identifica
//! con la clase de su código»).
//!
//! El modelo que estas pruebas fijan: **un tema es una página** y el nombre del
//! tema **es** la ruta de su fichero. `content/` entrega **siete** páginas —una
//! por tema— en su **raíz**: `default.html`, `minimal.html`, `terminal.html`,
//! `paper.html`, `amber.html`, `compact.html` y `matrix.html`. Cada una es
//! **autocontenida**: su `<style>` va dentro (y su `<script>` también, si lo
//! lleva), así que ninguna pide nada por la red. Se retiraron la hoja suelta
//! `css/error.css`, las cinco hojas de `themes/*.css` y el marcador
//! `<link rel="stylesheet" href="/css/error.css" />`: ya no hay hoja que
//! incrustar al servir, y por eso estas pruebas ya no buscan ninguna.
//!
//! Estas pruebas no dependen del router: leen el directorio `content/` del
//! repositorio, el mismo que `CONTENT_DIR` apunta por defecto (`content`). Lo que
//! cierran no es un caso suelto sino **la clase entera de fallo**:
//!
//! - Si el repositorio entrega un número de páginas distinto de siete, un nombre
//!   de tema sin fichero o un fichero cuyo tallo no sea un nombre de tema, la
//!   primera prueba lo dice: el registro de temas y el directorio no pueden
//!   divergir sin que alguien lo note.
//! - Si una página deja de llevar `{{code}}`, `{{reason}}` o `{{description}}`,
//!   el cuerpo servido mentiría sobre el código —la plantilla es lo único que
//!   rellena el servidor al servir—.
//! - Si una de las seis páginas sobrias diverge de las otras, la tercera prueba
//!   lo caza: comparten su marcado **palabra por palabra** y solo cambia su
//!   `<style>`; es el coste de mantener seis ficheros y la prueba es lo que lo
//!   vigila (una guarda textual no puede demostrar que seis ficheros no diverjan).
//! - Si alguien vuelve a colgar una página de una hoja externa —un `<link>` o un
//!   `href` a un `.css`—, la cuarta prueba lo dice: en el patrón del middleware
//!   `errors` de Traefik la página se sirve en el host de la aplicación, donde
//!   este servicio no sirve assets; el navegador pediría un fichero que contesta
//!   el upstream y la página llegaría sin estilos.
//! - Si un `href` local apunta a un fichero que no existe, la quinta prueba lo
//!   dice, página a página.
//! - Si el `body` de una página pierde el marcador `c{{code}}`, la sexta prueba
//!   lo dice: la clase y el estado no pueden discrepar, porque quien la escribe
//!   es la plantilla, con el mismo código resuelto que el estado.
//!
//! Decisiones:
//!
//! - Ruta por `env!("CARGO_MANIFEST_DIR")`, no por directorio de trabajo: las
//!   pruebas de integración se ejecutan con el cwd del crate, pero fijar la ruta
//!   en tiempo de compilación evita depender de eso.
//! - Sin dependencias nuevas: los `href` se extraen con una búsqueda literal de
//!   `href="` … `"`, y el bloque `<style>` con una búsqueda literal de su
//!   apertura y su cierre. Es suficiente para el HTML que entrega el proyecto y
//!   no arrastra un parser completo.
//! - Las pruebas 2 a 6 iteran sobre **los siete nombres de tema**, no sobre los
//!   `*.html` que haya en `content/`: así miden el contrato sobre las siete
//!   páginas con nombre, sin depender de que el directorio esté ya limpio. Solo
//!   la primera —el recuento— mira el directorio entero, porque su contrato es
//!   justamente cuántas páginas hay.
//! - Los fallos se **acumulan** en vez de abortar en el primero: cada aserción
//!   **nombra el fichero** que falla, y un solo `cargo test` deja la lista
//!   completa de lo que incumple el contrato.

use std::fs;
use std::path::{Path, PathBuf};

/// Los siete nombres de tema, en el orden en que el proyecto los enumera. El
/// tallo del fichero de cada tema es su nombre: `default` → `default.html`.
const TEMAS: [&str; 7] = [
    "default", "minimal", "terminal", "paper", "amber", "compact", "matrix",
];

/// Las **seis páginas sobrias**: comparten el marcado palabra por palabra y solo
/// cambia su `<style>`. `matrix` queda fuera a propósito: es un documento propio
/// —canvas, glitch, terminal, minijuego— y no tiene por qué llevar `h2` ni `hr`.
const SOBRIAS: [&str; 6] = [
    "default", "minimal", "terminal", "paper", "amber", "compact",
];

/// Directorio de contenido del repositorio.
fn content_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("content")
}

/// La página del tema `nombre` dentro de `dir`: su nombre **es** su ruta.
fn pagina_de(dir: &Path, nombre: &str) -> PathBuf {
    dir.join(format!("{nombre}.html"))
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

/// El documento **sin su primer bloque `<style>…</style>`**.
///
/// Se quita **el primero**, delimitado por su propio `</style>`: es el bloque que
/// cada página lleva en la cabeza. Para las seis páginas sobrias, lo que queda es
/// su marcado —idéntico entre las seis—; comparar esos marcados byte a byte es lo
/// que vigila que no divergan, porque una guarda textual no puede demostrar que
/// seis ficheros sigan iguales.
fn sin_el_primer_estilo(html: &str) -> Option<String> {
    const ABRE: &str = "<style";
    const CIERRA: &str = "</style>";

    let inicio = html.find(ABRE)?;
    let desde = &html[inicio..];
    let fin = desde.find(CIERRA)? + CIERRA.len();

    let mut resultado = String::with_capacity(html.len());
    resultado.push_str(&html[..inicio]);
    resultado.push_str(&desde[fin..]);
    Some(resultado)
}

/// El valor del atributo `class` del elemento `body`, si lo lleva.
fn clase_del_body(html: &str) -> Option<String> {
    const PREFIJO: &str = "class=\"";

    let etiqueta = &html[html.find("<body")?..];
    let etiqueta = &etiqueta[..etiqueta.find('>')?];
    let tras_prefijo = &etiqueta[etiqueta.find(PREFIJO)? + PREFIJO.len()..];

    Some(tras_prefijo[..tras_prefijo.find('"')?].to_string())
}

/// El elemento **completo** —etiqueta de apertura incluida y hasta su etiqueta
/// de cierre—, si el documento lo lleva: `<footer …>…</footer>`.
fn elemento_completo<'a>(html: &'a str, etiqueta: &str) -> Option<&'a str> {
    let apertura = format!("<{etiqueta}");
    let cierre = format!("</{etiqueta}>");

    let desde = html.find(&apertura)?;
    let resto = &html[desde..];
    let fin = resto.find(&cierre)? + cierre.len();

    Some(&resto[..fin])
}

/// El contenido **interno** de un elemento —lo que va entre su etiqueta de
/// apertura y su etiqueta de cierre—, si el documento lo lleva.
fn contenido_del_elemento<'a>(html: &'a str, etiqueta: &str) -> Option<&'a str> {
    let apertura = format!("<{etiqueta}");
    let cierre = format!("</{etiqueta}>");

    let desde = html.find(&apertura)?;
    let tras_apertura = &html[desde + apertura.len()..];
    let cuerpo = &tras_apertura[tras_apertura.find('>')? + 1..];
    let fin = cuerpo.find(&cierre)?;

    Some(&cuerpo[..fin])
}

/// El valor de **todos** los atributos `=«…»` de un documento, con las dos
/// comillas: lo que va entre la comilla de apertura y la de cierre.
///
/// Se recorre buscando `=`, y si tras él —saltando espacios— viene una comilla,
/// se captura hasta la comilla de cierre. Es suficiente para el HTML que entrega
/// el proyecto y no arrastra un parser.
fn valores_de_atributo(html: &str) -> Vec<&str> {
    let mut valores = Vec::new();
    let mut resto = html;

    while let Some(eq) = resto.find('=') {
        let tras_eq = &resto[eq + 1..];
        let sin_espacios = tras_eq.trim_start();

        let Some(comilla) = sin_espacios
            .chars()
            .next()
            .filter(|c| *c == '"' || *c == '\'')
        else {
            resto = &resto[eq + 1..];
            continue;
        };

        let desde = &sin_espacios[comilla.len_utf8()..];
        let Some(fin) = desde.find(comilla) else {
            resto = &resto[eq + 1..];
            continue;
        };

        valores.push(&desde[..fin]);
        resto = &desde[fin + comilla.len_utf8()..];
    }

    valores
}

/// Los marcadores `{{…}}` que aparecen dentro de un texto, con las llaves.
fn marcadores_en(texto: &str) -> Vec<&str> {
    let mut encontrados = Vec::new();
    let mut resto = texto;

    while let Some(inicio) = resto.find("{{") {
        let desde = &resto[inicio..];
        let Some(fin) = desde.find("}}") else {
            break;
        };
        encontrados.push(&desde[..fin + 2]);
        resto = &desde[fin + 2..];
    }

    encontrados
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

/// El proyecto entrega **exactamente siete** páginas HTML —una por tema—, el
/// tallo de cada fichero es uno de los siete nombres y ningún nombre se queda
/// sin fichero ni tiene dos.
///
/// Mira el directorio **entero** (`*.html`), no la lista de nombres: su contrato
/// es justamente cuántas páginas hay. Un fichero de más —el `404.html` del código
/// por defecto, que este change retira— o de menos cuenta como incumplimiento.
#[test]
fn cada_tema_tiene_su_pagina_y_el_tallo_coincide() {
    let dir = content_dir();
    let paginas = paginas_html(&dir);

    let mut fallos = Vec::new();

    if paginas.len() != TEMAS.len() {
        fallos.push(format!(
            "content/ tiene que entregar exactamente {} páginas HTML (una por tema), no {}: {paginas:?}",
            TEMAS.len(),
            paginas.len()
        ));
    }

    // El tallo de cada fichero presente es un nombre de tema.
    let mut tallos = Vec::new();
    for pagina in &paginas {
        let tallo = pagina
            .file_stem()
            .and_then(|tallo| tallo.to_str())
            .unwrap_or_default()
            .to_string();
        if !TEMAS.contains(&tallo.as_str()) {
            fallos.push(format!(
                "{} no es la página de ningún tema: su tallo `{tallo}` no está en {TEMAS:?}",
                pagina.display()
            ));
        }
        tallos.push(tallo);
    }

    // Cada nombre de tema tiene su fichero...
    for tema in TEMAS {
        let pagina = pagina_de(&dir, tema);
        if !pagina.is_file() {
            fallos.push(format!(
                "falta la página del tema `{tema}`: {}",
                pagina.display()
            ));
        }
    }

    // ...y no hay dos ficheros para el mismo nombre.
    for (indice, tallo) in tallos.iter().enumerate() {
        if tallos[..indice].contains(tallo) {
            fallos.push(format!(
                "hay más de un fichero con el tallo `{tallo}`: cada nombre tiene una sola página"
            ));
        }
    }

    exigir_sin_fallos(
        "cada tema tiene su página y el tallo del fichero coincide con el nombre",
        &fallos,
    );
}

/// Cada una de las **siete** páginas lleva los tres marcadores de la plantilla
/// (`{{code}}`, `{{reason}}`, `{{description}}`): el código, la razón y la
/// descripción los rellena el servidor **al servir**, así que una página que
/// pierda un marcador miente sobre el código.
#[test]
fn cada_pagina_lleva_los_tres_marcadores() {
    let dir = content_dir();

    let mut fallos = Vec::new();

    for tema in TEMAS {
        let pagina = pagina_de(&dir, tema);
        let html = leer(&pagina);

        for marcador in ["{{code}}", "{{reason}}", "{{description}}"] {
            if !html.contains(marcador) {
                fallos.push(format!(
                    "{} no lleva el marcador `{marcador}`: los rellena el servidor al servir",
                    pagina.display()
                ));
            }
        }
    }

    exigir_sin_fallos(
        "cada página lleva `{{code}}`, `{{reason}}` y `{{description}}`",
        &fallos,
    );
}

/// Las **seis páginas sobrias** comparten su marcado **palabra por palabra**:
/// quitado el bloque `<style>…</style>` de cada una, las seis son idénticas
/// byte a byte. El marcado resultante lleva `<main>`, `<h1>`, `<h2>`, `<hr>` y
/// `<p>`, y firma con `<footer>errserve</footer>` **sin** `href`.
///
/// `matrix` queda **fuera**: es un documento propio y no tiene por qué llevar
/// `h2` ni `hr`.
#[test]
fn las_seis_sobrias_comparten_el_marcado_quitado_su_estilo() {
    let dir = content_dir();

    let mut fallos = Vec::new();
    let mut marcados: Vec<(String, String)> = Vec::new();

    for tema in SOBRIAS {
        let pagina = pagina_de(&dir, tema);
        let html = leer(&pagina);

        match sin_el_primer_estilo(&html) {
            Some(marcado) => marcados.push((tema.to_string(), marcado)),
            None => fallos.push(format!(
                "{} no lleva un bloque `<style>…</style>` que quitar: cada página sobria \
                 tiene su CSS dentro",
                pagina.display()
            )),
        }
    }

    if let Some((referencia, marcado)) = marcados.first() {
        // Las seis comparten el marcado: se compara cada una con la primera.
        for (tema, otro) in marcados.iter().skip(1) {
            if otro != marcado {
                fallos.push(format!(
                    "la página sobria `{tema}` diverge del marcado de `{referencia}` al \
                     quitarle su `<style>`: las seis sobrias comparten el marcado palabra por \
                     palabra"
                ));
            }
        }

        // El esqueleto que el contrato exige al marcado resultante.
        for etiqueta in ["main", "h1", "h2", "hr", "p"] {
            if !marcado.contains(&format!("<{etiqueta}")) {
                fallos.push(format!(
                    "el marcado de las sobrias no lleva `<{etiqueta}>` (comparado sobre \
                     `{referencia}`)"
                ));
            }
        }

        // La firma: `<footer>errserve</footer>`, sin ningún `href`.
        match elemento_completo(marcado, "footer") {
            Some(footer) => {
                if !contenido_del_elemento(marcado, "footer")
                    .is_some_and(|texto| texto.contains("errserve"))
                {
                    fallos.push(format!(
                        "el `<footer>` de las sobrias no firma con `errserve`: {footer}"
                    ));
                }
                if footer.contains("href") {
                    fallos.push(format!(
                        "el `<footer>` de las sobrias lleva un `href`: la firma no referencia \
                         nada. Footer: {footer}"
                    ));
                }
            }
            None => fallos
                .push("el marcado de las sobrias no lleva `<footer>errserve</footer>`".to_string()),
        }
    }

    exigir_sin_fallos(
        "las seis páginas sobrias comparten el marcado quitado su `<style>`",
        &fallos,
    );
}

/// Cada una de las **siete** páginas lleva su **estilo dentro**: un bloque
/// `<style>`, ningún `<link>` y ninguna referencia local a un fichero `.css`.
///
/// Es el contrato de autocontención en su forma comprobable: sin hoja externa, no
/// hay nada que pedir por la red y la página se ve en el host de la aplicación,
/// donde este servicio no sirve assets.
#[test]
fn cada_pagina_lleva_su_estilo_dentro_y_ninguna_hoja_externa() {
    let dir = content_dir();

    let mut fallos = Vec::new();

    for tema in TEMAS {
        let pagina = pagina_de(&dir, tema);
        let html = leer(&pagina);

        if !html.contains("<style") {
            fallos.push(format!(
                "{} no lleva su CSS en un bloque `<style>`: cada página es autocontenida",
                pagina.display()
            ));
        }

        if html.contains("<link") {
            fallos.push(format!(
                "{} lleva un `<link>`: ninguna página puede cargar nada de fuera",
                pagina.display()
            ));
        }

        for href in hrefs_locales(&html) {
            if relativa_de_href(&href).ends_with(".css") {
                fallos.push(format!(
                    "{} referencia la hoja `{href}`: la página lleva su CSS dentro, no en un \
                     fichero suelto",
                    pagina.display()
                ));
            }
        }
    }

    exigir_sin_fallos(
        "cada página lleva su estilo dentro y ninguna hoja externa",
        &fallos,
    );
}

/// Ningún `href` local de ninguna de las siete páginas apunta a un fichero que no
/// exista en `content/`. Hoy las páginas no deberían llevar **ningún** `href`: el
/// `<footer>` no firma con enlace y no hay hoja que cargar.
#[test]
fn ninguna_pagina_referencia_ficheros_inexistentes() {
    let dir = content_dir();

    let mut fallos = Vec::new();

    for tema in TEMAS {
        let pagina = pagina_de(&dir, tema);
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
        "ninguna página debe referenciar ficheros que no existen en el directorio de contenido",
        &fallos,
    );
}

/// El `body` de cada una de las **siete** páginas lleva el marcador `c{{code}}`:
/// sobre una sola página por tema, la clase es lo que diferencia la apariencia
/// por código, y quien la garantiza es la plantilla —con el código resuelto—, así
/// que la clase del cuerpo y el estado no pueden discrepar.
#[test]
fn el_body_de_cada_pagina_lleva_la_clase_de_su_codigo() {
    /// El marcador del código en la clase del `body`: el servidor lo sustituye
    /// por el código resuelto.
    const CLASE_DE_PLANTILLA: &str = "c{{code}}";

    let dir = content_dir();

    let mut fallos = Vec::new();

    for tema in TEMAS {
        let pagina = pagina_de(&dir, tema);
        let html = leer(&pagina);

        match clase_del_body(&html) {
            Some(clase)
                if clase
                    .split_whitespace()
                    .any(|token| token == CLASE_DE_PLANTILLA) => {}
            Some(clase) => fallos.push(format!(
                "{} tiene el body con clase {clase:?}; debería llevar {CLASE_DE_PLANTILLA:?}, \
                 el marcador que el servidor rellena con el código resuelto",
                pagina.display()
            )),
            None => fallos.push(format!(
                "{} no tiene un body con atributo class; debería llevar {CLASE_DE_PLANTILLA:?}",
                pagina.display()
            )),
        }
    }

    exigir_sin_fallos(
        "el body de cada página lleva la clase `c{{code}}` de su código",
        &fallos,
    );
}

/// La **invariante que sostiene el escape**: dentro del valor de **cualquier**
/// atributo de las siete páginas solo puede aparecer el marcador `{{code}}`.
///
/// El escaper del servidor (`render::escape_html`) escapa `&`, `<` y `>` pero
/// **no** la comilla —decisión documentada—, y eso solo es seguro porque el
/// único marcador que cae dentro de un valor de atributo (`class="c{{code}}"`
/// en las siete, `data-text="{{code}}"` en `matrix`) es el **código**, que es un
/// `u16` re-serializado y solo puede producir dígitos. `{{reason}}` y
/// `{{description}}` son **prosa** y pueden traer una comilla
/// (`canonical_reason(418) == "I'm a teapot"`): si una página futura metiera
/// `{{reason}}` en un atributo, esa comilla cerraría el valor antes de tiempo y
/// el escape de tres caracteres no bastaría —sería un XSS en la página servida—.
/// Esta prueba fija la invariante aquí, en el contenido, para que la página que
/// la rompa falle **en `cargo test`** y no en producción.
#[test]
fn ningun_marcador_de_prosa_cae_dentro_de_un_atributo() {
    let dir = content_dir();

    let mut fallos = Vec::new();

    for tema in TEMAS {
        let pagina = pagina_de(&dir, tema);
        let html = leer(&pagina);

        for valor in valores_de_atributo(&html) {
            for marcador in marcadores_en(valor) {
                if marcador != "{{code}}" {
                    fallos.push(format!(
                        "{} mete `{marcador}` dentro del valor de un atributo (`{valor}`): el \
                         escape no cubre la comilla, así que dentro de un atributo solo puede ir \
                         `{{{{code}}}}`",
                        pagina.display()
                    ));
                }
            }
        }
    }

    exigir_sin_fallos(
        "ningún marcador de prosa cae dentro de un valor de atributo (solo `{{code}}`)",
        &fallos,
    );
}
