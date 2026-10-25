# Tasks: self-contained-themes

> Dos prestaciones en un change: el **arreglo** del bug —el HTML servido pasa a ser autocontenido— y los
> **temas**, que son la misma máquina. La fase RED son pruebas que **fallan contra el árbol de hoy**: la
> sustitución del marcador por un `<style>`, la selección de tema y la reproducción del bug en el ejemplo.

## 0. Prerrequisito

- [x] 0.1 Dejar constancia de la medición que abre el change: en un host real, `GET /boom` responde
      `503 text/html` con la página correcta y **sin estilos**, porque `GET /css/error.css` en ese host
      responde `404 text/plain; charset=utf-8` y el navegador rechaza la hoja por su `Content-Type`.
- [x] 0.2 Dejar constancia de que la misma página en el host del respaldo se ve bien, y por qué: allí este
      servicio **sí** sirve el asset `/css/error.css` (garantía 4), así que el enlace resuelve.
- [x] 0.3 Confirmar que el ejemplo del repositorio **reproduce** el caso: `app.localtest.me/boom` es el
      `503` del backend de `examples/traefik/`, detrás del middleware `errors`.
      > **Medido con la imagen construida desde el árbol** (no la publicada): el `503` de ese host sale
      > con el cuerpo correcto y sin enlace vivo a `/css/error.css`; ver 3.2.

## 1. RED

- [x] 1.1 `src/render.rs` (`#[cfg(test)]`): pruebas unitarias de la sustitución. El marcador se sustituye
      por un `<style>` con la hoja y no queda `rel="stylesheet"`; una página **sin** marcador sale byte a
      byte igual; con **dos** `<link>` solo se sustituye el marcador; con el marcador en otro orden de
      atributos también se sustituye.
- [x] 1.2 `src/main.rs`: `GET /404` y `GET /503` con `Accept: text/html` devuelven un cuerpo con `<style`
      y **sin** `rel="stylesheet"`.
      > **Más una guarda que ya pasaba:** `x_theme_no_afecta_al_json` —el tema no toca un JSON— es una
      > prueba de no-regresión, no de la prestación nueva: se escribió en RED y nació en verde.
- [x] 1.3 `src/main.rs`: sin `THEME`, la hoja incrustada es la de `content/css/error.css` (comparación de
      bytes con `include_str!`); con `THEME=terminal`, es la del tema `terminal`.
- [x] 1.4 `src/main.rs`: `X-Theme: paper` cambia la hoja incrustada; un `X-Theme` desconocido **no** cambia
      la del tema configurado y **no** es un error; `Vary` incluye `X-Theme` en los dos casos.
- [x] 1.5 `src/config.rs`: sin `THEME` el valor por defecto es `default`; `THEME=neon` es un error de
      arranque cuyo mensaje **nombra `THEME`** y **lista los seis nombres**; `THEME=""` también es error.
- [x] 1.6 Paridad: cada hoja de tema es byte a byte la de `content/` (extiende la prueba que ya cubre las
      páginas y la hoja única), y cada hoja define `body`, `h1`, `h2`, `hr` y `p`.
- [x] 1.7 `examples/traefik/check.sh`: bloque nuevo sobre `app.localtest.me/boom` con dos ayudantes que
      manden `Accept: text/html` —el cuerpo contiene `<style`, el cuerpo **no** contiene
      `rel="stylesheet"`— y una comparación de cuerpos que demuestre que `X-Theme: terminal` cambia la hoja
      de esa petición. Con el árbol de hoy, los tres casos **SHALL** salir en rojo.
- [x] 1.8 Ajustar la prueba que este change invalida:
      `src/main.rs::sin_directorio_la_pagina_embebida_enlaza_a_la_hoja_embebida` exigía que la página
      embebida **enlazara** a `css/error.css`; pasa a exigir que se sirva **autocontenida** y que
      `GET /css/error.css` **siga** respondiendo `200 text/css`.

## 2. GREEN

- [x] 2.1 `src/theme.rs` (nuevo): `Theme`, `THEMES`, `DEFAULT_THEME_NAME`, la lista de nombres válidos,
      la búsqueda por nombre y la resolución —`X-Theme` conocido gana a `THEME`; desconocido o ausente, el
      configurado—.
- [x] 2.2 Las cinco hojas nuevas `content/themes/{minimal,terminal,paper,amber,compact}.css`, sobrias y
      **sin recursos externos**, con su comentario de cabecera; y el comentario de `content/css/error.css`
      puesto al día (ya no es la única hoja del proyecto). Sus **reglas** no cambian.
- [x] 2.3 `src/config.rs`: `THEME` con defecto `default`, la variante de error que nombra la variable y
      lista los nombres, y el campo del tema en `Config`.
- [x] 2.4 `src/embedded.rs`: la constante única de la hoja de `default`, compartida con el asset
      `/css/error.css` (un solo `include_bytes!`).
- [x] 2.5 `src/render.rs`: la sustitución del marcador —solo si existe; la hoja se lee de `CONTENT_DIR` y,
      si no se puede, se usa la embebida con un `warn!`— y el hilo del tema hasta el cuerpo HTML.
- [x] 2.6 `src/main.rs`: `X-Theme` entre las pistas de la petición, la resolución del tema por petición y
      `X-Theme` añadido a `Vary`.
- [x] 2.7 Todas las pruebas de la fase RED en verde, y los 148 tests previos que sigan siendo válidos.

## 3. Verificación

- [x] 3.1 `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` y
      `cargo deny check -D warnings`.
- [x] 3.2 `check.sh` completo en verde con el ejemplo levantado: los cinco bloques de siempre **más** el
      nuevo, sin que el arreglo se lleve por delante `/css/error.css` (que sigue siendo `200 text/css`) ni
      el respaldo.
      > **Medido de extremo a extremo, con la imagen construida desde el árbol** (`podman build`, no la
      > publicada): `check.sh` pasa de **3 rojos y `exit 1`** a **15 ok / «todo correcto» / `exit 0`**,
      > con los seis bloques en verde. `/css/error.css` sigue siendo `200 text/css` en el host del
      > respaldo, y el respaldo sigue sirviendo el código real.
- [x] 3.3 Medir en el ejemplo: el `503` de `app.localtest.me/boom` llega **con la hoja dentro** y sin
      enlace, y el `404` del respaldo (`nada.localtest.me/404`) sigue igual.
- [x] 3.4 `cargo package --list --offline`: las cinco hojas nuevas y `content/css/error.css` viajan en el
      `.crate` (las macros de incrustación las exigen al compilar).
      > **Ejecutado en esta sesión:** las seis hojas —`content/css/error.css` y
      > `content/themes/*.css`— aparecen en el listado del paquete.
- [ ] 3.5 Dejar anotado que el host real necesita la **imagen nueva**; no se despliega desde aquí.
      > **Pendiente del despliegue, y es una acción del operador, no del change.** El árbol no arregla el
      > host real: lo que falta es publicar la imagen nueva y desplegarla. Queda abierta a propósito.

## 4. Documentación

- [x] 4.1 `README.md`: `THEME` en la tabla de variables, la cabecera `X-Theme`, el párrafo de
      autocontención con su motivo medido y la paridad con `error-pages` (`TEMPLATE_NAME`), y la lista de
      los seis temas con sus rutas.
- [x] 4.2 `AGENTS.md`: el quirk de la autocontención (el marcador en el fichero, la hoja incrustada al
      servir, por qué sin marcador no se lee ninguna hoja, y por qué la hoja del operador gana sin poder
      divergir del asset), `THEME` en la tabla de configuración, `X-Theme` en `Vary`, y el recuento de
      tests actualizado.
- [x] 4.3 `examples/traefik/README.md` y `compose.yml`: el bloque nuevo y la variable, donde corresponda.

## 5. Cierre

- [x] 5.1 `openspec validate self-contained-themes --strict` limpio.
- [x] 5.2 `openspec archive self-contained-themes`, para fusionar las tres deltas.
      > **Marcada al archivar**, que es cuando la fusión existe: `validate --strict` limpio (5.1) y el
      > archivado como `2026-10-25-self-contained-themes`, con `+ 2, ~ 3, - 0, → 0`. Las tres deltas
      > —`error-page`, `http-server` y `response-extras`— se fusionaron sobre los specs vigentes, que
      > quedan en **15** requisitos en `error-page`, **6** en `http-server` y **3** en
      > `response-extras` (`grep -c '^### Requirement'`). El escenario
      > `la página embebida y su hoja se sirven juntas` sobrevive con su cuerpo nuevo: la página se
      > sirve **autocontenida** y `GET /css/error.css` sigue respondiendo `200 text/css`. Quedan sin
      > marcar a propósito 3.5 —la imagen nueva en el host real, acción del operador— y 5.3, que la
      > cierra el operador a mano.
- [ ] 5.3 Borrar `PLAN.md`, que quedó como fichero de trabajo sin versionar.
      > Sin commit ni PR: el trabajo se queda en el árbol, como en el change anterior.

## Notas de cierre

1. **La auditoría encontró cuatro cosas que el plan no había previsto, y las cuatro se arreglaron.**
   - **El escáner pasó a trabajar por bytes.** La primera versión decodificaba a `str` y, ante un
     cuerpo con bytes no-UTF-8, lo devolvía **sin incrustar nada**. Como el marcador es ASCII puro,
     eso era un fallo **silencioso** del mismo tipo que el bug: una página del operador en ISO-8859-1
     con el marcador conservaba el enlace roto. Ahora el escaneo es por bytes y hay una prueba con un
     cuerpo con bytes UTF-8 inválidos.
   - **La prueba de identidad de la hoja `default` se reescribió.** Leía las fuentes `src/**` y
     contaba un `include_bytes!`: medía la **forma del texto**, la rompía un refactor inocuo y se le
     escapaba una copia con otra ruta. Ahora se comprueba **por puntero** (`ptr::eq`) sobre
     `embedded::DEFAULT_SHEET_BYTES`, un `static` **de tipo array**: un
     `static X: &[u8] = include_bytes!(…)` **no** garantiza dirección única —al leerlo en el
     inicializador de otro `static` el compilador re-inlinea el `include_bytes!` y con varias *codegen
     units* las direcciones difieren (medido: `ptr::eq` da `false` con `cargo test`)—.
   - **Faltaba cobertura de un `SHALL`:** no había ninguna prueba de que un
     `content/themes/<nombre>.css` legible gane a la copia embebida para un tema que **no** sea
     `default`. Ahora está, con su fixture.
   - **El escenario «la hoja existe y no se puede leer» no se medía:** ahora sí, con un **directorio**
     donde el tema espera su hoja —los permisos `000` no valen, porque en CI como `root` se leen
     igual—.
2. **Dos comentarios de la primera versión se corrigieron** porque decían algo falso o circular: el que
   daba por imposible escanear un cuerpo no-UTF-8, y el que justificaba sustituir dentro de un
   comentario con «lo fija la prueba». El motivo real: un `<link>` **sin escapar** solo puede estar
   como etiqueta o dentro de un comentario —para mostrarlo como texto hay que escaparlo (`&lt;link`) y
   entonces ya no casa—, así que sustituirlo no toca nada que el usuario vea; lo que **no** puede
   quedar en ningún caso es un enlace vivo a `css/error.css`.
3. **Una guarda nueva, barata:** ninguna hoja de `THEMES` puede contener `-->` ni `</style>`, porque el
   marcador también se sustituye dentro de un comentario HTML y una hoja con `-->` cerraría ese
   comentario y dejaría ver el `</style>` que la envuelve.
4. **Dos efectos colaterales medidos, que conviene saber** —hechos, no problemas—: (a) el cuerpo
   servido lleva un bloque `<style>` con la hoja, y el **comentario de cabecera de la hoja menciona
   `<link>`**, así que una guarda escrita como «el cuerpo no contiene `<link>`» daría un **falso
   rojo**; lo que hay que afirmar es la ausencia de `rel="stylesheet"`; y (b) ese comentario viaja con
   cada página (~1 KB de los ~2,9 KB de `error.css`), un dato a tener en cuenta si algún día se recorta
   la hoja.
5. **Lo que queda fuera, y no es un olvido:** la rotación de temas (`ROTATION_MODE` de `error-pages`),
   los temas personalizados —el registro es incorporado, así que un fichero del operador **reemplaza**
   la hoja de un tema que existe, pero no añade un nombre nuevo— y el **despliegue en el host real**,
   que necesita la imagen nueva (3.5, acción del operador). El árbol cierra con **176 tests** (168
   unitarios + 3 de `tests/content_pages.rs` + 5 de `tests/traefik_catchall.rs`).
