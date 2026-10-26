# Tasks: themes-from-content-dir

> La lista de temas deja de ser una **tabla cerrada en el binario**. El directorio `CONTENT_DIR`
> **añade** temas al **registro**: los siete incorporados se quedan como base —un `cargo install` sigue
> sirviendo los siete diseños sin ficheros de datos— y un `*.html` de más en la **raíz** del directorio
> es un tema nuevo. El barrido es **al arrancar**; `THEME` valida contra el **registro efectivo** y
> sigue abortando el arranque si su valor no está en él; y la página del operador **no** se inspecciona
> (los tres contratos siguen midiendo solo las páginas incorporadas). El release (fase 5) queda
> pendiente de OK explícito del usuario.

## 0. Prerrequisito

- [x] 0.1 Confirmar contra el árbol que las premisas del change siguen siendo ciertas: `THEMES` es
      `static [Theme; 7]` (`src/theme.rs`); `THEME_NAMES` es una constante de texto (`src/theme.rs`);
      `Config.theme: &'static Theme` (`src/config.rs:172`); `resolve()` devuelve `&'static Theme`
      (`src/theme.rs:204`); las dos guardas de conteo viven en `src/embedded.rs` (~línea 161) y
      `tests/content_pages.rs` (~línea 278); y `content/` tiene **siete** `*.html`. Si alguna línea se
      ha movido, actualizar `proposal.md` y `design.md` **antes** de empezar la fase RED.
      > **Medido:** `THEMES` es `static [Theme; 7]` en `src/theme.rs:140`; `THEME_NAMES` en
      `src/theme.rs:180`; `Config.theme: &'static Theme` en `src/config.rs:172`; `resolve()` en
      `src/theme.rs:204`; las dos guardas de conteo en `src/embedded.rs:150`
      (`el_directorio_entrega_exactamente_las_siete_paginas_de_tema`, sobre su tabla `PAGINAS`) y
      `tests/content_pages.rs:277` (`cada_tema_tiene_su_pagina_y_el_tallo_coincide`); `content/`
      tiene **siete** `*.html`. Ninguna línea se ha movido: `proposal.md` y `design.md` no se tocan.
- [x] 0.2 Confirmar los puntos de uso del tema, uno a uno: `src/config.rs` (validación de `THEME`,
      `ConfigError::UnknownTheme`), `src/theme.rs` (`theme_by_name`, `default_theme`, `resolve`,
      `Theme::page_path`, `Theme::page`), `src/render.rs` (`html_body`), `src/main.rs`
      (`hints.x_theme`, `config_con_theme`, los tests de router). Anotar cuáles asumen `&'static`.
      > **Medido:** los puntos de uso son `src/config.rs` (la validación de `THEME` dentro de
      `from_env_iter`, `ConfigError::UnknownTheme` y `Config.theme`), `src/theme.rs` (`THEMES`,
      `THEME_NAMES`, `theme_by_name`, `default_theme`, `resolve`, `Theme::page_path`,
      `Theme::page`), `src/render.rs` (`html_body`) y `src/main.rs` (`hints.x_theme` y la llamada
      `theme::resolve` en la línea 579, más el ayudante `pagina_del_tema` del `mod tests`). Asumen
      `&'static`: `Config.theme`, el valor de retorno de `resolve`, el tipo de `Theme::embedded`
      (`&'static str`) y `default_theme()`.
- [x] 0.3 Confirmar que `tests/fixtures/content/` **no** añade ningún tema —tiene `default.html` y
      `css/404.css`, y su `default.html` es el *override* del tema incorporado, no un tema nuevo—, así
      que las pruebas de descubrimiento necesitan su **propio** directorio de fixtures (por ejemplo
      `tests/fixtures/temas-extras/`).
      > **Medido:** `tests/fixtures/content/` tiene `default.html` y `css/404.css`. El `default.html`
      es el *override* del tema incorporado (su tallo coincide con un nombre incorporado, así que no
      añade tema) y el `404.css` está en un subdirectorio, así que tampoco. **No añade ningún
      tema**: el descubrimiento necesitaba sus propios fixtures, `tests/fixtures/temas-extras/` y
      `tests/fixtures/temas-extension/`.

## 1. RED (pruebas que fallan primero)

- [x] 1.1 **El descubrimiento** en el `#[cfg(test)]` de `src/theme.rs`, con un directorio temporal:
      un `neon.html` de más **es** un tema; un `default.html` **no** añade un `default` de más (es la
      página del incorporado); un `404.html` **no** es un tema y deja constancia; un
      `temas/neon.html` **no** es un tema; `neon.htm`, `neon.HTML` y `neon.css` **no** son temas; un
      directorio inexistente da los siete **sin** error; un directorio ilegible da los siete y deja
      constancia.
      > **Medido:** **11** pruebas nuevas en el `#[cfg(test)]` de `src/theme.rs`
      (`el_registro_enumera_los_nombres_en_orden`, `un_fichero_de_mas_es_un_tema`,
      `un_fichero_con_el_tallo_de_un_incorporado_no_anade_tema`,
      `un_fichero_con_nombre_de_codigo_no_es_un_tema`, `solo_se_mira_la_raiz_de_content_dir`,
      `la_extension_tiene_que_ser_html`, `un_directorio_que_no_existe_da_los_siete`,
      `un_directorio_que_no_se_puede_listar_deja_los_siete`,
      `un_enlace_simbolico_a_un_fichero_legible_es_un_tema`,
      `el_tema_descubierto_cuyo_fichero_desaparece_es_un_error_de_e_s`,
      `la_ruta_sale_del_nombre_del_registro`), sobre **dos** directorios de fixtures versionados
      más un directorio temporal. Los fixtures de extensión van en un directorio **aparte**
      (`temas-extension/`) a propósito, para no colisionar en un sistema de ficheros insensible
      a mayúsculas con `neon.html`.
- [x] 1.2 **La invariante de la ruta con `X-Theme`** en `src/theme.rs`: un `X-Theme: ../../etc/passwd`
      cae al tema configurado y **no** lee ningún fichero fuera de `CONTENT_DIR`.
      > **Medido:** `la_ruta_sale_del_nombre_del_registro` (`src/theme.rs`) prueba los cinco
      intentos (`../../etc/passwd`, `/etc/passwd`, `..`, `neon/../../x`, `neon.html`) contra el
      registro, y `el_nombre_de_x_theme_no_compone_ninguna_ruta` (`src/main.rs`) prueba cuatro de
      ellos **por el router**, exigiendo el cuerpo del tema configurado y nunca un `500`.
- [x] 1.3 **La validación de `THEME` contra el registro** en `src/config.rs`: `THEME=neon` con un
      `CONTENT_DIR` que tiene `neon.html` **arranca** y el tema es `neon`; el mismo `THEME=neon` sin el
      fichero **falla** nombrando `THEME` y enumerando los nombres del registro (el caso del volumen
      sin montar).
      > **Medido:** `un_theme_descubierto_arranca` (arranca y el tema es `neon`, sin copia embebida),
      `un_theme_correcto_con_el_volumen_sin_montar_no_arranca` (aborta nombrando `THEME` y
      enumerando los siete, sin `neon`) y `el_error_enumera_los_nombres_del_registro` (con temas
      propios, el error enumera los siete **y** `neon`), más `un_theme_desconocido_no_arranca`,
      `el_theme_matrix_es_valido` y `un_theme_presente_pero_vacio_no_arranca` retocados
      a la API nueva.
- [x] 1.4 **Los tests de router** en `src/main.rs`: un tema descubierto servido por `THEME` y por
      `X-Theme`; el fichero desaparecido entre el arranque y la petición responde el **respaldo de
      texto** con el estado intacto y **no** `500`; `X-Theme` no afecta a JSON, XML ni texto.
      > **Medido:** cinco pruebas de router en `src/main.rs`:
      `un_fichero_de_mas_en_content_dir_es_un_tema`, `un_tema_descubierto_se_elige_por_x_theme`,
      `el_fichero_de_un_tema_descubierto_desaparecido_responde_el_respaldo` (estado `404`,
      `text/plain`, **no** `500`), `el_nombre_de_x_theme_no_compone_ninguna_ruta` y
      `x_theme_no_cambia_los_formatos_que_no_tienen_pagina`.
- [x] 1.5 **Las dos guardas de conteo retocadas**: `src/embedded.rs` y `tests/content_pages.rs` siguen
      exigiendo exactamente siete páginas en el `content/` **del repositorio**, y un `content/neon.html`
      de más las sigue poniendo en rojo (el descubrimiento es del operador, no del repositorio).
      > **Medido:** las dos guardas **no** necesitaron retoque y **no** se relajan: ninguna usa
      `THEMES` ni `THEME_NAMES` (la de `src/embedded.rs` itera su propia tabla `PAGINAS`; la de
      `tests/content_pages.rs` lista `content/`), así que siguen exigiendo exactamente **siete**
      páginas en el `content/` del repositorio.
- [x] 1.6 Correr `cargo test --offline` y **medir el rojo**: anotar cuántas pruebas pasan y cuántas
      fallan, y confirmar que las que fallan son **solo** las nuevas que se espera que fallen (no una
      regresión).
      > **Medido:** `cargo test --offline` **no compila** — **11 errores** y **cero** pruebas
      ejecutadas: `E0432` (imports `super::INCORPORATED` y `super::Themes`), `E0425`
      (`cannot find value INCORPORATED`), `E0599` ×2 (`is_none` y `expect` sobre `&'static str`),
      `E0609` (`no field themes on type config::Config`) y `E0061` ×6 (la aridad nueva de `resolve`,
      que hoy toma 2 argumentos y las pruebas le pasan 3).

### Desviaciones del plan (fase 1)

- **El rojo de esta fase es un error de compilación, no un rojo de aserciones.** El change es un
  **cambio de tipo del crate entero** —`Theme` deja de ser `&'static` y el registro pasa a ser un valor
  que `Config` **posee**—, así que ninguna prueba puede compilar mientras los tipos nuevos no existan.
  Medido: **11 errores** y **cero** pruebas ejecutadas. Consecuencia metodológica, escrita para que no
  se lea como un hueco: el «verde heredado» **no es medible en RED**; la evidencia de que no hay
  regresión es el **baseline medido antes de RED** —**235** en verde: 210 unitarias, 7 de
  `content_pages`, 9 de `puertas` y 9 de `traefik_catchall`— más el verde de la fase 2.
- **El baseline de `181 passed; 29 failed` era un artefacto, no un rojo del árbol.** El binario de test
  que Cargo reutilizaba tenía grabado `CARGO_MANIFEST_DIR=/tmp/opencode/probe2` —una ruta de una
  sesión anterior—, así que las 29 pruebas de contenido buscaban un `content/` que no existía allí.
  Cargo **no** lo reconstruía porque los fuentes no habían cambiado. Con `cargo clean -p errserve`
  el paquete se recompiló y las tres puertas quedaron verdes con **235**. **Trampa de medición,
  para la próxima vez**: si `cargo test` da fallos masivos de `NotFound` sobre `content/`, mirar con
  `grep -a` el `CARGO_MANIFEST_DIR` grabado en el binario de `target/debug/deps/` **antes** de
  creerse el rojo.
- **rustc no nombra los métodos de `Themes` mientras el tipo no exista.** Los errores de la primera
  mitad nombran `INCORPORATED`, `Themes` y la **aridad** de `resolve` (`E0061`), pero **no**
  `Themes::descubrir` ni `Themes::names`: el compilador aborta en el primer tipo ausente, así que el
  listado de elementos que faltan por `grep` solo da dos nombres únicos. El contrato de la API queda
  fijado por las pruebas, no por los mensajes del compilador.
- **El `Display` de `ConfigError::UnknownTheme` conserva el valor ofensor, y la aserción negativa del
  test se corrigió a lo preciso.** Las otras cuatro variantes del enumerado citan su valor con
  `{value:?}` (`InvalidPort`, `UnknownLogFormat`, `DefaultErrorPageOutOfRange`, `InvalidRetryAfter`);
  `UnknownTheme` hace lo mismo —el operador tiene que ver qué escribió— y el mensaje enumera después los
  nombres del registro. El `!message.contains("neon")` del test era un **proxy** de «`neon` no está
  entre los válidos» y chocaba con la cita; la prueba fija ahora las dos cosas por separado: el valor
  ofensor se cita (`"neon"`) y la **enumeración** es exactamente la del registro de ese arranque.
- **El test del enlace simbólico usa el `dir_temporal` que ya existía en `src/theme.rs`** (línea 1643,
  el de la sección de `Theme::page`), no uno nuevo: el encargo lo daba por nuevo y no hacía falta.

## 2. GREEN (implementación mínima)

- [x] 2.1 `src/theme.rs`: la función que construye el **registro efectivo** desde un `CONTENT_DIR` —los
      siete incorporados **más** los descubiertos en la raíz, extensión exacta `html`, el tallo como
      nombre, el nombre de un código fuera con constancia—; `Theme` deja de ser `&'static` y un tema
      descubierto **no** lleva copia embebida; `theme_by_name`, `default_theme` y `resolve` operan sobre
      el registro.
      > **Medido:** `static INCORPORATED: [Theme; 7]` (`src/theme.rs:177`) con `name: Cow::Borrowed(…)`
      y `embedded: Some(embedded::…_PAGE)`; `Theme { name: Cow<'static, str>, embedded:
      Option<&'static str> }` con `#[derive(Debug, Clone)]` (línea 151); `DEFAULT_THEME_NAME` (209);
      `default_theme() -> &'static Theme` como `&INCORPORATED[0]` (391), **sin pánicos** —el crate no
      tiene ni un `expect`/`unwrap`/`panic!` en producción—; `struct Themes` (223) con `incorporados()`,
      `descubrir(content_dir)`, `by_name`, `names()` e `iter()`; `resolve(registry, requested,
      configured) -> &Theme` con comparación **exacta** (409); `Theme::page_path` derivada del nombre
      (429) y `Theme::page` (453) con la regla nueva (disco si se puede leer; `NotFound` con
      `embedded: Some` → la copia embebida **sin** `warn!`; `NotFound` con `embedded: None` → `warn!`
      que nombra el tema y el path, y `Err`, que el render convierte en el respaldo de texto con el
      estado intacto). `THEMES`, `THEME_NAMES` y `theme_by_name` **retirados**. Las reglas del barrido
      implementadas (`tema_descubierto`, 337): solo la raíz; `entrada.path().is_file()` (sigue
      enlaces); extensión **exactamente** `html`; tallo UTF-8 no vacío; el tallo de un incorporado no
      añade tema; un tallo que **reclama la resolución de códigos** queda fuera **preguntándoselo a
      `crate::resolve::code_from_path(&format!("/{fichero}"))`** y no comparando el rango `400..=599`;
      descubiertos ordenados por nombre; `NotFound` del directorio en silencio y cualquier otro error
      con `warn!`; entradas ilegibles ignoradas.
- [x] 2.2 `src/config.rs`: la `Config` **posee** el registro; `THEME` valida contra él y el mensaje de
      `ConfigError::UnknownTheme` enumera los nombres **en ejecución**; `Config.theme` deja de ser
      `&'static Theme`.
      > **Medido:** `Config { …, themes: Themes, theme: Theme }` (`src/config.rs:191` y `195`) —el tema
      **por valor**, ya no `&'static`—; `ConfigError::UnknownTheme { value: String, validos: String }`
      (219) cuyo `Display` **cita el valor ofensor** (`{value:?}`, como las otras cuatro variantes) y
      enumera después los nombres del **registro** (268); `from_env_iter` parsea el `content_dir`
      **antes** del bloque del tema (378) y construye el registro con `Themes::descubrir` (388).
- [x] 2.3 `src/render.rs` y `src/main.rs`: el tema viaja como **referencia del registro** hasta el
      render; `resolve()` deja de devolver `&'static Theme`; el valor de `X-Theme` **no** compone
      ninguna ruta (D4).
      > **Medido:** `error_response` resuelve el tema **antes** del `spawn_blocking`
      (`src/main.rs:589`: `theme::resolve(&config.themes, hints.x_theme.as_deref(), &config.theme)
      .clone()`) y mueve la **copia en propiedad**, porque el cierre exige `'static` y no puede llevarse
      un préstamo de `state.config`. El valor de `X-Theme` **nunca** compone una ruta (D4).
      `src/render.rs` **sin cambios**: firmas de `html_body`/`render_page` intactas y `tema_default()`
      sigue valiendo porque `theme::default_theme()` conserva la forma `&'static Theme`.
- [x] 2.4 `cargo test --offline` en **verde** y `cargo check --offline` sin errores.
      > **Medido:** `cargo test --offline` en verde —cuatro líneas `test result: ok.` y **0 failed**
      (229 + 7 + 9 + 9 = **254**)— y `cargo check --offline` termina sin errores.

### Desviaciones del plan (fase 2)

- **Un `E0308` en el `mod tests` de la fase RED, y su arreglo de un token.** `src/theme.rs:1757` (1744
  antes del `cargo fmt`) comparaba `&Cow<'static, str>` contra `&&str` (`!SOBRIAS.contains(&matrix.name)`);
  el arreglo autorizado fue `!SOBRIAS.contains(&matrix.name.as_ref())`. La aserción dice **exactamente lo
  mismo** y con la misma fuerza: no se relajó ninguna.
- **`cargo fmt` tocó solo líneas del `mod tests`** de `src/theme.rs` (≥1360) y `src/main.rs` (820):
  reformateo, ni una palabra ni una aserción.
- **Un `grep` demasiado ancho, y por qué el comentario se queda.** `src/theme.rs:2296` (2283 antes del
  `cargo fmt`) cita `THEME_NAMES` en un comentario **dentro de `mod tests`** para explicar qué lo
  sustituye: es
  documentación útil, no andamiaje. El criterio es que **producción** no referencie el modelo viejo, y
  no lo hace.
- **`src/render.rs` no necesitó cambios**, verificado leyéndolo: sus firmas no cambiaron y
  `tema_default()` sigue valiendo.
- **La resolución del tema antes del `spawn_blocking`**: el préstamo no podía entrar en el cierre (exige
  `'static`), así que se resuelve contra el registro y se mueve una **copia** del tema. `Theme` es
  `Clone`: gratis para un nombre `Borrowed`, una asignación corta para uno `Owned`.

## 3. REFACTOR / verificación

- [x] 3.1 Las puertas, en el orden obligatorio: `cargo fmt --check`,
      `cargo clippy --all-targets --offline -- -D warnings`, `cargo test --offline` (contando los
      tests) y `cargo deny check -D warnings`.
      > **Medido:** las cuatro en verde: `cargo fmt --check` OK; `cargo clippy --all-targets --offline
      -- -D warnings` **sin un solo `warning:` ni `error:`**; `cargo test --offline` → **cuatro** líneas
      `test result: ok.` con **0 failed**: **229** unitarias, **7** de `content_pages`, **9** de
      `puertas` y **9** de `traefik_catchall` = **254** en total (frente a las **235** de partida), el
      **254** de esta fase; `cargo deny check -D warnings` → `advisories ok, bans ok, licenses ok,`
      `sources ok`. (**256** tras la auditoría: las **dos** pruebas de guarda de la sección «Auditoría
      del change»; ver ahí. El total vigente es **256**, con **231** unitarias.)
- [x] 3.2 **Medición a mano** con el binario y un `CONTENT_DIR` propio con un `neon.html`, con la
      salida literal en el informe: que `THEME=neon` sirva la página de `neon`; que `GET /neon.html`
      siga siendo un `200` **crudo** con los marcadores a la vista; que `X-Theme: neon` la sirva; que
      `THEME=neon` **sin** el fichero aborte enumerando los nombres del registro; y que **borrar** el
      fichero con el servicio en marcha dé el respaldo de texto con el estado intacto y **no** `500`.
      > **Medido:** la medición entera con `./target/debug/errserve` y un `CONTENT_DIR` propio con el
      `neon.html` del fixture, los nueve puntos **todos en verde**: (a) `THEME=neon` **sin** el fichero
      → `exit=1` con ``Error: `THEME` está presente pero no es un tema válido: "neon". Se espera uno de
      default, minimal, terminal, paper, amber, compact, matrix (¿una variable vacía?). Omitir la
      variable usa `default`.``; (b) `THEME=neon` **con** el fichero → `GET /404` con `Accept:
      text/html` sirve el cuerpo del fichero (`NEON-FIXTURE`), **cero** marcadores, `404` y
      `text/html`; (c) `GET /neon.html` → **`200` crudo** con los **tres** marcadores a la vista (la
      garantía 4 intacta); (d) `X-Theme: neon` con `THEME` sin poner → sirve la página del tema
      descubierto; (e) `X-Theme` desconocido → cae al configurado; (f) **borrar** el fichero con el
      servicio en marcha → `404`, `text/plain`, `x-robots-tag` y `Vary` con `X-Theme`, con el `WARN
      errserve::theme: el tema descubierto no tiene fichero ni copia embebida…`, **nunca un `500`**;
      (g) **editar** el fichero de un tema ya descubierto → se ve **sin reiniciar** (`NEON-FIXTURE` →
      `EDITADO-EN-CALIENTE`); (h) **añadir** un fichero nuevo → **no** es un tema hasta reiniciar (y
      tras reiniciar, sí: `THEME=tarde` sirve `TARDE-FIXTURE`); (i) un `503` con un tema descubierto →
      `503`, `text/html` y `Retry-After: 120`.
- [x] 3.3 **Decidir y reportar** el bloque del smoke test: si `.github/scripts/smoke.sh` arranca el
      contenedor con un `CONTENT_DIR` montado y un tema de más en las **dos** puertas (`ci.yml` y
      `release.yml`), y cómo montarlo sin duplicar aserciones fuera del script. Si no se puede, dejarlo
      escrito con el motivo. Es una **decisión**, no una tarea ya hecha.
      > **Medido:** **sí se añade** al smoke test, en un contenedor **aparte**. El motivo, medido: la
      imagen corre como uid **10001** y lee un `CONTENT_DIR` **montado**; un directorio en **`0700`**
      (lo que crea `mktemp -d`) responde **`404`** y con **`chmod 755`** responde **`200`** —invisible
      para `cargo test`, que lee el directorio con el uid de quien compila—. Y **no** se monta en el
      primer contenedor porque dejaría **fuera** el `content/` de la imagen y con él la aserción del
      leak (`/minimal.html -> 200`). Medido después: `just engine=podman smoke` → **21 `ok`**, exit 0, y
      la última línea `smoke test: todas las aserciones pasaron contra ghcr.io/atareao/errserve:v0.8.1`,
      con las tres aserciones nuevas (`THEME=neon` sirve el fichero montado; `X-Theme: minimal` da otro
      tema sin `NEON-SMOKE` y con `<style`; `X-Theme: noexiste` → `404` y el tema configurado) y
      `ok healthcheck -> healthy`.
- [x] 3.4 `openspec validate --all --strict` limpio (los cuatro specs vivos más este change).
      > **Medido:** `openspec validate --all --strict` → `Totals: 5 passed, 0 failed (5 items)`.

### Desviaciones del plan (fase 3)

- **La decisión del smoke test** (resumen de lo anterior) y **por qué se reporta aquí**: era una
  pregunta abierta de `design.md` (§Preguntas abiertas 1), no una tarea ya resuelta.
- **Se quitó `--rm` del contenedor del volumen, a propósito.** Medido: con `--rm`, un arranque que
  **aborta** deja el contenedor retirado y el volcado de `fallo` no sirve para nada (`Error: no
  container with name or ID "errserve-smoke-volumen" found: no such container`); sin `--rm`, `fallo`
  puede volcar `ExitCode: 1` y el mensaje del aborto, que es justo el diagnóstico que hace falta. Lo
  retira `limpiar`, que corre siempre por el `trap`.
- **Siguen siendo tres aserciones**, aunque la tercera comprueba dos cosas: el **estado** (`404`) y el
  **cuerpo** (el tema configurado). Se hizo en la misma línea `ok` para no inflar la cuenta, y el motivo
  está escrito: el cuerpo del configurado saldría igual si la petición fallara, así que el estado se
  mide aparte.
- **La frase nueva fue a `POR QUÉ EXISTE` y no a `QUÉ NO HACE`**: lo que se añadió es una comprobación
  **que sí se hace**, y `QUÉ NO HACE` es la lista de lo que **no**.
- **La frase del `HEALTHCHECK` era inexacta y se precisó.** El bloque `QUÉ NO HACE` decía que `podman`
  construye en formato OCI «que lo descarta», pero `.justfile` pasa `--format docker` cuando el motor es
  `podman`, así que el `HEALTHCHECK` **sí** se declara y **sí** se comprueba —medido: `ok healthcheck ->
  healthy`—. La exigencia sigue siendo **condicional** (si la imagen no lo declara, se dice en voz alta
  y no se cuenta como fallo) y ahora nombra la causa exacta: `podman build` **sin** `--format docker`.

### Auditoría del change (post-fase 3)

- **El revisor —solo lectura, sobre el diff completo— no encontró ningún defecto de corrección ni de
  seguridad.** Verificó que el barrido implementa las reglas del diseño: solo la raíz (D1/D6);
  `is_file()`, que sigue enlaces; la extensión comparada como cadena `== Some("html")`; el tallo UTF-8
  como nombre; el tallo de un incorporado no añade tema; el tallo que reclama la resolución de códigos
  se le pregunta a `crate::resolve::code_from_path` en vez de reimplementar el rango; `NotFound` del
  directorio en silencio y cualquier otro error con `warn!`; y los descubiertos ordenados por nombre
  detrás de los siete. La **invariante D4** se sostiene: `x_theme` tiene **un solo** consumidor,
  `theme::resolve` → `Themes::by_name`, y la ruta sale **siempre** de `Theme::page_path` =
  `format!("{}.html", self.name)`, sin ningún `join(format!(…))` ni `format!("{}.html", requested)`.
  Y **no hay** `unwrap`/`expect`/`panic!`/`unsafe`/`Box::leak` en producción; y `ConfigError::UnknownTheme`
  sigue el patrón de las otras cuatro variantes.
- **H1 — cerrado: el escenario «añadir un tema con el servicio en marcha no lo convierte en tema» no
  tenía ninguna prueba automática** (solo estaba medido a mano, `3.2` apartado h). Ahora lo fija
  `anadir_un_tema_con_el_servicio_en_marcha_no_lo_convierte_en_tema` (`src/theme.rs`), con las **dos**
  mitades: el registro ya construido **no** crece cuando el fichero aparece después, y el **arranque
  siguiente** (`Themes::descubrir` otra vez) **sí** lo ve.
- **H3 — cerrado, y medido que la prueba es portante.** La regla «los descubiertos van detrás de los
  siete y ordenados por **nombre**» **no la fijaba ninguna prueba**, y estaba medido que **no**: los
  fixtures solo tienen **un** tema descubierto válido (`neon.html`), así que **borrar el `sort_by`
  dejaba las 254 pruebas en verde**. Ahora `los_descubiertos_van_tras_los_siete_y_ordenados_por_nombre`
  compara la cadena **entera** de `names()`, y se midió la portancia: con el `sort_by` quitado **se cae
  la prueba y solo ella** (`230 passed; 1 failed`, `left: …, zeta, media, alfa` frente a `right: …,
  alfa, media, zeta`), y el `sort_by` quedó **restaurado**. Un detalle que hizo falta para que mordiera,
  y que merece quedar escrito: en esta máquina `/tmp` es **tmpfs** y `read_dir` devuelve las entradas en
  **orden inverso al de creación**, así que los ficheros se crean en orden **ascendente** (`alfa`,
  `media`, `zeta`) para que el orden del sistema de ficheros **no** coincida con el alfabético —con el
  orden inverso que parecía natural, el FS habría devuelto justo el alfabético y la prueba **no** habría
  mordido—. Límite declarado: en un sistema de ficheros cuyo `read_dir` devolviera ya el orden
  alfabético, la prueba pasaría igual con y sin `sort_by` —guarda más débil, pero **nunca** un falso
  rojo, porque con `sort_by` el resultado es siempre el alfabético—.
- **H2 — declarado, no cerrado.** Dos escenarios del delta exigen además un `warn` que **nombre** el
  fichero (el tallo que reclama la resolución de códigos) o el **directorio y su causa** (el directorio
  que existe y no se puede listar); el código lo emite, pero el crate **no tiene arnés de captura de
  `tracing`**, así que las pruebas fijan el **registro** resultante, no el aviso. Añadir ese arnés es un
  asunto propio —y una dependencia que este change no introduce—: queda **declarado** en el «Lo que no
  está medido» de `design.md`.
- **H4 — declarado, no cerrado.** El `config.clone()` que el `spawn_blocking` de `error_response` mueve
  por petición **ahora clona el registro entero**: antes la `Config` guardaba un `theme: &'static Theme`
  y ahora **posee** `themes: Themes`, así que ese `clone` copia el `Vec<Theme>` —una asignación por cada
  tema **descubierto** (`Cow::Owned`), nada por los incorporados (`Cow::Borrowed`)— en el camino que más
  se recorre durante una incidencia. Es **evitable** (`render_page` solo consume `content_dir` y
  `retry_after`; el tema viaja aparte, y **ese** `clone` sí es necesario por el `'static` del cierre),
  pero **no se arregla aquí a propósito**: el arreglo cambia la firma de `src/render.rs`, que es otro
  asunto y este change decidió no tocarlo. Queda **declarado** en `design.md`, con el número de temas
  descubiertos como la variable que lo escala.
- **H5 — corregido y medido.** `fallo()` del smoke test volcaba **los dos** contenedores aunque el
  segundo no existiera todavía, y el motor imprimía `no such container` (con `|| true`, sin romper nada,
  pero ensuciando el diagnóstico del caso típico: el arranque con `THEME=neon` aborta). Ahora se
  comprueba la existencia y el que no existe se salta en silencio. **Medido con A/B** contra una imagen
  falsa que no responde en `/healthz` (forzando el fallo en la aserción 1, con el segundo contenedor aún
  sin crear): antes salían **dos** líneas `no such container`; después, el volcado del que sí existe sale
  **igual** y con el **mismo** `exit 1` y el mismo `::error::`.
- **Dos preguntas abiertas de `design.md` quedan resueltas** y la tercera sigue abierta: la **1** (el
  bloque del smoke test) se cerró en la fase 3 —sí se añade, en contenedor aparte, con el motivo medido
  (`0700` → `404`, `0755` → `200` con el uid 10001)—; la **3** (la firma del registro) se cerró en la
  fase RED —`Config` **posee** el registro y el tema **por valor**, con `Cow<'static, str>` y sin
  `Box::leak`—; y la **2** (el coste del barrido al arrancar) sigue **abierta**, sin medir con miles de
  entradas ni con un directorio en red. Las resoluciones están escritas en `design.md`, no aquí.

### Desviaciones del plan (fase 5)

- **La sección del CHANGELOG se regeneró porque los primeros mensajes de commit no llevaban
  acentos.** Los cuatro commits se rehicieron (`git reset --mixed` al punto de partida de la rama y
  los cuatro commits otra vez, con su ortografía) y la sección se volvió a generar con
  `just changelog v0.9.0` desde los commits definitivos, reponiendo después la prosa **literal**.
  Motivo: el CHANGELOG es la cara pública del release y `anade`/`mas`/`al dia` son erratas.
- **La sección generada trae una viñeta de más, y se deja a propósito.** `v0.8.1` **no es ancestro de
  `development`** —el tag vive en `main` y `development` nunca mergeó `main` de vuelta—, así que
  git-cliff procesa `v0.8.1..HEAD` y le entra el commit `3985680 docs(openspec): archiva el change de
  la 0.8.1`. Se deja porque es **el patrón histórico del fichero**: ya hay **seis** viñetas
  `- **openspec**: Archiva el change de la 0.X` en versiones anteriores (líneas 46, 182, 199, 201,
  221, 225, 306, 307). No es un defecto de esta versión.
- **El `grep` de erratas que se usó daba falsos positivos** por la subcadena `mas ` dentro de
  «te**mas** »; comprobado con palabra completa, la sección no tiene ninguna errata. Merece quedar
  escrito para que nadie «arregle» un problema que no existe.

## 4. DOCS

- [x] 4.1 `README.md`: el modelo nuevo —la tabla de temas deja de ser cerrada; el directorio **añade**
      temas; el barrido es al arrancar y editar un tema se ve sin reiniciar; `THEME` valida contra el
      registro; la página del operador no se inspecciona; el leak no se reabre porque el respaldo de
      Traefik bloquea **por forma**—.
      > **Medido:** los **diez** pasajes tocados (el apartado «El tema es una página» vale por dos: el
      > reescrito y el párrafo **nuevo** de las reglas), con su contenido: (a) la tabla de configuración
      > —`CONTENT_DIR` pasa a «el directorio del operador **y la fuente de temas**»: *override* de la
      > página de un incorporado y los `*.html` de su **raíz** **añaden** al registro— y `THEME` a «uno
      > de los siete **incorporados** (`default`, …, `matrix`) **o** un tema descubierto en
      > `CONTENT_DIR`»; (b) el párrafo del aborto, que ya no dice «enumera los siete» sino **los nombres
      > del registro de ese arranque**, más la **trampa del montaje** (D7) escrita ahí mismo —un `THEME`
      > **correcto** cuyo fichero **todavía** no esté montado **aborta**; la mitigación es del operador—;
      > (c) el párrafo de `CONTENT_DIR`, que ya no es «solo un *override*»; (d) el tercer eje de «Cómo se
      > resuelve» —`X-Theme` gana cuando nombra un tema del **registro**, los siete incorporados **más**
      > los descubiertos—; (e) «El tema es una página», con el barrido al arrancar, **añadir** exige
      > **reiniciar**, **editar** se ve **sin** reiniciar y un tema descubierto sin copia embebida,
      > **más** el párrafo **nuevo** con las reglas del barrido (solo la raíz; extensión **exactamente**
      > `html`; el tallo como nombre; el tallo de un incorporado no añade; un tallo que reclama la
      > resolución de códigos fuera; ordenados por nombre detrás de los siete); (f) la intro de «Los
      > siete temas», que ahora los presenta como la **base** del registro; (g) la selección del tema
      > —`X-Theme` **conocido**, un tema del registro, gana a `THEME`—; (h) el **inciso del leak** —un
      > tema descubierto nuevo queda protegido **sin añadirlo a ninguna lista**, porque la regla del
      > respaldo bloquea **por forma**, no por nombre—; e (i) los ***no-goals***, que **decían lo
      > contrario de lo que hace el código** («los siete están incorporados, así que `THEME` no admite
      > un nombre que no sea uno de ellos … porque eso haría que `THEME` dependiera del sistema de
      > ficheros al arrancar») y ahora dicen la verdad: los siete **siguen** incorporados **y** el
      > directorio **añade**; `THEME` **sí** depende del sistema de ficheros al arrancar, con su precio
      > escrito (**añadir** exige **reiniciar**). Y el bloque de la conjunción de Traefik **no** se tocó
      > —lo lee `tests/traefik_catchall.rs`—.
- [x] 4.2 `AGENTS.md`: la **garantía 7** (el binario sirve los siete sin ficheros de datos) y la tabla
      de configuración (`THEME`, «uno de los siete incorporados o un tema descubierto en
      `CONTENT_DIR`»), el inventario de changes archivados, y un **quirk** del barrido al arrancar con
      la **trampa del montaje** (D7: un `THEME` correcto con el volumen sin montar aborta).
      > **Medido:** (a) la **garantía 7** ampliada: el directorio **añade** al **registro** construido
      > **al arrancar** —los siete incorporados **más** los descubiertos, con las reglas del barrido y
      > la trampa del montaje remitidas al *quirk* del barrido—, con el precio —**añadir** exige
      > **reiniciar**; los **bytes** de uno ya descubierto se leen al servir, así que **editar** se ve
      > **sin** reiniciar— y la **trampa del montaje** (D7); **sin debilitar** lo que ya decía: las dos
      > copias de un **incorporado** no pueden divergir (aclarado que la paridad es de las páginas
      > **incorporadas**, que son las que tienen copia embebida); (b) la **tabla de configuración**, con
      > `CONTENT_DIR` («el directorio del operador: *override* de la página de un tema incorporado **y**
      > fuente de temas nuevos») y `THEME` («uno de los siete **incorporados** … **o** un tema
      > descubierto en `CONTENT_DIR`; un nombre que no esté en el **registro** de ese arranque, o vacía,
      > es error de arranque»); (c) el ***quirk* del smoke test** reescrito para los **dos**
      > contenedores (`PORT_VOLUMEN`, por defecto `PORT` + 1) y con el tiempo remedido (**~20,6 s** con
      > los dos, frente a los **~10,4 s** de uno); y (d) un ***quirk* nuevo del barrido**, con las reglas
      > (solo la raíz; extensión **exactamente** `html`; el tallo como nombre; el tallo de un incorporado
      > no añade; un **tallo de código fuera** con el motivo medido —`code_from_path` **parsea** el
      > número e `is_error_page_path` decide **sin** filtrar `400..=599`—; ordenados por nombre;
      > directorio inexistente **sin** `warn` e ilegible **con** él; la **trampa del montaje** D7; y la
      > **no-inspección** D9 de la página del operador). El inventario de changes archivados **no** se
      > tocó a propósito —lo pone al día la **5.5**, al archivar—: describe el estado **commiteado** y
      > sigue siendo cierto (ver la desviación 3).

### Desviaciones del plan (fase 4)

- **El tiempo del smoke se volvió a medir y cambió.** El *quirk* decía «~10,4 s», medido con **un**
  contenedor; con los **dos** que arranca ahora el script, medido con `time` contra la imagen ya
  construida: **20,6 s** (exit 0, **21 `ok`**). Se actualizó la cifra y se dejó dicho que es con los
  **dos**. El `docker build` en frío (**29,7 s**) no se toca: es de otro asunto.
- **La frase del `HEALTHCHECK` era inexacta en tres sitios y se precisó en los tres.** Decía que
  `podman` construye en formato OCI «que lo descarta», y `.justfile` pasa `--format docker` cuando el
  motor es `podman`, así que aquí el `HEALTHCHECK` **sí** se declara y **sí** se comprueba —medido: `ok
  healthcheck -> healthy`—. La exigencia sigue siendo **condicional** (si la imagen no lo declara, se
  dice en voz alta y no se cuenta como fallo) y ahora nombra la causa exacta: `podman build` **sin**
  `--format docker`. Los tres: los dos comentarios de `.github/scripts/smoke.sh` (el bloque `QUÉ NO
  HACE` y el paso 4) y el *quirk* de `AGENTS.md`.
- **El inventario de changes archivados y la frase «No hay ningún change en curso» NO se tocaron a
  propósito.** Describen el estado **commiteado** del repositorio, y ahora mismo sigue siendo cierto:
  el change está sin commitear. Se pondrán al día al **archivar** (tarea **5.5**, donde está la lista
  completa de lo que hay que actualizar, incluidos los dos sitios que dicen «235 tests» y el *bullet*
  de `src/theme.rs`, que todavía nombra `THEMES`, `THEME_NAMES` y `theme_by_name`).
- **Una corrección de la lista del encargo:** uno de los puntos apuntaba a una frase («cuando nombra
  uno de los siete temas») que estaba en el **tercer eje** de «Cómo se resuelve» y no en el párrafo de
  la selección del tema; se actualizaron **las dos**.
- **El bloque de la conjunción de Traefik no se tocó** en ninguno de los dos ficheros, y
  `tests/traefik_catchall.rs` —que lo lee del `README.md`— quedó **verde**, igual que
  `tests/puertas.rs` (que mira los workflows): **256 pruebas, 0 fallos**.

## 5. RELEASE — pendiente de OK explícito del usuario

> **No se hace commit, ni PR, ni etiqueta, ni publicación hasta que el usuario lo decida.** Esta fase
> se queda marcada y **no se ejecuta** en la fase de documentación.

- [x] 5.1 `Cargo.toml`: bump de versión (minor, permitido en `0.x`). `CHANGELOG.md` con la entrada de
      esa versión: la tabla de temas deja de ser cerrada y el directorio añade temas.
      > **Medido:** `Cargo.toml` pasó de `0.8.1` a **`0.9.0`** (minor) y `Cargo.lock` solo cambió la
      > versión del paquete (una línea). `CHANGELOG.md` tiene la sección `## [0.9.0]` con **cuatro**
      > párrafos de prosa escritos a mano —qué cambia; los precios (el barrido al arrancar y la trampa
      > del volumen sin montar); el eje que no se toca y el leak; y por qué es un minor—, más **tres**
      > grupos generados por `just changelog v0.9.0`: `### Añadido` (`theme`, `ci`) y
      > `### Documentación` (`openspec: Archiva el change de la 0.8.1`,
      > `openspec: Propone themes-from-content-dir` y el `docs` del README/AGENTS). Commit
      > `ac661f0 chore(release): 0.9.0`.
- [x] 5.2 `cargo publish --dry-run --locked` y `cargo package --list --offline`: el paquete sigue
      llevando las siete páginas.
      > **Medido:** `cargo publish --dry-run --locked` → exit 0: empaqueta **49 files, 625.7KiB
      > (169.5KiB compressed)**, compila el paquete extraído (`target/package/errserve-0.9.0`) y
      > termina con `aborting upload due to dry run`. `cargo package --list --offline` lleva las
      > **siete** páginas de tema: `content/amber.html`, `content/compact.html`, `content/default.html`,
      > `content/matrix.html`, `content/minimal.html`, `content/paper.html` y `content/terminal.html`.
- [x] 5.3 Commit, PR a `development` y PR de `development` a `main`; etiqueta `v*`; imagen, crate y
      Release.
      > **Medido:** PR **#64** (`feature/themes-from-content-dir` → `development`) y PR **#65**
      > (`release: 0.9.0`, `development` → `main`), con los **tres** checks en verde en los dos
      > (`fmt, clippy y tests`, `cargo-deny` e `imagen`). Tag **anotado** `v0.9.0` → `9ec4f84`, sobre
      > `fad93ab` (el merge del #65 en `main`). El workflow de release (`40390265184`) terminó con sus
      > **tres** jobs en `success`: `comprobar la etiqueta y la versión`, `construir, probar y
      > publicar` y `publicar el crate`. **Los tres artefactos, verificados contra los registros**: en
      > GHCR los tags `0.9.0` y `latest` comparten el **mismo** digest de índice
      > (`sha256:41ed68c7f9ce8139ac877b103cefcc14bf4bf11ef3975a2d3bf29cddda6fdd19`), distinto del de
      > la `0.8.1` (`sha256:edf7c8eb…`), y el `tags/list` anónimo ya lista `0.9.0`; en crates.io
      > `errserve 0.9.0` está publicada (2026-10-26T19:52:29Z) y **no** yanked; y el Release `v0.9.0`
      > existe, **no** es draft ni prerelease.
- [x] 5.4 `openspec archive themes-from-content-dir` **después** del OK y con la fase 3 en verde; el
      archivado fusiona las deltas en `openspec/specs/error-page/spec.md` y
      `openspec/specs/http-server/spec.md`.
      > **Medido:** `openspec archive themes-from-content-dir -y` → `Change 'themes-from-content-dir'
      > archived as '2026-10-26-themes-from-content-dir'`, tras fusionar las deltas
      > (`error-page`: **+1** añadido y **~2** modificados; `http-server`: **~1** modificado;
      > totales `+ 1, ~ 3, - 0, → 0`). `openspec validate --all --strict` → `Totals: 4 passed, 0
      > failed (4 items)`; `openspec/changes/` solo tiene `archive/`, con **16** directorios; y el
      > `tasks.md` viajó al archive con sus **24** casillas marcadas y **ninguna** sin marcar.
- [x] 5.5 Al archivar, poner al día el «Estado real del repositorio (verificado)» de `AGENTS.md`: (a)
      los **dos** sitios que dicen «**235 tests**» (la línea de «Puertas en verde» y el comentario de
      `cargo test` del bloque de comandos) pasan a **256** (231 unitarias + 7 de `content_pages` + 9 de
      `puertas` + 9 de `traefik_catchall`); (b) el *bullet* de `src/theme.rs` nombra `THEMES`,
      `THEME_NAMES` y `theme_by_name`, que este change sustituye por `INCORPORATED`, `Themes::names`/
      `Themes::by_name` y `Themes::descubrir` —hay que reescribirlo—; (c) el inventario de changes
      archivados pasa de **quince** a **dieciséis**, con `themes-from-content-dir` y su fecha, y la
      frase «No hay ningún change en curso» vuelve a ser cierta con `openspec validate --all --strict`
      dando **4 passed, 0 failed** (sin el change, ya fusionado en los cuatro specs); (d) el inventario
      de ficheros del «Estado real» tiene que nombrar `src/theme.rs` con el registro y los fixtures
      nuevos (`tests/fixtures/temas-extras/`, `tests/fixtures/temas-extension/`), y
      `.github/scripts/smoke.sh` con su segundo contenedor.
      > **Medido:** el «Estado real» puesto al día: los **dos** sitios con «**235 tests**» pasaron a
      > **256** —la línea de «Puertas en verde», cuyo desglose es ahora **231** unitarias + **7** de
      > `content_pages` + **9** de `puertas` + **9** de `traefik_catchall`, y el comentario de
      > `cargo test` del bloque de comandos—; el *bullet* de `src/theme.rs` se reescribió al
      > **registro** (`INCORPORATED`, `struct Themes` con `incorporados`/`descubrir`/`by_name`/`names`/
      > `iter` y `resolve(registry, requested, configured)`); el inventario pasó de **quince** a
      > **dieciséis** changes, con `themes-from-content-dir` en el grupo del `2026-10-26` (que pasa de
      > dos a **tres** entradas) y el párrafo «los **dos últimos**» reescrito como «los **tres
      > últimos**» con lo que entró este change (en `error-page` y `http-server`, +1 añadido,
      > ~3 modificados, 0 retirados); los *bullets* de `src/config.rs` (`themes: Themes`, `theme: Theme`
      > por valor y `ConfigError::UnknownTheme { value, validos }`) y `src/main.rs` (el tema contra el
      > registro antes del `spawn_blocking`; `X-Theme` nunca compone una ruta); el de
      > `.github/scripts/smoke.sh` con sus **dos** contenedores (`PORT_VOLUMEN`, por defecto `PORT` + 1,
      > y el uid 10001); el de `tests/` con los fixtures `tests/fixtures/temas-extras/` y
      > `tests/fixtures/temas-extension/`; y el bloque de comandos, con la línea de `just smoke`
      > mencionando también `PORT_VOLUMEN=…`. La frase «No hay ningún change en curso» se mantiene y
      > `openspec validate --all --strict` sigue dando **4 passed, 0 failed**. Única desviación: las
      > **dos** referencias a `THEMES[i].embedded` y a la tabla `THEMES` que quedaban en el *quirk* de
      > las tres guardas se corrigieron a `INCORPORATED`, porque el símbolo ya no existe y esta misma
      > tarea exige que el fichero no lo nombre.
