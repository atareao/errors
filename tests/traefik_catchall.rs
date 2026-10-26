//! Pruebas de integración del **router de respaldo** que este repositorio
//! entrega. La guarda cubre **dos** propiedades, las dos en el mismo router
//! *catch-all* de Traefik: que no publique el *endpoint de servicio* `/metrics`
//! (`catchall-metrics-exclusion`) y que no publique las **páginas de tema
//! crudas** (`themes-as-pages`).
//!
//! Qué clase de fallo cierra cada una:
//!
//! - El router *catch-all* atiende cualquier Host que no case con otro router, y
//!   con él publica también los *endpoints de servicio* del propio binario
//!   —entre ellos `/metrics`, que expone el contador `errors_pages_total`, o sea
//!   el perfil de fallos de la infraestructura—.
//! - Y publica, además, la **página de cada tema**: el nombre del tema **es** la
//!   ruta de su página (`content/<nombre>.html`, siete ficheros), y las seis que
//!   **no** son el alias del código por defecto se sirven como *assets* crudos,
//!   con los marcadores `{{code}}`/`{{reason}}`/`{{description}}` a la vista.
//!   Servirlas a cualquiera es filtrar la plantilla del servicio, así que el
//!   respaldo las excluye con una segunda conjunción:
//!   `!PathRegexp(`\.html?((%2[fF])|/|$)`)`.
//!
//! La excepción son los **dos alias** `/default.html` y `/default.htm`: su path
//! se clasifica como el código por defecto, el servicio lo **intercepta** y
//! responde el código resuelto —**nunca** `200`—, así que no son fuga y la
//! exclusión los deja fuera a propósito. Excluirlos quitaría del respaldo la
//! única página de tema que **no** filtra, así que van en el `||` de la
//! conjunción: `!PathRegexp(`\.html?((%2[fF])|/|$)`) || Path(`/default.html`) ||
//! Path(`/default.htm`)`.
//!
//! La comprobación de extremo a extremo necesita un motor de contenedores y **no**
//! corre en el CI, así que sin esta guarda la fuga solo se descubriría volviendo
//! a desplegar. La prueba lee los ficheros versionados y vuelve a fallar en
//! cuanto una de las definiciones de respaldo pierde una de las **dos**
//! conjunciones —`!Path(`/metrics`)` y la exclusión del leak—, o en cuanto
//! desaparece (un renombrado del router no debe dejar la guarda huérfana y en
//! verde).
//!
//! Por qué se identifica el respaldo **por el nombre del router** y no por la
//! forma de su regla: `examples/traefik/README.md` ilustra la trampa de
//! sintaxis v2/v3 con un router llamado `errserve` —cuyas reglas
//! `HostRegexp(`{host:.+}`)` y `HostRegexp(`.+`)` **no** son un respaldo— y que
//! no debe llevar la exclusión. Se busca el nombre del router —el trozo entre
//! `traefik.http.routers.` y `.rule:` que termina en `catchall`—, no el texto de
//! la regla.
//!
//! Decisiones (las tres primeras cierran otros tantos modos de burlar la guarda):
//!
//! - **Se descubre, no se lista.** La guarda recorre el árbol del repositorio y
//!   examina cada `*.yml`, `*.yaml` y `*.md` —saltando `.git/`, `target/`,
//!   `openspec/` y el fichero de trabajo `PLAN.md`— en vez de leer una lista
//!   cerrada de tres ficheros. Con la lista cerrada, un cuarto fichero de
//!   despliegue con la regla **sin** la exclusión no se miraba: la prueba salía
//!   en verde con la fuga presente. El descubrimiento la caza.
//! - **La cobertura es un suelo, no un techo.** `SUELO` nombra los ficheros que
//!   **deben** aportar al menos una definición; si uno deja de aportarla, la
//!   prueba falla diciendo qué hacer. Pero un fichero descubierto que no esté en
//!   `SUELO` y sea conforme **no** es un fallo: exigir «exactamente N» castigaba
//!   una ampliación legítima (un cuarto respaldo correcto ponía la prueba en
//!   rojo).
//! - **Se exige sobre el *valor* de la regla, no sobre la línea.** Escribir la
//!   exclusión tras un ` #` —`… HostRegexp(`.+`) # && !Path(`/metrics`)`— deja la
//!   subcadena en el texto pero **no** en el valor: en YAML, ` #` abre un
//!   comentario, así que la regla que Traefik lee es `HostRegexp(`.+`)` y `/metrics`
//!   queda publicado. La guarda corta el comentario de cola y quita las comillas
//!   que envuelvan el valor (el fragmento de *provider de fichero* del README va
//!   entrecomillado) antes de exigir las subcadenas. Hay una prueba sintética que
//!   muerde este caso.
//! - **La barra de la regex se desdobla solo en un valor entrecomillado.** En un
//!   escalar YAML **entrecomillado**, `\.` no es un escape válido y el *provider*
//!   entero deja de cargar (`found unknown escape character`), con el contenedor
//!   arriba y **cero routers**, en silencio (medido): por eso ahí se escribe la
//!   barra **doblada** (`\\.`), que YAML lee como `\.`. En un escalar **plano**,
//!   en cambio, la barra es literal y una barra doblada se queda doblada. La
//!   guarda lee el valor como lo lee YAML —desdobla `\\.` a `\.` **solo** si el
//!   valor venía entrecomillado—, de modo que una página que use la forma
//!   canónica no se lleve un rojo falso. Hay pruebas sintéticas que muerden las
//!   dos ramas.
//! - **La conjunción cubre tres formas, y hay que decirlo con las tres**: fin de
//!   cadena, **barra final** o segmento detrás, y barra **percent-encoded**
//!   (`%2F`, en las dos cajas) —`!PathRegexp(`\.html?((%2[fF])|/|$)`)`—. Una regla
//!   anclada solo al fin de cadena (`\.html?$`) deja `/minimal.html/` y
//!   `/minimal.html%2F` sirviendo el fichero crudo con `200` y sus marcadores:
//!   **medido** contra Traefik v3.6.25, **17** formas abiertas frente a **0** de
//!   la final. Traefik decodifica pero **no** normaliza el path —la barra final
//!   rompe el `$`— y contrasta la regla contra el path con la barra
//!   **percent-encoded** sin decodificar (`%2F` sobrevive); de ahí la rama
//!   `%2[fF]`, que cierra esa barra: Traefik normaliza los escapes de los
//!   caracteres **no reservados** (`%2E`, `%6C`, que por eso lee como
//!   `minimal.html`) pero **no** el de la barra. Y la forma final **no** depende
//!   de `sanitizePath`: da la misma tabla con su defecto y en `false`.
//! - **Una regla con la sintaxis v2 no es un respaldo.** `HostRegexp(`{host:.+}`)`
//!   —placeholder `{nombre:regexp}` dentro del `HostRegexp`— **no casa con
//!   ninguna petición** en Traefik v3 (medido): no puede publicar `/metrics` ni
//!   nada, así que no es un vector de fuga y la guarda no la trata como respaldo.
//!   Es la excepción que evita exigir la exclusión a la línea «MAL» del ejemplo
//!   didáctico si algún día se alinea al nombre del resto del repositorio.
//! - **Ruta por `env!("CARGO_MANIFEST_DIR")`**, no por directorio de trabajo: las
//!   pruebas de integración se ejecutan con el cwd del crate, pero fijar la raíz
//!   en tiempo de compilación evita depender de eso.
//! - **Sin dependencias nuevas:** el descubrimiento usa `std::fs` con recursión a
//!   mano, y la forma de cada respaldo se reconoce con búsqueda literal de
//!   subcadenas sobre el valor extraído. Un parser de YAML completo solo añadiría
//!   dependencia; el crate compila `--offline` y `cargo deny` vigila el árbol.
//! - **Los fallos se acumulan** en vez de abortar en el primero: cada aserción
//!   **nombra el fichero y la línea**, y un solo `cargo test` deja la lista
//!   completa de lo que incumple el contrato.
//! - **Lo que la guarda NO puede demostrar.** Comprueba que el *texto* de cada
//!   respaldo lleva las dos conjunciones, **no** que la regex **case** con las
//!   peticiones que debe bloquear —eso lo mide `check.sh` con el ejemplo
//!   levantado, y aquí no hay motor de contenedores—. Y **no** es un parser de
//!   YAML: corta el comentario de cola y desdobla la barra del entrecomillado
//!   **sin** validar que el entrecomillado sea YAML válido, así que una regla con
//!   la barra **simple** entre comillas —la que rompe el *provider* en silencio—
//!   no la caza.
//! - **Lo que la guarda NO puede demostrar aquí**: que la regex cierre **todas**
//!   las codificaciones que un cliente pueda inventar —eso lo mide `check.sh` con
//!   el ejemplo levantado, forma a forma— ni que la tabla que se midió sea
//!   exhaustiva. La guarda guarda **la subcadena**, que es lo que una prueba
//!   textual puede guardar.

use std::fs;
use std::path::{Path, PathBuf};

/// Extensiones de los ficheros que pueden declarar un respaldo desplegable.
const EXTENSIONES: [&str; 3] = ["yml", "yaml", "md"];

/// Directorios que no se recorren: control de versiones, artefactos de
/// compilación y los documentos de trabajo del flujo OpenSpec.
const DIRECTORIOS_IGNORADOS: [&str; 3] = [".git", "target", "openspec"];

/// Fichero de trabajo sin versionar que menciona la regla y no debe contar.
const FICHERO_IGNORADO: &str = "PLAN.md";

/// **Suelo** de cobertura: ficheros que **deben** aportar al menos una definición
/// de respaldo. Un fichero descubierto que no esté aquí y sea conforme no falla;
/// uno de estos que deje de aportar, sí. No es la lista de lo que se lee (eso lo
/// decide el descubrimiento), sino la de lo que no puede faltar.
const SUELO: [&str; 4] = [
    "compose.yml",
    "examples/traefik/compose.yml",
    "examples/traefik/README.md",
    "README.md",
];

/// Prefijo de un label de router de Docker (`traefik.http.routers.<nombre>…`).
const PREFIJO_LABEL: &str = "traefik.http.routers.";

/// Sufijo que marca la clave de la regla en un label (`.rule:`).
const SUFIJO_REGLA: &str = ".rule:";

/// Cómo ha de terminar el nombre de un router de respaldo.
const NOMBRE_RESPALDO: &str = "catchall";

/// Cuántas líneas tras la clave `catchall:` se miran para dar con su `rule:`.
/// Acota la búsqueda al bloque, no recorre el fichero entero.
const VENTANA_REGLA: usize = 10;

/// Subcadenas que **toda** regla de respaldo debe llevar, **una por conjunción**:
/// `HostRegexp(` da sentido al respaldo, `!Path(`/metrics`)` excluye el *endpoint
/// de servicio* y la tercera excluye el **leak de las páginas de tema** —salvo
/// sus dos alias, que el servicio intercepta y no filtran—. La tercera cubre las
/// **tres** formas con las que el respaldo servía la página cruda —fin de cadena,
/// **barra final** o segmento detrás, y barra **percent-encoded** (`%2F`, en las
/// dos cajas)—, las tres **medidas** abiertas contra Traefik v3.6.25 con la forma
/// anclada al fin de cadena; y **no** depende de `sanitizePath`. Va con la barra
/// **simple** porque se compara contra el valor **ya leído**: si el valor venía
/// entrecomillado, `valor_de_regla` ya desdobló `\\.` a `\.`.
const DEBE_LLEVAR: [&str; 3] = [
    "HostRegexp(",
    "!Path(`/metrics`)",
    r"(!PathRegexp(`\.html?((%2[fF])|/|$)`) || Path(`/default.html`) || Path(`/default.htm`))",
];

/// Marca de la sintaxis de Traefik **v2**: un placeholder `{nombre:regexp}`
/// dentro del `HostRegexp`. Esa regla no casa con ninguna petición en v3, así
/// que **no** es un respaldo (ver la nota de decisiones, arriba).
const SINTAXIS_V2: &str = "HostRegexp(`{";

/// Una definición de router de respaldo localizada en un fichero.
struct Respaldo {
    /// Fichero relativo a la raíz del repositorio, para nombrarlo en el fallo.
    fichero: String,
    /// Línea (1-based) donde se define, para nombrarla en el fallo.
    linea: usize,
    /// El **valor** de la regla, si se pudo leer; comentario de cola y comillas
    /// fuera.
    regla: Option<String>,
}

/// Descubre, desde la raíz del repositorio, todos los ficheros con extensión
/// `yml`, `yaml` o `md`, saltando los directorios ignorados y el fichero de
/// trabajo. Devuelve rutas **relativas** (con `/`) ordenadas, para que los fallos
/// salgan siempre en el mismo orden.
fn descubrir_ficheros() -> Vec<String> {
    let raiz = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut encontrados = Vec::new();
    recorrer(&raiz, &raiz, &mut encontrados);
    encontrados.sort();
    encontrados
}

/// Recorre `dir` acumulando, en `encontrados`, las rutas relativas a `raiz` de
/// los ficheros de interés. La recursión es a mano y sin dependencias.
fn recorrer(dir: &Path, raiz: &Path, encontrados: &mut Vec<String>) {
    let Ok(entradas) = fs::read_dir(dir) else {
        return;
    };

    for entrada in entradas.flatten() {
        let ruta = entrada.path();
        let nombre = entrada.file_name();
        let nombre = nombre.to_string_lossy();

        if ruta.is_dir() {
            if !DIRECTORIOS_IGNORADOS.contains(&nombre.as_ref()) {
                recorrer(&ruta, raiz, encontrados);
            }
            continue;
        }

        if nombre.as_ref() == FICHERO_IGNORADO {
            continue;
        }

        let extension = ruta
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default();
        if !EXTENSIONES.contains(&extension) {
            continue;
        }

        if let Ok(relativa) = ruta.strip_prefix(raiz) {
            encontrados.push(relativa.to_string_lossy().replace('\\', "/"));
        }
    }
}

/// El **valor** de una regla, tal como lo lee Traefik: fuera el comentario de
/// cola de YAML —todo desde el primer ` #`—, fuera las comillas que envuelvan el
/// valor y, si venía entrecomillado, la barra **desdoblada** (`\\.` → `\.`). Sin
/// el corte del comentario, la subcadena `!Path(`/metrics`)` de un comentario
/// haría pasar como conforme una regla que no lo es.
///
/// Por qué el desdoblamiento **solo** en el caso entrecomillado: en un escalar
/// YAML entrecomillado, `\.` **no** es un escape válido y el *provider* entero
/// deja de cargar (`found unknown escape character`) —en **silencio**: el
/// contenedor queda arriba y con **cero routers**—, así que ahí la barra se
/// escribe doblada (`\\.`) y YAML la lee como `\.`; en un escalar **plano** la
/// barra es literal y una barra doblada se queda doblada. El límite, declarado:
/// esta función **no** es un parser de YAML y **no** valida que el
/// entrecomillado sea YAML válido —solo corta el comentario de cola, quita las
/// comillas que envuelvan el valor y desdobla la barra del entrecomillado, y nada
/// más—.
fn valor_de_regla(cruda: &str) -> String {
    let sin_comentario = cruda.split(" #").next().unwrap_or_default().trim();

    // Tras el `trim()`, unas comillas dobles que **empiecen y terminen** el valor
    // (y no sean una sola) lo marcan como escalar entrecomillado de YAML; solo
    // entonces se desdobla la barra.
    if sin_comentario.len() >= 2 && sin_comentario.starts_with('"') && sin_comentario.ends_with('"')
    {
        sin_comentario[1..sin_comentario.len() - 1].replace("\\\\", "\\")
    } else {
        sin_comentario.to_string()
    }
}

/// Localiza, en el texto de un fichero, cada definición de router de respaldo y
/// la regla que declara. Reconoce las dos formas desplegables:
///
/// - **label** de Docker: una línea que, quitándole el `#` inicial y los
///   espacios (la plantilla de producción los tiene comentados), empieza por
///   `traefik.http.routers.` y cuyo nombre de router termina en `catchall`;
/// - ***provider de fichero***: la clave `catchall:` y, en las líneas siguientes
///   acotadas por `VENTANA_REGLA`, la que empieza por `rule:`.
///
/// En ambos casos se guarda el **valor** de la regla (comentario de cola y
/// comillas fuera), y una regla con la **sintaxis v2** se descarta: no casa con
/// ninguna petición, así que no es un respaldo.
fn respaldos_de(fichero: &str, texto: &str) -> Vec<Respaldo> {
    let lineas: Vec<&str> = texto.lines().collect();
    let mut encontrados = Vec::new();

    for (indice, linea) in lineas.iter().enumerate() {
        let numero = indice + 1;

        // Forma de label: el nombre es el trozo entre el prefijo y `.rule:`.
        let como_label = linea.trim_start_matches(['#', ' ', '\t']);
        let respaldo_label = como_label
            .strip_prefix(PREFIJO_LABEL)
            .and_then(|resto| resto.find(SUFIJO_REGLA).map(|posicion| (resto, posicion)))
            .filter(|(resto, posicion)| {
                let nombre = &resto[..*posicion];
                !nombre.contains('.') && nombre.ends_with(NOMBRE_RESPALDO)
            });
        if let Some((resto, posicion)) = respaldo_label {
            let regla = valor_de_regla(&resto[posicion + SUFIJO_REGLA.len()..]);
            if !regla.contains(SINTAXIS_V2) {
                encontrados.push(Respaldo {
                    fichero: fichero.to_string(),
                    linea: numero,
                    regla: Some(regla),
                });
            }
            continue;
        }

        // Forma de provider de fichero: la clave `catchall:` y su `rule:`.
        if linea.trim() == "catchall:" {
            let hallada = (indice + 1..lineas.len())
                .take(VENTANA_REGLA)
                .find_map(|j| {
                    lineas[j]
                        .trim()
                        .strip_prefix("rule:")
                        .map(|regla| (j, regla))
                });
            match hallada {
                // La aserción apunta a la línea de la regla, que es la que manda.
                Some((j, cruda)) => {
                    let regla = valor_de_regla(cruda);
                    if !regla.contains(SINTAXIS_V2) {
                        encontrados.push(Respaldo {
                            fichero: fichero.to_string(),
                            linea: j + 1,
                            regla: Some(regla),
                        });
                    }
                }
                // Sin regla que leer: se nombra la línea de la clave.
                None => encontrados.push(Respaldo {
                    fichero: fichero.to_string(),
                    linea: numero,
                    regla: None,
                }),
            }
        }
    }

    encontrados
}

/// Los incumplimientos de **una** definición: su regla debe llevar todas las
/// subcadenas exigidas. Una definición sin regla legible también incumple, con
/// un mensaje distinto.
fn incumplimientos(respaldo: &Respaldo) -> Vec<String> {
    let Some(regla) = &respaldo.regla else {
        return vec![format!(
            "{}:{} — la clave `catchall:` no declara una `rule:` en las {VENTANA_REGLA} líneas siguientes",
            respaldo.fichero, respaldo.linea
        )];
    };

    let mut fallos = Vec::new();
    for exigida in DEBE_LLEVAR {
        if !regla.contains(exigida) {
            fallos.push(format!(
                "{}:{} — la regla de respaldo {regla:?} no lleva `{exigida}`",
                respaldo.fichero, respaldo.linea
            ));
        }
    }
    fallos
}

/// Los incumplimientos de **cobertura**: cada fichero del `SUELO` ha de haber
/// aportado al menos una definición. Un fichero de más no es un fallo —el suelo
/// no es un techo—.
fn fallos_de_cobertura(aportados: &[(String, usize)]) -> Vec<String> {
    let mut fallos = Vec::new();
    for esperado in SUELO {
        let aporta = aportados
            .iter()
            .find(|(fichero, _)| fichero.as_str() == esperado)
            .map(|(_, cuantos)| *cuantos)
            .unwrap_or(0);
        if aporta == 0 {
            fallos.push(format!(
                "{esperado} — no aportó ninguna definición de router de respaldo; \
                 si lo has quitado a propósito, quítalo también de `SUELO` en esta prueba"
            ));
        }
    }
    fallos
}

/// Falla con la lista completa de incumplimientos, uno por línea y precedido del
/// motivo, en vez de con el primero encontrado.
fn exigir_sin_fallos(motivo: &str, fallos: &[String]) {
    assert!(
        fallos.is_empty(),
        "{motivo} ({} incumplimiento(s)):\n- {}",
        fallos.len(),
        fallos.join("\n- ")
    );
}

/// Cada definición de router de respaldo **descubierta** en el repositorio
/// excluye `/metrics` de su regla manteniendo el `HostRegexp` que da sentido al
/// respaldo. Un fichero nuevo con la fuga se caza; un fichero nuevo conforme no
/// molesta. La prueba también falla si alguno de los ficheros del `SUELO` deja de
/// aportar una definición (un renombrado del router no debe dejarla huérfana) o
/// si un fichero descubierto no se puede leer.
#[test]
fn cada_respaldo_de_traefik_catchall_excluye_metrics() {
    let mut aportados: Vec<(String, usize)> = Vec::new();
    let mut fallos = Vec::new();

    for fichero in descubrir_ficheros() {
        let ruta = Path::new(env!("CARGO_MANIFEST_DIR")).join(&fichero);
        match fs::read_to_string(&ruta) {
            Ok(texto) => {
                let respaldos = respaldos_de(&fichero, &texto);
                aportados.push((fichero.clone(), respaldos.len()));
                for respaldo in &respaldos {
                    fallos.extend(incumplimientos(respaldo));
                }
            }
            Err(error) => fallos.push(format!("no se pudo leer {fichero}: {error}")),
        }
    }

    fallos.extend(fallos_de_cobertura(&aportados));

    exigir_sin_fallos(
        "cada router de respaldo del repositorio debe excluir `/metrics` de su regla",
        &fallos,
    );
}

/// **R1** — la exclusión que solo vive tras un comentario de cola de YAML **no**
/// cuenta: el valor de la regla no la lleva y Traefik publicaría `/metrics`. El
/// caso está en las dos formas desplegables.
#[test]
fn r1_una_exclusion_tras_comentario_de_cola_no_cuenta() {
    // Forma de label de Docker.
    let texto = "traefik.http.routers.x-catchall.rule: HostRegexp(`.+`) # && !Path(`/metrics`)\n";
    let respaldos = respaldos_de("sintetico.yml", texto);
    assert_eq!(
        respaldos.len(),
        1,
        "la línea debe reconocerse como definición de respaldo"
    );
    let regla = respaldos[0]
        .regla
        .as_deref()
        .expect("la regla se pudo leer");
    assert_eq!(
        regla, "HostRegexp(`.+`)",
        "el comentario de cola no forma parte del valor de la regla"
    );
    assert!(
        !incumplimientos(&respaldos[0]).is_empty(),
        "una definición cuyo valor no lleva la exclusión debe considerarse incumplidora"
    );

    // Forma de provider de fichero, entrecomillada, con comentario de cola.
    let texto = "catchall:\n      rule: \"HostRegexp(`.+`)\" # && !Path(`/metrics`)\n";
    let respaldos = respaldos_de("sintetico.yaml", texto);
    assert_eq!(respaldos.len(), 1);
    let regla = respaldos[0]
        .regla
        .as_deref()
        .expect("la regla se pudo leer");
    assert_eq!(regla, "HostRegexp(`.+`)");
    assert!(
        !incumplimientos(&respaldos[0]).is_empty(),
        "la forma entrecomillada con comentario de cola tampoco lleva la exclusión"
    );
}

/// **R2** — la línea «MAL» de la trampa v2/v3, con un placeholder `{…}` dentro
/// del `HostRegexp`, **no** se reconoce como respaldo aunque el router se llame
/// `…-catchall`: no casa con ninguna petición, así que no puede publicar
/// `/metrics` ni nada. El mismo nombre con la sintaxis de v3 sí es un respaldo.
#[test]
fn r2_una_regla_con_sintaxis_v2_no_es_un_respaldo() {
    let texto = "traefik.http.routers.errserve-catchall.rule: HostRegexp(`{host:.+}`)\n";
    assert!(
        respaldos_de("sintetico.yml", texto).is_empty(),
        "una regla v2 no casa con ninguna petición: no es un respaldo"
    );

    let texto =
        "traefik.http.routers.errserve-catchall.rule: HostRegexp(`.+`) && !Path(`/metrics`)\n";
    assert_eq!(
        respaldos_de("sintetico.yml", texto).len(),
        1,
        "el mismo nombre con la sintaxis de v3 sí es un respaldo"
    );
}

/// **R3** — la cobertura es un **suelo**, no un techo: un fichero de más que sea
/// conforme no genera ningún fallo, mientras que un fichero del suelo que deje
/// de aportar sí, y el mensaje dice cuál.
#[test]
fn r3_la_cobertura_es_un_suelo_no_un_techo() {
    let completos = vec![
        ("compose.yml".to_string(), 1),
        ("examples/traefik/compose.yml".to_string(), 1),
        ("examples/traefik/README.md".to_string(), 1),
        ("README.md".to_string(), 1),
        ("examples/traefik/dynamic.yaml".to_string(), 2),
    ];
    assert!(
        fallos_de_cobertura(&completos).is_empty(),
        "un fichero de más y conforme no debe ser un fallo"
    );

    let cojo = vec![
        ("compose.yml".to_string(), 0),
        ("examples/traefik/compose.yml".to_string(), 1),
        ("examples/traefik/README.md".to_string(), 1),
        ("README.md".to_string(), 1),
    ];
    let fallos = fallos_de_cobertura(&cojo);
    assert_eq!(
        fallos.len(),
        1,
        "solo el fichero del suelo que dejó de aportar"
    );
    assert!(
        fallos[0].starts_with("compose.yml —"),
        "el fallo debe nombrar el fichero que dejó de aportar: {:?}",
        fallos[0]
    );
}

/// **R4** — una definición **conforme** en un fichero que **no** está en el suelo
/// no genera incumplimiento: la guarda la descubre y la acepta, no la castiga por
/// no estar en la lista. La regla lleva las **dos** conjunciones y, por ser un
/// valor **entrecomillado**, la barra **doblada** (`\\.`); que siga siendo
/// conforme demuestra de paso que el desdoblamiento no se lleva un rojo falso.
#[test]
fn r4_un_fichero_conforme_fuera_del_suelo_no_genera_incumplimiento() {
    let texto = "http:\n  routers:\n    catchall:\n      rule: \"HostRegexp(`.+`) && !Path(`/metrics`) && (!PathRegexp(`\\\\.html?((%2[fF])|/|$)`) || Path(`/default.html`) || Path(`/default.htm`))\"\n";
    let respaldos = respaldos_de("examples/traefik/dynamic.yaml", texto);
    assert_eq!(respaldos.len(), 1);
    assert!(
        incumplimientos(&respaldos[0]).is_empty(),
        "una regla conforme no incumple por estar en un fichero fuera del suelo"
    );
    assert!(
        !SUELO.contains(&"examples/traefik/dynamic.yaml"),
        "el fichero del caso ha de estar, a propósito, fuera del suelo"
    );
}

/// **R5** — un respaldo que excluye `/metrics` pero **no** el leak de las páginas
/// de tema **incumple**, y el fallo **nombra** el fichero, la línea y la
/// conjunción que falta. El caso está en las dos formas desplegables.
#[test]
fn r5_un_respaldo_sin_la_exclusion_del_leak_incumple() {
    // Forma de label de Docker, sin la segunda conjunción.
    let texto = "traefik.http.routers.x-catchall.rule: HostRegexp(`.+`) && !Path(`/metrics`)\n";
    let respaldos = respaldos_de("sintetico.yml", texto);
    assert_eq!(
        respaldos.len(),
        1,
        "la línea debe reconocerse como definición de respaldo"
    );
    let fallos = incumplimientos(&respaldos[0]);
    assert!(
        !fallos.is_empty(),
        "una definición sin la exclusión del leak debe considerarse incumplidora"
    );
    let mensaje = fallos.join("\n");
    assert!(
        mensaje.contains("sintetico.yml:1"),
        "el fallo debe nombrar el fichero y la línea: {mensaje:?}"
    );
    assert!(
        mensaje.contains(DEBE_LLEVAR[2]),
        "el fallo debe nombrar la conjunción que falta: {mensaje:?}"
    );

    // Forma de provider de fichero, sin la segunda conjunción.
    let texto = "catchall:\n      rule: \"HostRegexp(`.+`) && !Path(`/metrics`)\"\n";
    let respaldos = respaldos_de("sintetico.yaml", texto);
    assert_eq!(respaldos.len(), 1);
    let fallos = incumplimientos(&respaldos[0]);
    assert!(
        !fallos.is_empty(),
        "la forma de provider de fichero tampoco lleva la exclusión del leak"
    );
    let mensaje = fallos.join("\n");
    assert!(
        mensaje.contains("sintetico.yaml:2"),
        "el fallo debe nombrar el fichero y la línea: {mensaje:?}"
    );
    assert!(
        mensaje.contains(DEBE_LLEVAR[2]),
        "el fallo debe nombrar la conjunción que falta: {mensaje:?}"
    );
}

/// **R6** — la conjunción del leak colocada **solo tras un `#` de cola** no
/// cuenta —el caso simétrico de R1—: el valor de la regla no la lleva, así que la
/// definición **incumple** aunque el texto de la línea la contenga.
#[test]
fn r6_una_conjuncion_del_leak_tras_comentario_de_cola_no_cuenta() {
    let texto = "traefik.http.routers.x-catchall.rule: HostRegexp(`.+`) && !Path(`/metrics`) # && (!PathRegexp(`\\.html?((%2[fF])|/|$)`) || Path(`/default.html`) || Path(`/default.htm`))\n";
    let respaldos = respaldos_de("sintetico.yml", texto);
    assert_eq!(
        respaldos.len(),
        1,
        "la línea debe reconocerse como definición de respaldo"
    );
    let regla = respaldos[0]
        .regla
        .as_deref()
        .expect("la regla se pudo leer");
    assert!(
        !regla.contains(DEBE_LLEVAR[2]),
        "el comentario de cola no forma parte del valor: la conjunción del leak no debe estar en la regla leída, pero la regla es {regla:?}"
    );
    assert!(
        !incumplimientos(&respaldos[0]).is_empty(),
        "sin la conjunción en el valor, la definición incumple"
    );
}

/// **R7** — `valor_de_regla` **desdobla** la barra de un valor **entrecomillado**
/// —como lo hace YAML— y **no** la de uno **plano**. Y el caso de uso real: un
/// valor entrecomillado con la barra **doblada** y la conjunción **completa** es
/// **conforme**.
///
/// Límite declarado: el mismo valor con la barra **simple** entre comillas **no**
/// se mide —ahí el parser de YAML fallaría (`found unknown escape character`) y la
/// guarda no puede saberlo—; por eso la forma entrecomillada se escribe siempre
/// con la barra doblada.
#[test]
fn r7_valor_de_regla_desdobla_la_barra_solo_si_va_entrecomillado() {
    assert_eq!(
        valor_de_regla(r#" "a\\.b" "#),
        r"a\.b",
        "un valor entrecomillado se lee con la barra desdoblada, como lo lee YAML"
    );
    assert_eq!(
        valor_de_regla(r" a\\.b "),
        r"a\\.b",
        "en un valor plano la barra doblada se queda doblada: no es un escape de YAML"
    );

    let texto = "catchall:\n      rule: \"HostRegexp(`.+`) && !Path(`/metrics`) && (!PathRegexp(`\\\\.html?((%2[fF])|/|$)`) || Path(`/default.html`) || Path(`/default.htm`))\"\n";
    let respaldos = respaldos_de("sintetico.yaml", texto);
    assert_eq!(respaldos.len(), 1);
    assert!(
        incumplimientos(&respaldos[0]).is_empty(),
        "un valor entrecomillado con la barra doblada y la conjunción completa es conforme"
    );
}

/// **R8** — la regresión del defecto que este change encontró midiendo: una regla
/// con la conjunción **vieja**, anclada al **fin de cadena** (`\.html?$`), junto a
/// los dos alias, **incumple**. Esa forma dejaba las páginas de tema crudas al
/// alcance del respaldo con `200`: **medido** contra Traefik v3.6.25, **17**
/// formas abiertas —`/minimal.html/`, `/minimal.html%2F`, `/matrix.html/`,
/// `/matrix.html%2f`…— frente a **0** con la conjunción final. El fallo **nombra**
/// el fichero, la línea y la subcadena que falta.
#[test]
fn r8_la_forma_anclada_al_fin_de_cadena_incumple() {
    let texto = "traefik.http.routers.x-catchall.rule: HostRegexp(`.+`) && !Path(`/metrics`) && (!PathRegexp(`\\.html?$`) || Path(`/default.html`) || Path(`/default.htm`))\n";
    let respaldos = respaldos_de("sintetico.yml", texto);
    assert_eq!(
        respaldos.len(),
        1,
        "la línea debe reconocerse como definición de respaldo"
    );
    let fallos = incumplimientos(&respaldos[0]);
    assert!(
        !fallos.is_empty(),
        "la forma anclada al fin de cadena ha de incumplir"
    );
    let mensaje = fallos.join("\n");
    assert!(
        mensaje.contains("sintetico.yml:1"),
        "el fallo debe nombrar el fichero y la línea: {mensaje:?}"
    );
    assert!(
        mensaje.contains(DEBE_LLEVAR[2]),
        "el fallo debe nombrar la subcadena que falta: {mensaje:?}"
    );
}
