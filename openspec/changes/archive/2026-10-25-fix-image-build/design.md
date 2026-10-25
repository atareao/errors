# Design: fix-image-build

## Context

Ver `proposal.md` — Why. Lo que condiciona el diseño:

- **El embedding es en tiempo de compilación.** `src/embedded.rs` embebe las once páginas y
  `content/css/error.css` con `include_str!`/`include_bytes!`, y esas macros resuelven la ruta
  **cuando `cargo` construye el crate**. Si el fichero no está en ese instante, no hay binario: no es
  un fallo de ejecución que se pueda mitigar, es un error de compilación.
- **El `Dockerfile` es multi-stage y separa compilar de empaquetar.** El stage `builder` (líneas
  25-48) copia `Cargo.toml`, `Cargo.lock`, un `src/main.rs` de relleno y luego `COPY src ./src`, y
  compila; el stage `runtime` (líneas 53-97) copia `content/` (línea 76) —y el binario ya construido—
  a la imagen final. El segundo corre **después** del primero, así que un `content/` que solo esté en
  `runtime` **nunca** está presente al compilar.
- **La única puerta que construye la imagen es `release.yml`**, y se dispara al etiquetar. `ci.yml`
  corre `cargo test` con el árbol del repositorio delante —donde `content/` existe— y **no construye
  la imagen**, así que el empaquetado es el único camino del proyecto sin puerta.
- **El fallo ya está medido**, no es hipotético: `exit code 101`, doce errores `couldn't read
  src/../content/<código>.html` (el último, `content/css/error.css`), reproducido en local con
  podman. Nada se publicó: GHCR sigue con `['0.3.0', 'latest']`, no hay Release `v0.4.0` y crates.io
  sigue en `0.3.0`.

## Goals / Non-Goals

**Goals**

- Que la imagen vuelva a construirse **desde el árbol versionado**, en cualquier host y sin caché
  previa, con las páginas dentro.
- Que un fallo de empaquetado se descubra **en cada propuesta de cambio**, no la primera vez que se
  publica una etiqueta.
- Que la documentación describa el `Dockerfile` que de verdad existe.

**Non-Goals**

- No se cambia el embedding ni su motivo (un `cargo install` sirviendo páginas de verdad).
- No se decide aquí si `content/` sigue copiándose en el stage `runtime` (Open Question, abajo).
- No se toca el código, ni las pruebas, ni la lógica del contenedor.

## Decisions

### D1 — El `COPY` va en el `builder`, y no hay otra salida

El embebido es en **tiempo de compilación**: cuando `cargo build` llega a
`include_str!("../content/404.html")`, el fichero o está o el crate no compila. Poner `content/` a
disposición del stage `builder` **no es una preferencia de esta decisión, es la única forma** de que
la imagen se construya con el embedding actual. Las alternativas se descartan, con su porqué:

- **(a) Leer `content/` en tiempo de ejecución.** Es exactamente lo que el change anterior vino a
  corregir: un binario instalado no lleva ficheros de datos, y el HTML del disco deja de ser el único
  cuerpo que no se genera. Volver a leer al arrancar reintroduce el fallo que `embedded-error-pages`
  cerró.
- **(b) Cambiar las macros por una dependencia (`rust-embed`, `include_dir`).** Es una dependencia
  nueva para hacer lo que el compilador ya hace, y la puerta de `cargo deny check -D warnings` se
  paga por cada una (licencias, avisos, duplicaciones). Además no elimina el problema: `rust-embed`
  también resuelve rutas al compilar, así que seguiría haciendo falta el fichero en el `builder`.

Conclusión: `COPY content ./content` en el `builder`. El coste —que la capa del crate real se
invalide cuando cambie un HTML— es el correcto, y se razona en *Risks / Trade-offs*.

### D2 — La puerta va en `ci.yml`, no se confía en el workflow de release

El release es **tarde y escaso**: solo se ejecuta al etiquetar, y un fallo ahí se descubre con el tag
ya empujado y el release a medias —medido: `v0.4.0` quedó con etiqueta, sin imagen y sin Release—.
Cuando el CI es el que falla, el arreglo es barato: la rama todavía no está fusionada y nada se ha
publicado. Por eso la imagen se construye en **cada propuesta de cambio y en cada push**.

El coste se acepta explícitamente: el job recompila el árbol de dependencias en cada PR —con el
docker del runner y sin caché compartida—, unos minutos por ejecución. Se paga a cambio de que el
empaquetado deje de ser el único camino sin puerta. La configuración es sencilla a propósito:
`docker build` a secas, con el docker ya presente en el runner, sin actions de terceros nuevas.

### D3 — La *Open Question* del `content/` en la imagen, revisitada

El change anterior la dejó abierta con la recomendación de **conservar** la copia del stage
`runtime`, razonando que era «redundante pero no inocua» (la capa del disco gana, así que quitarla
cambiaría qué bytes se sirven). Este fallo **corrige el marco**: `content/` en el **contexto de
build** pasa a ser **obligatorio** (D1), así que la pregunta ya no es *si* se copia, sino *si se
copia además en el stage `runtime`*. Se conserva, y la pregunta sigue abierta con ese alcance más
estrecho: una cosa es que el `builder` **necesite** los ficheros para compilar, y otra distinta que la
imagen final tenga que **llevarlos** para servirlos por encima de la copia embebida.

### D4 — Se mueve la etiqueta, en vez de republicarla

`release.yml` admite un `workflow_dispatch` con la etiqueta a republicar, pero ese camino construye
desde la **rama** (`main`), no desde el árbol que señala la etiqueta, así que la imagen `0.4.0`
**no correspondería** a lo que `v0.4.0` apunta —una imagen cuyo contenido no es el del tag que
nombra—. Y **no hay nada publicado al que ser infiel**: medido, GHCR sigue con `['0.3.0', 'latest']`,
no hay Release `v0.4.0` y crates.io sigue en `0.3.0`. Mover la etiqueta —borrarla y recrearla en el
commit del arreglo— deja que el tag señale un árbol que **sí** construye la imagen, y la ventana de
riesgo es cero: nadie puede haber consumido una etiqueta que nunca produjo artefacto.

Nota para el futuro: esto **no** vale para una etiqueta ya publicada —con imagen o Release
asociados—. Ahí el camino es el `workflow_dispatch`, y hay que asumir que la imagen se reconstruye
desde la rama; mover una etiqueta de la que ya cuelgan artefactos sería reescribir historia.

## Risks / Trade-offs

- [El job nuevo alarga el CI] → se acepta: unos minutos por PR contra haber descubierto el fallo de
  empaquetado con el tag ya empujado. El job no comparte caché con `puertas` y recompila el árbol de
  dependencias, que es el grueso del tiempo; es el precio de tener una puerta para el empaquetado.
- [`content/` viaja al `builder` en toda construcción, así que su `COPY` invalida la capa del crate
  real cuando cambia un HTML] → es **correcto**, no un defecto de caché: esos bytes van **dentro del
  binario**, así que cambiar un HTML es un cambio de contenido del binario, y reconstruirlo es lo que
  debe pasar. Lo que sigue cacheado es la capa del árbol de dependencias (la del `main.rs` de
  relleno), que es la que de verdad ahorra tiempo.

## Migration Plan

1. Añadir `COPY content ./content` al stage `builder`, antes de `COPY src ./src`.
2. Añadir el job `imagen` a `ci.yml` que construye la imagen (`docker build`), **antes** de aplicar
   el arreglo, para verlo en rojo en el PR.
3. Aplicar el `COPY`, con la imagen en verde y el smoke test con podman.
4. Marcar el check nuevo como requerido en `main` y en `development`.
5. Corregir `AGENTS.md` y la entrada de la `0.4.0` del `CHANGELOG.md`.
6. Mover la etiqueta `v0.4.0` al commit del arreglo, para publicar los artefactos (D4).

## Open Questions

**Abierta — ¿Se retira `content/` del stage `runtime` (y del `.dockerignore`)?** Con `content/`
obligatorio en el **contexto de build** (D1), la pregunta pierde la mitad de su alcance —copiarlo al
`builder` ya no se discute—, pero sigue en pie la otra mitad: que la **imagen final** lleve los
ficheros. Como la capa del disco gana, quitarlos cambiaría qué bytes se sirven en producción
(pasarían a ser los del binario embebido).

- **Recomendación: conservarlos.** El comportamiento de la imagen no cambia, sin superficie nueva, y
  la prueba de paridad del change anterior (D5 de `embedded-error-pages`) garantiza que las dos copias
  dicen lo mismo, de modo que la redundancia no puede convertirse en divergencia.
- **Alternativa: quitarlos del `Dockerfile` y del `.dockerignore`.** Una sola fuente de verdad —lo
  que sirve la imagen es lo que compila el binario— y unos kilobytes menos de imagen. Coste: se toca
  el empaquetado y hay que reverificar la imagen entera en un change que no necesita nada de eso.
- Resolverla **no** cambia ni los specs de este change ni su tabla de tareas: es un `COPY` menos. Por
  eso se deja abierta aquí y no bloquea el trabajo, ahora con el alcance estrecho que D3 describe.
