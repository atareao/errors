//! Pruebas de integración del **artefacto único** donde viven las aserciones
//! sobre la imagen arrancada (`smoke-test-and-catchall-html-exclusion`).
//!
//! Qué clase de fallo cierra: la `v0.8.0` se publicó al **segundo** intento. El
//! primer fallo llegó **con la etiqueta ya empujada**, en el job `imagen` de
//! `.github/workflows/release.yml`, con `GET /css/error.css devolvió 404; se
//! esperaba 200`. **Ninguna** de las tres puertas requeridas del PR lo vio:
//! `fmt, clippy y tests` y `cargo-deny` no tienen la imagen delante, y la puerta
//! `imagen` del CI **solo construye** la imagen, no la arranca. Y no salvó nada
//! que la aserción fuese de una línea: vivía en **un solo sitio** —`release.yml`—
//! y el cambio del contrato no pasó por ahí.
//!
//! El change mueve las aserciones sobre la imagen arrancada a **un único
//! artefacto versionado**, `.github/scripts/smoke.sh`, y hace que los dos
//! workflows que necesitan probar la imagen lo **invoquen**. Esta prueba guarda
//! esa propiedad, que es una propiedad de los **ficheros versionados**, no del
//! servicio: la comprobación de extremo a extremo necesita un motor de
//! contenedores y **no** corre en el CI.
//!
//! Tres propiedades, las tres guardadas aquí:
//!
//! 1. **Ningún workflow lleva sus propias aserciones** —ni define ni llama a los
//!    ayudantes del smoke test, ni lleva su propio bucle de arranque contra el
//!    contenedor—.
//! 2. **Los dos workflows que necesitan probar la imagen invocan el script**, y
//!    el script **existe**.
//! 3. **`examples/traefik/check.sh` conserva el caso del `502`**: un upstream que
//!    **rechaza** la conexión → Traefik `502` → la página del servicio. Un caso
//!    que desaparece en silencio deja el escenario que justifica el proyecto sin
//!    ninguna puerta.
//!
//! Decisiones (copiadas de `tests/traefik_catchall.rs`, que ya las tiene
//! resueltas, y cada una cierra un modo de burlar la guarda):
//!
//! - **Se descubre, no se lista.** La guarda recorre `.github/workflows` buscando
//!   cada `*.yml` y `*.yaml` en vez de leer una lista cerrada de dos ficheros. Con
//!   la lista cerrada, un cuarto workflow con sus propias aserciones no se miraba:
//!   la prueba salía en verde con la divergencia dentro.
//! - **La cobertura es un suelo, no un techo.** `DEBEN_INVOCAR` nombra los
//!   workflows que **deben** invocar el script; uno descubierto que **no** esté
//!   ahí y no lo invoque **no** es un fallo (exigir «exactamente N» castigaría una
//!   ampliación legítima).
//! - **Las listas cerradas se declaran.** `DEBEN_INVOCAR`, `AYUDANTES` y
//!   `ARRANQUE_PROPIO` son constantes con su motivo escrito; una lista que
//!   cambiase según el fichero no le serviría a nadie.
//! - **Solo cuentan las líneas que son código.** Un comentario que diga «el fallo
//!   que esta puerta viene a cazar» coincide con el ayudante `fallo ` —la palabra
//!   castellana, no la llamada— y sería un **rojo falso**: medido hoy en
//!   `.github/workflows/ci.yml`, en el comentario que explica por qué la puerta
//!   `imagen` no usa caché. Por eso la guarda ignora las líneas cuyo primer
//!   carácter no blanco sea `#` (comentario de YAML o de shell, sangrado aparte),
//!   y lo mismo vale para los dos tokens de `ARRANQUE_PROPIO`: una aserción
//!   comentada no comprueba nada. Límite que eso introduce: una aserción
//!   **comentada** deja de contarse —correcto, porque no comprueba nada—, pero la
//!   guarda sigue sin poder demostrar que el script lo compruebe todo.
//! - **Los fallos se acumulan** en vez de abortar en el primero: cada aserción
//!   **nombra el fichero y la línea**, y un solo `cargo test` deja la lista
//!   completa.
//! - **Ruta por `env!("CARGO_MANIFEST_DIR")`**, no por directorio de trabajo.
//! - **Sin dependencias nuevas:** `std::fs` con recursión a mano y búsqueda
//!   literal de subcadenas. Nada de `regex`.
//!
//! Lo que esta guarda **NO** puede demostrar, escrito en lugar de escondido:
//!
//! - Que el script compruebe **todas** las aserciones del contrato. Eso no lo
//!   mide ninguna prueba textual: lo mide la revisión, y lo mide correr el script
//!   contra una imagen de verdad.
//! - Que el caso del `502` de `check.sh` **corra**: necesita el ejemplo levantado
//!   y un motor de contenedores.
//! - Que el upstream de ese caso **rechace** de verdad la conexión. Eso vive en
//!   los labels del ejemplo y lo mide la fase 3 a mano. Una prueba textual no
//!   puede leer intenciones, así que el caso del `502` se guarda con una
//!   **palabra** en un comentario (`MOTIVO_502`): es lo único que un test de
//!   texto alcanza a exigir.

use std::fs;
use std::path::{Path, PathBuf};

/// Directorio de los workflows, relativo a la raíz del repositorio.
const DIRECTORIO_WORKFLOWS: &str = ".github/workflows";

/// Extensiones de un workflow.
const EXTENSIONES: [&str; 2] = ["yml", "yaml"];

/// El artefacto único donde viven las aserciones sobre la imagen arrancada.
const SCRIPT: &str = ".github/scripts/smoke.sh";

/// **Suelo** de invocación: los workflows que **deben** invocar el script. No es
/// la lista de lo que se lee (eso lo decide el descubrimiento), sino la de lo que
/// no puede faltar.
const DEBEN_INVOCAR: [&str; 2] = [".github/workflows/ci.yml", ".github/workflows/release.yml"];

/// Los ayudantes de aserción del smoke test. Lista **cerrada** y declarada: un
/// `curl` suelto **no** es incumplimiento —el workflow del release usa `curl`
/// contra la API de GHCR para comprobar la visibilidad del paquete, y eso no es
/// una aserción sobre el servicio—, así que la lista no puede ser «todo lo que
/// huela a prueba». Y solo cuentan las líneas que **no** son comentarios: `fallo `
/// es además la palabra castellana, y un comentario en prosa que la use —«el fallo
/// que esta puerta viene a cazar»— no es una aserción.
const AYUDANTES: [&str; 3] = ["comprobar(", "cabecera(", "fallo "];

/// Firma de un bucle de arranque propio: un contenedor arrancado en segundo plano
/// **y** una espera contra `/healthz` en el mismo fichero. Hacen falta las dos: el
/// smoke test es lo único que arranca el contenedor, así que la conjunción es la
/// que lo delata.
const ARRANQUE_PROPIO: [&str; 2] = ["--detach", "/healthz"];

/// El script de extremo a extremo del ejemplo, donde vive el caso del upstream
/// caído.
const CHECK_SH: &str = "examples/traefik/check.sh";

/// El código que el caso del `502` ejercita, como **token** de una línea que no
/// sea un comentario: `502` suelto en una frase no es un caso.
const CASO_502: &str = "502";

/// La palabra con la que el repositorio documenta **por qué** el caso no se
/// construye parando el backend: un servicio **sin servidores** da `503`, no
/// `502` (medido), así que el upstream ha de **rechazar** la conexión. Una prueba
/// textual no puede leer intenciones, así que guarda la palabra; el límite queda
/// escrito.
const MOTIVO_502: &str = "rechaza";

/// Descubre, desde `.github/workflows`, todos los ficheros con extensión `yml` o
/// `yaml`. Devuelve rutas **relativas** a la raíz del repositorio (con `/`)
/// ordenadas, para que los fallos salgan siempre en el mismo orden.
fn descubrir_workflows() -> Vec<String> {
    let raiz = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut encontrados = Vec::new();
    recorrer(&raiz.join(DIRECTORIO_WORKFLOWS), &raiz, &mut encontrados);
    encontrados.sort();
    encontrados
}

/// Recorre `dir` acumulando, en `encontrados`, las rutas relativas a `raiz` de
/// los workflows. La recursión es a mano y sin dependencias.
fn recorrer(dir: &Path, raiz: &Path, encontrados: &mut Vec<String>) {
    let Ok(entradas) = fs::read_dir(dir) else {
        return;
    };

    for entrada in entradas.flatten() {
        let ruta = entrada.path();

        if ruta.is_dir() {
            recorrer(&ruta, raiz, encontrados);
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

/// ¿Es `linea` un comentario de YAML o de shell? Da igual el sangrado: manda el
/// primer carácter no blanco. Una aserción comentada no comprueba nada, así que no
/// cuenta para la lista cerrada de ayudantes.
fn es_comentario(linea: &str) -> bool {
    linea.trim_start().starts_with('#')
}

/// La primera línea **que no sea un comentario** (1-based) que contiene la aguja,
/// si la hay. Es lo que permite que cada fallo **nombre la línea** y no solo el
/// fichero, sin que un comentario en prosa dispare un rojo falso.
fn primera_linea_de_codigo_con(texto: &str, aguja: &str) -> Option<usize> {
    texto
        .lines()
        .enumerate()
        .filter(|(_, linea)| !es_comentario(linea))
        .find(|(_, linea)| linea.contains(aguja))
        .map(|(indice, _)| indice + 1)
}

/// ¿Aparece la aguja en alguna línea que **no** sea un comentario?
fn hay_en_codigo(texto: &str, aguja: &str) -> bool {
    primera_linea_de_codigo_con(texto, aguja).is_some()
}

/// Las aserciones propias de un workflow: por cada ayudante del smoke test
/// **presente en una línea que no sea un comentario**, un fallo que nombra el
/// fichero y la línea; y si están **todos** los de `ARRANQUE_PROPIO` (también en
/// líneas de código), otro fallo que dice que el workflow lleva su propio bucle de
/// arranque. La lista de ayudantes es cerrada a propósito: un `curl` suelto no
/// incumple, y un comentario en prosa tampoco —`fallo ` es también la palabra
/// castellana—.
fn aserciones_propias(fichero: &str, texto: &str) -> Vec<String> {
    let mut fallos = Vec::new();

    for ayudante in AYUDANTES {
        if let Some(linea) = primera_linea_de_codigo_con(texto, ayudante) {
            fallos.push(format!(
                "{fichero}:{linea} — lleva el ayudante {ayudante:?} del smoke test; \
                 las aserciones sobre la imagen arrancada viven solo en {SCRIPT}"
            ));
        }
    }

    if ARRANQUE_PROPIO
        .iter()
        .all(|aguja| hay_en_codigo(texto, aguja))
    {
        let linea = primera_linea_de_codigo_con(texto, ARRANQUE_PROPIO[0]).unwrap_or(1);
        fallos.push(format!(
            "{fichero}:{linea} — lleva su propio bucle de arranque (`{}` junto a `{}`); \
             solo {SCRIPT} arranca el contenedor",
            ARRANQUE_PROPIO[0], ARRANQUE_PROPIO[1]
        ));
    }

    fallos
}

/// ¿Invoca el workflow el artefacto único? Búsqueda literal de la ruta del
/// script.
fn invoca_el_script(texto: &str) -> bool {
    texto.contains(SCRIPT)
}

/// El artefacto único tiene que existir y poder leerse: un workflow que invoca un
/// fichero que no está es una invocación que no prueba nada. El fallo lo dice con
/// esas palabras.
fn fallos_del_script() -> Vec<String> {
    let ruta = Path::new(env!("CARGO_MANIFEST_DIR")).join(SCRIPT);
    match fs::read_to_string(&ruta) {
        Ok(_) => Vec::new(),
        Err(error) => vec![format!(
            "{SCRIPT} — el artefacto único de las aserciones sobre la imagen arrancada \
             no se pudo leer ({error}); un workflow que invoca un fichero que no está \
             es una invocación que no prueba nada"
        )],
    }
}

/// Recorre `DEBEN_INVOCAR`: si un workflow del suelo no se descubrió, falla
/// diciendo que no está; si está y no invoca el script, falla nombrando el
/// fichero. Un workflow descubierto fuera del suelo no se castiga por no invocar:
/// el suelo no es un techo.
fn fallos_de_invocacion(leidos: &[(String, String)]) -> Vec<String> {
    let mut fallos = Vec::new();

    for exigido in DEBEN_INVOCAR {
        match leidos.iter().find(|(fichero, _)| fichero == exigido) {
            None => fallos.push(format!(
                "{exigido} — no se descubrió en {DIRECTORIO_WORKFLOWS}; los workflows \
                 que deben probar la imagen han de existir e invocar {SCRIPT}"
            )),
            Some((_, texto)) => {
                if !invoca_el_script(texto) {
                    fallos.push(format!(
                        "{exigido} — no invoca {SCRIPT}: las aserciones sobre la imagen \
                         arrancada han de vivir solo en ese script"
                    ));
                }
            }
        }
    }

    fallos
}

/// Los incumplimientos del caso del `502` en `CHECK_SH`: (a) al menos una línea
/// que **no sea un comentario** —cuyo primer carácter no blanco no sea `#`— que
/// lleve `CASO_502` como **token separado por espacios** (buscado a mano, sin
/// `regex`); y (b) la palabra `MOTIVO_502` en algún sitio del fichero. Los dos
/// fallos nombran `CHECK_SH`.
fn fallos_del_caso_502(texto: &str) -> Vec<String> {
    let mut fallos = Vec::new();

    let tiene_caso = texto
        .lines()
        .filter(|linea| !linea.trim_start().starts_with('#'))
        .any(|linea| linea.split_whitespace().any(|token| token == CASO_502));

    if !tiene_caso {
        fallos.push(format!(
            "{CHECK_SH} — no hay ninguna línea que no sea un comentario con `{CASO_502}` \
             como token propio; el caso del upstream que rechaza la conexión (Traefik 502 \
             con la página del servicio) ha desaparecido"
        ));
    }

    if !texto.contains(MOTIVO_502) {
        fallos.push(format!(
            "{CHECK_SH} — no conserva la palabra {MOTIVO_502:?}, que documenta por qué el \
             upstream ha de rechazar la conexión en vez de pararse (un servicio sin \
             servidores da 503, no 502)"
        ));
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

/// Los dos workflows que **deben** probar la imagen invocan el artefacto único, y
/// el artefacto **existe** y se puede leer. Se descubren los workflows en vez de
/// listarlos: uno de más es conforme y no molesta; uno del suelo que no invoque,
/// falla. Si un workflow descubierto no se puede leer, también falla.
#[test]
fn cada_workflow_que_prueba_la_imagen_invoca_el_smoke_test() {
    let mut leidos: Vec<(String, String)> = Vec::new();
    let mut fallos = fallos_del_script();

    for fichero in descubrir_workflows() {
        let ruta = Path::new(env!("CARGO_MANIFEST_DIR")).join(&fichero);
        match fs::read_to_string(&ruta) {
            Ok(texto) => leidos.push((fichero, texto)),
            Err(error) => fallos.push(format!("no se pudo leer {fichero}: {error}")),
        }
    }

    fallos.extend(fallos_de_invocacion(&leidos));

    exigir_sin_fallos(
        "los workflows que prueban la imagen deben invocar el artefacto único con las aserciones",
        &fallos,
    );
}

/// **Ningún** workflow descubierto lleva sus propias aserciones: ni los ayudantes
/// del smoke test ni su propio bucle de arranque. Un cuarto workflow con
/// aserciones propias se caza por el descubrimiento; uno conforme no molesta.
#[test]
fn ningun_workflow_lleva_las_aserciones_del_smoke_test() {
    let mut fallos = Vec::new();

    for fichero in descubrir_workflows() {
        let ruta = Path::new(env!("CARGO_MANIFEST_DIR")).join(&fichero);
        match fs::read_to_string(&ruta) {
            Ok(texto) => fallos.extend(aserciones_propias(&fichero, &texto)),
            Err(error) => fallos.push(format!("no se pudo leer {fichero}: {error}")),
        }
    }

    exigir_sin_fallos(
        "ningún workflow debe llevar sus propias aserciones sobre la imagen arrancada",
        &fallos,
    );
}

/// `examples/traefik/check.sh` conserva el caso del `502` —una línea que no sea
/// comentario con `502` como token propio— y la palabra que documenta por qué el
/// upstream ha de rechazar la conexión. Si el fichero no se puede leer, el fallo
/// lo dice y nombra la ruta.
#[test]
fn el_caso_del_502_sigue_en_check_sh() {
    let mut fallos = Vec::new();

    let ruta = Path::new(env!("CARGO_MANIFEST_DIR")).join(CHECK_SH);
    match fs::read_to_string(&ruta) {
        Ok(texto) => fallos.extend(fallos_del_caso_502(&texto)),
        Err(error) => fallos.push(format!("no se pudo leer {CHECK_SH}: {error}")),
    }

    exigir_sin_fallos(
        "el script del ejemplo debe conservar el caso del upstream que rechaza la conexión (502)",
        &fallos,
    );
}

/// **R1** — un workflow con sus propias aserciones incumple: los ayudantes
/// `comprobar()` y `fallo "…"` producen fallo, y el fallo nombra el fichero y el
/// ayudante.
#[test]
fn r1_un_workflow_con_sus_propias_aserciones_incumple() {
    let texto = "comprobar() {\n  real=$(curl -s -o /dev/null -w '%{http_code}' \"$base$1\")\n}\n\
                 comprobar /healthz 200\nfallo \"el contenedor no respondió en /healthz\"\n";
    let fallos = aserciones_propias("sintetico.yml", texto);

    assert!(
        !fallos.is_empty(),
        "un workflow con sus propias aserciones debe incumplir"
    );
    assert!(
        fallos
            .iter()
            .any(|fallo| fallo.contains("sintetico.yml") && fallo.contains("comprobar(")),
        "algún fallo debe nombrar el fichero y el ayudante `comprobar(`: {fallos:?}"
    );
}

/// **R2** — un workflow que solo invoca el script y no tiene aserciones cumple:
/// el path del script se reconoce como invocación y no hay ninguna aserción
/// propia.
#[test]
fn r2_un_workflow_que_invoca_el_script_y_no_tiene_aserciones_cumple() {
    let texto =
        "      - name: Smoke test\n        run: |\n          bash .github/scripts/smoke.sh\n";

    assert!(
        invoca_el_script(texto),
        "el path del script debe reconocerse como invocación"
    );
    assert!(
        aserciones_propias("sintetico.yml", texto).is_empty(),
        "un workflow que solo invoca el script no lleva aserciones propias"
    );
}

/// **R3** — un `curl` suelto no es incumplimiento. Es el texto **real** de
/// `release.yml` que pregunta por la visibilidad del paquete en GHCR: no lleva
/// ninguno de los ayudantes del smoke test. Es la prueba de que la lista de
/// ayudantes no se pasa de larga.
#[test]
fn r3_un_curl_suelto_no_es_incumplimiento() {
    let texto = "          codigo=$(curl -s -o /dev/null -w '%{http_code}' \\\n\
                 \x20           \"https://ghcr.io/token?scope=repository:$REPO:pull\")\n";

    assert!(
        aserciones_propias(".github/workflows/release.yml", texto).is_empty(),
        "un curl contra la API de GHCR no es una aserción sobre el servicio"
    );
}

/// **R4** — un bucle de arranque propio incumple: un contenedor arrancado en
/// segundo plano **y** una espera contra `/healthz` en el mismo fichero delatan
/// que el workflow arranca el contenedor por su cuenta, en vez de delegar en el
/// script.
#[test]
fn r4_un_bucle_de_arranque_propio_incumple() {
    let texto = "docker run --detach --rm --name smoke imagen:tag\n\
                 for i in $(seq 1 30); do\n\
                 \x20 if curl -fsS -o /dev/null \"$base/healthz\"; then break; fi\n\
                 \x20 sleep 1\n\
                 done\n";
    let fallos = aserciones_propias("sintetico.yml", texto);

    assert!(
        !fallos.is_empty(),
        "un workflow que arranca el contenedor debe incumplir"
    );
    assert!(
        fallos
            .iter()
            .any(|fallo| fallo.contains("bucle de arranque")),
        "el fallo debe decir que el workflow lleva su propio bucle de arranque: {fallos:?}"
    );
}

/// **R5** — el caso del `502` de `check.sh`: sin `502` falla y el fallo nombra la
/// ruta; con una línea real `… /x 502 "… rechaza …"` no falla; y un `502` que solo
/// aparece en un comentario **falla**, que es lo que hace que la regla sea cerrada
/// y no un «contiene 502 en algún sitio».
#[test]
fn r5_un_check_sh_sin_el_caso_del_502_incumple() {
    let sin_caso = "#!/usr/bin/env sh\nset -eu\n\
                    code app.localtest.me /boom 503 \"el 503 del backend\"\n";
    let fallos = fallos_del_caso_502(sin_caso);
    assert!(
        !fallos.is_empty(),
        "un check.sh sin el caso del 502 debe incumplir"
    );
    assert!(
        fallos.iter().any(|fallo| fallo.contains(CHECK_SH)),
        "el fallo debe nombrar la ruta del script: {fallos:?}"
    );

    let con_caso = "# nada\ncode caido.localtest.me /x 502 \"el upstream rechaza la conexion\"\n";
    assert!(
        fallos_del_caso_502(con_caso).is_empty(),
        "un caso 502 real con la palabra `rechaza` cumple"
    );

    let solo_comentario =
        "# code caido.localtest.me /x 502 \"el upstream rechaza la conexion\"\nset -eu\n";
    assert!(
        !fallos_del_caso_502(solo_comentario).is_empty(),
        "un 502 que solo aparece en un comentario no es un caso"
    );
}

/// **R6** — la palabra «fallo » de un comentario en prosa **no** es el ayudante del
/// smoke test. Es el texto **real** de `.github/workflows/ci.yml` —las tres líneas
/// del comentario que explica por qué la puerta `imagen` no usa caché—: si contase,
/// un comentario legítimo pondría la guarda en rojo y obligaría a reescribirlo para
/// contentarla. La misma palabra en una línea de código **sí** cuenta.
#[test]
fn r6_la_palabra_fallo_de_un_comentario_no_es_el_ayudante() {
    let texto = "      # Sin caché a propósito: lo que se comprueba es que la imagen se construye\n      # **desde el árbol**, en frío, como en un host cualquiera. Una caché de\n      # capas taparía justo el fallo que esta puerta viene a cazar —un `COPY` que\n";
    assert!(
        aserciones_propias("sintetico.yml", texto).is_empty(),
        "la palabra «fallo » de un comentario en prosa no es el ayudante del smoke test: es prosa castellana, y marcarla sería un rojo falso"
    );

    let codigo = "        || fallo \"el contenedor no respondió\"\n";
    assert!(
        aserciones_propias("sintetico.yml", codigo)
            .iter()
            .any(|fallo| fallo.contains("fallo ")),
        "la misma palabra en una línea que no es un comentario sí cuenta: {codigo:?}"
    );
}
