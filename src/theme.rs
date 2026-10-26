//! Temas de las páginas de error: **un tema es una página**.
//!
//! Un **tema** es **una página**: su **nombre** es el de su fichero y la ruta se
//! **deriva** de él —`CONTENT_DIR/<nombre>.html`, en la **raíz** de
//! `CONTENT_DIR`—, de modo que no exista una tabla de rutas que pueda divergir
//! del nombre. Las **siete** páginas van **embebidas** en el binario
//! ([`crate::embedded`]), así que un servicio arrancado sin `content/` sirve los
//! siete diseños, y borrar una página es un **error de compilación**, no un
//! respaldo silencioso.
//!
//! ---------------------------------------------------------------------------
//! API pública del módulo:
//!
//! ```ignore
//! #[derive(Debug)]
//! pub struct Theme {
//!     pub name: &'static str,
//!     pub embedded: &'static str,   // la página del tema, embebida con include_str!
//! }
//!
//! pub static THEMES: [Theme; 7];
//! pub const DEFAULT_THEME_NAME: &str = "default";
//! pub const THEME_NAMES: &str =
//!     "default, minimal, terminal, paper, amber, compact, matrix";
//!
//! pub fn theme_by_name(name: &str) -> Option<&'static Theme>;
//! pub fn default_theme() -> &'static Theme;
//! pub fn resolve(requested: Option<&str>, configured: &'static Theme) -> &'static Theme;
//!
//! impl Theme {
//!     pub fn page_path(&self) -> String;
//!     pub fn page(&self, content_dir: &Path) -> Result<Vec<u8>, std::io::Error>;
//! }
//! ```
//!
//! # Por qué un tema es una página y no una hoja
//!
//! El modelo anterior hacía de un tema **una sola hoja de estilos** que se
//! incrustaba al servir sustituyendo el marcador
//! `<link rel="stylesheet" href="/css/error.css" />`. Ese eje solo toca el
//! **aspecto**: un tema que quiere entregar una **experiencia** —su marcado, su
//! `<script>`, su `<canvas>`, como el tema `matrix`— no cabe en una hoja. Con el
//! modelo nuevo, cada tema **es** su documento, autocontenido: su `<style>` dentro
//! y su `<script>` también dentro si lo lleva.
//!
//! # Por qué la página se resuelve **como un asset**
//!
//! [`Theme::page`] aplica a la página la misma regla que a cualquier asset: un
//! fichero **legible** en `CONTENT_DIR/<nombre>.html` gana a la copia embebida.
//! Así el **punto de personalización** del operador es su página de tema
//! (`content/default.html` para el tema por defecto), y se lee **al servir**, de
//! modo que un operador que edite su página la ve cambiar sin reiniciar. Un
//! fichero **ausente** cae a la copia embebida **sin** `warn` —es lo normal en un
//! binario instalado sin ficheros de datos—; un fichero **presente pero ilegible**
//! es un error de E/S: la página es **contenido**, no adorno, así que no se tapa
//! en silencio con la copia embebida, y el render lo convierte en el **respaldo de
//! texto** con el estado intacto (requisito «Un fallo al leer el HTML no cambia el
//! estado»).
//!
//! # Los tres contratos de una página de tema
//!
//! Una página **incorporada al binario** tiene que cumplir tres cosas, y son tres
//! cosas distintas. Las tres se comprueban en el `#[cfg(test)]` de este módulo,
//! porque cubren **lo que el proyecto entrega**: la página del operador en
//! `CONTENT_DIR` es suya y el servicio **no** la parsea al servir.
//!
//! 1. **No puede hacer que el navegador pida nada por la red** —«Ninguna página
//!    pide nada por la red»—. Ni `@import`, ni un `url(...)` que no sea una
//!    referencia interna (`url(#…)`) o un dato incrustado (`url(data:…)`), ni un
//!    `<script src=…>`, ni una imagen, ni un `fetch(`.
//! 2. **Si anima, tiene que poder apagarse** —«Una página que anima puede
//!    apagarse»—. El movimiento es una decisión de aspecto, no un defecto de
//!    autocontención, pero una página de error que se mueve mientras el usuario
//!    intenta leer qué ha pasado es peor página de error.
//! 3. **Cada página sobria cubre el marcado** que le da forma —`body`, `h1`,
//!    `h2`, `hr`, `p` y `footer`—. `matrix` es un documento propio y **queda
//!    fuera** de este contrato.
//!
//! # Lo que las guardas ven y lo que no
//!
//! Las guardas son **textuales y sobre bytes** —una página puede no estar en
//! UTF-8—, sin parser ni `regex` (el árbol compila sin red). Limpian los
//! **comentarios** antes de escanear, y lo hacen **por regiones**: el `<!-- … -->`
//! en todo el documento; el bloque `/* … */` solo dentro de `<style>` o
//! `<script>` —es la sintaxis de CSS y de JavaScript, no de HTML—; y la línea
//! `// …` solo dentro de `<script>`, porque fuera de un guion `//` es **texto
//! visible** y tratarlo como comentario taparía lo que venga detrás. Dentro de una
//! región, el limpiador respeta las cadenas. El motivo de limpiar: un comentario
//! no puede pedir nada ni animar, y la propia cabecera de una página documenta por
//! escrito qué no puede hacer. La detección de **etiquetas y atributos HTML** es
//! **insensible a mayúsculas**; la de los **tokens de JavaScript**, **sensible**
//! —son identificadores—.
//!
//! Cada contrato mide **la lista cerrada que declara y nada más**, y lo dice en
//! lugar de esconderlo: la guarda de red no ve una URL construida en tiempo de
//! ejecución (`fetch(base + path)`), ni una librería que pida por su cuenta, ni un
//! `<img>` que se cree por JavaScript después de cargar; la de movimiento sobre el
//! guion solo comprueba la **mención** de `prefers-reduced-motion`, no que la rama
//! del `if` sea correcta; y la del CSS, que se describe en su propia sección más
//! abajo, garantiza el **patrón canónico** de neutralización **por familia**, no
//! que todos los navegadores apaguen el movimiento. Esos límites están escritos,
//! con el mismo criterio con el que se documentó el contraste bajo del `<hr>`: son
//! decisiones medidas y declaradas, no descuidos.

use crate::embedded;
use std::io::ErrorKind;
use std::path::Path;

/// Un tema: su nombre y la página que lo define, embebida en el binario.
///
/// Los dos campos van en la tabla del binario porque el tema tiene que estar
/// disponible en un servicio arrancado **sin ficheros de datos**: un binario
/// instalado con `cargo install` sirve la página de su tema aunque no haya
/// `content/`. La ruta de esa página **no** es un campo —se **deriva** del
/// nombre—, así que no puede divergir de él.
#[derive(Debug)]
pub struct Theme {
    /// Nombre público del tema, el que se escribe en `THEME` y en `X-Theme`, y
    /// del que se deriva la ruta de su página (`<nombre>.html`).
    pub name: &'static str,
    /// La página del tema, embebida en el binario: la copia de respaldo de
    /// [`Theme::page`].
    pub embedded: &'static str,
}

/// Los **siete** temas incorporados, y la **única** fuente de verdad de qué temas
/// hay.
///
/// `default` es el tema por defecto (`THEMES[0]`) y el único de las **seis
/// sobrias** que **anima** —su `<main>` entra con un movimiento que neutraliza en
/// `prefers-reduced-motion: reduce`—; los cinco sobrios siguientes comparten una
/// **decisión de diseño**, no un contrato: son deliberadamente sobrios porque son
/// páginas de **error** y su trabajo es decir qué ha pasado. `matrix` es un
/// **documento propio** —canvas de lluvia, glitch, terminal, minijuego, y también
/// anima— y su marcado puede diferir del de las seis sobrias.
///
/// Es un `static` y no una `const`: se recorre con `THEMES.iter()`, nunca por
/// valor —mover de un `static` no está permitido—, y da una única dirección a lo
/// largo del binario.
pub static THEMES: [Theme; 7] = [
    Theme {
        name: DEFAULT_THEME_NAME,
        embedded: embedded::DEFAULT_PAGE,
    },
    Theme {
        name: "minimal",
        embedded: embedded::MINIMAL_PAGE,
    },
    Theme {
        name: "terminal",
        embedded: embedded::TERMINAL_PAGE,
    },
    Theme {
        name: "paper",
        embedded: embedded::PAPER_PAGE,
    },
    Theme {
        name: "amber",
        embedded: embedded::AMBER_PAGE,
    },
    Theme {
        name: "compact",
        embedded: embedded::COMPACT_PAGE,
    },
    Theme {
        name: "matrix",
        embedded: embedded::MATRIX_PAGE,
    },
];

/// Nombre del tema por defecto, que es también el de `THEMES[0]`.
pub const DEFAULT_THEME_NAME: &str = "default";

/// Los nombres válidos, en el orden de [`THEMES`], para los mensajes de error.
///
/// Es la lista que el error de `THEME` enumera: quien escribe la variable cree
/// que está configurando algo, así que el error tiene que decirle qué puede
/// escribir en su lugar, no solo que se equivocó. Una prueba comprueba que
/// enumera los siete y solo los siete.
pub const THEME_NAMES: &str = "default, minimal, terminal, paper, amber, compact, matrix";

/// El tema incorporado con ese nombre, si lo hay.
///
/// La comparación es **exacta**, sensible a mayúsculas: `Terminal` no es el tema
/// `terminal`. Adivinar la intención de quien escribe la cabecera sería peor que
/// no aceptarla.
pub fn theme_by_name(name: &str) -> Option<&'static Theme> {
    THEMES.iter().find(|theme| theme.name == name)
}

/// El tema por defecto (`default`), que es el de `THEMES[0]`.
pub fn default_theme() -> &'static Theme {
    &THEMES[0]
}

/// Resuelve el tema efectivo de una petición.
///
/// Con el principio de siempre —el **primer** origen que haga match—, un
/// `X-Theme` **conocido** gana al tema configurado (`THEME`); **ausente o
/// desconocido** cae al configurado. Es deliberado que un valor desconocido no
/// sea un error: esa cabecera la elige quien llama, y un cliente no puede romper
/// el servicio. La asimetría con `THEME` —que sí aborta el arranque— es la
/// misma que ya rige entre `X-Code`/`X-Format` y las variables del operador.
pub fn resolve(requested: Option<&str>, configured: &'static Theme) -> &'static Theme {
    requested.and_then(theme_by_name).unwrap_or(configured)
}

impl Theme {
    /// La ruta de la página del tema, **relativa a `CONTENT_DIR`**:
    /// `"<nombre>.html"`. Derivada del nombre, no guardada en una tabla.
    ///
    /// El motivo de derivarla y no guardarla: una tabla `name -> ruta` puede
    /// divergir del nombre —un tema `matrix` con su página en `themes/matrix.html`—
    /// y eso es un fallo que el compilador no ve. Con la ruta derivada, la
    /// divergencia es **inexpresable**. El precio, escrito: el fichero tiene que
    /// llamarse exactamente como el tema, y así lo exige la prueba «cada nombre
    /// deriva su ruta y tiene su fichero».
    pub fn page_path(&self) -> String {
        format!("{}.html", self.name)
    }

    /// La página del tema: la de `CONTENT_DIR/<nombre>.html` si se puede leer, y
    /// si no la embebida.
    ///
    /// Un fichero **ausente** no es un fallo —es lo normal en un binario
    /// instalado sin ficheros de datos—, así que cae a la copia embebida **sin**
    /// `warn`. Un fichero **presente pero ilegible** deja un `warn!` que **nombra
    /// el fichero y su causa** y devuelve el `io::Error`: el render lo convierte
    /// en [`crate::render::RenderError::Io`] y la respuesta es el **respaldo de
    /// texto** con el estado intacto.
    ///
    /// Un fichero del operador roto **no** se tapa en silencio con la copia
    /// embebida: la página es **contenido**, y decidir que un directorio roto se
    /// resuelve con la copia del binario ocultaría el fallo. El `warn` lo delata y
    /// el estado no cambia. Es la Opción A de `embedded-error-pages`, sin cambios
    /// para las páginas.
    pub fn page(&self, content_dir: &Path) -> Result<Vec<u8>, std::io::Error> {
        let path = content_dir.join(self.page_path());

        match std::fs::read(&path) {
            Ok(bytes) => Ok(bytes),
            Err(err) if err.kind() == ErrorKind::NotFound => Ok(self.embedded.as_bytes().to_vec()),
            Err(err) => {
                tracing::warn!(
                    theme = self.name,
                    path = %path.display(),
                    error = %err,
                    "no se pudo leer la página del tema: se responderá el respaldo de texto"
                );

                Err(err)
            }
        }
    }
}

/// Un incumplimiento de un contrato por una página de tema **incorporada**.
///
/// Existe para poder comprobar cada cláusula de los contratos con un caso
/// pequeño, en lugar de solo el conjunto sobre las siete páginas del binario: la
/// comprobación deja de ser un bucle dentro de una prueba y pasa a ser una
/// función pura sobre bytes.
///
/// Va bajo `#[cfg(test)]` **a propósito**: el servicio no valida HTML ni CSS —ni
/// al arrancar ni al servir—, así que este código existe solo para `cargo test`,
/// que es donde se comprueba **lo que el proyecto entrega**. El binario no lo
/// lleva.
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Incumplimiento {
    /// La página puede hacer que el navegador **pida** algo por la red. Lleva
    /// consigo el token que lo provoca —`@import`, `url(`, `<script src=`…—
    /// para que el incumplimiento se pueda nombrar en el mensaje.
    Peticion(&'static str),
    /// La página declara una animación que no neutraliza, o un guion que anima sin
    /// mencionar `prefers-reduced-motion`. Lleva consigo el token que anima, el de
    /// la **primera** declaración sin neutralizar.
    Movimiento(&'static str),
}

/// Los contratos que incumple una **página** de tema, con el token que los rompe.
///
/// Son dos, y son dos cosas distintas: **no poder hacer que el navegador pida
/// nada por la red** y **si anima, poder apagarlo**. Se escanean los bytes de la
/// **página entera** —su HTML, sus estilos incrustados y sus guiones—, limpiando
/// antes los comentarios.
///
/// Va bajo `#[cfg(test)]` **a propósito**: el servicio no valida la página al
/// servir.
#[cfg(test)]
pub fn incumplimientos(pagina: &[u8]) -> Vec<Incumplimiento> {
    let mut fallos = Vec::new();
    let pagina = pagina_sin_comentarios(pagina);

    if let Some(token) = token_que_pide(&pagina) {
        fallos.push(Incumplimiento::Peticion(token));
    }

    if let Some(token) = token_que_anima(&pagina) {
        fallos.push(Incumplimiento::Movimiento(token));
    }

    fallos
}

/// La página **sin sus comentarios**.
///
/// Un comentario no es una regla: no puede hacer que el navegador pida nada ni
/// puede animar, así que no puede incumplir ninguno de los contratos. Sin esto,
/// la propia documentación de una página —su cabecera diciendo qué no puede
/// hacer— contaría como un incumplimiento y la guarda mediría algo distinto de lo
/// que anuncia.
///
/// La limpieza **respeta las regiones** del documento, y esto importa:
///
/// - El comentario HTML `<!-- … -->` se limpia en **todo** el documento —se salta
///   entero, sin mirar cadenas dentro—.
/// - El bloque `/* … */` se limpia **solo dentro de `<style>` o `<script>`**: es
///   la sintaxis de comentario de CSS y de JavaScript, no de HTML. Limpiarlo en
///   prosa HTML sería un error —no hay tal comentario—, y **no** limpiarlo dentro
///   de `<style>` es peor: la cabecera de una página documenta el `@media
///   (prefers-reduced-motion: reduce)` que trae, y leer esa prosa como una regla
///   escondería el bloque de verdad.
/// - La línea `// …` se limpia **solo dentro de `<script>`**: es la sintaxis de
///   JavaScript. Fuera de un guion, `//` es **texto visible** —`content/matrix.html`
///   lo usa así, en `<p class="boot">// errserve — terminal de errores</p>`—, y
///   tratarlo como comentario se comería el resto de la línea y **taparía** una
///   petición que viniera detrás: el caso adversario que rechaza de más, que es la
///   dirección segura.
///
/// El limpiador es **textual**, pero conoce las **cadenas** dentro de las
/// regiones (`"…"`, `'…'` y el *template literal* de un guion, con acentos
/// graves): dentro de una cadena un `/*` es un literal —una URL con barras de
/// directorio— y no abre nada, así que las cadenas se copian tal cual, saltando
/// sus escapes (`\"`). Sin eso, un `content: "/*";` se llevaría por delante el
/// resto de la página y **escondería** un incumplimiento de verdad.
///
/// Un `<!--` o un `/*` que **no** cierra se lleva el resto de la página: nadie
/// puede recuperar lo que está dentro de un comentario sin cerrar.
#[cfg(test)]
fn pagina_sin_comentarios(pagina: &[u8]) -> Vec<u8> {
    let mut salida = Vec::with_capacity(pagina.len());
    let mut i = 0;
    // La región en la que estamos: `None` fuera, `Some("style")` dentro de un
    // `<style>` y `Some("script")` dentro de un `<script>`.
    let mut region: Option<&'static str> = None;

    while i < pagina.len() {
        // El comentario HTML se limpia en toda la página.
        if pagina[i..].starts_with(b"<!--") {
            match busca(&pagina[i + 4..], b"-->") {
                Some(fin) => i += 4 + fin + 3,
                None => return salida,
            }
            continue;
        }

        match region {
            None => {
                // La etiqueta de apertura de una región se copia entera —el
                // escáner de etiquetas y el de movimiento la necesitan— y abre la
                // región.
                if let Some((false, nombre, fin)) = etiqueta_de_region(pagina, i) {
                    salida.extend_from_slice(&pagina[i..fin]);
                    region = Some(nombre);
                    i = fin;
                    continue;
                }

                salida.push(pagina[i]);
                i += 1;
            }
            Some(nombre) => {
                // Su etiqueta de cierre se copia entera y cierra la región.
                if let Some((true, cerrada, fin)) = etiqueta_de_region(pagina, i)
                    && cerrada == nombre
                {
                    salida.extend_from_slice(&pagina[i..fin]);
                    region = None;
                    i = fin;
                    continue;
                }

                // El bloque `/* … */`: vale en las dos regiones (CSS y JS).
                if pagina[i] == b'/' && pagina.get(i + 1) == Some(&b'*') {
                    match busca(&pagina[i + 2..], b"*/") {
                        Some(cierre) => i += 2 + cierre + 2,
                        None => return salida,
                    }
                    continue;
                }

                // La línea `// …`: solo en el guion.
                if nombre == "script" && pagina[i] == b'/' && pagina.get(i + 1) == Some(&b'/') {
                    let desde = i + 2;
                    match pagina[desde..].iter().position(|&byte| byte == b'\n') {
                        Some(fin) => i = desde + fin,
                        None => return salida,
                    }
                    continue;
                }

                // Las cadenas se copian tal cual, saltando sus escapes.
                if matches!(pagina[i], b'"' | b'\'' | b'`') {
                    let comilla = pagina[i];
                    salida.push(comilla);
                    i += 1;

                    while i < pagina.len() {
                        let byte = pagina[i];
                        salida.push(byte);
                        i += 1;

                        if byte == b'\\' {
                            if let Some(escapado) = pagina.get(i) {
                                salida.push(*escapado);
                                i += 1;
                            }
                        } else if byte == comilla {
                            break;
                        }
                    }
                    continue;
                }

                salida.push(pagina[i]);
                i += 1;
            }
        }
    }

    salida
}

/// Si en `pagina[i..]` empieza una etiqueta `<style>`/`<script>` —o su cierre—,
/// devuelve `(es_cierre, nombre, fin)`: el nombre normalizado a minúsculas y el
/// índice tras el `>` que cierra la etiqueta.
///
/// El nombre se compara **sin distinguir mayúsculas** —HTML no lo hace—, y solo
/// `style` y `script` son regiones: cualquier otra etiqueta devuelve `None`.
#[cfg(test)]
fn etiqueta_de_region(pagina: &[u8], i: usize) -> Option<(bool, &'static str, usize)> {
    if pagina.get(i) != Some(&b'<') {
        return None;
    }

    let mut j = i + 1;
    let cierre = pagina.get(j) == Some(&b'/');
    if cierre {
        j += 1;
    }

    let inicio = j;
    while pagina
        .get(j)
        .copied()
        .is_some_and(|byte| byte.is_ascii_alphabetic())
    {
        j += 1;
    }
    let nombre = &pagina[inicio..j];

    let nombre = if nombre.eq_ignore_ascii_case(b"script") {
        "script"
    } else if nombre.eq_ignore_ascii_case(b"style") {
        "style"
    } else {
        return None;
    };

    let fin = j + pagina[j..].iter().position(|&byte| byte == b'>')? + 1;

    Some((cierre, nombre, fin))
}

/// El token que puede provocar una petición, si lo hay.
///
/// La lista es **cerrada**, en el orden en que se declara: `@import` manda sobre
/// `url(`, porque una página que importa otra puede no llevar ningún `url(`; los
/// tokens de HTML y de JavaScript van después. Devolver el **primer** token que
/// incumple es lo que permite nombrarlo en el mensaje.
#[cfg(test)]
fn token_que_pide(pagina: &[u8]) -> Option<&'static str> {
    if contiene(pagina, "@import") {
        return Some("@import");
    }

    if tiene_url_externa(pagina) {
        return Some("url(");
    }

    if let Some(token) = etiqueta_que_pide(pagina) {
        return Some(token);
    }

    if contiene_ignore_case(pagina, "srcset") {
        return Some("srcset");
    }

    TOKENS_JS
        .iter()
        .copied()
        .find(|token| contiene(pagina, token))
}

/// Las etiquetas y atributos de HTML que pueden pedir algo por la red, con el
/// token con el que se nombran.
///
/// El destino de todas menos `<script src=…>` **puede** ser un dato incrustado
/// (`data:`) o una referencia interna (`#…`), y entonces no hay petición que
/// hacer. `<script src=…>` se prohíbe en cualquier forma: un guion de fuera no es
/// un dato ni una referencia.
#[cfg(test)]
const ETIQUETAS_QUE_PIDEN: [(&str, &str, &str); 10] = [
    ("script", "src", "<script src="),
    ("img", "src", "<img src="),
    ("source", "src", "<source src="),
    ("video", "src", "<video src="),
    ("audio", "src", "<audio src="),
    ("iframe", "src", "<iframe src="),
    ("embed", "src", "<embed src="),
    ("object", "data", "<object data="),
    ("form", "action", "<form action="),
    ("link", "href", "<link href="),
];

/// Los tokens de JavaScript que pueden pedir algo por la red, **sensibles a
/// mayúsculas** —son identificadores—.
#[cfg(test)]
const TOKENS_JS: [&str; 6] = [
    "fetch(",
    "XMLHttpRequest",
    "WebSocket",
    "EventSource",
    "sendBeacon",
    "new Image",
];

/// La primera etiqueta prohibida de la página, si la hay.
///
/// Recorre las etiquetas de apertura —nombre comparado **sin distinguir
/// mayúsculas**, que es lo que hace HTML— y pregunta, para cada una, si lleva el
/// atributo que pide algo por la red con un destino que no sea un dato incrustado
/// o una referencia interna.
#[cfg(test)]
fn etiqueta_que_pide(pagina: &[u8]) -> Option<&'static str> {
    let mut i = 0;

    while i < pagina.len() {
        if pagina[i] != b'<' {
            i += 1;
            continue;
        }

        // Etiqueta de cierre (`</…>`) o cualquier cosa que no sea una etiqueta:
        // el nombre se lee hasta el primer byte que no forme parte de él.
        let mut j = i + 1;
        if pagina.get(j) == Some(&b'/') {
            i = j + 1;
            continue;
        }

        let inicio_nombre = j;
        while pagina
            .get(j)
            .copied()
            .is_some_and(|byte| byte.is_ascii_alphanumeric())
        {
            j += 1;
        }
        let nombre = pagina[inicio_nombre..j].to_ascii_lowercase();

        let Some(fin_rel) = pagina[j..].iter().position(|&byte| byte == b'>') else {
            break;
        };
        let fin = j + fin_rel;
        let cuerpo = &pagina[j..fin];

        if let Some(token) = etiqueta_prohibida(&nombre, cuerpo) {
            return Some(token);
        }

        i = fin + 1;
    }

    None
}

/// ¿`cuerpo` es el de una etiqueta prohibida? `cuerpo` son los bytes de la
/// etiqueta **después** del nombre, hasta el `>`.
#[cfg(test)]
fn etiqueta_prohibida(nombre: &[u8], cuerpo: &[u8]) -> Option<&'static str> {
    for &(etiqueta, atributo, token) in ETIQUETAS_QUE_PIDEN.iter() {
        if nombre != etiqueta.as_bytes() {
            continue;
        }

        let valor = valor_de_atributo(cuerpo, atributo)?;

        // `<script src=…>` incumple en cualquier forma; en las demás, un dato
        // incrustado o una referencia interna no es una petición.
        if etiqueta == "script" || !es_destino_incrustado(&valor) {
            return Some(token);
        }

        return None;
    }

    None
}

/// El valor del atributo `atributo` en el cuerpo de una etiqueta, si lo lleva.
///
/// El nombre del atributo se busca **sin distinguir mayúsculas** y con frontera de
/// identificador por los dos lados —`data` no casa dentro de `metadata`—, se
/// saltan los espacios, se exige `=`, se saltan los espacios y se lee el valor,
/// entrecomillado con las dos comillas o desnudo hasta el espacio o el `>`.
#[cfg(test)]
fn valor_de_atributo(cuerpo: &[u8], atributo: &str) -> Option<Vec<u8>> {
    let aguja = atributo.as_bytes();
    let mut k = 0;

    while let Some(rel) = buscar_ignore_case(cuerpo.get(k..).unwrap_or_default(), aguja) {
        let inicio = k + rel;
        let tras = inicio + aguja.len();

        let ok_antes = inicio == 0 || !es_byte_de_atributo(cuerpo[inicio - 1]);
        let ok_despues = cuerpo
            .get(tras)
            .copied()
            .is_none_or(|byte| !es_byte_de_atributo(byte));

        if ok_antes && ok_despues {
            let mut c = tras;
            while cuerpo.get(c).copied().is_some_and(es_espacio) {
                c += 1;
            }

            if cuerpo.get(c) == Some(&b'=') {
                c += 1;
                while cuerpo.get(c).copied().is_some_and(es_espacio) {
                    c += 1;
                }

                return Some(match cuerpo.get(c).copied() {
                    Some(comilla @ (b'"' | b'\'')) => {
                        let desde = c + 1;
                        let mut hasta = desde;
                        while cuerpo
                            .get(hasta)
                            .copied()
                            .is_some_and(|byte| byte != comilla)
                        {
                            hasta += 1;
                        }
                        cuerpo[desde..hasta].to_vec()
                    }
                    _ => {
                        let desde = c;
                        let mut hasta = desde;
                        while cuerpo
                            .get(hasta)
                            .copied()
                            .is_some_and(|byte| !es_espacio(byte) && byte != b'>')
                        {
                            hasta += 1;
                        }
                        cuerpo[desde..hasta].to_vec()
                    }
                });
            }
        }

        k = inicio + 1;
    }

    None
}

/// ¿`valor` apunta al propio documento o a un dato incrustado? Es decir: tras
/// saltar los espacios, empieza por `#` —una referencia interna— o por `data:`
/// —un dato incrustado—. Ninguna de las dos es una petición.
#[cfg(test)]
fn es_destino_incrustado(valor: &[u8]) -> bool {
    let prefijo: Vec<u8> = valor
        .iter()
        .skip_while(|byte| byte.is_ascii_whitespace())
        .copied()
        .take(5)
        .collect();

    prefijo.starts_with(b"#") || prefijo.starts_with(b"data:")
}

/// ¿`byte` puede formar parte del nombre de un atributo de HTML?
#[cfg(test)]
fn es_byte_de_atributo(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_'
}

/// Las cuatro propiedades cuyo valor distinto de `none` **anima**, y por las
/// que la guarda pregunta.
///
/// `animation-name` y `transition-property` van **explícitas** por una razón de
/// fronteras de identificador: en `animation-name` la `-` es carácter de
/// identificador, así que buscar `animation` **no** casa ahí —la frontera por la
/// derecha lo impide, y hace bien—. Tratar las cuatro por su nombre es la única
/// forma de verlas todas.
#[cfg(test)]
const PROPIEDADES_QUE_ANIMAN: [&str; 4] = [
    "animation",
    "animation-name",
    "transition",
    "transition-property",
];

/// Las formas que **neutralizan** una animación (declarada con `animation` o
/// `animation-name`): `animation: none` o `animation-name: none`.
#[cfg(test)]
const NEUTRALIZA_ANIMACION: [&str; 2] = ["animation", "animation-name"];

/// Las formas que **neutralizan** una transición (declarada con `transition` o
/// `transition-property`): `transition: none` o `transition-property: none`.
#[cfg(test)]
const NEUTRALIZA_TRANSICION: [&str; 2] = ["transition", "transition-property"];

/// La aguja del bloque de movimiento reducido.
///
/// Es **literal y sensible a mayúsculas**, con el espacio después del `:`: las
/// páginas son del proyecto y van en minúsculas, así que el rigor es el mismo que
/// el del resto de la guarda. Por eso `@media (prefers-reduced-motion:reduce)`
/// —sin el espacio— o `@media (PREFERS-REDUCED-MOTION: reduce)` —en otra caja—
/// **no** son la aguja: a la guarda no le constan.
#[cfg(test)]
const AGUJA_REDUCE: &[u8] = b"@media (prefers-reduced-motion: reduce)";

/// El primer token que **anima sin poder apagarse**, si lo hay.
///
/// Anima el CSS cuando declara `animation`, `animation-name`, `transition` o
/// `transition-property` con un valor distinto de `none` sin neutralizarlo en el
/// bloque reduce, y anima el guion cuando usa `requestAnimationFrame` o
/// `setInterval` sin mencionar `prefers-reduced-motion`.
#[cfg(test)]
fn token_que_anima(pagina: &[u8]) -> Option<&'static str> {
    if let Some(token) = css_que_anima(pagina) {
        return Some(token);
    }

    guion_que_anima(pagina)
}

/// El primer token de animación del CSS que **no se puede apagar**, si lo hay.
///
/// Animar es declarar `animation`, `animation-name`, `transition` o
/// `transition-property` con un valor distinto de `none`: una declaración con
/// valor `none` **no** anima, es precisamente la forma de apagar. `@keyframes`
/// **no** dispara nada por sí solo: define fotogramas y no anima mientras ninguna
/// declaración los use, así que exigirle la media query a una página que solo trae
/// fotogramas sería medir algo distinto del contrato.
///
/// Una animación declarada queda neutralizada si el bloque [`AGUJA_REDUCE`]
/// existe, **no está vacío** y trae la forma que le corresponde —ver
/// [`bloque_neutraliza`]—. La neutralización es **por familia, no por
/// declaración**: ligar cada declaración con *su* selector daría falsos positivos
/// —rechazaría un `* { animation: none }` legítimo—, y ese límite está escrito en
/// [`bloque_neutraliza`]. Se devuelve el token de la **primera** declaración que
/// incumpla, para poder nombrarla en el mensaje.
#[cfg(test)]
fn css_que_anima(pagina: &[u8]) -> Option<&'static str> {
    let bloque = bloque_reduce(pagina).filter(|cuerpo| es_bloque_con_contenido(cuerpo));

    let mut desde = 0;
    while let Some((posicion, propiedad, valor)) = primera_declaracion(pagina, desde) {
        desde = posicion + propiedad.len();

        // Una declaración con valor `none` no anima.
        if valor == b"none" {
            continue;
        }

        if !bloque.is_some_and(|cuerpo| bloque_neutraliza(cuerpo, propiedad)) {
            return Some(propiedad);
        }
    }

    None
}

/// El token del guion que **anima sin mencionar** la preferencia, si lo hay.
///
/// Una página que anime con `requestAnimationFrame` o `setInterval` tiene que
/// **consultar** `prefers-reduced-motion` —por ejemplo,
/// `matchMedia('(prefers-reduced-motion: reduce)')`—. La guarda mide **la
/// mención y nada más**: un guion que menciona la preferencia y luego anima igual
/// **pasa**; comprobar que la rama es correcta exigiría ejecutar el guion en un
/// navegador. Ese límite está escrito, con la misma honestidad que el contraste
/// bajo del `<hr>`.
#[cfg(test)]
fn guion_que_anima(pagina: &[u8]) -> Option<&'static str> {
    let token = if contiene(pagina, "requestAnimationFrame") {
        "requestAnimationFrame"
    } else if contiene(pagina, "setInterval") {
        "setInterval"
    } else {
        return None;
    };

    if contiene(pagina, "prefers-reduced-motion") {
        None
    } else {
        Some(token)
    }
}

/// El cuerpo del bloque `@media (prefers-reduced-motion: reduce)`, si la página
/// lo trae.
///
/// La delimitación es **a mano**, contando llaves: el bloque va desde la llave de
/// apertura de la aguja hasta **su** cierre **equilibrado**, no hasta la primera
/// `}` que aparezca. Sin eso, una regla anidada con su propia llave
/// —`@media (prefers-reduced-motion: reduce) { main { animation: none; } }`—
/// cortaría el bloque en la llave del `main` y se perdería la neutralización. Un
/// bloque que **no cierra** se lleva el resto de la página: nadie puede saber dónde
/// acaba. No hay `regex` ni parser: el árbol compila sin red.
#[cfg(test)]
fn bloque_reduce(pagina: &[u8]) -> Option<&[u8]> {
    let mut cursor = busca(pagina, AGUJA_REDUCE)? + AGUJA_REDUCE.len();

    while pagina.get(cursor).copied().is_some_and(es_espacio) {
        cursor += 1;
    }

    if pagina.get(cursor) != Some(&b'{') {
        return None;
    }
    cursor += 1;

    let inicio = cursor;
    let mut profundidad = 1u32;

    while let Some(&byte) = pagina.get(cursor) {
        match byte {
            b'{' => profundidad += 1,
            b'}' => {
                profundidad -= 1;
                if profundidad == 0 {
                    return Some(&pagina[inicio..cursor]);
                }
            }
            _ => {}
        }
        cursor += 1;
    }

    Some(&pagina[inicio..])
}

/// ¿El cuerpo del bloque reduce tiene algo que **no** sea espacio en blanco?
///
/// Es la cláusula «no está vacío»: un bloque vacío —o cuyo único contenido sea un
/// comentario, que ya se quitó antes de escanear— no apaga nada, así que no cuenta
/// como bloque que neutraliza.
#[cfg(test)]
fn es_bloque_con_contenido(cuerpo: &[u8]) -> bool {
    cuerpo.iter().any(|&byte| !es_espacio(byte))
}

/// ¿El bloque reduce trae la forma canónica de neutralización de la **familia**
/// de la animación declarada con `propiedad`?
///
/// La lista es **cerrada**, por familias: una animación —declarada con
/// `animation` o `animation-name`— se neutraliza con `animation: none` o
/// `animation-name: none`; una transición —declarada con `transition` o
/// `transition-property`— con `transition: none` o `transition-property: none`.
/// **Cada familia por su forma**: `animation: none` **no** neutraliza una
/// transición. Fuera de la lista quedan a propósito `animation-duration: 0s`,
/// `animation-play-state: paused` y cualquier otra forma que también apague en un
/// navegador real: una guarda textual no puede demostrar equivalencia entre dos
/// formas, así que exige la canónica y rechaza de más —la dirección segura—.
///
/// # El límite, por escrito: la guarda es **por familia**, no por declaración
///
/// Esta función comprueba que el bloque reduce **trae** la forma canónica de la
/// familia, pero **no liga** cada declaración concreta con *su* neutralización:
/// no pregunta «¿esta `.caret` que anima tiene su propio `animation: none`?».
/// Ligarlas exigiría resolver el selector de cada regla —un parser de CSS—, y el
/// árbol compila sin red y no tiene `regex`; y, peor, **rechazaría
/// neutralizaciones legítimas** hechas con otro selector (un comodín `* {
/// animation: none }`, o una lista de selectores), que es peor que el hueco. El
/// precio, escrito: dos animaciones declaradas y **una sola** neutralizada pasan
/// la guarda (el escenario «dos animaciones declaradas y una sola neutralizada
/// pasan la guarda» de la delta lo fija, y el fixture
/// `dos_animaciones_declaradas_y_una_sola_neutralizada_pasan_la_guarda` lo
/// prueba). Es el mismo tipo de límite que el de `prefers-reduced-motion` en el
/// guion: la guarda ve el patrón, no su eficacia.
#[cfg(test)]
fn bloque_neutraliza(cuerpo: &[u8], propiedad: &str) -> bool {
    let formas = if propiedad.starts_with("animation") {
        NEUTRALIZA_ANIMACION
    } else {
        NEUTRALIZA_TRANSICION
    };

    formas
        .iter()
        .any(|&forma| declara_con_valor(cuerpo, forma, b"none"))
}

/// ¿`pagina` declara `propiedad` con un valor cuyo primer token sea `valor`?
///
/// Recorre **todas** las apariciones: una declaración anterior con otro valor no
/// puede tapar la que sí neutraliza.
#[cfg(test)]
fn declara_con_valor(pagina: &[u8], propiedad: &str, valor: &[u8]) -> bool {
    let mut desde = 0;

    while let Some((posicion, encontrado)) = primera_declaracion_de(pagina, propiedad, desde) {
        if encontrado == valor {
            return true;
        }
        desde = posicion + propiedad.len();
    }

    false
}

/// La primera declaración de una de las cuatro propiedades que animan a partir de
/// `desde`: la propiedad, dónde empieza y el primer token de su valor.
///
/// Se elige la que **antes** aparece en el documento, para poder reportar el
/// primer incumplimiento.
#[cfg(test)]
fn primera_declaracion(pagina: &[u8], desde: usize) -> Option<(usize, &'static str, &[u8])> {
    PROPIEDADES_QUE_ANIMAN
        .iter()
        .filter_map(|&propiedad| {
            let (posicion, valor) = primera_declaracion_de(pagina, propiedad, desde)?;
            Some((posicion, propiedad, valor))
        })
        .min_by_key(|(posicion, _, _)| *posicion)
}

/// La primera declaración de `propiedad` a partir de `desde`, con el primer token
/// de su valor.
#[cfg(test)]
fn primera_declaracion_de<'a>(
    pagina: &'a [u8],
    propiedad: &str,
    desde: usize,
) -> Option<(usize, &'a [u8])> {
    let posicion = posicion_declaracion(pagina, propiedad, desde)?;
    Some((
        posicion,
        primer_token(&pagina[posicion + propiedad.len()..]),
    ))
}

/// La posición de la primera aparición de `propiedad` como **declaración** a
/// partir de `desde`, si la hay: nombre con frontera de identificador por los dos
/// lados y seguido de `:`.
///
/// Las fronteras son lo que hace que `animation` **no** case dentro de
/// `animation-name` —la `-` es carácter de identificador— ni dentro de
/// `animation-duration`: son propiedades distintas y la guarda las trata por su
/// nombre.
#[cfg(test)]
fn posicion_declaracion(pagina: &[u8], propiedad: &str, desde: usize) -> Option<usize> {
    let aguja = propiedad.as_bytes();
    let mut cursor = desde;

    loop {
        let posicion = cursor + busca(pagina.get(cursor..)?, aguja)?;

        if (posicion == 0 || !es_byte_de_identificador(pagina[posicion - 1]))
            && !pagina
                .get(posicion + aguja.len())
                .copied()
                .is_some_and(es_byte_de_identificador)
        {
            let mut dos_puntos = posicion + aguja.len();
            while pagina.get(dos_puntos).copied().is_some_and(es_espacio) {
                dos_puntos += 1;
            }

            if pagina.get(dos_puntos) == Some(&b':') {
                return Some(posicion);
            }
        }

        cursor = posicion + 1;
    }
}

/// El primer token del valor de una declaración.
///
/// `resto` empieza justo tras el nombre de la propiedad: se saltan los espacios,
/// el `:`, los espacios, y se devuelven los bytes hasta el primer espacio, el `;`
/// o el `}`. Es lo que distingue `none` de lo que anima.
#[cfg(test)]
fn primer_token(resto: &[u8]) -> &[u8] {
    let mut cursor = 0;

    while resto.get(cursor).copied().is_some_and(es_espacio) {
        cursor += 1;
    }
    if resto.get(cursor) == Some(&b':') {
        cursor += 1;
    }
    while resto.get(cursor).copied().is_some_and(es_espacio) {
        cursor += 1;
    }

    let inicio = cursor;
    while resto
        .get(cursor)
        .copied()
        .is_some_and(|byte| !es_espacio(byte) && byte != b';' && byte != b'}')
    {
        cursor += 1;
    }

    &resto[inicio..cursor]
}

/// ¿`byte` puede formar parte de un identificador de CSS?
#[cfg(test)]
fn es_byte_de_identificador(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_'
}

/// ¿`byte` es espacio en blanco de CSS?
#[cfg(test)]
fn es_espacio(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r')
}

/// ¿La página trae algún `url(...)` cuyo destino **no** sea una referencia al
/// propio documento (`#…`) ni un dato incrustado (`data:…`)?
///
/// `url(#gradiente)` —un filtro o un degradado SVG— y `url(data:…)` —un dato
/// incrustado— no son peticiones y **no** cuentan. Cualquier otro destino sí: una
/// ruta relativa (`url(foto.png)`), una absoluta (`url(https://…)`) o una
/// entrecomillada.
///
/// Un `url()` vacío o un `url(` sin cerrar también cuentan como petición: son
/// páginas rotas, y rechazar de más es la dirección segura para un contrato que
/// existe para que la página no dependa de la red.
///
/// El token es `url(` **sin** espacio: la sintaxis de CSS no admite espacio entre
/// el nombre de la función y el paréntesis, así que `url (x)` no es un `url()`.
#[cfg(test)]
fn tiene_url_externa(pagina: &[u8]) -> bool {
    const AGUJA: &[u8] = b"url(";
    let mut desde = 0;

    while let Some(pos) = busca(pagina.get(desde..).unwrap_or_default(), AGUJA) {
        let inicio = desde + pos + AGUJA.len();
        let destino = pagina.get(inicio..).unwrap_or_default();

        if !es_destino_incrustado(destino) {
            return true;
        }

        desde = inicio;
    }

    false
}

/// ¿`pajar` contiene `aguja`? Decodifica con pérdida y compara como texto: para
/// agujas ASCII —las únicas que se buscan aquí— es lo mismo que comparar bytes.
/// El match es **sensible a mayúsculas**, como el de los tokens de JavaScript.
#[cfg(test)]
fn contiene(pajar: &[u8], aguja: &str) -> bool {
    String::from_utf8_lossy(pajar).contains(aguja)
}

/// ¿`pajar` contiene `aguja` sin distinguir mayúsculas? Es lo que pide la
/// detección de etiquetas y atributos de HTML, que no distingue cajas.
#[cfg(test)]
fn contiene_ignore_case(pajar: &[u8], aguja: &str) -> bool {
    buscar_ignore_case(pajar, aguja.as_bytes()).is_some()
}

/// La posición de la primera aparición de `aguja` en `pajar`, sin distinguir
/// mayúsculas.
#[cfg(test)]
fn buscar_ignore_case(pajar: &[u8], aguja: &[u8]) -> Option<usize> {
    if aguja.is_empty() || aguja.len() > pajar.len() {
        return None;
    }

    pajar
        .windows(aguja.len())
        .position(|ventana| ventana.eq_ignore_ascii_case(aguja))
}

/// Busca `aguja` en `pajar` y devuelve la posición de su primera aparición.
///
/// Es una copia local de la búsqueda de [`crate::render`] para las pruebas de este
/// módulo: el crate no tiene *lib target* y aquí se escanean páginas, no cuerpos
/// servidos, así que se mantiene la misma semántica sin depender del módulo de
/// producción para una función pura de bytes.
#[cfg(test)]
fn busca(pajar: &[u8], aguja: &[u8]) -> Option<usize> {
    if aguja.is_empty() || aguja.len() > pajar.len() {
        return None;
    }

    pajar
        .windows(aguja.len())
        .position(|ventana| ventana == aguja)
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_THEME_NAME, Incumplimiento, THEME_NAMES, THEMES, default_theme, incumplimientos,
        resolve, theme_by_name,
    };
    use std::fs;
    use std::path::PathBuf;

    /// Directorio de contenido del repositorio: la copia que la tabla embebida
    /// tiene que igualar byte a byte (D5, como en [`crate::embedded`]).
    fn content_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("content")
    }

    /// Los siete nombres, escritos **literalmente** para no depender de `THEMES`
    /// al comprobar que `THEMES` es el conjunto correcto.
    const NOMBRES: [&str; 7] = [
        "default", "minimal", "terminal", "paper", "amber", "compact", "matrix",
    ];

    /// Las **seis páginas sobrias**: comparten el marcado palabra por palabra y
    /// solo cambia su `<style>`. `matrix` queda fuera a propósito.
    const SOBRIAS: [&str; 6] = [
        "default", "minimal", "terminal", "paper", "amber", "compact",
    ];

    /// Un byte que puede formar parte de un identificador de CSS.
    fn es_de_identificador(byte: u8) -> bool {
        byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_'
    }

    fn es_espacio(byte: u8) -> bool {
        matches!(byte, b' ' | b'\t' | b'\n' | b'\r')
    }

    /// El bloque `<style>…</style>` de una página, o la página entera si no lo
    /// lleva (para los fixtures que son solo CSS).
    fn estilo(pagina: &[u8]) -> &[u8] {
        let Some(inicio) = super::busca(pagina, b"<style") else {
            return pagina;
        };
        let desde = &pagina[inicio..];
        let Some(abre) = desde.iter().position(|&byte| byte == b'>') else {
            return pagina;
        };
        let cuerpo = &desde[abre + 1..];
        let Some(cierra) = super::busca(cuerpo, b"</style>") else {
            return pagina;
        };

        &cuerpo[..cierra]
    }

    /// Quita los comentarios `/* … */` de un texto, para no confundir una palabra
    /// mencionada en la cabecera con una regla de verdad.
    fn sin_comentarios(css: &str) -> String {
        let mut salida = String::with_capacity(css.len());
        let mut resto = css;

        while let Some(inicio) = resto.find("/*") {
            salida.push_str(&resto[..inicio]);

            match resto[inicio + 2..].find("*/") {
                Some(fin) => resto = &resto[inicio + 2 + fin + 2..],
                // Comentario sin cerrar: se ignora lo que quede.
                None => return salida,
            }
        }

        salida.push_str(resto);

        salida
    }

    /// ¿El CSS declara una regla para ese selector? Vale una regla propia
    /// (`body {`) o una lista de selectores (`body, h1 {`).
    fn tiene_regla(css: &str, selector: &str) -> bool {
        let css = sin_comentarios(css);
        let bytes = css.as_bytes();

        for (inicio, _) in css.match_indices(selector) {
            // Frontera por la izquierda: no puede ser la cola de un identificador
            // más largo (`p` dentro de `padding`).
            if inicio > 0 && es_de_identificador(bytes[inicio - 1]) {
                continue;
            }

            let mut cursor = inicio + selector.len();

            // Frontera por la derecha: `h1` no vale si sigue `a` (`h1x`), pero sí
            // si sigue un espacio, una coma o la llave.
            if bytes.get(cursor).copied().is_some_and(es_de_identificador) {
                continue;
            }

            while bytes.get(cursor).copied().is_some_and(es_espacio) {
                cursor += 1;
            }

            if matches!(bytes.get(cursor), Some(b'{') | Some(b',')) {
                return true;
            }
        }

        false
    }

    /// ¿La página tiene algún incumplimiento de **movimiento**? Es la pregunta de
    /// casi todas las pruebas de ese contrato.
    fn incumple_movimiento(pagina: &str) -> bool {
        incumplimientos(pagina.as_bytes())
            .iter()
            .any(|i| matches!(i, Incumplimiento::Movimiento(_)))
    }

    /// ¿La página tiene algún incumplimiento de **red**?
    fn incumple_red(pagina: &str) -> bool {
        incumplimientos(pagina.as_bytes())
            .iter()
            .any(|i| matches!(i, Incumplimiento::Peticion(_)))
    }

    // --- Tabla y resolución de temas ---------------------------------------

    #[test]
    fn la_tabla_declara_los_siete_temas() {
        assert_eq!(
            THEMES.len(),
            NOMBRES.len(),
            "`THEMES` tiene que declarar los siete temas incorporados"
        );
        assert_eq!(
            THEMES[0].name, DEFAULT_THEME_NAME,
            "`THEMES[0]` tiene que ser el tema por defecto"
        );

        for (theme, nombre) in THEMES.iter().zip(NOMBRES) {
            assert_eq!(
                theme.name, nombre,
                "el orden de `THEMES` tiene que ser el de la lista de nombres"
            );
        }

        // `THEME_NAMES` enumera los siete y **solo** los siete: sin nombres de más
        // —una lista que anunciase uno que no existe— ni de menos.
        let enumerados: Vec<&str> = THEME_NAMES.split(',').map(str::trim).collect();
        assert_eq!(
            enumerados, NOMBRES,
            "`THEME_NAMES` tiene que enumerar los siete nombres: {THEME_NAMES}"
        );
    }

    #[test]
    fn theme_by_name_solo_acepta_los_siete() {
        for nombre in NOMBRES {
            assert!(
                theme_by_name(nombre).is_some(),
                "`theme_by_name` tiene que encontrar el tema `{nombre}`"
            );
        }

        // Comparación exacta: la cadena vacía —que es una variable presente pero
        // inválida, no una ausente— y cualquier nombre desconocido o con otra
        // caja caen a `None`.
        for desconocido in [
            "",
            "neon",
            "Default",
            "TERMINAL",
            "Themes/default",
            "default ",
        ] {
            assert!(
                theme_by_name(desconocido).is_none(),
                "`theme_by_name({desconocido:?})` tiene que ser `None`: la comparación es exacta"
            );
        }
    }

    #[test]
    fn resolve_el_conocido_gana_y_lo_demas_cae_al_configurado() {
        let configurado = theme_by_name("minimal").expect("`minimal` es uno de los siete");

        assert_eq!(
            resolve(Some("terminal"), configurado).name,
            "terminal",
            "un `X-Theme` conocido tiene que ganar al tema configurado"
        );
        assert_eq!(
            resolve(Some("neon"), configurado).name,
            "minimal",
            "un `X-Theme` desconocido cae al configurado, no a otro tema"
        );
        assert_eq!(
            resolve(Some(""), configurado).name,
            "minimal",
            "un `X-Theme` vacío no es un tema: cae al configurado"
        );
        assert_eq!(
            resolve(None, configurado).name,
            "minimal",
            "sin `X-Theme` el tema es el configurado"
        );
        assert_eq!(
            resolve(None, default_theme()).name,
            DEFAULT_THEME_NAME,
            "sin `X-Theme` y con el defecto configurado, el tema es `default`"
        );
    }

    // --- El nombre es la ruta, y las copias no divergen --------------------

    #[test]
    fn cada_nombre_deriva_su_ruta_y_tiene_su_fichero() {
        // D1: la ruta se **deriva** del nombre (`<nombre>.html`, en la raíz de
        // `CONTENT_DIR`), y cada tema tiene su fichero en `content/` con ese
        // tallo. Una tabla de rutas podría divergir del nombre; derivada, no.
        for theme in THEMES.iter() {
            let esperada = format!("{}.html", theme.name);
            assert_eq!(
                theme.page_path(),
                esperada,
                "la ruta de la página del tema `{}` tiene que derivarse de su nombre",
                theme.name
            );

            let fichero = content_dir().join(&esperada);
            assert!(
                fichero.is_file(),
                "el tema `{}` tiene que tener su página en {}",
                theme.name,
                fichero.display()
            );
        }
    }

    #[test]
    fn las_paginas_embebidas_son_byte_a_byte_las_de_content() {
        // D5: los bytes embebidos de cada tema igualan la lectura de
        // `content/<nombre>.html`. Detecta una copia embebida obsoleta o distinta.
        for theme in THEMES.iter() {
            let ruta = content_dir().join(theme.page_path());
            let del_disco = match fs::read(&ruta) {
                Ok(bytes) => bytes,
                Err(error) => panic!(
                    "no se pudo leer {} (tema `{}`): {error}",
                    ruta.display(),
                    theme.name
                ),
            };

            assert_eq!(
                theme.embedded.as_bytes(),
                del_disco.as_slice(),
                "la página embebida del tema `{}` no coincide byte a byte con {}",
                theme.name,
                ruta.display()
            );
        }
    }

    // --- `Theme::page`: legible, ausente e ilegible ------------------------

    /// Un directorio temporal y **vacío**, único por prueba. Se limpia antes por
    /// si quedó algo de una ejecución anterior.
    fn dir_temporal(nombre: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("errserve-theme-{}-{nombre}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("el directorio temporal tiene que poder crearse");

        dir
    }

    #[test]
    fn la_pagina_legible_del_disco_gana_a_la_embebida() {
        let dir = dir_temporal("legible");
        let propia = b"<p>pagina propia del operador, distinguible</p>".to_vec();
        fs::write(dir.join("matrix.html"), &propia).expect("el fixture tiene que poder escribirse");

        let theme = theme_by_name("matrix").expect("`matrix` es uno de los siete");
        assert_eq!(
            theme
                .page(&dir)
                .expect("la página legible del disco tiene que ganar"),
            propia,
            "un fichero legible en CONTENT_DIR/<nombre>.html tiene que ganar a la copia embebida"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn la_pagina_ausente_cae_a_la_embebida_sin_warn() {
        let dir = dir_temporal("ausente");

        for theme in THEMES.iter() {
            let pagina = theme
                .page(&dir)
                .expect("un fichero ausente no es un fallo: cae a la copia embebida");
            assert_eq!(
                pagina.as_slice(),
                theme.embedded.as_bytes(),
                "sin fichero en el disco, el tema `{}` tiene que servir su copia embebida",
                theme.name
            );
        }

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn la_pagina_presente_pero_ilegible_es_un_error_de_e_s() {
        let dir = dir_temporal("ilegible");
        // Un **directorio** donde el tema espera su fichero: `std::fs::read` de un
        // directorio falla en Linux (EISDIR), un error de lectura real y
        // reproducible.
        fs::create_dir(dir.join("matrix.html")).expect("el fixture tiene que poder crearse");

        let theme = theme_by_name("matrix").expect("`matrix` es uno de los siete");
        assert!(
            theme.page(&dir).is_err(),
            "una página del operador que existe y no se puede leer tiene que ser un error de E/S, \
             no la copia embebida"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    // --- Contrato 3: cobertura de marcado de las sobrias -------------------

    #[test]
    fn cada_pagina_sobria_cubre_el_marcado() {
        // Sin esto, un tema que se olvide de un elemento deja una página a medio
        // vestir y nadie se entera: las etiquetas son las que lleva el marcado de
        // las seis sobrias. `main` **no** está en la lista: es estructura y no
        // todos los temas necesitan una regla para él, mientras que `footer` lo
        // estilan las seis. `matrix` queda **fuera**: es un documento propio.
        let mut fallos = Vec::new();

        for nombre in SOBRIAS {
            let theme = theme_by_name(nombre).expect("es uno de los siete");
            let css = String::from_utf8_lossy(estilo(theme.embedded.as_bytes())).into_owned();

            for selector in ["body", "h1", "h2", "hr", "p", "footer"] {
                if !tiene_regla(&css, selector) {
                    fallos.push(format!(
                        "la página del tema `{nombre}` no estila `{selector}`"
                    ));
                }
            }
        }

        assert!(
            fallos.is_empty(),
            "cada página sobria tiene que cubrir el marcado ({} incumplimiento(s)):\n- {}",
            fallos.len(),
            fallos.join("\n- ")
        );
    }

    #[test]
    fn matrix_queda_fuera_de_la_cobertura_de_marcado() {
        // Escrito a propósito: `matrix` es un **documento propio** —canvas,
        // glitch, terminal, minijuego— y NO se le exige el marcado de las sobrias
        // (ni `h2` ni `hr`). Esta prueba deja constancia de que su exclusión es
        // una decisión, no un olvido: si algún día se le exigiera, se caería aquí.
        let matrix = theme_by_name("matrix").expect("`matrix` es uno de los siete");
        assert!(
            !SOBRIAS.contains(&matrix.name),
            "`matrix` no puede estar en la lista de las seis sobrias"
        );
        assert_eq!(matrix.page_path(), "matrix.html");
    }

    // --- Contrato 1: ninguna página pide nada por la red -------------------

    #[test]
    fn ninguna_pagina_puede_hacer_una_peticion() {
        // El primer contrato, sobre las **siete páginas incorporadas**: ninguna
        // puede hacer que el navegador **pida** algo por la red. El motivo es el
        // del requisito «Ninguna página pide nada por la red»: la página se sirve
        // en el host de la aplicación, donde este servicio no sirve assets, así
        // que ese pedido no lo puede resolver nadie.
        let mut fallos = Vec::new();

        for theme in THEMES.iter() {
            for incumplimiento in incumplimientos(theme.embedded.as_bytes()) {
                if let Incumplimiento::Peticion(token) = incumplimiento {
                    fallos.push(format!(
                        "la página del tema `{}` contiene `{token}`: el navegador tendría que \
                         pedir algo por la red",
                        theme.name
                    ));
                }
            }
        }

        assert!(
            fallos.is_empty(),
            "ninguna página puede hacer que el navegador pida algo por la red ({} \
             incumplimiento(s)):\n- {}",
            fallos.len(),
            fallos.join("\n- ")
        );
    }

    /// Una página autocontenida: su `<style>` incrustado, su `<canvas>` y su
    /// guion, sin ningún token de la lista cerrada. No pide nada por la red.
    const AUTOCONTENIDA: &str = r#"
<style>.caja { fill: url(#gradiente); }</style>
<canvas id="lluvia"></canvas>
<script>var ctx = document.querySelector('canvas').getContext('2d'); ctx.fillRect(0,0,1,1);</script>
"#;

    /// Una página con un dato incrustado y una referencia interna: ninguna de las
    /// dos es una petición.
    const CON_DATOS: &str = r#"
<style>body { background: url(data:image/png;base64,AAAA); } .a { fill: url(#gradiente); }</style>
<img src="data:image/png;base64,AAAA" alt="" />
"#;

    /// El mismo contrato, pero **en prosa**: la página lo dice en su cabecera y no
    /// lo hace en ninguna regla. Un comentario no pide nada.
    const COMENTADA: &str = r#"
<!-- Esta página no usa `fetch(` ni `<script src=`. -->
<style>body { color: #222; }</style>
"#;

    /// Un comentario que abre y no cierra: lo que queda detrás es comentario, así
    /// que no hay ninguna regla que pueda incumplir nada.
    const COMENTARIO_ABIERTO: &str = r#"
<!-- sin cerrar: `fetch(` y `<script src=` aquí dentro ya no son etiquetas
<style>body { color: #222; }</style>
"#;

    /// Una cadena que **contiene** `/*`: es válido —un literal, o una URL con
    /// barras de directorio—, no abre ningún comentario, y por tanto no puede
    /// tapar la petición que venga detrás.
    const CADENA_CON_BARRAS: &str = r#"
<style>a::before { content: "/*"; } body { background: url(foto.png); }</style>
"#;

    #[test]
    fn una_pagina_autocontenida_no_hace_ninguna_peticion() {
        assert!(
            incumplimientos(AUTOCONTENIDA.as_bytes()).is_empty(),
            "una página con su estilo, su canvas y su guion, sin tokens de la lista cerrada, \
             tiene que cumplir. Página: {AUTOCONTENIDA:?}"
        );
    }

    #[test]
    fn una_referencia_interna_y_un_dato_incrustado_no_son_una_peticion() {
        assert!(
            incumplimientos(CON_DATOS.as_bytes()).is_empty(),
            "`url(#…)`, `url(data:…)` y `<img src=\"data:…\">` no son peticiones: no hay nada que \
             el navegador tenga que pedir. Página: {CON_DATOS:?}"
        );
    }

    #[test]
    fn una_peticion_en_el_guion_incumple() {
        for (pagina, token) in [
            (r#"<script>fetch('/api');</script>"#, "fetch("),
            (
                r#"<script>var x = new XMLHttpRequest();</script>"#,
                "XMLHttpRequest",
            ),
            (
                r#"<script>var ws = new WebSocket('wss://ejemplo');</script>"#,
                "WebSocket",
            ),
            (
                r#"<script>new EventSource('/sse');</script>"#,
                "EventSource",
            ),
            (
                r#"<script>navigator.sendBeacon('/x', 'y');</script>"#,
                "sendBeacon",
            ),
            (
                r#"<script>var i = new Image(); i.src = '/x.png';</script>"#,
                "new Image",
            ),
        ] {
            assert!(
                incumplimientos(pagina.as_bytes()).contains(&Incumplimiento::Peticion(token)),
                "esta página pide algo con `{token}` y tiene que incumplirlo **nombrando ese \
                 token**: {pagina:?}"
            );
        }
    }

    #[test]
    fn un_recurso_externo_en_el_marcado_incumple() {
        for (pagina, token) in [
            (
                r#"<script src="https://ejemplo/x.js"></script>"#,
                "<script src=",
            ),
            (r#"<img src="/foto.png" alt="" />"#, "<img src="),
            (
                r#"<link rel="stylesheet" href="https://ejemplo/x.css" />"#,
                "<link href=",
            ),
            (r#"<iframe src="/marco"></iframe>"#, "<iframe src="),
            (r#"<source src="/v.mp4" />"#, "<source src="),
            (r#"<video src="/v.mp4"></video>"#, "<video src="),
            (r#"<audio src="/a.mp3"></audio>"#, "<audio src="),
            (r#"<embed src="/x" />"#, "<embed src="),
            (r#"<object data="/x"></object>"#, "<object data="),
            (r#"<form action="/enviar"></form>"#, "<form action="),
            (r#"<img srcset="/x1.png 1x" alt="" />"#, "srcset"),
        ] {
            assert!(
                incumplimientos(pagina.as_bytes()).contains(&Incumplimiento::Peticion(token)),
                "esta página carga algo de fuera con `{token}` y tiene que incumplirlo: {pagina:?}"
            );
        }
    }

    #[test]
    fn un_url_externo_en_un_estilo_incrustado_incumple() {
        assert!(
            incumplimientos(br#"<style>body { background: url(fondo.png); }</style>"#)
                .contains(&Incumplimiento::Peticion("url(")),
            "un `url(fondo.png)` en un `<style>` incrustado pide un fichero y tiene que incumplir"
        );
        assert!(
            incumplimientos(br#"<style>@import "otra.css";</style>"#)
                .contains(&Incumplimiento::Peticion("@import")),
            "un `@import` —en su forma sin `url(`— carga otra hoja y tiene que incumplir"
        );
    }

    #[test]
    fn un_comentario_no_es_una_peticion() {
        assert!(
            incumplimientos(COMENTADA.as_bytes()).is_empty(),
            "un `fetch(` o un `<script src=` dentro de un comentario no son el contrato: un \
             comentario no puede hacer que el navegador pida nada. Página: {COMENTADA:?}"
        );
        assert!(
            incumplimientos(COMENTARIO_ABIERTO.as_bytes()).is_empty(),
            "un comentario que abre y no cierra llega hasta el final de la página: lo que queda \
             detrás es prosa, no etiquetas. Página: {COMENTARIO_ABIERTO:?}"
        );
    }

    #[test]
    fn una_etiqueta_en_mayusculas_incumple() {
        // HTML no distingue mayúsculas en el nombre de la etiqueta ni en el
        // atributo, así que la detección tampoco.
        assert!(
            incumplimientos(br#"<SCRIPT SRC="https://ejemplo/x.js"></SCRIPT>"#)
                .contains(&Incumplimiento::Peticion("<script src=")),
            "`<SCRIPT SRC=…>` tiene que incumplir: HTML no distingue mayúsculas"
        );
    }

    #[test]
    fn una_cadena_con_barras_no_abre_un_comentario() {
        assert!(
            incumplimientos(CADENA_CON_BARRAS.as_bytes())
                .contains(&Incumplimiento::Peticion("url(")),
            "un `/*` dentro de una cadena es válido, no un comentario: no puede tapar el \
             `url(foto.png)` que viene detrás. Página: {CADENA_CON_BARRAS:?}"
        );
    }

    #[test]
    fn una_barra_doble_en_prosa_html_no_tapa_una_peticion() {
        // El `//` solo es comentario **dentro de un guion**. Fuera, en prosa HTML,
        // es **texto visible** —`content/matrix.html` lo usa así, en su
        // `<p class="boot">// errserve — terminal de errores</p>`—, así que no
        // puede comerse el resto de la línea y tapar la petición que venga detrás.
        // Es el caso adversario del limpiador por regiones: rechazar de más.
        assert!(
            incumplimientos(br#"<p>// nota visible</p><img src="/foto.png" alt="" />"#)
                .contains(&Incumplimiento::Peticion("<img src=")),
            "un `//` en prosa HTML no puede tapar el `<img src=` que viene detrás en la misma línea"
        );

        // Y dentro de un guion sí sigue siendo comentario: lo que hay detrás en su
        // línea no cuenta como regla.
        assert!(
            incumplimientos(br#"<script>var x = 1; // fetch('/tapado')</script>"#).is_empty(),
            "un `//` dentro de un guion sí es comentario: su `fetch(` no cuenta"
        );
    }

    // --- Contrato 2: una página que anima puede apagarse -------------------

    /// Una página que anima y **sabe apagarse**.
    const ANIMADA_CON_FRENO: &str = r#"
<style>@keyframes parpadeo { from { opacity: 1 } to { opacity: 0 } }
body { animation: parpadeo 2s infinite; }
@media (prefers-reduced-motion: reduce) { body { animation: none; } }</style>
"#;

    /// Una página que anima y **no** sabe apagarse.
    const ANIMADA_SIN_FRENO: &str = r#"
<style>body { transition: opacity 0.2s; }</style>
"#;

    /// Una página quieta: no pide nada y no anima.
    const QUIETA: &str = r#"
<style>body { color: #222; }</style>
"#;

    /// Una página que anima y **solo nombra** la media query: el bloque reduce
    /// existe, pero su único contenido es un comentario, así que no apaga nada.
    const NOMBRA_SIN_NEUTRALIZAR: &str = r#"
<style>main { animation: entrada 180ms ease-out; }
@media (prefers-reduced-motion: reduce) { /* esta animación se apaga sola */ }</style>
"#;

    /// Una página que anima y neutraliza bajo la preferencia **contraria**: el
    /// bloque existe, pero es el que ve quien **no** pide movimiento reducido.
    const NEUTRALIZA_BAJO_LA_OTRA_PREFERENCIA: &str = r#"
<style>main { animation: entrada 180ms ease-out; }
@media (prefers-reduced-motion: no-preference) { main { animation: none; } }</style>
"#;

    /// Una página que anima y trae el bloque reduce **vacío**.
    const REDUCE_VACIO: &str = r#"
<style>main { animation: entrada 180ms ease-out; }
@media (prefers-reduced-motion: reduce) { }</style>
"#;

    /// La forma canónica: fotogramas, la `animation` declarada y el bloque reduce
    /// que la **neutraliza**.
    const REDUCE_QUE_NEUTRALIZA: &str = r#"
<style>@keyframes entrada { from { opacity: 0 } to { opacity: 1 } }
main { animation: entrada 180ms ease-out; }
@media (prefers-reduced-motion: reduce) { main { animation: none; } }</style>
"#;

    /// Una página que declara una `transition` y cuyo bloque reduce solo
    /// neutraliza una `animation`: la transición declarada sigue viva.
    const REDUCE_QUE_NEUTRALIZA_OTRA: &str = r#"
<style>main { transition: opacity 200ms; }
@media (prefers-reduced-motion: reduce) { main { animation: none; } }</style>
"#;

    /// Unos fotogramas **sin ninguna declaración** que los use.
    const SOLO_FOTOGRAMAS: &str = r#"
<style>@keyframes entrada { from { opacity: 0 } to { opacity: 1 } }</style>
"#;

    /// La neutralización vive en una **regla anidada** del bloque reduce y **no**
    /// es la primera que se cierra.
    const REDUCE_CON_REGLA_ANIDADA: &str = r#"
<style>main { animation: entrada 180ms ease-out; }
@media (prefers-reduced-motion: reduce) {
  main { color: inherit; }
  main { animation: none; }
}</style>
"#;

    /// La forma canónica para una transición.
    const TRANSICION_CON_FRENO: &str = r#"
<style>.pastilla { transition: transform 120ms ease; }
@media (prefers-reduced-motion: reduce) { .pastilla { transition: none; } }</style>
"#;

    /// Una página cuya **única** declaración que anima está **dentro** del bloque
    /// reduce: el bloque reduce tiene que **apagar**, no animar, así que no
    /// neutraliza nada y la página incumple. Es el caso más cercano al hueco por
    /// abajo —una declaración que anima sin su forma de apagado—.
    const REDUCE_QUE_ANIMA: &str = r#"
<style>@media (prefers-reduced-motion: reduce) { main { animation: entrada 180ms ease-out; } }</style>
"#;

    /// El **límite declarado** de la neutralización **por familia**: dos
    /// animaciones declaradas y **solo una** neutralizada en el bloque reduce. La
    /// guarda comprueba que el bloque trae la forma canónica de la familia, **no**
    /// que ligue cada declaración con *su* neutralización —ligarlas exigiría un
    /// parser de selectores y rechazaría neutralizaciones legítimas como
    /// `* { animation: none }`—. Pasa, y está escrito (escenario «dos animaciones
    /// declaradas y una sola neutralizada pasan la guarda» de la delta).
    const DOS_ANIMACIONES_UNA_NEUTRALIZADA: &str = r#"
<style>.glitch { animation: glitch-x 1.6s infinite; }
.caret { animation: parpadeo 1s infinite; }
@media (prefers-reduced-motion: reduce) { .glitch { animation: none; } }</style>
"#;

    #[test]
    fn una_pagina_quieta_cumple_los_dos_contratos() {
        assert!(
            incumplimientos(QUIETA.as_bytes()).is_empty(),
            "una página que no pide nada y no anima tiene que cumplir: {QUIETA:?}"
        );
    }

    #[test]
    fn una_pagina_que_anima_solo_cumple_si_se_puede_apagar() {
        assert!(
            incumple_movimiento(ANIMADA_SIN_FRENO),
            "una página que anima y no declara `prefers-reduced-motion` tiene que incumplir: \
             {ANIMADA_SIN_FRENO:?}"
        );
        assert!(
            !incumple_movimiento(ANIMADA_CON_FRENO),
            "una página que anima **y** neutraliza dentro del bloque reduce tiene que cumplir: \
             {ANIMADA_CON_FRENO:?}"
        );
        assert!(
            !incumple_movimiento(QUIETA),
            "una página sin animación tiene que cumplir sin declarar nada: {QUIETA:?}"
        );
    }

    #[test]
    fn un_bloque_reduce_vacio_no_basta() {
        assert!(
            incumple_movimiento(REDUCE_VACIO),
            "el bloque `@media (prefers-reduced-motion: reduce)` tiene que **tener** la \
             neutralización: uno vacío no apaga nada. Página: {REDUCE_VACIO:?}"
        );
    }

    #[test]
    fn un_bloque_reduce_que_neutraliza_la_animacion_cumple() {
        assert!(
            !incumple_movimiento(REDUCE_QUE_NEUTRALIZA),
            "una página que anima y neutraliza su animación dentro del bloque reduce tiene que \
             cumplir: {REDUCE_QUE_NEUTRALIZA:?}"
        );
    }

    #[test]
    fn una_transicion_neutralizada_con_su_forma_cumple() {
        assert!(
            !incumple_movimiento(TRANSICION_CON_FRENO),
            "una transición neutralizada con `transition: none` tiene que cumplir: \
             {TRANSICION_CON_FRENO:?}"
        );
    }

    #[test]
    fn una_pagina_que_solo_nombra_prefers_reduced_motion_incumple() {
        assert!(
            incumple_movimiento(NOMBRA_SIN_NEUTRALIZAR),
            "nombrar `prefers-reduced-motion` no neutraliza nada: el bloque reduce trae solo un \
             comentario. Página: {NOMBRA_SIN_NEUTRALIZAR:?}"
        );
    }

    #[test]
    fn una_media_query_que_no_es_la_de_movimiento_reducido_no_basta() {
        assert!(
            incumple_movimiento(NEUTRALIZA_BAJO_LA_OTRA_PREFERENCIA),
            "neutralizar bajo `prefers-reduced-motion: no-preference` no es la puerta que el \
             usuario cierra. Página: {NEUTRALIZA_BAJO_LA_OTRA_PREFERENCIA:?}"
        );
    }

    #[test]
    fn neutralizar_otra_animacion_distinta_de_la_declarada_no_basta() {
        assert!(
            incumple_movimiento(REDUCE_QUE_NEUTRALIZA_OTRA),
            "la página declara una `transition` y el bloque reduce solo trae `animation: none`: \
             la transición declarada sigue sin neutralizarse. Página: {REDUCE_QUE_NEUTRALIZA_OTRA:?}"
        );
    }

    #[test]
    fn unos_fotogramas_sin_declaracion_que_los_use_no_obligan() {
        assert!(
            !incumple_movimiento(SOLO_FOTOGRAMAS),
            "una página cuyo único contenido son `@keyframes` —sin ninguna declaración de \
             `animation` ni de `transition`— tiene que cumplir sin necesitar el bloque reduce, \
             porque definir fotogramas no anima nada. Página: {SOLO_FOTOGRAMAS:?}"
        );
    }

    #[test]
    fn una_regla_anidada_dentro_del_bloque_reduce_cuenta() {
        assert!(
            !incumple_movimiento(REDUCE_CON_REGLA_ANIDADA),
            "la neutralización dentro de una regla anidada del bloque reduce tiene que contar, \
             aunque no sea la primera regla que se cierra: {REDUCE_CON_REGLA_ANIDADA:?}"
        );
    }

    #[test]
    fn una_animacion_declarada_dentro_del_bloque_reduce_no_se_neutraliza_sola() {
        // El bloque reduce tiene que **apagar**, no animar: la única declaración
        // que anima está dentro del propio bloque, así que no hay ninguna forma
        // canónica que la neutralice y la página tiene que incumplir. Es el caso
        // más cercano al hueco que la guarda **sí** caza.
        assert!(
            incumple_movimiento(REDUCE_QUE_ANIMA),
            "una declaración que anima **dentro** del bloque reduce no se neutraliza sola, así \
             que la página tiene que incumplir. Página: {REDUCE_QUE_ANIMA:?}"
        );
    }

    #[test]
    fn dos_animaciones_declaradas_y_una_sola_neutralizada_pasan_la_guarda() {
        // El **límite declarado** de la neutralización por familia: con la forma
        // canónica de la familia presente en el bloque reduce, la página **pasa**
        // aunque `.caret` no quede ligada a su propia neutralización. La guarda es
        // por familia, no por declaración —ligarlas exigiría un parser de
        // selectores y rechazaría neutralizaciones legítimas—, y este caso lo deja
        // por escrito, con el mismo criterio que el del guion que menciona la
        // preferencia y anima igual.
        assert!(
            !incumple_movimiento(DOS_ANIMACIONES_UNA_NEUTRALIZADA),
            "la guarda es **por familia**, no por declaración: con `animation: none` en el bloque \
             reduce, la página pasa aunque `.caret` no quede ligada. Página: \
             {DOS_ANIMACIONES_UNA_NEUTRALIZADA:?}"
        );
    }

    #[test]
    fn un_guion_que_anima_sin_mencionar_la_preferencia_incumple() {
        assert!(
            incumple_movimiento(
                r#"<script>requestAnimationFrame(function f(){ requestAnimationFrame(f); });</script>"#
            ),
            "un guion que anima con `requestAnimationFrame` y no menciona `prefers-reduced-motion` \
             tiene que incumplir"
        );
        assert!(
            incumple_movimiento(r#"<script>setInterval(function () {}, 1000);</script>"#),
            "un guion que anima con `setInterval` y no menciona la preferencia tiene que incumplir"
        );
        assert!(
            !incumple_movimiento(
                r#"<script>var r = matchMedia('(prefers-reduced-motion: reduce)'); if (!r.matches) requestAnimationFrame(f);</script>"#
            ),
            "un guion que anima **y** menciona `prefers-reduced-motion` tiene que cumplir"
        );
    }

    #[test]
    fn un_guion_que_menciona_la_preferencia_y_anima_igual_pasa_la_guarda() {
        // El límite declarado, probado a propósito: la guarda mide la **mención**,
        // no la eficacia de la rama. Un guion que menciona la preferencia y luego
        // arranca la animación sin comprobarla **pasa**; comprobar que la rama es
        // correcta exigiría ejecutar el guion en un navegador. Es una decisión
        // medida y declarada, no un descuido.
        assert!(
            !incumple_movimiento(
                r#"<script>var r = matchMedia('(prefers-reduced-motion: reduce)'); requestAnimationFrame(function f(){ requestAnimationFrame(f); });</script>"#
            ),
            "un guion que menciona `prefers-reduced-motion` y luego anima igual tiene que **pasar** \
             la guarda: mide la mención, no la eficacia de la rama"
        );
    }

    #[test]
    fn un_comentario_no_anima() {
        // Una página que mencione `animation` o `requestAnimationFrame` **solo
        // dentro de un comentario** no tiene que neutralizar nada, porque un
        // comentario no anima.
        assert!(
            incumplimientos(
                br#"<!-- esta pagina no usa animation ni requestAnimationFrame -->
<style>body { color: #222; }</style>"#
            )
            .is_empty(),
            "mencionar `animation` o `requestAnimationFrame` en un comentario no obliga a nada"
        );
    }

    #[test]
    fn ninguna_pagina_que_anima_deja_de_poder_apagarse() {
        // El barrido de las siete páginas incorporadas: el día que una anime sin
        // neutralizar, esta guarda es la que lo exige.
        let mut fallos = Vec::new();

        for theme in THEMES.iter() {
            for incumplimiento in incumplimientos(theme.embedded.as_bytes()) {
                if let Incumplimiento::Movimiento(token) = incumplimiento {
                    fallos.push(format!(
                        "la página del tema `{}` anima con `{token}` y no se puede apagar",
                        theme.name
                    ));
                }
            }
        }

        assert!(
            fallos.is_empty(),
            "una página que anima tiene que poder apagarse ({} incumplimiento(s)):\n- {}",
            fallos.len(),
            fallos.join("\n- ")
        );
    }

    #[test]
    fn incumplimientos_distingue_red_de_movimiento() {
        // La función devuelve las dos familias por separado: una página solo de
        // red no declara movimiento, y una solo de movimiento no pide nada.
        assert!(incumple_red(r#"<img src="/x.png" alt="" />"#));
        assert!(!incumple_movimiento(r#"<img src="/x.png" alt="" />"#));
        assert!(incumple_movimiento(ANIMADA_SIN_FRENO));
        assert!(!incumple_red(ANIMADA_SIN_FRENO));
    }
}
