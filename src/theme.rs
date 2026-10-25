//! Temas de las páginas de error: nombre, hoja y bytes embebidos.
//!
//! Un **tema** es **una sola hoja de estilos**: añadir un tema es añadir un
//! fichero, no duplicar las once páginas. El HTML que se sirve es autocontenido
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
/// `include_bytes!`, cero copias de los mismos bytes. Los cinco restantes son
/// deliberadamente **sobrios** —sin animaciones, sin imágenes, sin recursos
/// externos—: son páginas de **error**, y su trabajo es decir qué ha pasado.
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

#[cfg(test)]
mod tests {
    use super::{DEFAULT_THEME_NAME, THEME_NAMES, THEMES, default_theme, resolve, theme_by_name};
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
        // vestir y nadie se entera: las cinco etiquetas son las que llevan las
        // once páginas de error.
        let mut fallos = Vec::new();

        for theme in THEMES.iter() {
            let hoja = String::from_utf8_lossy(theme.embedded);

            for selector in ["body", "h1", "h2", "hr", "p"] {
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
    fn ninguna_hoja_referencia_recursos_externos() {
        // Una página que necesita la red para verse es el mismo defecto que este
        // change corrige, así que ninguna hoja puede traer un `@import`, un
        // `url(…)`, una fuente descargada ni una animación.
        const PROHIBIDOS: [&str; 6] = [
            "@import",
            "url(",
            "@font-face",
            "animation",
            "transition",
            "keyframes",
        ];

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
            "ninguna hoja puede referenciar recursos externos ({} incumplimiento(s)):\n- {}",
            fallos.len(),
            fallos.join("\n- ")
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
