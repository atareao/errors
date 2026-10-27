//! Temas de las páginas de error: **un tema es una página**.
//!
//! Un **tema** es **una página**: su **nombre** es el de su fichero y la ruta se
//! **deriva** de él —`CONTENT_DIR/<nombre>.html`, en la **raíz** de
//! `CONTENT_DIR`—, de modo que no exista una tabla de rutas que pueda divergir
//! del nombre. Las páginas van **embebidas** en el binario
//! ([`crate::embedded`]) tras descubrirlas al compilar, así que un servicio
//! arrancado sin `content/` sirve los diseños, y borrar una página es un
//! **error de compilación**, no un respaldo silencioso.
//!
//! ---------------------------------------------------------------------------
//! API pública del módulo:
//!
//! ```ignore
//! use std::borrow::Cow;
//!
//! #[derive(Debug, Clone)]
//! pub struct Theme {
//!     pub name: Cow<'static, str>,
//!     pub embedded: Option<&'static str>,   // la página embebida, si es incorporado
//! }
//!
//! pub const DEFAULT_THEME_NAME: &str = "default";
//!
//! #[derive(Debug, Clone)]
//! pub struct Themes { /* el registro efectivo, en orden */ }
//!
//! impl Themes {
//!     pub fn incorporados() -> Self;
//!     pub fn descubrir(content_dir: &Path) -> Self;
//!     pub fn by_name(&self, name: &str) -> Option<&Theme>;
//!     pub fn names(&self) -> String;
//!     pub fn iter(&self) -> std::slice::Iter<'_, Theme>;
//! }
//!
//! pub fn resolve<'a>(
//!     registry: &'a Themes,
//!     requested: Option<&str>,
//!     configured: &'a Theme,
//! ) -> &'a Theme;
//!
//! impl Theme {
//!     pub fn page_path(&self) -> String;
//!     pub fn page(&self, content_dir: &Path) -> Result<Vec<u8>, std::io::Error>;
//! }
//! ```
//!
//! # El registro: embebidos **más** descubiertos, al arrancar
//!
//! El **registro efectivo** de temas es lo que se **descubrió al compilar** y
//! viaja dentro del binario ([`crate::embedded::EMBEBIDOS`]) **más** los
//! **descubiertos** en la raíz de `CONTENT_DIR` al arrancar
//! ([`Themes::descubrir`]). Los embebidos se **quedan**: un binario instalado sin
//! ficheros de datos sigue sirviendo los diseños, y el directorio del operador
//! **añade**, no sustituye.
//!
//! **El precio, escrito**: el barrido es **al arrancar**, así que **añadir** un
//! tema exige reiniciar; los **bytes** de la página se leen **al servir**, así que
//! **editar** un tema que ya estaba se ve sin reiniciar. El servicio no juzga el
//! contenido de ninguna página: las páginas son **opacas**.
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

use crate::embedded;
use std::borrow::Cow;
use std::io::ErrorKind;
use std::path::Path;

/// Un tema: su **nombre** y, si es incorporado, la página embebida que lo define.
///
/// El **nombre** es lo que se escribe en `THEME` y en `X-Theme`, y de él se
/// **deriva** la ruta de su página (`<nombre>.html`): la ruta **no** es un campo,
/// así que no puede divergir del nombre. Es un [`Cow`] porque un tema
/// **incorporado** tiene un nombre de cadena literal ([`Cow::Borrowed`]) y uno
/// **descubierto** en `CONTENT_DIR` tiene un nombre que solo se conoce al arrancar
/// ([`Cow::Owned`]).
///
/// `embedded` es la copia de respaldo de [`Theme::page`]: `Some` para los
/// **embebidos** —que van dentro del binario, así que un servicio sin ficheros de
/// datos sirve los diseños— y `None` para los descubiertos, cuya **única** copia es
/// su fichero.
#[derive(Debug, Clone)]
pub struct Theme {
    /// Nombre público del tema, el que se escribe en `THEME` y en `X-Theme`, y
    /// del que se deriva la ruta de su página (`<nombre>.html`).
    pub name: Cow<'static, str>,
    /// La página del tema embebida en el binario, si es un **embebido**; `None`
    /// para un tema descubierto, cuya única copia es su fichero.
    pub embedded: Option<&'static str>,
}

/// Nombre del tema por defecto, que es también el del tema `default`.
///
/// Se conserva como valor documentado de `THEME`: el registro base se construye
/// desde [`crate::embedded::EMBEBIDOS`], descubierto al compilar, así que el tema
/// por defecto tiene que estar entre los embebidos.
pub const DEFAULT_THEME_NAME: &str = "default";

/// El **registro efectivo** de temas: los embebidos **más** los descubiertos en la
/// raíz de `CONTENT_DIR` **al arrancar**.
///
/// Es un valor **con dueño** (no una tabla estática) porque un tema descubierto
/// lleva un nombre que solo se conoce en ejecución. El registro vive en la
/// [`crate::config::Config`], que ya viaja por referencia a cada petición, así que
/// no se filtra ni se copia por petición.
///
/// **El precio, escrito**: el barrido es **al arrancar**, así que **añadir** un
/// tema exige reiniciar. Los **bytes** de la página se leen **al servir**, así que
/// **editar** un tema que ya estaba se ve sin reiniciar.
#[derive(Debug, Clone)]
pub struct Themes {
    /// Los temas en orden: los embebidos primero y los descubiertos detrás,
    /// ordenados por nombre.
    temas: Vec<Theme>,
}

impl Themes {
    /// El registro de solo los **embebidos**, sin barrer ningún directorio.
    ///
    /// Es lo que ve un binario instalado sin ficheros de datos y lo que usan las
    /// pruebas que no se ocupan del descubrimiento. El conjunto se descubre al
    /// compilar desde `content/*.html` ([`crate::embedded::EMBEBIDOS`]).
    pub fn incorporados() -> Self {
        Self {
            temas: embedded::EMBEBIDOS
                .iter()
                .map(|(name, page)| Theme {
                    name: Cow::Borrowed(name),
                    embedded: Some(page),
                })
                .collect(),
        }
    }

    /// El **registro efectivo**: los **embebidos** **más** los **descubiertos** en
    /// la **raíz** de `content_dir`.
    ///
    /// Las reglas del barrido, una a una:
    ///
    /// - **Solo la raíz**: un `temas/neon.html` no es un tema. Un tema es **una
    ///   página** con un nombre, y admitir rutas convertiría el nombre en una ruta
    ///   que el registro tendría que validar.
    /// - **Un fichero regular, siguiendo enlaces simbólicos**:
    ///   `entrada.path().is_file()` —**no** `file_type()`—, para que un enlace a un
    ///   fichero legible sí sea un tema y un enlace roto o a un directorio no.
    /// - **Extensión exactamente `html`**, en minúsculas: `neon.htm`, `neon.HTML`
    ///   y `neon.css` no son temas.
    /// - **El nombre es el tallo**, UTF-8 y no vacío.
    /// - **El tallo de un embebido no añade un tema**: `default.html` es la página
    ///   del `default` embebido en el disco, no un `default` de más.
    /// - **El orden es determinista**: los embebidos primero y los descubiertos
    ///   ordenados por nombre detrás (`read_dir` no promete ningún orden).
    ///
    /// Un directorio que **no existe** deja los embebidos y **no** falla, **sin**
    /// `warn!` (es lo normal en un binario sin ficheros de datos). Uno que existe y
    /// **no se puede listar** deja los embebidos y **no** falla, con un `warn!` que
    /// **nombra el directorio y la causa** (el caso medido es un fichero regular
    /// donde se espera un directorio, `NotADirectory`, que **no** es `NotFound`).
    /// Las entradas que no se pueden leer se **ignoran** (`filter_map(Result::ok)`).
    pub fn descubrir(content_dir: &Path) -> Self {
        let mut registro = Self::incorporados();
        let mut descubiertos: Vec<Theme> = Vec::new();

        match std::fs::read_dir(content_dir) {
            Ok(entradas) => {
                for entrada in entradas.filter_map(Result::ok) {
                    if let Some(tema) = tema_descubierto(&entrada) {
                        descubiertos.push(tema);
                    }
                }
            }
            // Un directorio que no existe es lo normal en un binario sin ficheros
            // de datos: se dejan los embebidos y **no** se deja constancia.
            Err(err) if err.kind() == ErrorKind::NotFound => {}
            // Un directorio que existe y no se puede listar es un fallo del
            // directorio del operador: se deja constancia y se dejan los embebidos.
            Err(err) => {
                tracing::warn!(
                    dir = %content_dir.display(),
                    error = %err,
                    "no se pudo listar el directorio de contenido: se sirven solo los temas embebidos"
                );
            }
        }

        // `read_dir` no promete ningún orden: se ordenan por nombre para que el
        // registro no dependa del del sistema de ficheros.
        descubiertos.sort_by(|a, b| a.name.as_ref().cmp(b.name.as_ref()));
        registro.temas.extend(descubiertos);

        registro
    }

    /// El tema del registro con ese nombre, si lo hay.
    ///
    /// La comparación es **exacta**, sensible a mayúsculas: `Terminal` no es el
    /// tema `terminal`. Adivinar la intención de quien escribe la cabecera sería
    /// peor que no aceptarla.
    pub fn by_name(&self, name: &str) -> Option<&Theme> {
        self.temas.iter().find(|theme| theme.name == name)
    }

    /// Los nombres del registro, en orden, separados por `", "`.
    ///
    /// Es la lista que el error de `THEME` enumera: quien escribe la variable cree
    /// que está configurando algo, así que el error tiene que decirle qué puede
    /// escribir en su lugar —los nombres que el registro tenga **en ese
    /// arranque**—, no solo que se equivocó.
    pub fn names(&self) -> String {
        self.iter()
            .map(|theme| theme.name.as_ref())
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Recorre los temas del registro en orden.
    pub fn iter(&self) -> std::slice::Iter<'_, Theme> {
        self.temas.iter()
    }
}

/// El tema **descubierto** que representa una entrada de `CONTENT_DIR`, si la
/// entrada es un tema.
///
/// Devuelve `None` —y deja constancia cuando el motivo lo merece— si la entrada
/// no es un fichero de tema. Las reglas están en [`Themes::descubrir`].
fn tema_descubierto(entrada: &std::fs::DirEntry) -> Option<Theme> {
    let path = entrada.path();

    // Un fichero regular, **siguiendo enlaces simbólicos**: `is_file()` (y no
    // `file_type()`) hace que un enlace a un fichero legible sea un tema y un
    // enlace roto o a un directorio no. Un directorio llamado `neon.html` tampoco.
    if !path.is_file() {
        return None;
    }

    // La extensión tiene que ser **exactamente** `html`, comparada como cadena
    // (una comparación de `OsStr` podría escaparse a mayúsculas).
    if path.extension().and_then(|ext| ext.to_str()) != Some("html") {
        return None;
    }

    // El nombre es el tallo del fichero: tiene que ser UTF-8 y no estar vacío.
    let stem = path.file_stem().and_then(|stem| stem.to_str())?;
    if stem.is_empty() {
        return None;
    }

    // El tallo de un embebido no añade un tema: es la página de ese tema en el
    // disco, que es la regla de siempre —un fichero legible gana a la copia
    // embebida—.
    if embedded::EMBEBIDOS.iter().any(|(name, _)| *name == stem) {
        return None;
    }

    Some(Theme {
        name: Cow::Owned(stem.to_string()),
        embedded: None,
    })
}

/// Resuelve el tema efectivo de una petición **contra el registro**.
///
/// Con el principio de siempre —el **primer** origen que haga match—, un
/// `X-Theme` **conocido** gana al tema configurado (`THEME`); **ausente o
/// desconocido** cae al configurado. Es deliberado que un valor desconocido no
/// sea un error: esa cabecera la elige quien llama, y un cliente no puede romper
/// el servicio. La asimetría con `THEME` —que sí aborta el arranque— es la misma
/// que ya rige entre `X-Code`/`X-Format` y las variables del operador.
///
/// **Invariante que no se puede romper**: el valor de `X-Theme` **nunca** compone
/// una ruta. La ruta sale del **registro**, que se construyó con el barrido del
/// directorio: se resuelve el nombre a una entrada y se sirve **su** ruta. Si se
/// implementara como `content_dir.join(format!("{}.html", requested))`, un
/// `X-Theme: ../../etc/passwd` sería una lectura arbitraria de ficheros.
pub fn resolve<'a>(
    registry: &'a Themes,
    requested: Option<&str>,
    configured: &'a Theme,
) -> &'a Theme {
    requested
        .and_then(|nombre| registry.by_name(nombre))
        .unwrap_or(configured)
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
    /// si no la copia embebida —que un tema **descubierto** no tiene—.
    ///
    /// Un fichero **ausente** de un tema **incorporado** no es un fallo —es lo
    /// normal en un binario instalado sin ficheros de datos—, así que cae a su
    /// copia embebida **sin** `warn`. Un fichero **ausente** de un tema
    /// **descubierto** sí deja constancia: sin copia a la que caer, el silencio
    /// escondería un fichero que se ha esfumado, y devuelve el `io::Error`.
    ///
    /// Cualquier **otro** error de E/S —un fichero que existe y no se puede
    /// leer— deja un `warn!` que **nombra el fichero y su causa** y devuelve el
    /// `io::Error`: el render lo convierte en [`crate::render::RenderError::Io`] y
    /// la respuesta es el **respaldo de texto** con el estado intacto, nunca un
    /// `500`.
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
            Err(err) if err.kind() == ErrorKind::NotFound => match self.embedded {
                // Un incorporado ausente cae a su copia embebida, **sin** `warn`.
                Some(embedded) => Ok(embedded.as_bytes().to_vec()),
                // Un descubierto ausente no tiene a qué caer: deja constancia.
                None => {
                    tracing::warn!(
                        theme = %self.name,
                        path = %path.display(),
                        "el tema descubierto no tiene fichero ni copia embebida: se responderá el respaldo de texto"
                    );

                    Err(err)
                }
            },
            Err(err) => {
                tracing::warn!(
                    theme = %self.name,
                    path = %path.display(),
                    error = %err,
                    "no se pudo leer la página del tema: se responderá el respaldo de texto"
                );

                Err(err)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DEFAULT_THEME_NAME, Theme, Themes, resolve};
    use crate::embedded::EMBEBIDOS;
    use std::fs;
    use std::path::PathBuf;

    /// La página embebida de un tema. Todos la tienen en `EMBEBIDOS`.
    fn embebida(theme: &Theme) -> &'static str {
        theme
            .embedded
            .expect("un tema embebido siempre tiene copia embebida")
    }

    /// El directorio de un fixture bajo `tests/fixtures/`.
    fn fixtures_dir(nombre: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(nombre)
    }

    /// El registro descubierto en un directorio de fixtures del repositorio.
    fn descubrir(fixture: &str) -> Themes {
        Themes::descubrir(&fixtures_dir(fixture))
    }

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
    fn embebidos_no_esta_vacio_y_los_nombres_son_unicos() {
        assert!(!EMBEBIDOS.is_empty(), "EMBEBIDOS no puede estar vacío");

        let mut nombres: Vec<&str> = EMBEBIDOS.iter().map(|(n, _)| *n).collect();
        let mut unicos = nombres.clone();
        unicos.sort();
        unicos.dedup();
        assert_eq!(
            nombres.len(),
            unicos.len(),
            "los nombres de los temas embebidos tienen que ser únicos"
        );

        nombres.sort();
        assert_eq!(
            nombres, unicos,
            "EMBEBIDOS tiene que estar ordenado por nombre"
        );
    }

    #[test]
    fn incorporados_carga_todos_los_embebidos() {
        let registro = Themes::incorporados();
        assert_eq!(registro.iter().count(), EMBEBIDOS.len());
        for (nombre, _) in EMBEBIDOS {
            assert!(
                registro.by_name(nombre).is_some(),
                "falta el tema `{nombre}` en incorporados()"
            );
        }
    }

    #[test]
    fn by_name_es_exacto() {
        let registro = Themes::incorporados();
        assert!(
            registro.by_name(DEFAULT_THEME_NAME).is_some(),
            "el tema por defecto tiene que estar en el registro"
        );
        assert!(
            registro.by_name("Default").is_none(),
            "la comparación es sensible a mayúsculas"
        );
        assert!(
            registro.by_name("").is_none(),
            "la cadena vacía no es un tema"
        );
    }

    #[test]
    fn descubrir_anade_temas_del_directorio() {
        let registro = descubrir("temas-extras");
        let neon = registro
            .by_name("neon")
            .expect("neon.html es un tema descubierto");
        assert!(
            neon.embedded.is_none(),
            "un tema descubierto no tiene copia embebida"
        );
        assert_eq!(neon.page_path(), "neon.html");
    }

    #[test]
    fn descubrir_salta_los_embebidos() {
        let registro = descubrir("temas-extras");
        let default = registro
            .by_name("default")
            .expect("default es un tema embebido");
        assert!(
            default.embedded.is_some(),
            "el `default` del registro es el embebido: conserva su copia"
        );
        assert_eq!(
            registro.iter().filter(|t| t.name == "default").count(),
            1,
            "`default.html` no puede añadir un tema duplicado"
        );
    }

    #[test]
    fn descubrir_acepta_cualquier_html() {
        // El modelo nuevo no excluye tallos numéricos: cualquier .html es un tema.
        let registro = descubrir("temas-extras");

        for nombre in ["200", "404", "999"] {
            assert!(
                registro.by_name(nombre).is_some(),
                "`{nombre}.html` tiene que ser un tema: el modelo nuevo no excluye tallos numéricos"
            );
        }
    }

    #[test]
    fn descubrir_ignora_subdirectorios() {
        let registro = descubrir("temas-extras");
        assert!(
            registro.by_name("profundo").is_none(),
            "un fichero en un subdirectorio no es un tema: el nombre de un tema no es una ruta"
        );
        assert!(
            registro.by_name("neon").is_some(),
            "neon.html está en la raíz, así que sí es un tema"
        );
    }

    #[test]
    fn descubrir_ignora_extensiones_que_no_son_html() {
        let registro = descubrir("temas-extension");
        assert_eq!(
            registro.iter().count(),
            EMBEBIDOS.len(),
            "neon.htm, neon.HTML y neon.css no añaden ningún tema"
        );
        assert!(
            registro.by_name("neon").is_none(),
            "la extensión tiene que ser exactamente html"
        );
    }

    #[test]
    fn descubrir_sin_directorio_no_falla() {
        let registro = Themes::descubrir(&fixtures_dir("inexistente-y-ademas-inexistente"));
        assert_eq!(
            registro.iter().count(),
            EMBEBIDOS.len(),
            "un CONTENT_DIR que no existe deja los embebidos sin fallar"
        );
    }

    #[test]
    fn descubrir_con_directorio_no_listable_no_falla() {
        // Un fichero regular donde se espera un directorio.
        let registro = Themes::descubrir(&fixtures_dir("temas-extras").join("neon.html"));
        assert_eq!(
            registro.iter().count(),
            EMBEBIDOS.len(),
            "un CONTENT_DIR que existe y no se puede listar deja los embebidos sin fallar"
        );
    }

    #[test]
    fn la_pagina_legible_del_disco_gana_a_la_embebida() {
        let dir = dir_temporal("legible");
        let propia = b"<p>pagina propia del operador, distinguible</p>".to_vec();
        fs::write(dir.join("matrix.html"), &propia).expect("el fixture tiene que poder escribirse");

        let registro = Themes::incorporados();
        let theme = registro.by_name("matrix").expect("`matrix` es un embebido");
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

        let registro = Themes::incorporados();
        for (nombre, _) in EMBEBIDOS {
            let theme = registro.by_name(nombre).expect("tiene que estar");
            let pagina = theme
                .page(&dir)
                .expect("un fichero ausente no es un fallo: cae a la copia embebida");
            assert_eq!(
                pagina.as_slice(),
                embebida(theme).as_bytes(),
                "sin fichero en el disco, el tema `{nombre}` tiene que servir su copia embebida"
            );
        }

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn la_pagina_presente_pero_ilegible_es_un_error_de_e_s() {
        let dir = dir_temporal("ilegible");
        fs::create_dir(dir.join("matrix.html")).expect("el fixture tiene que poder crearse");

        let registro = Themes::incorporados();
        let theme = registro.by_name("matrix").expect("`matrix` es un embebido");
        assert!(
            theme.page(&dir).is_err(),
            "una página del operador que existe y no se puede leer tiene que ser un error de E/S, \
             no la copia embebida"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn el_tema_descubierto_cuyo_fichero_desaparece_es_un_error_de_e_s() {
        let dir = dir_temporal("descubierto-desaparece");
        let bytes = fs::read(fixtures_dir("temas-extras").join("neon.html"))
            .expect("el fixture tiene que poder leerse");
        fs::write(dir.join("neon.html"), &bytes).expect("el fixture tiene que poder escribirse");

        let registro = Themes::descubrir(&dir);
        let neon = registro
            .by_name("neon")
            .expect("`neon.html` es un tema descubierto");

        let con_fichero = neon
            .page(&dir)
            .expect("con el fichero presente, la página se lee");
        assert_eq!(
            con_fichero, bytes,
            "con el fichero presente, `page` devuelve sus bytes"
        );

        fs::remove_file(dir.join("neon.html")).expect("el fichero tiene que poder borrarse");

        assert!(
            neon.page(&dir).is_err(),
            "un tema descubierto cuyo fichero desaparece es un error de E/S: no tiene copia \
             embebida a la que caer, así que la respuesta del servicio será el respaldo de texto \
             con el estado intacto, y **no** un 500"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn la_ruta_sale_del_nombre_del_registro() {
        // La invariante que impide una lectura arbitraria de ficheros: el valor de
        // `X-Theme` **nunca** compone una ruta; la ruta sale del registro.
        let registro = descubrir("temas-extras");
        let configurado = registro
            .by_name("minimal")
            .expect("`minimal` es un embebido");

        for intento in [
            "../../etc/passwd",
            "/etc/passwd",
            "..",
            "neon/../../x",
            "neon.html",
        ] {
            let elegido = resolve(&registro, Some(intento), configurado);

            assert_eq!(
                elegido.name, "minimal",
                "`X-Theme: {intento}` no es un nombre del registro: tiene que caer al configurado"
            );
            assert_eq!(
                elegido.page_path(),
                "minimal.html",
                "la ruta sale del registro, nunca del valor de `X-Theme`: `{intento}` no puede \
                 componer una ruta"
            );
        }
    }

    #[test]
    fn anadir_un_tema_con_el_servicio_en_marcha_no_lo_convierte_en_tema() {
        let dir = dir_temporal("anadir-en-marcha");

        let registro = Themes::descubrir(&dir);
        assert!(
            registro.by_name("nuevo").is_none(),
            "en un directorio vacío el registro son los embebidos y nada más"
        );

        fs::write(dir.join("nuevo.html"), b"<p>tema que llega tarde</p>")
            .expect("el tema tiene que poder escribirse");

        assert!(
            registro.by_name("nuevo").is_none(),
            "el registro ya construido no ve un fichero que apareció después: añadir un tema exige \
             reiniciar"
        );

        let siguiente = Themes::descubrir(&dir);
        assert!(
            siguiente.by_name("nuevo").is_some(),
            "un `nuevo.html` presente al arrancar sí es un tema"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn los_descubiertos_van_tras_los_embebidos_y_ordenados_por_nombre() {
        let dir = dir_temporal("orden-alfabetico");
        for nombre in ["alfa", "media", "zeta"] {
            fs::write(dir.join(format!("{nombre}.html")), b"<p>tema</p>")
                .expect("el tema tiene que poder escribirse");
        }

        let registro = Themes::descubrir(&dir);

        let nombres: Vec<&str> = registro.iter().map(|t| t.name.as_ref()).collect();
        let mut embebidos: Vec<&str> = EMBEBIDOS.iter().map(|(n, _)| *n).collect();
        embebidos.sort();
        let mut esperados = embebidos;
        esperados.extend(["alfa", "media", "zeta"]);

        assert_eq!(
            nombres, esperados,
            "los descubiertos van **tras** los embebidos y **ordenados por nombre**"
        );

        let _ = fs::remove_dir_all(&dir);
    }
}
