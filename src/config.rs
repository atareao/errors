//! Configuración de la aplicación, leída **solo** del entorno.
//!
//! ---------------------------------------------------------------------------
//! API pública del módulo:
//!
//! ```ignore
//! #[derive(Debug, Clone, Copy, Default)]
//! pub enum LogFormat {
//!     #[default]
//!     Text,
//!     Json,
//! }
//!
//! #[derive(Debug, Clone)]
//! pub struct Config {
//!     pub port: u16,
//!     pub log_level: String,
//!     pub log_format: LogFormat,
//!     pub content_dir: PathBuf,
//!     pub default_error_page: u16,
//!     pub retry_after: u32,
//!     /// El registro efectivo de temas: los siete incorporados más los
//!     /// descubiertos en `content_dir` **al arrancar**.
//!     pub themes: Themes,
//!     /// El tema del servicio, **ya validado** contra `themes`.
//!     pub theme: Theme,
//! }
//!
//! #[derive(Debug)]
//! pub enum ConfigError {
//!     InvalidPort(String),
//!     UnknownLogFormat(String),
//!     DefaultErrorPageOutOfRange(String),
//!     InvalidRetryAfter(String),
//!     UnknownTheme { value: String, validos: String },
//! }
//! impl std::fmt::Display for ConfigError { /* ... */ }
//! impl std::error::Error for ConfigError {}
//!
//! impl Config {
//!     /// Construye la configuración a partir de un iterador de pares
//!     /// `(clave, valor)`. Es la vía inyectable: permite probar sin tocar el
//!     /// entorno del proceso (ninguna prueba puede mutar `std::env`).
//!     pub fn from_env_iter<I>(iter: I) -> Result<Config, ConfigError>
//!     where
//!         I: IntoIterator<Item = (String, String)>;
//!
//!     /// Azúcar: `Self::from_env_iter(std::env::vars())`.
//!     pub fn from_env() -> Result<Config, ConfigError>;
//! }
//! ```
//!
//! Contrato de variables (spec `http-server`, requisito "La configuración se
//! lee solo del entorno"):
//!
//! | Variable             | Defecto   |
//! |----------------------|-----------|
//! | `PORT`               | `8080`    |
//! | `LOG_LEVEL`          | `info`    |
//! | `LOG_FORMAT`         | `text`    |
//! | `CONTENT_DIR`        | `content` |
//! | `DEFAULT_ERROR_PAGE` | `404`     |
//! | `RETRY_AFTER`        | `120`     |
//! | `THEME`              | `default` |
//!
//! No se lee ningún fichero de configuración (`config.yml` ni similar).
//!
//! `THEME` (spec `error-page`) elige la **página** del tema que se sirve, entre
//! los nombres del **registro** del arranque: los siete incorporados al binario
//! **más** los descubiertos en la raíz de `CONTENT_DIR` ([`crate::theme::Themes`]).
//! Un valor que no sea uno de esos nombres —**o la cadena vacía**— es un error de
//! arranque, con el mismo criterio que `LOG_FORMAT` y `DEFAULT_ERROR_PAGE`: la
//! variable la escribe el **operador** y quien la escribe cree que está
//! configurando algo, así que un `default` silencioso le ocultaría el error. La
//! cabecera `X-Theme` **no** sigue esa regla, y la asimetría es deliberada: esa la
//! elige **quien llama**, y un cliente no puede romper el servicio, así que un
//! `X-Theme` desconocido cae al tema configurado sin error (ver
//! [`crate::theme::resolve`]).
//!
//! **El precio, escrito**: como la lista es el registro de **ese** arranque, un
//! `THEME` **correcto** cuyo fichero **todavía** no esté en `CONTENT_DIR` —un
//! volumen que aún no se ha montado cuando el contenedor arranca— **aborta** el
//! arranque. Es la contrapartida de mantener el aborto: la variable la escribe el
//! operador, y un arranque con un tema que no existe es peor que un arranque que
//! falla diciendo por qué. La mitigación es del operador: declarar la dependencia
//! del volumen, o arrancar sin `THEME` y con el defecto.
//!
//! `LOG_FORMAT` (change `release-hardening`, spec `observability`) decide el
//! formato de los registros: `text`, el de siempre, por defecto, y `json` para
//! que un colector los parsee sin expresiones regulares. Es un enumerado
//! ([`LogFormat`]) y no una cadena porque solo hay dos valores válidos.
//!
//! `RETRY_AFTER` (change `retry-after-configurable`, spec `error-page`) fija los
//! segundos de la cabecera `Retry-After` de los **dos** códigos que la llevan,
//! `429` y `503`; qué códigos la llevan no se configura, lo decide el código. El
//! valor es un `delay-seconds` de RFC 9110 —nunca una fecha HTTP—, así que `0`
//! es válido («reintenta ya») y no se acota por arriba: un valor enorme no rompe
//! ningún contrato. Una variable presente pero no interpretable como `u32` es un
//! error de arranque, no un `120` silencioso.
//!
//! Decisión de la fase RED, justificada: la spec exige que una variable
//! **presente pero vacía no se confunda con una ausente**, y `tasks.md` 3.2
//! pide que los errores de configuración sean errores de arranque tipados. Por
//! eso el constructor devuelve `Result<Config, ConfigError>` (y no un `Config`
//! desnudo que tragaría con un valor inválido): una `PORT` presente pero no
//! parseable —incluida la cadena vacía— es un `ConfigError::InvalidPort`, no un
//! silencioso `8080`.

use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;

use crate::resolve::{MAX_ERROR_CODE, MIN_ERROR_CODE, is_usable_code};
use crate::theme::{self, DEFAULT_THEME_NAME, Theme, Themes};

/// Puerto de escucha por defecto (el que asume la guía de Traefik).
const DEFAULT_PORT: u16 = 8080;
/// Filtro de registro por defecto.
const DEFAULT_LOG_LEVEL: &str = "info";
/// Formato de registro por defecto: el de texto, el que había antes de
/// `LOG_FORMAT`.
const DEFAULT_LOG_FORMAT: LogFormat = LogFormat::Text;
/// Directorio de páginas y assets por defecto. Es **la única** ruta de
/// estáticos: la comparten el código, el `Dockerfile` y `.dockerignore`.
const DEFAULT_CONTENT_DIR: &str = "content";
/// Código de error por defecto cuando la petición no da ninguna pista.
const DEFAULT_ERROR_PAGE: u16 = 404;
/// Segundos de la cabecera `Retry-After` por defecto: el valor actual del
/// servicio y el de `error-pages`, así que quien no configure no ve cambios.
const DEFAULT_RETRY_AFTER: u32 = 120;

const PORT_VAR: &str = "PORT";
const LOG_LEVEL_VAR: &str = "LOG_LEVEL";
const LOG_FORMAT_VAR: &str = "LOG_FORMAT";
const CONTENT_DIR_VAR: &str = "CONTENT_DIR";
const DEFAULT_ERROR_PAGE_VAR: &str = "DEFAULT_ERROR_PAGE";
const RETRY_AFTER_VAR: &str = "RETRY_AFTER";
const THEME_VAR: &str = "THEME";

/// Formato de los registros del servicio.
///
/// Es un enumerado y no una cadena porque solo hay dos valores válidos: un
/// `String` admitiría cualquier cosa y trasladaría el error al punto donde se
/// construye el `subscriber`, al arrancar y ya sin contexto. Un valor que no sea
/// uno de estos dos es un [`ConfigError::UnknownLogFormat`], no un silencioso
/// «usa el defecto»: quien escribe `LOG_FORMAT` cree que está configurando algo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LogFormat {
    /// Registro de texto, legible por una persona (el defecto).
    #[default]
    Text,
    /// Registro JSON, una línea por suceso, para que lo parsee un colector.
    Json,
}

impl LogFormat {
    /// Interpreta el valor de `LOG_FORMAT`; `None` si no es ninguno de los dos.
    fn from_value(value: &str) -> Option<Self> {
        match value {
            "text" => Some(LogFormat::Text),
            "json" => Some(LogFormat::Json),
            _ => None,
        }
    }
}

/// Configuración efectiva del servicio.
///
/// Se construye una sola vez, al arrancar, con [`Config::from_env`].
#[derive(Debug, Clone)]
pub struct Config {
    /// Puerto de escucha en `0.0.0.0`.
    pub port: u16,
    /// Filtro de `tracing-subscriber` (p. ej. `info`, `errserve=debug`).
    pub log_level: String,
    /// Formato de los registros: texto o JSON.
    pub log_format: LogFormat,
    /// Directorio de páginas HTML de error y assets estáticos.
    pub content_dir: PathBuf,
    /// Código que se sirve cuando la petición no aporta ningún código.
    pub default_error_page: u16,
    /// Segundos de la cabecera `Retry-After` de los códigos que la llevan
    /// (`429` y `503`). `0` es válido («reintenta ya»).
    pub retry_after: u32,
    /// El **registro efectivo** de temas: los siete incorporados al binario más
    /// los descubiertos en la raíz de `content_dir` **al arrancar**.
    ///
    /// La `Config` **posee** el registro (no es una tabla estática) porque un tema
    /// descubierto lleva un nombre que solo se conoce en ejecución, y ya viaja por
    /// referencia a cada petición dentro del estado del router.
    pub themes: Themes,
    /// El tema del servicio, **ya validado** contra `themes`: la página del tema
    /// que se sirve en el HTML. Es por **valor** (una copia corta) y no un
    /// `&'static`, porque un tema descubierto no vive en el binario.
    pub theme: Theme,
}

/// Error de arranque por una variable de entorno presente pero inválida.
///
/// Cada variante lleva el valor tal cual llegó —la cadena vacía incluida—, para
/// que el mensaje pueda señalarlo sin ambigüedad. Los nombres no comparten
/// prefijo a propósito: clippy (`enum_variant_names`) marca un enumerado en el
/// que todas las variantes empiezan igual, y aquí el prefijo no aportaría nada
/// que no diga ya el tipo; cada nombre dice qué le pasó a *ese* valor.
pub enum ConfigError {
    /// `PORT` no es un entero sin signo de 16 bits (la cadena vacía incluida).
    InvalidPort(String),
    /// `LOG_FORMAT` no es `text` ni `json` (la cadena vacía incluida).
    UnknownLogFormat(String),
    /// `DEFAULT_ERROR_PAGE` no es un código de error utilizable
    /// (`400..=599`), o no es un entero sin signo de 16 bits.
    DefaultErrorPageOutOfRange(String),
    /// `RETRY_AFTER` no es un entero sin signo de 32 bits (la cadena vacía
    /// incluida).
    InvalidRetryAfter(String),
    /// `THEME` está presente pero no es un nombre del **registro** del arranque
    /// (la cadena vacía incluida). Lleva el valor ofensor y los nombres del
    /// registro, para que el mensaje enumere lo que sí vale **en ese arranque**.
    UnknownTheme {
        /// El valor tal cual llegó —la cadena vacía incluida—.
        value: String,
        /// Los nombres del registro del arranque, ya unidos por `", "`.
        validos: String,
    },
}

impl fmt::Debug for ConfigError {
    /// El `Debug` de este error **es** su mensaje.
    ///
    /// `main` devuelve `Result<(), Box<dyn std::error::Error>>`, así que el
    /// runtime imprime el `Debug` del error al abortar el arranque: con un
    /// `Debug` derivado el operador vería `UnknownLogFormat("basura")`, que no
    /// nombra la variable y por tanto no le dice qué corregir. La spec exige que
    /// el error **nombre `LOG_FORMAT`**, así que el canal por el que sale el
    /// mensaje es el que tiene que llevarlo.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::InvalidPort(value) => write!(
                f,
                "`{PORT_VAR}` está presente pero no es un puerto válido: {value:?} \
                 (¿una variable vacía?). Omitir la variable usa {DEFAULT_PORT}."
            ),
            ConfigError::UnknownLogFormat(value) => write!(
                f,
                "`{LOG_FORMAT_VAR}` está presente pero no es un formato de registro válido: \
                 {value:?}. Se espera `text` o `json` (¿una variable vacía?). Omitir la \
                 variable usa `text`."
            ),
            ConfigError::DefaultErrorPageOutOfRange(value) => write!(
                f,
                "`{DEFAULT_ERROR_PAGE_VAR}` está presente pero no es un código de error \
                 utilizable: {value:?}. Se espera un entero entre {MIN_ERROR_CODE} y \
                 {MAX_ERROR_CODE} (¿una variable vacía?). Omitir la variable usa \
                 {DEFAULT_ERROR_PAGE}."
            ),
            ConfigError::InvalidRetryAfter(value) => write!(
                f,
                "`{RETRY_AFTER_VAR}` está presente pero no son segundos válidos: {value:?}. \
                 Se espera un entero de segundos (¿una variable vacía?). Omitir la variable \
                 usa {DEFAULT_RETRY_AFTER}."
            ),
            ConfigError::UnknownTheme { value, validos } => write!(
                f,
                "`{THEME_VAR}` está presente pero no es un tema válido: {value:?}. \
                 Se espera uno de {validos} (¿una variable vacía?). Omitir la variable \
                 usa `{DEFAULT_THEME_NAME}`."
            ),
        }
    }
}

impl std::error::Error for ConfigError {}

impl Config {
    /// Construye la configuración a partir de un iterador de pares
    /// `(clave, valor)`. Es la vía inyectable: permite probar sin tocar el
    /// entorno del proceso (ninguna prueba puede mutar `std::env`).
    ///
    /// `PORT` y `DEFAULT_ERROR_PAGE` son obligatoriamente numéricas cuando
    /// están presentes: una variable presente pero vacía no cae al defecto,
    /// es un [`ConfigError`]. `DEFAULT_ERROR_PAGE` tiene que estar además en el
    /// rango utilizable `400..=599`. `LOG_FORMAT` tiene que ser `text` o `json`
    /// con el mismo criterio, y `THEME` un nombre del **registro** —los siete
    /// incorporados **más** los descubiertos en `content_dir`—, incluida la
    /// cadena vacía en el rechazo. `RETRY_AFTER` también es obligatoriamente
    /// numérica cuando está presente, pero **sin** rango: `0` es válido. Las
    /// claves desconocidas se ignoran.
    ///
    /// El **orden importa**: `CONTENT_DIR` se parsea **antes** del bloque del
    /// tema, porque el registro se construye a partir de él.
    ///
    /// # Errors
    ///
    /// Devuelve [`ConfigError::InvalidPort`] si `PORT` está presente y su valor
    /// no se puede interpretar como `u16`, [`ConfigError::UnknownLogFormat`] si
    /// `LOG_FORMAT` está presente y su valor no es `text` ni `json`,
    /// [`ConfigError::DefaultErrorPageOutOfRange`] si `DEFAULT_ERROR_PAGE` está
    /// presente y su valor no es un entero de `400` a `599`,
    /// [`ConfigError::InvalidRetryAfter`] si `RETRY_AFTER` está presente y su
    /// valor no se puede interpretar como `u32`, y [`ConfigError::UnknownTheme`]
    /// si `THEME` está presente y su valor no es un nombre del **registro** del
    /// arranque.
    pub fn from_env_iter<I>(iter: I) -> Result<Config, ConfigError>
    where
        I: IntoIterator<Item = (String, String)>,
    {
        let vars: HashMap<String, String> = iter.into_iter().collect();

        // Una variable presente pero vacía no es una variable ausente: el
        // parseo falla y el arranque se aborta con un error tipado.
        let port = match vars.get(PORT_VAR) {
            Some(raw) => raw
                .parse::<u16>()
                .map_err(|_| ConfigError::InvalidPort(raw.clone()))?,
            None => DEFAULT_PORT,
        };

        // `DEFAULT_ERROR_PAGE` pasa dos controles: tiene que ser numérica y
        // estar en el rango utilizable (`400..=599`). Un defecto fuera de rango
        // haría que toda petición sin pistas —incluida `/`— respondiera un
        // código que no es una página de error (`200`, `204`…), así que es un
        // error de arranque, no un valor degradado en caliente.
        let default_error_page = match vars.get(DEFAULT_ERROR_PAGE_VAR) {
            Some(raw) => {
                let code = raw
                    .parse::<u16>()
                    .map_err(|_| ConfigError::DefaultErrorPageOutOfRange(raw.clone()))?;

                if !is_usable_code(code) {
                    return Err(ConfigError::DefaultErrorPageOutOfRange(raw.clone()));
                }

                code
            }
            None => DEFAULT_ERROR_PAGE,
        };

        // `RETRY_AFTER` es un `delay-seconds` de RFC 9110: solo la forma
        // numérica, nunca una fecha HTTP (aceptar texto libre convertiría una
        // variable de entorno en un canal para escribir cabeceras arbitrarias).
        // No se acota: `0` significa «reintenta ya» y un valor enorme no rompe
        // ningún contrato. Pero una variable presente y no interpretable —la
        // cadena vacía incluida— sí aborta el arranque, como las demás
        // numéricas: quien la escribe cree que está configurando algo.
        let retry_after = match vars.get(RETRY_AFTER_VAR) {
            Some(raw) => raw
                .parse::<u32>()
                .map_err(|_| ConfigError::InvalidRetryAfter(raw.clone()))?,
            None => DEFAULT_RETRY_AFTER,
        };

        // `LOG_FORMAT` solo admite los dos formatos que el servicio sabe
        // emitir. Igual que con las numéricas, una variable presente pero vacía
        // es un error: no hay un tercer valor del que este enumerado pueda
        // hacer de chivo expiatorio.
        let log_format = match vars.get(LOG_FORMAT_VAR) {
            Some(raw) => LogFormat::from_value(raw)
                .ok_or_else(|| ConfigError::UnknownLogFormat(raw.clone()))?,
            None => DEFAULT_LOG_FORMAT,
        };

        // Estas dos son cadenas: no hay valor inválido que rechazar, pero
        // también se respetan tal cual cuando llegan vacías (nunca se
        // sustituyen por el defecto).
        let log_level = match vars.get(LOG_LEVEL_VAR) {
            Some(value) => value.clone(),
            None => DEFAULT_LOG_LEVEL.to_string(),
        };

        // `CONTENT_DIR` se parsea **antes** del tema: el registro se construye a
        // partir de él, así que el barrido de temas lo necesita primero.
        let content_dir = match vars.get(CONTENT_DIR_VAR) {
            Some(value) => PathBuf::from(value),
            None => PathBuf::from(DEFAULT_CONTENT_DIR),
        };

        // El registro se construye **una vez**, al arrancar: los siete
        // incorporados más los descubiertos en la raíz de `content_dir`. El
        // barrido es al arrancar (no por petición), así que **añadir** un tema
        // exige reiniciar; los **bytes** de la página se leen al servir, así que
        // **editar** uno que ya estaba se ve sin reiniciar.
        let themes = Themes::descubrir(&content_dir);

        // `THEME` la escribe el operador y selecciona de los nombres del
        // **registro** —los siete incorporados más los descubiertos en
        // `content_dir`—, así que un valor que no esté en él (la cadena vacía
        // incluida) aborta el arranque: quien la escribe cree que está
        // configurando algo, y un `default` silencioso le ocultaría el error. El
        // canal de la cabecera `X-Theme` es el otro carril, y ahí un valor
        // desconocido cae al configurado sin error (ver [`crate::theme::resolve`]).
        // Sin la variable, el tema es el de siempre: `default`.
        //
        // **El precio**: el error enumera los nombres del registro **de ese
        // arranque**, así que un `THEME` correcto cuyo fichero todavía no esté en
        // `content_dir` —un volumen sin montar— aborta el arranque. Es la
        // contrapartida de mantener el aborto, y está escrita para que no
        // sorprenda.
        let theme = match vars.get(THEME_VAR) {
            Some(raw) => themes
                .by_name(raw)
                .ok_or_else(|| ConfigError::UnknownTheme {
                    value: raw.clone(),
                    validos: themes.names(),
                })?
                .clone(),
            None => theme::default_theme().clone(),
        };

        Ok(Config {
            port,
            log_level,
            log_format,
            content_dir,
            default_error_page,
            retry_after,
            themes,
            theme,
        })
    }

    /// Azúcar: [`Config::from_env_iter`] sobre `std::env::vars_os()`.
    ///
    /// Se usa `vars_os` (y no `vars`) porque `std::env::vars` entra en pánico
    /// si alguna variable no es UTF-8; aquí esos pares simplemente se omiten.
    ///
    /// # Errors
    ///
    /// Las mismas que [`Config::from_env_iter`].
    pub fn from_env() -> Result<Config, ConfigError> {
        let vars = std::env::vars_os().filter_map(|(key, value)| {
            let key = key.into_string().ok()?;
            let value = value.into_string().ok()?;
            Some((key, value))
        });

        Self::from_env_iter(vars)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Construye un iterador de pares `(clave, valor)` como el de `env::vars`.
    fn env(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect()
    }

    /// Un `CONTENT_DIR` que **no existe**: el registro son los siete incorporados y
    /// nada más (el caso del binario instalado sin ficheros de datos).
    fn sin_contenido_dir() -> String {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/inexistente-y-ademas-inexistente");
        assert!(
            !dir.exists(),
            "el fixture `sin contenido` no puede existir: {}",
            dir.display()
        );
        dir.to_string_lossy().into_owned()
    }

    /// El `CONTENT_DIR` de los fixtures con un tema de más (`neon.html`).
    fn temas_extras_dir() -> String {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/temas-extras")
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn sin_port_el_puerto_por_defecto_es_8080() {
        let config = Config::from_env_iter(env(&[])).unwrap();
        assert_eq!(config.port, 8080);
    }

    #[test]
    fn port_cambia_el_puerto() {
        let config = Config::from_env_iter(env(&[("PORT", "9999")])).unwrap();
        assert_eq!(config.port, 9999);
    }

    #[test]
    fn el_resto_de_defectos_de_la_spec() {
        let config = Config::from_env_iter(env(&[])).unwrap();
        assert_eq!(config.log_level, "info");
        assert_eq!(config.content_dir, PathBuf::from("content"));
        assert_eq!(config.default_error_page, 404);
    }

    #[test]
    fn las_variables_del_entorno_se_respetan() {
        let config = Config::from_env_iter(env(&[
            ("PORT", "8081"),
            ("LOG_LEVEL", "debug"),
            ("CONTENT_DIR", "/srv/pages"),
            ("DEFAULT_ERROR_PAGE", "503"),
        ]))
        .unwrap();
        assert_eq!(config.port, 8081);
        assert_eq!(config.log_level, "debug");
        assert_eq!(config.content_dir, PathBuf::from("/srv/pages"));
        assert_eq!(config.default_error_page, 503);
    }

    #[test]
    fn una_variable_presente_pero_vacia_no_es_una_variable_ausente() {
        // `PORT=""` está presente: no es un 8080 silencioso, es un error tipado.
        let result = Config::from_env_iter(env(&[("PORT", "")]));
        assert!(result.is_err(), "una PORT vacía debe ser un error, no 8080");

        // `DEFAULT_ERROR_PAGE=""` incurre en lo mismo.
        let result = Config::from_env_iter(env(&[("DEFAULT_ERROR_PAGE", "")]));
        assert!(
            result.is_err(),
            "un DEFAULT_ERROR_PAGE vacío debe ser un error, no 404"
        );
    }

    #[test]
    fn una_port_no_numerica_es_un_error_tipado() {
        assert!(Config::from_env_iter(env(&[("PORT", "ochenta")])).is_err());
        assert!(Config::from_env_iter(env(&[("DEFAULT_ERROR_PAGE", "muchos")])).is_err());
    }

    #[test]
    fn un_default_error_page_fuera_de_rango_no_arranca() {
        // Auditoría B-2: `DEFAULT_ERROR_PAGE=200` haría que toda petición sin
        // pistas respondiera `200`, justo lo que el proyecto corrige.
        for value in ["200", "100", "204", "304", "0", "399", "600", "1000"] {
            let result = Config::from_env_iter(env(&[("DEFAULT_ERROR_PAGE", value)]));

            match result {
                Err(err) => {
                    let message = err.to_string();
                    assert!(
                        message.contains("DEFAULT_ERROR_PAGE"),
                        "el error de `{value}` debe nombrar la variable: {message}"
                    );
                }
                Ok(config) => panic!(
                    "`DEFAULT_ERROR_PAGE={value}` debe ser un error de arranque, no {:?}",
                    config.default_error_page
                ),
            }
        }
    }

    #[test]
    fn un_default_error_page_del_rango_arranca() {
        for (value, expected) in [("400", 400), ("404", 404), ("503", 503), ("599", 599)] {
            let config = Config::from_env_iter(env(&[("DEFAULT_ERROR_PAGE", value)])).unwrap();
            assert_eq!(config.default_error_page, expected);
        }
    }

    #[test]
    fn las_claves_desconocidas_se_ignoran() {
        let config =
            Config::from_env_iter(env(&[("PATH", "/usr/bin"), ("HOME", "/root")])).unwrap();
        assert_eq!(config.port, 8080);
        assert_eq!(config.default_error_page, 404);
    }

    // --- LOG_FORMAT (change `release-hardening`, tasks.md 1.8) -------------
    //
    // Estas tres pruebas observan el `Debug` de `Config` en vez del campo
    // tipado: `assert_eq!(config.log_format, LogFormat::Json)` leería el tipo
    // directamente. Sigue siendo una deuda pendiente —a diferencia de las de
    // `RETRY_AFTER`, ya tipadas—, porque convertir estas aserciones es trabajo
    // de otro change. El `Debug` observado exige un campo `log_format` cuyo
    // `Debug` sea un identificador PascalCase (`Json`) y no la cadena `"json"`,
    // de modo que un `String` suelto no satisface la prueba.

    #[test]
    fn log_format_json_declara_el_formato_json() {
        let config = Config::from_env_iter(env(&[("LOG_FORMAT", "json")])).unwrap();
        let debug = format!("{config:?}");

        assert!(
            debug.contains("log_format"),
            "el `Config` debe declarar el formato de registro: {debug}"
        );
        assert!(
            debug.contains("Json"),
            "`LOG_FORMAT=json` debe declarar el formato JSON: {debug}"
        );
    }

    #[test]
    fn log_format_text_declara_el_formato_de_texto() {
        let config = Config::from_env_iter(env(&[("LOG_FORMAT", "text")])).unwrap();
        let debug = format!("{config:?}");

        assert!(
            debug.contains("log_format"),
            "el `Config` debe declarar el formato de registro: {debug}"
        );
        assert!(
            !debug.contains("Json"),
            "`LOG_FORMAT=text` no puede declarar el formato JSON: {debug}"
        );
    }

    #[test]
    fn sin_log_format_el_defecto_es_texto() {
        // El defecto es el registro actual, en texto: no un JSON silencioso.
        let config = Config::from_env_iter(env(&[])).unwrap();
        let debug = format!("{config:?}");

        assert!(
            debug.contains("log_format"),
            "el `Config` debe declarar el formato de registro: {debug}"
        );
        assert!(
            !debug.contains("Json"),
            "sin `LOG_FORMAT` el defecto es texto, no JSON: {debug}"
        );
    }

    #[test]
    fn un_log_format_desconocido_no_arranca() {
        // Un valor distinto de `text` y `json` es error de arranque, no un
        // silencioso «usa el defecto»: quien lo escribe cree que configura algo.
        match Config::from_env_iter(env(&[("LOG_FORMAT", "basura")])) {
            Err(err) => {
                let message = err.to_string();
                assert!(
                    message.contains("LOG_FORMAT"),
                    "el error debe nombrar la variable: {message}"
                );
            }
            Ok(config) => {
                panic!("`LOG_FORMAT=basura` debe ser un error de arranque, no {config:?}")
            }
        }
    }

    #[test]
    fn un_log_format_presente_pero_vacio_no_arranca() {
        // Misma decisión que con `PORT` y `DEFAULT_ERROR_PAGE`: una variable
        // presente pero vacía no es una variable ausente.
        match Config::from_env_iter(env(&[("LOG_FORMAT", "")])) {
            Err(err) => {
                let message = err.to_string();
                assert!(
                    message.contains("LOG_FORMAT"),
                    "el error debe nombrar la variable: {message}"
                );
            }
            Ok(config) => {
                panic!("un `LOG_FORMAT` vacío debe ser un error de arranque, no {config:?}")
            }
        }
    }

    // --- RETRY_AFTER (change `retry-after-configurable`, tasks.md 1.1-1.4) -
    //
    // La fase RED observó el `Debug` de `Config` —el campo no existía aún— para
    // fallar por aserción y no por compilación; con el campo ya `pub`, las
    // aserciones de abajo son tipadas.

    #[test]
    fn sin_retry_after_el_defecto_es_120() {
        let config = Config::from_env_iter(env(&[])).unwrap();

        assert_eq!(
            config.retry_after, 120,
            "sin `RETRY_AFTER` el defecto es 120, el valor actual y el de `error-pages`: {config:?}"
        );
    }

    #[test]
    fn el_retry_after_configurado_se_respeta() {
        let config = Config::from_env_iter(env(&[("RETRY_AFTER", "30")])).unwrap();

        assert_eq!(
            config.retry_after, 30,
            "`RETRY_AFTER=30` debe respetarse: {config:?}"
        );
    }

    #[test]
    fn un_retry_after_presente_pero_invalido_no_arranca() {
        // Una variable presente pero vacía no es una variable ausente, y una no
        // numérica tampoco cae al defecto en silencio: quien escribe
        // `RETRY_AFTER` cree que está configurando algo. La forma numérica es la
        // única admitida (un `delay-seconds` de RFC 9110), así que `-1` —que no
        // es un `u32`— tampoco vale.
        for value in ["", "medio minuto", "-1"] {
            match Config::from_env_iter(env(&[("RETRY_AFTER", value)])) {
                Err(err) => {
                    let message = err.to_string();
                    assert!(
                        message.contains("RETRY_AFTER"),
                        "el error de `{value:?}` debe nombrar la variable: {message}"
                    );
                }
                Ok(config) => {
                    panic!("`RETRY_AFTER={value:?}` debe ser un error de arranque, no {config:?}")
                }
            }
        }
    }

    #[test]
    fn un_retry_after_cero_arranca() {
        // `0` es un `delay-seconds` legal de RFC 9110 («reintenta ya»), no un
        // error: la spec no acota la variable por arriba ni por abajo, a
        // diferencia de `DEFAULT_ERROR_PAGE`, cuyo rango sí importa.
        let config = Config::from_env_iter(env(&[("RETRY_AFTER", "0")]))
            .expect("`RETRY_AFTER=0` debe ser válido, no un error de arranque");

        assert_eq!(
            config.retry_after, 0,
            "`RETRY_AFTER=0` debe llegar tal cual, no como 120: {config:?}"
        );
    }

    // --- THEME (change `themes-as-pages`, tasks.md de la fase GREEN) --------
    //
    // `THEME` la escribe el **operador**, así que se comporta como `LOG_FORMAT`
    // y `DEFAULT_ERROR_PAGE`: un valor que no sea uno de los **siete** temas
    // incorporados —o la cadena vacía— aborta el arranque.

    #[test]
    fn un_theme_desconocido_no_arranca() {
        // El registro de este arranque son los siete incorporados y nada más
        // (`CONTENT_DIR` no existe), así que `THEME=neon` no es un nombre válido.
        let dir = sin_contenido_dir();
        let err = Config::from_env_iter(env(&[("CONTENT_DIR", dir.as_str()), ("THEME", "neon")]))
            .expect_err("`THEME=neon` debe ser un error de arranque, no un `Ok`");
        let message = err.to_string();

        assert!(
            message.contains("THEME"),
            "el error debe nombrar la variable: {message}"
        );

        // La comprobación de la lista es **a propósito**: quien escribe `THEME`
        // cree que está configurando algo, así que el error tiene que decirle
        // qué puede escribir en su lugar —los nombres del registro—, no solo
        // que se equivocó.
        for nombre in [
            "default", "minimal", "terminal", "paper", "amber", "compact", "matrix",
        ] {
            assert!(
                message.contains(nombre),
                "el error tiene que enumerar los temas válidos: falta `{nombre}` en {message}"
            );
        }
    }

    #[test]
    fn el_theme_matrix_es_valido() {
        // `matrix` es el séptimo tema: `THEME=matrix` tiene que arrancar, no ser
        // un valor desconocido.
        let dir = sin_contenido_dir();
        let config =
            Config::from_env_iter(env(&[("CONTENT_DIR", dir.as_str()), ("THEME", "matrix")]))
                .expect("`THEME=matrix` es uno de los siete temas válidos");
        assert_eq!(
            config.theme.name, "matrix",
            "`THEME=matrix` tiene que elegir el tema `matrix`"
        );
    }

    #[test]
    fn un_theme_presente_pero_vacio_no_arranca() {
        // Misma decisión que con las demás variables: una variable presente
        // pero vacía no es una variable ausente. Un `THEME=""` es un error que
        // nombra la variable, no un `default` silencioso.
        let dir = sin_contenido_dir();
        let err = Config::from_env_iter(env(&[("CONTENT_DIR", dir.as_str()), ("THEME", "")]))
            .expect_err("un `THEME` vacío debe ser un error de arranque, no `default`");
        let message = err.to_string();

        assert!(
            message.contains("THEME"),
            "el error de `THEME` vacío debe nombrar la variable: {message}"
        );
    }

    // --- THEME contra el registro efectivo (change `themes-from-content-dir`) ---
    //
    // El registro deja de ser una tabla cerrada: son los siete incorporados
    // **más** los `*.html` de la raíz de `CONTENT_DIR`, descubiertos al arrancar.
    // `THEME` valida contra ese registro —y sigue abortando el arranque si su
    // valor no está en él—, y el error enumera los nombres **de ese arranque**.

    #[test]
    fn un_theme_descubierto_arranca() {
        // Escenario «un THEME descubierto arranca»: con un `CONTENT_DIR` que trae
        // `neon.html`, `THEME=neon` es un nombre del registro —los siete
        // incorporados **más** lo descubierto— y el arranque no aborta.
        let dir = temas_extras_dir();
        let config =
            Config::from_env_iter(env(&[("CONTENT_DIR", dir.as_str()), ("THEME", "neon")]))
                .expect("un tema descubierto en CONTENT_DIR tiene que ser un `THEME` válido");

        assert_eq!(
            config.theme.name, "neon",
            "`THEME=neon` tiene que elegir el tema descubierto `neon`"
        );
        assert!(
            config.theme.embedded.is_none(),
            "un tema descubierto **no** tiene copia embebida: su fichero es su única copia"
        );
        assert!(
            config.themes.by_name("neon").is_some(),
            "el tema descubierto tiene que estar en el registro efectivo"
        );
    }

    #[test]
    fn un_theme_correcto_con_el_volumen_sin_montar_no_arranca() {
        // El **precio** de mantener el aborto: el error enumera los nombres que el
        // registro tenga **en ese arranque**, así que un `THEME` **correcto** cuyo
        // fichero **todavía** no esté en `CONTENT_DIR` —un volumen que aún no se ha
        // montado cuando el contenedor arranca— **aborta** el arranque. Es la
        // contrapartida de la validación estricta: la variable la escribe el
        // operador, y un arranque con un tema que no existe es peor que un arranque
        // que falla diciendo por qué. La mitigación es del operador: declarar la
        // dependencia del volumen, o arrancar sin `THEME` y con el defecto.
        let dir = sin_contenido_dir();
        let err = Config::from_env_iter(env(&[("CONTENT_DIR", dir.as_str()), ("THEME", "neon")]))
            .expect_err("`THEME=neon` sin `neon.html` en CONTENT_DIR tiene que abortar");
        let message = err.to_string();

        assert!(
            message.contains("THEME"),
            "el error debe nombrar la variable: {message}"
        );

        // Enumera los siete del registro **de ese arranque**, y **no** `neon`, que
        // es justo lo que el operador cree que ha configurado.
        for nombre in [
            "default", "minimal", "terminal", "paper", "amber", "compact", "matrix",
        ] {
            assert!(
                message.contains(nombre),
                "el error tiene que enumerar los temas del registro: falta `{nombre}` en {message}"
            );
        }
        // El mensaje **cita** el valor ofensor, como los otros cuatro errores de
        // configuración, que citan el suyo: el operador tiene que ver qué escribió.
        // La comprobación de que `neon` **no** está entre los válidos es entonces
        // sobre la **enumeración**, no sobre el mensaje entero.
        assert!(
            message.contains("\"neon\""),
            "el error tiene que citar el valor ofensor, como los demás errores de \
             configuración: {message}"
        );

        let enumeracion = "default, minimal, terminal, paper, amber, compact, matrix";
        assert!(
            message.contains(enumeracion),
            "el error tiene que enumerar los nombres del registro de ese arranque, en orden: \
             {message}"
        );
        assert!(
            !message.contains(&format!("{enumeracion}, neon")),
            "sin el fichero, `neon` no está en el registro y no puede aparecer en la \
             enumeración: {message}"
        );
    }

    #[test]
    fn el_error_enumera_los_nombres_del_registro() {
        // Lo que distingue el **registro efectivo** de la constante de antes: con
        // un `CONTENT_DIR` con temas propios, el error enumera los que **ese
        // arranque** tiene —los siete incorporados **y** `neon`—, no una lista
        // fija.
        let dir = temas_extras_dir();
        let err =
            Config::from_env_iter(env(&[("CONTENT_DIR", dir.as_str()), ("THEME", "noexiste")]))
                .expect_err("`THEME=noexiste` no es un nombre del registro: tiene que abortar");
        let message = err.to_string();

        for nombre in [
            "default", "minimal", "terminal", "paper", "amber", "compact", "matrix", "neon",
        ] {
            assert!(
                message.contains(nombre),
                "el error tiene que enumerar los nombres del registro de ese arranque: falta \
                 `{nombre}` en {message}"
            );
        }
    }
}
