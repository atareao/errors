//! Configuración de la aplicación, leída **solo** del entorno.
//!
//! ---------------------------------------------------------------------------
//! API que asume la fase RED (la fase GREEN debe implementarla tal cual):
//!
//! ```ignore
//! #[derive(Debug, Clone)]
//! pub struct Config {
//!     pub port: u16,
//!     pub log_level: String,
//!     pub content_dir: PathBuf,
//!     pub default_error_page: u16,
//! }
//!
//! #[derive(Debug)]
//! pub enum ConfigError {
//!     InvalidPort(String),
//!     InvalidDefaultErrorPage(String),
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
//! | `CONTENT_DIR`        | `content` |
//! | `DEFAULT_ERROR_PAGE` | `404`     |
//!
//! No se lee ningún fichero de configuración (`config.yml` ni similar).
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

/// Puerto de escucha por defecto (el que asume la guía de Traefik).
const DEFAULT_PORT: u16 = 8080;
/// Filtro de registro por defecto.
const DEFAULT_LOG_LEVEL: &str = "info";
/// Directorio de páginas y assets por defecto. Es **la única** ruta de
/// estáticos: la comparten el código, el `Dockerfile` y `.dockerignore`.
const DEFAULT_CONTENT_DIR: &str = "content";
/// Código de error por defecto cuando la petición no da ninguna pista.
const DEFAULT_ERROR_PAGE: u16 = 404;

const PORT_VAR: &str = "PORT";
const LOG_LEVEL_VAR: &str = "LOG_LEVEL";
const CONTENT_DIR_VAR: &str = "CONTENT_DIR";
const DEFAULT_ERROR_PAGE_VAR: &str = "DEFAULT_ERROR_PAGE";

/// Configuración efectiva del servicio.
///
/// Se construye una sola vez, al arrancar, con [`Config::from_env`].
#[derive(Debug, Clone)]
pub struct Config {
    /// Puerto de escucha en `0.0.0.0`.
    pub port: u16,
    /// Filtro de `tracing-subscriber` (p. ej. `info`, `errors=debug`).
    pub log_level: String,
    /// Directorio de páginas HTML de error y assets estáticos.
    pub content_dir: PathBuf,
    /// Código que se sirve cuando la petición no aporta ningún código.
    pub default_error_page: u16,
}

/// Error de arranque por una variable de entorno presente pero inválida.
#[derive(Debug)]
pub enum ConfigError {
    /// `PORT` no es un entero sin signo de 16 bits (la cadena vacía incluida).
    InvalidPort(String),
    /// `DEFAULT_ERROR_PAGE` no es un código de error utilizable
    /// (`400..=599`), o no es un entero sin signo de 16 bits.
    InvalidDefaultErrorPage(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::InvalidPort(value) => write!(
                f,
                "`{PORT_VAR}` está presente pero no es un puerto válido: {value:?} \
                 (¿una variable vacía?). Omitir la variable usa {DEFAULT_PORT}."
            ),
            ConfigError::InvalidDefaultErrorPage(value) => write!(
                f,
                "`{DEFAULT_ERROR_PAGE_VAR}` está presente pero no es un código de error \
                 utilizable: {value:?}. Se espera un entero entre {MIN_ERROR_CODE} y \
                 {MAX_ERROR_CODE} (¿una variable vacía?). Omitir la variable usa \
                 {DEFAULT_ERROR_PAGE}."
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
    /// rango utilizable `400..=599`. Las claves desconocidas se ignoran.
    ///
    /// # Errors
    ///
    /// Devuelve [`ConfigError::InvalidPort`] si `PORT` está presente y su valor
    /// no se puede interpretar como `u16`, y
    /// [`ConfigError::InvalidDefaultErrorPage`] si `DEFAULT_ERROR_PAGE` está
    /// presente y su valor no es un entero de `400` a `599`.
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
                    .map_err(|_| ConfigError::InvalidDefaultErrorPage(raw.clone()))?;

                if !is_usable_code(code) {
                    return Err(ConfigError::InvalidDefaultErrorPage(raw.clone()));
                }

                code
            }
            None => DEFAULT_ERROR_PAGE,
        };

        // Estas dos son cadenas: no hay valor inválido que rechazar, pero
        // también se respetan tal cual cuando llegan vacías (nunca se
        // sustituyen por el defecto).
        let log_level = match vars.get(LOG_LEVEL_VAR) {
            Some(value) => value.clone(),
            None => DEFAULT_LOG_LEVEL.to_string(),
        };

        let content_dir = match vars.get(CONTENT_DIR_VAR) {
            Some(value) => PathBuf::from(value),
            None => PathBuf::from(DEFAULT_CONTENT_DIR),
        };

        Ok(Config {
            port,
            log_level,
            content_dir,
            default_error_page,
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
}
