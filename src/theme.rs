//! Temas de las páginas de error: nombre, hoja y bytes embebidos.
//!
//! Un **tema** es **una sola hoja de estilos**: añadir un tema es añadir un
//! fichero, no duplicar la página. Hay **una página** —la del código por
//! defecto, con `{{code}}`, `{{reason}}` y `{{description}}`, que se rellenan al
//! servir— y **una hoja por tema**. El HTML que se sirve es autocontenido
//! —[`crate::render`] sustituye el marcador `<link rel="stylesheet"
//! href="/css/error.css" />` por un `<style>` con la hoja del tema elegido—, así
//! que el navegador no pide ningún fichero para ver la página.
//!
//! ---------------------------------------------------------------------------
//! API que asume la fase RED (la fase GREEN la implementa tal cual):
//!
//! ```ignore
//! #[derive(Debug)]
//! pub struct Theme {
//!     pub name: &'static str,
//!     pub sheet_path: &'static str,   // relativo a CONTENT_DIR
//!     pub embedded: &'static [u8],
//! }
//!
//! pub static THEMES: [Theme; 6];
//! pub const DEFAULT_THEME_NAME: &str = "default";
//! pub const THEME_NAMES: &str = "default, minimal, terminal, paper, amber, compact";
//!
//! pub fn theme_by_name(name: &str) -> Option<&'static Theme>;
//! pub fn default_theme() -> &'static Theme;
//! pub fn resolve(requested: Option<&str>, configured: &'static Theme) -> &'static Theme;
//!
//! impl Theme {
//!     pub fn sheet(&self, content_dir: &Path) -> Vec<u8>;
//! }
//! ```
//!
//! La rutas dentro de `CONTENT_DIR` son estables y documentadas: `css/error.css`
//! para `default` —es el **path público** del asset que el proyecto ya sirve, y
//! moverlo rompería ese contrato— y `themes/<nombre>.css` para los otros cinco,
//! que no tienen path público que preservar.
//!
//! # Por qué la hoja se resuelve **como un asset**
//!
//! [`Theme::sheet`] aplica a la hoja la misma regla que a cualquier asset: un
//! fichero **legible** en `CONTENT_DIR/<sheet_path>` gana a la copia embebida.
//! La alternativa —incrustar siempre los bytes del binario e ignorar el disco—
//! es más simple pero tiene el defecto que el proyecto ya rechazó una vez:
//! **divergencia silenciosa**. Con un `content/css/error.css` propio del
//! operador, `GET /css/error.css` serviría una hoja y la página incrustaría otra,
//! y el aspecto de la página no tendría nada que ver con el asset que el mismo
//! servicio publica. Resolviendo las dos por la misma regla, discrepar es
//! imposible; y la hoja se lee **al servir**, así que un operador que edite su
//! hoja la ve cambiar sin reiniciar, como le pasa hoy con sus páginas.
//!
//! # Los dos contratos de una hoja de tema
//!
//! Una hoja **incorporada al binario** tiene que cumplir dos cosas, y son dos
//! cosas distintas:
//!
//! 1. **No puede hacer que el navegador pida nada por la red.** Nada de
//!    `@import` —en ninguna de sus formas, incluida `@import "otra.css";`, que no
//!    lleva `url(`—, y nada de `url(...)` que no sea `url(#…)`: una referencia al
//!    propio documento, como un filtro o un degradado SVG. Un `@font-face` con
//!    `src: local(...)` **no** incumple —usa una fuente ya instalada, y solo
//!    podría pedir algo por la red a través de las dos formas que ya están
//!    prohibidas—.
//! 2. **Si anima, tiene que poder apagarse.** `animation`, `transition` y
//!    `@keyframes` son cómputo local: no piden nada. Animar es una decisión de
//!    aspecto, no un defecto de autocontención. Pero una hoja que anime tiene que
//!    declarar `prefers-reduced-motion`: una página de error que se mueve
//!    mientras el usuario intenta leer qué ha pasado es peor página de error.
//!
//! Las dos guardas —`ninguna_hoja_puede_hacer_una_peticion` y
//! `una_hoja_que_anima_respeta_reduced_motion`— miden **estos dos contratos y
//! nada más**, y cubren las hojas **incorporadas**: la hoja del operador en
//! `CONTENT_DIR` es suya, y el servicio **no** parsea CSS al servir —la hoja es
//! adorno y un CSS roto no puede tumbar una página—. La comprobación vive en el
//! módulo de pruebas porque es de **lo que el proyecto entrega**, y corre en
//! `cargo test`, no en el servidor.
//!
//! En los dos casos la guarda mira las **reglas**, no la prosa: los comentarios
//! `/* … */` de la hoja no cuentan —un comentario no pide nada ni anima—, así que
//! la cabecera que documenta por escrito qué no puede hacer una hoja no puede
//! convertirse ella misma en un incumplimiento.

use crate::embedded;
use std::io::ErrorKind;
use std::path::Path;

/// Un tema: un nombre, la ruta de su hoja dentro de `CONTENT_DIR` y sus bytes
/// embebidos.
///
/// Los tres campos van en la tabla del binario porque el tema tiene que estar
/// disponible en un servicio arrancado **sin ficheros de datos**: un binario
/// instalado con `cargo install` sirve la hoja de su tema aunque no haya
/// `content/`.
#[derive(Debug)]
pub struct Theme {
    /// Nombre público del tema, el que se escribe en `THEME` y en `X-Theme`.
    pub name: &'static str,
    /// Ruta de la hoja **relativa a `CONTENT_DIR`** (`css/error.css` para
    /// `default`, `themes/<nombre>.css` para los demás).
    pub sheet_path: &'static str,
    /// La hoja, embebida en el binario: la copia de respaldo de
    /// [`Theme::sheet`].
    pub embedded: &'static [u8],
}

/// Los seis temas incorporados, y la **única** fuente de verdad de qué temas hay.
///
/// `default` es la hoja que el proyecto ya trae (reglas intactas: quien no
/// configure `THEME` no ve ningún cambio de aspecto) y comparte la constante
/// [`embedded::DEFAULT_SHEET`] con el asset `/css/error.css`: un solo
/// `include_bytes!`, cero copias de los mismos bytes. Los cinco restantes
/// comparten una **decisión de diseño**, no un contrato: son deliberadamente
/// **sobrios** —sin adornos que compitan con el mensaje— porque son páginas de
/// **error** y su trabajo es decir qué ha pasado. Lo que sí es contrato lo miden
/// las guardas del módulo de pruebas: no poder provocar una petición, y poder
/// apagar el movimiento. Una hoja que anime respetando `prefers-reduced-motion`
/// cumple el contrato.
///
/// Es un `static` y no una `const` **a propósito**: el campo `embedded` del tema
/// `default` apunta al `static` de tipo array [`embedded::DEFAULT_SHEET_BYTES`],
/// cuyo respaldo es [`embedded::DEFAULT_SHEET`] —también un `static`—, así que
/// las dos lecturas —la del asset y la de este tema— referencian la **misma**
/// dirección y `std::ptr::eq` lo comprueba (ver `la_hoja_default_es_la_del_asset`
/// y la documentación de [`embedded::DEFAULT_SHEET_BYTES`]). Con una `const` la
/// identidad no sería estable. Y por ser un `static`, se recorre con
/// `THEMES.iter()`, nunca por valor: mover de un `static` no está permitido.
pub static THEMES: [Theme; 6] = [
    Theme {
        name: DEFAULT_THEME_NAME,
        sheet_path: "css/error.css",
        embedded: embedded::DEFAULT_SHEET,
    },
    Theme {
        name: "minimal",
        sheet_path: "themes/minimal.css",
        embedded: include_bytes!("../content/themes/minimal.css"),
    },
    Theme {
        name: "terminal",
        sheet_path: "themes/terminal.css",
        embedded: include_bytes!("../content/themes/terminal.css"),
    },
    Theme {
        name: "paper",
        sheet_path: "themes/paper.css",
        embedded: include_bytes!("../content/themes/paper.css"),
    },
    Theme {
        name: "amber",
        sheet_path: "themes/amber.css",
        embedded: include_bytes!("../content/themes/amber.css"),
    },
    Theme {
        name: "compact",
        sheet_path: "themes/compact.css",
        embedded: include_bytes!("../content/themes/compact.css"),
    },
];

/// Nombre del tema por defecto, que es también el de `THEMES[0]`.
pub const DEFAULT_THEME_NAME: &str = "default";

/// Los nombres válidos, en el orden de [`THEMES`], para los mensajes de error.
///
/// Es la lista que el error de `THEME` enumera: quien escribe la variable cree
/// que está configurando algo, así que el error tiene que decirle qué puede
/// escribir en su lugar, no solo que se equivocó. Una prueba comprueba que
/// enumera los seis y solo los seis.
pub const THEME_NAMES: &str = "default, minimal, terminal, paper, amber, compact";

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
    /// La hoja del tema: la de `CONTENT_DIR/<sheet_path>` si se puede leer, y si
    /// no la embebida.
    ///
    /// Un fichero **ausente** no es un fallo —es lo normal en un binario
    /// instalado sin ficheros de datos—, así que cae a la copia embebida **sin**
    /// `warn`. Un fichero **presente pero ilegible** se registra con un `warn`
    /// que nombra el fichero y la causa, y también se usa la embebida.
    ///
    /// Un CSS del operador roto **no** puede tumbar la página al respaldo de
    /// texto: la hoja es **adorno** y la página es **contenido**, así que
    /// degradar un `503` a texto plano porque un CSS no se pudo leer empeoraría
    /// el síntoma que el servicio existe para aliviar. El `warn` ya impide que
    /// el fallo pase inadvertido. Es una divergencia consciente de la Opción A
    /// de `embedded-error-pages`, que para las **páginas** manda el respaldo de
    /// texto precisamente para no tapar un directorio del operador roto.
    pub fn sheet(&self, content_dir: &Path) -> Vec<u8> {
        let path = content_dir.join(self.sheet_path);

        match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == ErrorKind::NotFound => self.embedded.to_vec(),
            Err(err) => {
                tracing::warn!(
                    theme = self.name,
                    path = %path.display(),
                    error = %err,
                    "no se pudo leer la hoja del tema: se usa la copia embebida"
                );

                self.embedded.to_vec()
            }
        }
    }
}

/// Un incumplimiento de un contrato por una hoja de tema **incorporada**.
///
/// Existe para poder comprobar cada cláusula del contrato con un caso pequeño,
/// en lugar de solo el conjunto sobre las seis hojas del binario: la
/// comprobación deja de ser un bucle dentro de una prueba y pasa a ser una
/// función pura sobre bytes.
///
/// Va bajo `#[cfg(test)]` **a propósito**: el servicio no valida CSS —ni al
/// arrancar ni al servir—, así que este código existe solo para `cargo test`,
/// que es donde se comprueba **lo que el proyecto entrega**. El binario no lo
/// lleva.
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Incumplimiento {
    /// La hoja puede hacer que el navegador **pida** algo por la red. Lleva
    /// consigo el token que lo provoca —`@import`, o `url(` cuando su destino
    /// **no** es el propio documento—, para que el incumplimiento se pueda
    /// nombrar en el mensaje.
    Peticion(&'static str),
    /// La hoja anima con ese token —`animation`, `transition` o `keyframes`— y
    /// no declara `prefers-reduced-motion`. Lleva consigo el token que anima.
    Movimiento(&'static str),
}

/// Los contratos que incumple una hoja de tema, con el token que los rompe.
///
/// Son dos, y son dos cosas distintas:
///
/// 1. **No poder hacer que el navegador pida nada por la red.** `@import` en
///    cualquiera de sus formas —incluida `@import "otra.css";`, que no lleva
///    `url(`—, y cualquier `url(...)` cuyo destino no sea **el propio
///    documento**. `url(#…)` —un filtro o un degradado SVG— **no** cuenta: es
///    una referencia interna y no hay ninguna petición que hacer.
/// 2. **Si anima, poder apagarlo.** `animation`, `transition` y `@keyframes` son
///    cómputo local: el navegador los ejecuta sin pedir nada a nadie, así que
///    animar es una decisión de aspecto. Pero una hoja que anime tiene que
///    declarar `prefers-reduced-motion`.
///
/// `@font-face` **no** está prohibido, y el motivo importa: solo puede pedir
/// algo por la red a través de `url(...)` o `@import`, que ya están prohibidos, y
/// un `src: local(...)` usa una fuente que el usuario **ya tiene instalada**.
/// Prohibirlo era prohibir un token sospechoso en vez de un contrato.
///
/// La detección es **literal y sensible a mayúsculas**: las hojas son del
/// proyecto y van en minúsculas.
///
/// SHALL mirar las **reglas**, no la prosa: los comentarios `/* … */` NO cuentan
/// como una petición ni como una animación, porque un comentario no puede hacer
/// que el navegador pida nada. Se quitan antes de escanear, con
/// [`hoja_sin_comentarios`], y el motivo importa: las cabeceras de las hojas
/// documentan por escrito qué no pueden hacer, así que leerlas como si fueran
/// reglas convertiría la propia documentación de la hoja en un incumplimiento.
/// El limpiador respeta además las **cadenas** —dentro de un `"…"` un `/*` es un
/// literal, no un comentario—, así que un `content: "/*";` no puede llevarse por
/// delante el resto de la hoja y esconder una petición de verdad.
///
/// Va bajo `#[cfg(test)]` **a propósito**: el servicio no valida CSS —ni al
/// arrancar ni al servir—, así que este código existe solo para `cargo test`,
/// que es donde se comprueba **lo que el proyecto entrega**. El binario no lo
/// lleva.
#[cfg(test)]
pub fn incumplimientos(hoja: &[u8]) -> Vec<Incumplimiento> {
    let mut fallos = Vec::new();
    let hoja = hoja_sin_comentarios(hoja);

    if let Some(token) = token_que_pide(&hoja) {
        fallos.push(Incumplimiento::Peticion(token));
    }

    if let Some(token) = token_que_anima(&hoja)
        && !puede_apagarse(&hoja)
    {
        fallos.push(Incumplimiento::Movimiento(token));
    }

    fallos
}

/// La hoja **sin sus comentarios** `/* … */`.
///
/// Un comentario no es una regla: no puede hacer que el navegador pida nada ni
/// puede animar, así que no puede incumplir ninguno de los dos contratos. Sin
/// esto, la propia documentación de una hoja —su cabecera diciendo qué no puede
/// hacer— contaría como un incumplimiento y la guarda mediría algo distinto de
/// lo que anuncia.
///
/// El limpiador es **textual**, pero conoce las **cadenas**: dentro de un `"…"` o
/// un `'…'` un `/*` no abre nada —es CSS válido, un literal o una URL con barras
/// de directorio—, así que las cadenas se copian tal cual, saltando sus escapes
/// (`\"`). Sin eso, un `content: "/*";` se llevaría por delante el resto de la
/// hoja y **escondería** un incumplimiento de verdad, que es peor que el defecto
/// que el limpiador viene a arreglar.
///
/// Un `/*` que **no** cierra se lleva el resto de la hoja: nadie puede recuperar
/// lo que está dentro de un comentario sin cerrar.
#[cfg(test)]
fn hoja_sin_comentarios(hoja: &[u8]) -> Vec<u8> {
    let mut salida = Vec::with_capacity(hoja.len());
    let mut i = 0;

    while i < hoja.len() {
        match hoja[i] {
            b'/' if hoja.get(i + 1) == Some(&b'*') => {
                match crate::render::busca(&hoja[i + 2..], b"*/") {
                    Some(cierre) => i += 2 + cierre + 2,
                    None => return salida,
                }
            }
            comilla @ (b'"' | b'\'') => {
                salida.push(comilla);
                i += 1;

                while i < hoja.len() {
                    let byte = hoja[i];
                    salida.push(byte);
                    i += 1;

                    if byte == b'\\' {
                        if let Some(escapado) = hoja.get(i) {
                            salida.push(*escapado);
                            i += 1;
                        }
                    } else if byte == comilla {
                        break;
                    }
                }
            }
            byte => {
                salida.push(byte);
                i += 1;
            }
        }
    }

    salida
}

/// El token que puede provocar una petición, si lo hay: `@import` manda sobre
/// `url(`, porque una hoja que importa otra puede no llevar ningún `url(`.
#[cfg(test)]
fn token_que_pide(hoja: &[u8]) -> Option<&'static str> {
    if contiene(hoja, "@import") {
        return Some("@import");
    }

    if tiene_url_externa(hoja) {
        return Some("url(");
    }

    None
}

/// El primer token que anima, si lo hay.
#[cfg(test)]
fn token_que_anima(hoja: &[u8]) -> Option<&'static str> {
    ["animation", "transition", "keyframes"]
        .into_iter()
        .find(|token| contiene(hoja, token))
}

/// ¿La hoja declara que el movimiento se puede apagar?
#[cfg(test)]
fn puede_apagarse(hoja: &[u8]) -> bool {
    contiene(hoja, "prefers-reduced-motion")
}

/// ¿La hoja trae algún `url(...)` cuyo destino **no** sea el propio documento?
///
/// `url(#gradiente)` apunta a un elemento del propio documento —un filtro o un
/// degradado SVG—, así que no hay petición que hacer y **no** cuenta. Cualquier
/// otro destino sí: una ruta relativa (`url(foto.png)`), una absoluta
/// (`url(https://…)`) o una entrecomillada.
///
/// Un `url()` vacío o un `url(` sin cerrar también cuentan como petición: son
/// hojas rotas, y rechazar de más es la dirección segura para un contrato que
/// existe para que la página no dependa de la red.
///
/// El token es `url(` **sin** espacio: la sintaxis de CSS no admite espacio entre
/// el nombre de la función y el paréntesis, así que `url (x)` no es un `url()`.
#[cfg(test)]
fn tiene_url_externa(hoja: &[u8]) -> bool {
    const AGUJA: &[u8] = b"url(";
    let mut desde = 0;

    while let Some(pos) = crate::render::busca(hoja.get(desde..).unwrap_or_default(), AGUJA) {
        let inicio = desde + pos + AGUJA.len();
        let destino = hoja
            .get(inicio..)
            .unwrap_or_default()
            .iter()
            .find(|byte| !byte.is_ascii_whitespace());

        if destino != Some(&b'#') {
            return true;
        }

        desde = inicio;
    }

    false
}

/// ¿`pajar` contiene `aguja`? Decodifica con pérdida y compara como texto: para
/// agujas ASCII —las únicas que se buscan aquí— es lo mismo que comparar bytes.
#[cfg(test)]
fn contiene(pajar: &[u8], aguja: &str) -> bool {
    String::from_utf8_lossy(pajar).contains(aguja)
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_THEME_NAME, Incumplimiento, THEME_NAMES, THEMES, default_theme, incumplimientos,
        resolve, theme_by_name,
    };
    use crate::embedded;
    use std::path::PathBuf;

    /// Directorio de contenido del repositorio: la copia que la tabla embebida
    /// tiene que igualar byte a byte (D5, como en [`crate::embedded`]).
    fn content_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("content")
    }

    /// Los seis nombres, escritos **literalmente** para no depender de `THEMES`
    /// al comprobar que `THEMES` es el conjunto correcto.
    const NOMBRES: [&str; 6] = [
        "default", "minimal", "terminal", "paper", "amber", "compact",
    ];

    /// Un byte que puede formar parte de un identificador de CSS.
    fn es_de_identificador(byte: u8) -> bool {
        byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_'
    }

    fn es_espacio(byte: u8) -> bool {
        matches!(byte, b' ' | b'\t' | b'\n' | b'\r')
    }

    /// Quita los comentarios `/* … */` de una hoja, para no confundir una
    /// palabra mencionada en la cabecera con una regla de verdad.
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

    /// ¿La hoja declara una regla para ese selector? Vale una regla propia
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

    /// ¿La hoja tiene algún incumplimiento de **movimiento**? Es la pregunta de
    /// casi todas las pruebas de este contrato: la lista de incumplimientos se
    /// consulta por el token, pero aquí lo único que importa es si hay alguno.
    fn incumple_movimiento(hoja: &str) -> bool {
        incumplimientos(hoja.as_bytes())
            .iter()
            .any(|i| matches!(i, Incumplimiento::Movimiento(_)))
    }

    #[test]
    fn las_hojas_embebidas_son_byte_a_byte_las_de_content() {
        // D5: los bytes embebidos de cada tema igualan la lectura de
        // `content/<sheet_path>`. Detecta una copia embebida obsoleta o distinta
        // —alguien retoca una hoja de `content/` y quien instaló el crate sigue
        // viendo la vieja—.
        for theme in THEMES.iter() {
            let ruta = content_dir().join(theme.sheet_path);
            let del_disco = match std::fs::read(&ruta) {
                Ok(bytes) => bytes,
                Err(error) => panic!(
                    "no se pudo leer {} (tema `{}`): {error}",
                    ruta.display(),
                    theme.name
                ),
            };

            assert_eq!(
                theme.embedded,
                del_disco.as_slice(),
                "la hoja embebida del tema `{}` no coincide byte a byte con {}",
                theme.name,
                ruta.display()
            );
        }
    }

    #[test]
    fn la_tabla_declara_los_seis_temas() {
        assert_eq!(
            THEMES.len(),
            NOMBRES.len(),
            "`THEMES` tiene que declarar los seis temas incorporados"
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

        // `THEME_NAMES` enumera los seis y **solo** los seis: sin nombres de más
        // —una lista que anunciase uno que no existe— ni de menos.
        let enumerados: Vec<&str> = THEME_NAMES.split(',').map(str::trim).collect();
        assert_eq!(
            enumerados, NOMBRES,
            "`THEME_NAMES` tiene que enumerar los seis nombres: {THEME_NAMES}"
        );
    }

    #[test]
    fn theme_by_name_solo_acepta_los_seis() {
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
        let configurado = theme_by_name("minimal").expect("`minimal` es uno de los seis");

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

    #[test]
    fn cada_hoja_cubre_el_marcado() {
        // Sin esto, un tema que se olvide de un elemento deja una página a medio
        // vestir y nadie se entera: las seis etiquetas son las que lleva la única
        // página de error. `main` **no** está en la lista a propósito: es
        // estructura —el bloque que `default` anima— y no todos los temas
        // necesitan una regla para él, mientras que `footer` lo estilan las seis.
        let mut fallos = Vec::new();

        for theme in THEMES.iter() {
            let hoja = String::from_utf8_lossy(theme.embedded);

            for selector in ["body", "h1", "h2", "hr", "p", "footer"] {
                if !tiene_regla(&hoja, selector) {
                    fallos.push(format!(
                        "la hoja del tema `{}` no define `{selector}`",
                        theme.name
                    ));
                }
            }
        }

        assert!(
            fallos.is_empty(),
            "cada hoja de tema tiene que cubrir el marcado ({} incumplimiento(s)):\n- {}",
            fallos.len(),
            fallos.join("\n- ")
        );
    }

    #[test]
    fn ninguna_hoja_puede_hacer_una_peticion() {
        // El primer contrato: una hoja **incorporada** no puede hacer que el
        // navegador **pida** algo por la red. El motivo es el del requisito
        // «El HTML servido es autocontenido»: la página se sirve en el host de la
        // aplicación, donde este servicio no sirve assets, así que ese pedido no
        // lo puede resolver nadie. `@import` carga otra hoja —y en su forma
        // `@import "otra.css";` ni siquiera lleva `url(`—, y un `url(...)` que no
        // sea una referencia al **propio documento** pide un fichero. `url(#…)`
        // —un filtro o un degradado SVG— y un `@font-face` con `src: local(...)`
        // **no** cuentan: apuntan al propio documento o a una fuente ya
        // instalada, así que no hay ninguna petición que hacer. Animar **no**
        // pertenece a este contrato: lo mide la guarda de al lado.
        let mut fallos = Vec::new();

        for theme in THEMES.iter() {
            for incumplimiento in incumplimientos(theme.embedded) {
                if let Incumplimiento::Peticion(token) = incumplimiento {
                    fallos.push(format!(
                        "la hoja del tema `{}` contiene `{token}`: el navegador tendría que pedir \
                         algo por la red",
                        theme.name
                    ));
                }
            }
        }

        assert!(
            fallos.is_empty(),
            "ninguna hoja puede hacer que el navegador pida algo por la red ({} \
             incumplimiento(s)):\n- {}",
            fallos.len(),
            fallos.join("\n- ")
        );
    }

    /// Una hoja que solo se refiere al **propio documento**: `url(#gradiente)`
    /// es un filtro o degradado SVG y `src: local("Arial")` usa una fuente que
    /// el usuario ya tiene instalada. No hay ninguna petición que hacer.
    const AUTOCONTENIDA: &str = r#"
.caja { fill: url(#gradiente); }
@font-face { font-family: Arial; src: local("Arial"); }
"#;

    /// Una hoja que anima y **sabe apagarse**.
    const ANIMADA_CON_FRENO: &str = r#"
@keyframes parpadeo { from { opacity: 1 } to { opacity: 0 } }
body { animation: parpadeo 2s infinite; }
@media (prefers-reduced-motion: reduce) { body { animation: none; } }
"#;

    /// Una hoja que anima y **no** sabe apagarse.
    const ANIMADA_SIN_FRENO: &str = r#"
body { transition: opacity 0.2s; }
"#;

    /// Una hoja quieta: no pide nada y no anima.
    const QUIETA: &str = r#"
body { color: #222; }
"#;

    /// Una hoja que anima y **solo nombra** la media query: el bloque reduce
    /// existe, pero su único contenido es un comentario, así que no apaga nada.
    const NOMBRA_SIN_NEUTRALIZAR: &str = r#"
main { animation: entrada 180ms ease-out; }
@media (prefers-reduced-motion: reduce) { /* esta animación se apaga sola */ }
"#;

    /// Una hoja que anima y neutraliza bajo la preferencia **contraria**: el
    /// bloque existe, pero es el que ve quien **no** pide movimiento reducido.
    const NEUTRALIZA_BAJO_LA_OTRA_PREFERENCIA: &str = r#"
main { animation: entrada 180ms ease-out; }
@media (prefers-reduced-motion: no-preference) { main { animation: none; } }
"#;

    /// Una hoja que anima y trae el bloque reduce **vacío**.
    const REDUCE_VACIO: &str = r#"
main { animation: entrada 180ms ease-out; }
@media (prefers-reduced-motion: reduce) { }
"#;

    /// La forma canónica de D2: fotogramas propios, la `animation` declarada y el
    /// bloque reduce que la **neutraliza**.
    const REDUCE_QUE_NEUTRALIZA: &str = r#"
@keyframes entrada { from { opacity: 0 } to { opacity: 1 } }
main { animation: entrada 180ms ease-out; }
@media (prefers-reduced-motion: reduce) { main { animation: none; } }
"#;

    /// Una hoja que declara una `transition` y cuyo bloque reduce solo neutraliza
    /// una `animation`: la transición declarada sigue viva.
    const REDUCE_QUE_NEUTRALIZA_OTRA: &str = r#"
main { transition: opacity 200ms; }
@media (prefers-reduced-motion: reduce) { main { animation: none; } }
"#;

    /// Una hoja cuyo bloque reduce **anima** en vez de apagar.
    const REDUCE_QUE_ANIMA: &str = r#"
@media (prefers-reduced-motion: reduce) { main { animation: entrada 180ms ease-out; } }
"#;

    /// Unos fotogramas **sin ninguna declaración** que los use: definen fotogramas
    /// y no animan a nadie, así que no obligan a nada.
    const SOLO_FOTOGRAMAS: &str = r#"
@keyframes entrada { from { opacity: 0 } to { opacity: 1 } }
"#;

    /// La neutralización vive en una **regla anidada** del bloque reduce y **no**
    /// es la primera que se cierra: hay una regla antes que la neutralizadora, así
    /// que el bloque tiene que delimitarse por su llave de apertura y **su** cierre
    /// equilibrado, no por la primera `}` que aparezca.
    const REDUCE_CON_REGLA_ANIDADA: &str = r#"
main { animation: entrada 180ms ease-out; }
@media (prefers-reduced-motion: reduce) {
  main { color: inherit; }
  main { animation: none; }
}
"#;

    /// Las tres formas de pedir un fichero que el contrato prohíbe, con el token
    /// que cada una tiene que delatar: una ruta relativa y una absoluta delatan
    /// `url(`, y un `@import` —que no lleva `url(`— delata `@import`.
    const PIDEN_FICHERO: [(&str, &str); 3] = [
        (r#"body { background: url(foto.png); }"#, "url("),
        (
            r#"body { background: url(https://ejemplo/x.png); }"#,
            "url(",
        ),
        (r#"@import "otra.css";"#, "@import"),
    ];

    /// El mismo contrato, pero **en prosa**: la hoja lo dice en su cabecera y no
    /// lo hace en ninguna regla. Un comentario no pide nada y no anima.
    const COMENTADA: &str = r#"
/* Esta hoja no usa `animation` ni `@import`, y no referencia `url(` remotos. */
body { color: #222; }
"#;

    /// La prosa **y** la regla: el `url(` del comentario no puede tapar la
    /// petición de verdad.
    const COMENTADA_CON_PETICION: &str = r#"
/* Nada de `url(` remotos por aquí. */
body { background: url(foto.png); }
"#;

    /// Un comentario que abre y no cierra: lo que queda detrás es comentario, así
    /// que no hay ninguna regla que pueda incumplir nada.
    const COMENTARIO_ABIERTO: &str = r#"
/* sin cerrar: `animation` y `@import` aquí dentro ya no son reglas
body { color: #222; }
"#;

    /// Varias referencias, todas internas, con espacios y con un fragmento vacío
    /// de por medio: el bucle tiene que recorrerlas todas sin confundirse.
    const AUTOCONTENIDA_MULTIPLE: &str = r#"
.a { fill: url(#uno); }
.b { fill: url(  #dos  ); }
.c { fill: url(#); }
"#;

    /// Dos referencias en la **misma** declaración: la primera interna y la
    /// segunda externa. La segunda no puede pasar desapercibida.
    const INTERNA_Y_EXTERNA: &str = r#"
.a { background: image-set(url(#uno) 1x, url(foto.png) 2x); }
"#;

    /// Una cadena que **contiene** `/*`: es CSS válido —un literal, o una URL con
    /// barras de directorio—, no abre ningún comentario, y por tanto no puede
    /// tapar la petición que venga detrás.
    const CADENA_CON_BARRAS: &str = r#"
a::before { content: "/*"; }
body { background: url(foto.png); }
"#;

    #[test]
    fn una_referencia_al_propio_documento_no_es_una_peticion() {
        assert!(
            incumplimientos(AUTOCONTENIDA.as_bytes()).is_empty(),
            "`url(#…)` y `src: local(…)` no son peticiones: no hay nada que el navegador tenga que \
             pedir, así que la hoja tiene que cumplir el contrato. Hoja: {AUTOCONTENIDA:?}"
        );
    }

    #[test]
    fn una_hoja_que_pide_un_fichero_incumple() {
        for (hoja, token) in PIDEN_FICHERO {
            assert!(
                incumplimientos(hoja.as_bytes()).contains(&Incumplimiento::Peticion(token)),
                "esta hoja pide un fichero con `{token}` y tiene que incumplirlo **nombrando ese \
                 token**: {hoja:?}"
            );
        }
    }

    #[test]
    fn una_hoja_que_anima_solo_cumple_si_se_puede_apagar() {
        // El contrato de movimiento, cláusula a cláusula, con casos pequeños:
        // una hoja que anima y no se puede apagar incumple; una que anima y
        // declara la media query cumple; y una quieta cumple sin declarar nada.
        assert!(
            incumplimientos(ANIMADA_SIN_FRENO.as_bytes())
                .iter()
                .any(|i| matches!(i, Incumplimiento::Movimiento(_))),
            "una hoja que anima y no declara `prefers-reduced-motion` tiene que incumplir: \
             {ANIMADA_SIN_FRENO:?}"
        );
        assert!(
            incumplimientos(ANIMADA_CON_FRENO.as_bytes()).is_empty(),
            "una hoja que anima **y** declara `prefers-reduced-motion` tiene que cumplir: \
             {ANIMADA_CON_FRENO:?}"
        );
        assert!(
            incumplimientos(QUIETA.as_bytes()).is_empty(),
            "una hoja sin animación tiene que cumplir sin declarar nada: {QUIETA:?}"
        );
    }

    #[test]
    fn una_hoja_que_anima_respeta_reduced_motion() {
        // El segundo contrato, cláusula a cláusula y sobre las hojas
        // **incorporadas** al binario. Animar **no** es un defecto de
        // autocontención: el navegador ejecuta la animación sin pedirle nada a
        // nadie, es cómputo local, y por eso es una decisión de aspecto y no se
        // prohíbe. Pero hay gente a la que el movimiento le produce mareo, y una
        // página de error que se mueve mientras el usuario intenta leer qué ha
        // pasado es peor página de error. La regla no prohíbe el adorno: prohíbe
        // el adorno que no se puede apagar, así que una hoja que anime tiene que
        // **neutralizar** cada animación declarada dentro de un bloque
        // `@media (prefers-reduced-motion: reduce)`.

        // 1. Incumple si anima y no neutraliza.
        assert!(
            incumple_movimiento(ANIMADA_SIN_FRENO),
            "una hoja que anima y no neutraliza dentro de `@media \
             (prefers-reduced-motion: reduce)` tiene que incumplir: {ANIMADA_SIN_FRENO:?}"
        );

        // 2. Cumple si neutraliza.
        assert!(
            !incumple_movimiento(ANIMADA_CON_FRENO),
            "una hoja que anima **y** neutraliza dentro de `@media \
             (prefers-reduced-motion: reduce)` tiene que cumplir: {ANIMADA_CON_FRENO:?}"
        );

        // 3. Una hoja sin animación cumple sin declarar nada.
        assert!(
            !incumple_movimiento(QUIETA),
            "una hoja sin animación tiene que cumplir sin declarar nada: {QUIETA:?}"
        );

        // Y el barrido de las hojas embebidas: hoy ninguna de las seis anima, así
        // que cumplen por vacío; el día que una anime, esta guarda es la que lo
        // exige.
        let mut fallos = Vec::new();

        for theme in THEMES.iter() {
            for incumplimiento in incumplimientos(theme.embedded) {
                if let Incumplimiento::Movimiento(token) = incumplimiento {
                    fallos.push(format!(
                        "la hoja del tema `{}` anima con `{token}` y no neutraliza la animación \
                         dentro de `@media (prefers-reduced-motion: reduce)`",
                        theme.name
                    ));
                }
            }
        }

        assert!(
            fallos.is_empty(),
            "una hoja que anima tiene que poder apagarse ({} incumplimiento(s)):\n- {}",
            fallos.len(),
            fallos.join("\n- ")
        );
    }

    #[test]
    fn una_hoja_que_solo_nombra_prefers_reduced_motion_incumple() {
        // Nombrar la media query no es apagar nada: si el bloque existe pero su
        // contenido es **solo un comentario**, la animación declarada sigue viva.
        assert!(
            incumple_movimiento(NOMBRA_SIN_NEUTRALIZAR),
            "nombrar `prefers-reduced-motion` no neutraliza nada: el bloque reduce trae solo un \
             comentario, así que la animación declarada sigue viva y la hoja tiene que incumplir. \
             Hoja: {NOMBRA_SIN_NEUTRALIZAR:?}"
        );
    }

    #[test]
    fn una_media_query_que_no_es_la_de_movimiento_reducido_no_basta() {
        // Neutralizar bajo `no-preference` no cuenta: quien pide movimiento
        // reducido no ve ese bloque, así que no es la puerta que el usuario cierra.
        assert!(
            incumple_movimiento(NEUTRALIZA_BAJO_LA_OTRA_PREFERENCIA),
            "neutralizar bajo `prefers-reduced-motion: no-preference` no es la puerta que el \
             usuario cierra, así que la hoja tiene que incumplir. \
             Hoja: {NEUTRALIZA_BAJO_LA_OTRA_PREFERENCIA:?}"
        );
    }

    #[test]
    fn un_bloque_reduce_vacio_no_basta() {
        // El bloque tiene que existir **y** tener la neutralización: uno vacío no
        // apaga nada, que es el agujero medido.
        assert!(
            incumple_movimiento(REDUCE_VACIO),
            "el bloque `@media (prefers-reduced-motion: reduce)` tiene que **tener** la \
             neutralización: uno vacío no apaga nada, así que la hoja tiene que incumplir. \
             Hoja: {REDUCE_VACIO:?}"
        );
    }

    #[test]
    fn un_bloque_reduce_que_neutraliza_la_animacion_cumple() {
        // La forma canónica: hay `@keyframes`, hay declaración que los usa y el
        // bloque reduce neutraliza esa animación.
        assert!(
            !incumple_movimiento(REDUCE_QUE_NEUTRALIZA),
            "una hoja que anima y neutraliza su animación dentro del bloque reduce tiene que \
             cumplir: {REDUCE_QUE_NEUTRALIZA:?}"
        );
    }

    #[test]
    fn neutralizar_otra_animacion_distinta_de_la_declarada_no_basta() {
        // Cada animación declarada tiene que quedar neutralizada por la forma que
        // le corresponde: `animation: none` no apaga una transición.
        assert!(
            incumple_movimiento(REDUCE_QUE_NEUTRALIZA_OTRA),
            "la hoja declara una `transition` y el bloque reduce solo trae `animation: none`: la \
             transición declarada sigue sin neutralizarse, así que la hoja tiene que incumplir. \
             Hoja: {REDUCE_QUE_NEUTRALIZA_OTRA:?}"
        );
    }

    #[test]
    fn una_animacion_declarada_dentro_del_bloque_reduce_no_se_neutraliza_sola() {
        // El bloque reduce tiene que **apagar**, no animar: una animación
        // declarada dentro del propio bloque no neutraliza la que la hoja anima.
        assert!(
            incumple_movimiento(REDUCE_QUE_ANIMA),
            "el bloque reduce trae una declaración que **anima** en vez de apagar, así que no \
             neutraliza nada y la hoja tiene que incumplir. Hoja: {REDUCE_QUE_ANIMA:?}"
        );
    }

    #[test]
    fn unos_fotogramas_sin_declaracion_que_los_use_no_obligan() {
        // `@keyframes` define fotogramas y no anima mientras ninguna declaración
        // los use: exigirle la media query a una hoja que no puede animar sería
        // medir algo distinto del contrato.
        assert!(
            !incumple_movimiento(SOLO_FOTOGRAMAS),
            "una hoja cuyo único contenido son `@keyframes` —sin ninguna declaración de \
             `animation` ni de `transition`— tiene que cumplir sin necesitar el bloque reduce, \
             porque definir fotogramas no anima nada. Hoja: {SOLO_FOTOGRAMAS:?}"
        );
    }

    #[test]
    fn una_regla_anidada_dentro_del_bloque_reduce_cuenta() {
        // La neutralización vive en una regla anidada y **no** es la primera que
        // se cierra: el bloque se delimita por su llave de apertura y **su** cierre
        // equilibrado, así que la neutralizadora —que va después— cuenta igual.
        assert!(
            !incumple_movimiento(REDUCE_CON_REGLA_ANIDADA),
            "la neutralización dentro de una regla anidada del bloque reduce tiene que contar, \
             aunque no sea la primera regla que se cierra: {REDUCE_CON_REGLA_ANIDADA:?}"
        );
    }

    #[test]
    fn una_hoja_quieta_cumple_los_dos_contratos() {
        assert!(
            incumplimientos(QUIETA.as_bytes()).is_empty(),
            "una hoja que no pide nada y no anima tiene que cumplir: {QUIETA:?}"
        );
    }

    #[test]
    fn un_comentario_no_es_una_peticion_ni_una_animacion() {
        assert!(
            incumplimientos(COMENTADA.as_bytes()).is_empty(),
            "un `url(`, un `@import` o un `animation` dentro de un comentario no son el contrato: \
             un comentario no puede hacer que el navegador pida nada ni puede animar. \
             Hoja: {COMENTADA:?}"
        );
        assert!(
            incumplimientos(COMENTADA_CON_PETICION.as_bytes())
                .iter()
                .any(|i| matches!(i, Incumplimiento::Peticion(_))),
            "limpiar los comentarios no puede tapar una petición de verdad: el comentario menciona \
             `url(` y la regla trae un `url(foto.png)`. Hoja: {COMENTADA_CON_PETICION:?}"
        );
    }

    #[test]
    fn un_comentario_sin_cerrar_se_ignora() {
        assert!(
            incumplimientos(COMENTARIO_ABIERTO.as_bytes()).is_empty(),
            "un comentario que abre y no cierra llega hasta el final de la hoja: lo que queda \
             detrás es prosa, no reglas. Hoja: {COMENTARIO_ABIERTO:?}"
        );
    }

    #[test]
    fn el_contrato_de_red_mira_todas_las_referencias() {
        assert!(
            incumplimientos(AUTOCONTENIDA_MULTIPLE.as_bytes()).is_empty(),
            "varias referencias internas —una con espacios de por medio y otra con el fragmento \
             vacío— siguen sin ser peticiones: {AUTOCONTENIDA_MULTIPLE:?}"
        );
        assert!(
            incumplimientos(INTERNA_Y_EXTERNA.as_bytes())
                .contains(&Incumplimiento::Peticion("url(")),
            "una interna **y** una externa en la misma declaración: la externa tiene que \
             detectarse igual: {INTERNA_Y_EXTERNA:?}"
        );
    }

    #[test]
    fn una_cadena_con_barras_no_abre_un_comentario() {
        assert!(
            incumplimientos(CADENA_CON_BARRAS.as_bytes())
                .contains(&Incumplimiento::Peticion("url(")),
            "un `/*` dentro de una cadena es CSS válido, no un comentario: no puede tapar el \
             `url(foto.png)` que viene detrás. Hoja: {CADENA_CON_BARRAS:?}"
        );
        assert!(
            incumplimientos(br#"a { background: url("a/*b.png"); }"#)
                .contains(&Incumplimiento::Peticion("url(")),
            "una URL con `/*` dentro sigue siendo una petición externa"
        );
        assert!(
            incumplimientos(br#"/* El operador dice: "no uses url( remotos". */ body { background: url(foto.png); }"#)
                .contains(&Incumplimiento::Peticion("url(")),
            "una comilla dentro de un comentario no puede desactivar el limpiador: el comentario se \
             va entero y la petición de detrás tiene que seguir viéndose"
        );
    }

    #[test]
    fn ninguna_hoja_puede_cerrar_el_comentario_ni_el_style() {
        // El escáner sustituye el marcador también **dentro de un comentario
        // HTML** —es textual y no respeta comentarios—. Si una hoja trajese
        // `-->`, cerraría ese comentario y dejaría ver el `</style>` que la
        // envuelve como texto; y un `</style>` propio cerraría el bloque antes
        // de tiempo. Ninguna de las seis lo tiene hoy: la guarda convierte esa
        // hipótesis en un invariante medido, y acumula los fallos nombrando el
        // tema para que el mensaje diga cuál incumple.
        const PROHIBIDOS: [&str; 2] = ["-->", "</style>"];

        let mut fallos = Vec::new();

        for theme in THEMES.iter() {
            let hoja = String::from_utf8_lossy(theme.embedded);

            for prohibido in PROHIBIDOS {
                if hoja.contains(prohibido) {
                    fallos.push(format!(
                        "la hoja del tema `{}` contiene `{prohibido}`",
                        theme.name
                    ));
                }
            }
        }

        assert!(
            fallos.is_empty(),
            "ninguna hoja puede contener `-->` ni `</style>` ({} incumplimiento(s)):\n- {}",
            fallos.len(),
            fallos.join("\n- ")
        );
    }

    #[test]
    fn la_hoja_default_es_la_del_asset() {
        // El asset `/css/error.css` y el tema `default` tienen que ser la **misma
        // constante**, no dos copias de los mismos bytes: es la única forma de
        // que no puedan separarse mañana.
        let (bytes, content_type) = embedded::asset("/css/error.css")
            .expect("la hoja `/css/error.css` tiene que estar en la tabla de assets");
        assert_eq!(
            content_type, "text/css",
            "Content-Type del asset de la hoja"
        );

        let default = default_theme();
        assert_eq!(
            default.sheet_path, "css/error.css",
            "la hoja del tema `default` es el asset público"
        );

        // Lo de «misma constante» se comprueba **por identidad de puntero**, y
        // esa es la comprobación buena porque la hoja está respaldada por un
        // `static` de tipo **array** ([`embedded::DEFAULT_SHEET_BYTES`]), y un
        // `static` tiene **una sola dirección**, garantizada por el lenguaje: las
        // dos lecturas —la del asset y la del tema— referencian el mismo array y
        // `ptr::eq` los ve iguales. Con una `const` esto no valdría —dos usos de
        // una `const` no tienen por qué compartir la promoción del array de
        // bytes—; y un `static &[u8]` que llevase el `include_bytes!` directamente
        // tampoco, porque leerlo en otro `static` re-inlinea el `include_bytes!` y
        // con varias *codegen units* el enlazador no funde las copias anónimas
        // (medido: `ptr::eq` da `false` con `cargo test`). El array intermedio es
        // lo que hace la identidad estable.
        assert!(
            std::ptr::eq(default.embedded, bytes),
            "el tema `default` y el asset tienen que **compartir la hoja** —el mismo puntero, no \
             solo los mismos bytes—, pero `THEMES[0].embedded` y el asset apuntan a direcciones \
             distintas"
        );
    }
}
