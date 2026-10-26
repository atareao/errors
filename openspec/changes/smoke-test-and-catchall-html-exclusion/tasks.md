# Tasks: smoke-test-and-catchall-html-exclusion

## 0. Prerrequisito

- [x] 0.1 **Medir `PathRegexp` contra Traefik v3.6** —la versión que fija `examples/traefik/compose.yml`— antes de escribirlo en ningún sitio. Una sintaxis que no case con nada deja el router `enabled` y no bloquea lo que debía, en silencio: es la trampa de ``HostRegexp(`{host:.+}`)`` de v2, ya medida y documentada en este repositorio. Dejar la medición escrita (petición, respuesta, versión). Si no casa, el matcher cambia y se reescribe la delta. **Medido** (v3.6.25, banco en `/tmp/opencode/phase0`): el router sale `enabled`, la regla se lee exacta, las seis páginas dejan de llegar al servicio y el control con la regla vieja da `200` con marcadores. Resultado en `design.md` (fase 0).
- [x] 0.2 Localizar los **cuatro** sitios que definen un respaldo desplegable y qué lleva cada uno hoy: `examples/traefik/compose.yml`, `compose.yml`, el fragmento de *provider de fichero* de `examples/traefik/README.md` y el bloque de labels del `README.md` raíz.
- [x] 0.3 Leer `tests/traefik_catchall.rs` entero y quedarse con su método: cómo descubre un respaldo por su nombre, cómo se queda con el **valor** de la regla y cómo trata un `#` de cola.
- [ ] 0.4 Medir el tiempo actual de la puerta `imagen` del CI, para poder decir cuánto cuesta el smoke test.
- [x] 0.5 **Medir el código de Traefik con el upstream caído.** **Medido**: un servidor declarado que **rechaza** la conexión da `502`; un servicio **sin servidores** da `503`. Decide cómo se construye el caso del ejemplo —no puede ser «paro el contenedor del backend»—.
- [x] 0.6 **Medir el escapado de la barra en YAML y el `$` de la regla.** **Medido**: en un escalar **entrecomillado**, `\.` mata el *provider* entero (`found unknown escape character`, contenedor arriba y cero routers); con la barra doblada o en escalar plano, Traefik lee `\.html?$`. Y el `$` final sobrevive a la interpolación de compose (ground truth: `podman inspect` del contenedor que `podman-compose up` crea).

## 1. RED (pruebas que fallan primero)

- [ ] 1.1 `tests/traefik_catchall.rs`: exigir la conjunción del leak junto a la de `/metrics`, sobre los cuatro sitios. **Rojo medido**: los cuatro llevan hoy solo la de `/metrics`.
- [ ] 1.2 Fixture: un respaldo con la exclusión de `/metrics` y **sin** la del leak incumple, y el fallo lo **nombra**.
- [ ] 1.3 Fixture: la conjunción del leak tras un `#` de cola no cuenta (el caso simétrico al de `/metrics`, porque esta conjunción es más larga y lleva `#` a mano).
- [ ] 1.4 Fixture: una regla con sintaxis de Traefik v2 sigue **sin** ser un respaldo y no se le exige la conjunción.
- [ ] 1.5 `tests/puertas.rs` — **nuevo**: guarda que ningún workflow defina o llame a los ayudantes de aserción (`comprobar(`, `cabecera(`, `fallo `) fuera del script, ni lleve su propio bucle de arranque contra el contenedor. **Rojo medido**: `release.yml` los lleva todos hoy.
- [ ] 1.6 `tests/puertas.rs`: guarda que **los dos** workflows invoquen `.github/scripts/smoke.sh`. **Rojo medido**: hoy no existe el script y `ci.yml` no invoca nada.
- [ ] 1.7 Fixture del 1.5-1.6: un workflow con aserciones propias incumple y el fallo lo nombra; un workflow que invoca el script y no lleva aserciones cumple; un `curl` suelto **no** es incumplimiento (la lista es cerrada y hay que probar que no se pasa de larga).
- [ ] 1.8 Dejar escrito el rojo: la salida de `cargo test` con las pruebas nuevas en rojo y el resto en verde.
- [ ] 1.9 Fixture: la lectura del **valor** desdobla la barra de un valor **entrecomillado** (`\\.` → `\.`) y **no** la de uno plano, que es lo que hace YAML (medido). Sin ese paso, un sitio que entrecomille su regla —forma legítima— se llevaría un rojo por una subcadena que sí está delante.
- [ ] 1.10 `tests/puertas.rs`: el caso del `502` en `check.sh` — un `check.sh` sin él incumple y el fallo lo nombra.

## 2. GREEN (implementación mínima)

- [ ] 2.1 `.github/scripts/smoke.sh` — las aserciones, una sola vez: arranque del contenedor en loopback con el motor y la imagen del entorno, espera de `/healthz`, los estados (`/404`, `/502`, `/503`, `/404.json`, `/healthz`, `/metrics`, `/minimal.html`, `/default.html`, `/default.htm`), las cabeceras (`X-Robots-Tag`, `Retry-After: 120` en el `503` y **ausencia** de `Retry-After` en el `502`), el cuerpo del `/404` con `Accept: text/html` (lleva `<style`, no lleva `<link`), el `USER` y el `HEALTHCHECK`. Retira el contenedor siempre, nombra la aserción que falla y sale distinto de cero.
- [ ] 2.2 `.github/workflows/ci.yml`: el job `imagen` construye **y** prueba, invocando el script.
- [ ] 2.3 `.github/workflows/release.yml`: retirar sus aserciones y llamar al script antes de publicar. El paso sigue llamándose «Probar la imagen con docker (smoke test)» o se renombra a algo que diga que invoca el script; el resto del job (publicar, `latest`, Release) no cambia.
- [ ] 2.4 La conjunción del leak en los cuatro sitios desplegables, **como escalar plano** (los tres labels ya lo son; el fragmento de *provider de fichero* del README del ejemplo deja de estar entrecomillado), con su comentario al lado y con la barra simple: `\.html?$`.
- [ ] 2.5 `examples/traefik/check.sh`: el caso de extremo a extremo del leak —`/minimal.html` en un Host sin router propio NO da `200` ni sirve los marcadores—, el de los dos alias, que siguen respondiendo el código resuelto, y el del **upstream caído**: un router cuyo servidor **rechaza** la conexión → Traefik `502` → la página del servicio, con su `X-Robots-Tag` (no un contenedor parado, que daría `503`).
- [ ] 2.6 `.justfile`: receta `smoke` que envuelve el script con el motor de esta máquina.
- [ ] 2.7 Puertas en verde: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --offline`.

## 3. REFACTOR / verificación

- [ ] 3.1 Correr el script **con podman** contra la imagen construida con la receta de esta máquina, y reportar la salida literal de cada aserción.
- [ ] 3.2 Levantar el ejemplo con Traefik v3.6 y medir: el leak (`/minimal.html` y `/matrix.html` en un Host sin router propio), los dos alias, `GET /` y `/503` como prueba de que la conjunción no rompe el respaldo, y el caso del **upstream caído** (`502` con la página del servicio). Salida literal.
- [ ] 3.3 Comprobar que la conjunción **no** se lleva ningún caso legítimo por delante más allá de `/404.html` y `/503.html`, que son el precio escrito.
- [ ] 3.4 Medir el tiempo de la puerta `imagen` del CI después del cambio y escribir el número.
- [ ] 3.5 `openspec validate smoke-test-and-catchall-html-exclusion --strict` y `openspec validate --all --strict`.
- [ ] 3.6 `cargo deny check -D warnings`.

### Desviaciones del plan (fase 3)

## 4. DOCS

- [ ] 4.1 `examples/traefik/README.md`: la sección «El leak de las páginas de tema» deja de decir que la mitigación es un no-goal sin implementar; la conjunción pasa a ser la que el repositorio **entrega** y una prueba guarda. La tabla de casos gana el del leak y el del upstream caído (`502`). Y la sección de trampas —la de `HostRegexp` de v2— gana la trampa medida del **escapado**: en un valor entrecomillado la barra va doblada, y con una sola el *provider* entero deja de cargar.
- [ ] 4.2 `README.md` raíz: la conjunción completa en «Operación y observabilidad», y el apartado de entrega menciona que el CI prueba la imagen en cada propuesta.
- [ ] 4.3 `AGENTS.md`: el quirk del smoke test —un artefacto, dos llamadores, la guarda y su límite— y el de la conjunción del leak, con el precio y el límite de las mayúsculas.
- [ ] 4.4 El comentario de cabecera de `.github/workflows/release.yml`: deja de ser «el único sitio donde el healthcheck y el `USER` se ejercitan contra docker de verdad».
- [ ] 4.5 El `CHANGELOG.md` — se escribe al etiquetar, no antes.

### Desviaciones del plan (fase 4)

## 5. RELEASE — pendiente de OK explícito del usuario

- [ ] 5.1 Versión (`0.8.1` propuesto: nada del servicio publicado cambia) y entrada del `CHANGELOG.md`.
- [ ] 5.2 `cargo publish --dry-run --locked` y `cargo package --list`.
- [ ] 5.3 Revisión a ojo, si toca: este change no cambia ninguna página, así que no debería tocar nada visual.
- [ ] 5.4 Commit, PR a `development`, PR a `main` y etiqueta — **con OK explícito del usuario**.
- [ ] 5.5 `openspec archive smoke-test-and-catchall-html-exclusion`, después del OK y con la fase 3 en verde.
