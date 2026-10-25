# Design: publish-crate-on-tag

## Context

Ver `proposal.md` — Why. Lo que condiciona el diseño:

- **El release publica la imagen y crea el Release, y para ahí.** `release.yml` es un solo job,
  `imagen` (`name: construir, probar y publicar`), y sus pasos van de construir a probar con docker
  real, entrar en GHCR, publicar y crear el Release. **Ninguno menciona `cargo publish`** —medido—,
  que es la causa directa de que crates.io siga en `0.3.0`.
- **Publicar en crates.io es irreversible.** Una versión publicada se puede *yank* —ocultar— pero no
  reemplazar ni borrar. Por eso la operación que no se puede deshacer va **después** de las que sí.
- **`cargo publish` publica lo que diga `Cargo.toml`, no lo que diga el tag.** El número de versión
  del paquete sale del manifiesto; la etiqueta es una convención del repositorio. Sin comprobación,
  los dos pueden discrepar sin que nadie se entere.
- **El workflow está pensado para re-ejecutarse.** El `workflow_dispatch` con input `tag` republica
  una etiqueta ya existente, y el paso del Release ya reutiliza el que ya está (`gh release view …`).
  Un `cargo publish` a secas rompería esa propiedad con un rojo falso.
- **La API de crates.io responde `403` a una petición sin `User-Agent`** —medido hoy al consultarla—,
  así que cualquier comprobación por HTTP tiene que mandar uno o dará un resultado engañoso.
- **`ci.yml` no verifica que el paquete sea publicable.** Tiene tres jobs —`puertas`
  (`fmt, clippy y tests`), `dependencias` (`cargo-deny`) e `imagen`—, los tres requeridos en `main` y
  en `development`, y ninguno empaqueta el crate.
- **El `exclude` de `Cargo.toml`** es `["/openspec", "/.github", "/AGENTS.md", "/cliff.toml",
  "/deny.toml", "/.justfile"]`, así que `/.github` no entra en el paquete: cablear el workflow no
  cambia el artefacto. `content/` **sí** entra, y es lo que hace que el binario instalado sirva las
  páginas.

## Goals / Non-Goals

**Goals**

- Que un tag `v*` publique **la imagen y el crate**, con la versión comprobada contra la etiqueta.
- Que lo irreversible ocurra **solo después** de que lo demás haya pasado, y **nunca** antes.
- Que republicar una etiqueta no rompa por una versión ya publicada.
- Que un paquete que no se puede publicar se descubra **en el PR**, no al publicar.

**Non-Goals**

- No se cambia la imagen, ni su smoke test, ni el `Dockerfile`.
- No se toca el código, ni las pruebas, ni el manifiesto, ni el `exclude`.
- No se publica nada en este change, ni se versiona el token en ningún sitio.

## Decisions

### D1 — Por qué un job aparte, y con `needs: imagen`

`cargo publish` es **irreversible**; construir y probar la imagen, no. Si el crate se publicara en
**paralelo** a la imagen y la imagen fallara, quedaría una versión publicada de la que no hay
artefacto: el peor estado posible, porque el crate **no se puede retirar** —solo *yank*, que oculta
pero deja la versión ocupada—. Con `needs: imagen`, el crate se publica **cuando ya hay una imagen
probada y publicada**, de modo que lo que no se puede deshacer va último y solo sobre terreno firme.

Va en **job aparte** —y no como un paso más del job `imagen`— por dos razones. La primera es de
responsabilidad: un job, una cosa que publicar; meter el crate en el job que publica la imagen
mezclaría dos artefactos con ciclos de vida distintos en un mismo sitio. La segunda es la herramienta:
el job `imagen` es de **docker**, no tiene **toolchain de Rust**, y añadir el crate ahí obligaría a
instalar Rust en un job que no lo necesita —solo para invocar `cargo publish`—. Un job `crate` con su
propio `rustup toolchain install stable` reintroduce la herramienta donde hace falta, y el `needs:
imagen` es lo que garantiza el orden.

### D2 — Por qué se comprueba la versión contra la etiqueta

`cargo publish` publica **lo que diga `Cargo.toml`**, no lo que diga el tag. Sin la comprobación,
etiquetar `v0.4.1` con `Cargo.toml` en `0.4.0` daría dos desenlaces, los dos malos:

- **Publicar la `0.4.0` otra vez.** Si la `0.4.0` ya está publicada, `cargo publish` falla —porque una
  versión no se puede reemplazar—, pero con un error que **no explica la causa real**: habla de
  duplicado, no de que la etiqueta y el manifiesto no cuadran. Quien lea ese rojo buscará el problema
  donde no está.
- **Publicar una versión que no es la que se anuncia.** Si la `0.4.0` no estuviera publicada, subiría
  ese número mientras la etiqueta dice `v0.4.1`: un artefacto cuyo nombre y cuyo contenido discrepan,
  y ya sin vuelta atrás.

La comprobación —comparar la versión de `Cargo.toml` con la de la etiqueta— convierte los dos casos en
**un error claro y temprano**, que nombra el desajuste. Es la clase de error que en un artefacto
irreversible hay que cazar **antes**, no después.

### D3 — Por qué el paso es idempotente

El workflow ya está pensado para re-ejecutarse sobre una etiqueta publicada: el `workflow_dispatch`
con input `tag` existe para eso, y el paso del Release ya reutiliza el existente si lo encuentra. Un
`cargo publish` a secas rompería esa propiedad: la segunda ejecución fallaría con «crate version
already uploaded», que es un **rojo falso** —la versión está publicada, que es justo lo que se quería—.
Así que el paso **consulta el registro** y, si la versión ya está, avisa y **continúa** en verde.

Ojo con el detalle medido: la API de crates.io responde **`403` sin `User-Agent`**, así que la consulta
tiene que mandar uno. Sin eso la comprobación daría un falso «no está publicado» —el `403` parece «no
encontrado» si no se mira— y el `cargo publish` posterior fallaría por duplicada, que es exactamente lo
que esta decisión existe para evitar.

### D4 — Por qué la puerta del paquete va en el CI

Es la misma lección de `fix-image-build`, ahora sobre el crate: hasta hoy **el paquete publicado solo
se verificaba en el momento de publicarlo**, y en parte **a mano** —una medición de `cargo package
--list` en una sesión—. `cargo publish --dry-run --locked` empaqueta, compila el paquete y valida sus
metadatos **sin subir nada**, y cabe en el job `puertas`, que **ya tiene toolchain y caché**: no
necesita un job nuevo ni una instalación de Rust, solo un paso más. Con eso, un paquete que no se puede
publicar —un `content/` que no viaja, un `Cargo.toml` inválido, una dependencia que falta— se ve **en
el PR**, no la primera vez que se publica una etiqueta. La puerta **no** necesita un check requerido
nuevo: va dentro de `puertas`, que ya lo es.

### D5 — Por qué la comprobación de la versión va la primera, y en un job propio

La comprobación nació en el **último** job —el que publica el crate—, y eso dejaba el desajuste
demasiado tarde. Medido con `Cargo.toml` en `0.4.0` y etiqueta `v0.4.1`: la imagen se construía y se
publicaba en GHCR etiquetada `0.4.1` pero desde el árbol de `0.4.0`, `latest` se movía, **y el
Release `v0.4.1` se creaba**, todo antes de que nadie comprobara nada; solo entonces fallaba el
crate. El desenlace era el peor de los posibles: un release a medias donde lo único que faltaba era
justo lo irreversible, y un GitHub Release que anunciaba una versión que no existía en el registro.

Ahora la comprobación vive en un job `version`, **el primero**, del que dependen `imagen` y `crate`:
resuelve la etiqueta, la versión y la imagen **una sola vez** y comprueba la versión **antes** de
que cualquier otro job haga nada. Un desajuste para el release entero: **no se publica nada en
absoluto** —ni imagen, ni Release, ni crate—. La razón de fondo es que la resolución estaba
**duplicada** entre `imagen` y `crate` —la misma lógica copiada en dos jobs—, y dos copias divergen:
basta editar una y no la otra para que cada job resuelva una etiqueta distinta, sin que nada lo
delate. El `needs` **no** protege de eso —ordena la ejecución, no comparte el dato—, así que la
única forma de que la comprobación sea una y valga para todo lo que se va a publicar es que **sea la
primera** y que su resultado sea el que los demás consumen. El `needs: imagen` de D1 sigue siendo la
razón de que el crate no se publique si la imagen falla, pero deja de ser el único orden: la
comprobación va antes que todo.

### D6 — Por qué ningún valor del evento entra en un `run:`

Los scripts interpolaban `${{ inputs.tag }}` y `${{ steps.v.outputs.version }}` directamente dentro
del cuerpo del `run:`. Eso no es un dato del script: `${{ … }}` lo expande la **plantilla del
workflow antes** de que bash parsee una sola línea, así que el valor se vuelve **código** del script
y la validación que venía a continuación —el filtro «solo forma de versión»— llegaba **tarde**:
cuando corría, la cadena ya estaba dentro del script, y el filtro solo decidía si se seguía
ejecutando, no si el valor era peligroso. Y su patrón, `^v[0-9]+(\.[0-9]+)*([-+].*)?$`, admitía
cualquier cosa **después** de un `-`: una etiqueta `v0.4.0-$(comando)` pasaba entera. Ese comando
correría en el runner con el `GITHUB_TOKEN` del workflow y, en el job `crate`, en el **mismo** job
que el paso que publica, así que podía llegar a interceptar `CARGO_REGISTRY_TOKEN`.

El arreglo tiene dos capas. La primera es de **transporte**: los valores entran por `env:`, que los
entrega al script como **variable de entorno** —dato, no código—, así que bash los ve como texto y
nunca los parsea. La segunda es el **contenido**: el filtro pasa a ser un *whitelist* de caracteres,
`^v[0-9]+\.[0-9]+\.[0-9]+([-+][0-9A-Za-z][0-9A-Za-z.-]*)?$`, que solo admite la forma de una versión
semántica y rechaza cualquier otra por su forma, **sin que ningún fragmento de la etiqueta llegue a
ejecutarse**. Con las dos, una etiqueta hostil corta la ejecución antes de publicar nada.

### D7 — Por qué el crate sale del árbol de la etiqueta, y por qué la comprobación no es una cadena

El `checkout` del job `crate` no fijaba `ref`, así que en un `workflow_dispatch` usaba el tip de la
**rama** (`main`). Con eso, publicar con el disparador manual sobre la etiqueta `vX.Y.Z` podía subir
a crates.io —de forma **irreversible**— el árbol de la rama bajo la versión de una etiqueta que
apunta a **otro** commit: el número anunciado y el contenido publicado describirían árboles
distintos, y ya sin vuelta atrás. Ahora el `ref` es el de la etiqueta, y un paso **afirma** que el
commit que se va a publicar es el mismo que comprobó el job `version`, de modo que la etiqueta de la
que sale el crate y la versión que se comprueba son la misma.

Y no es una promesa sin respaldo: `cargo package` mete dentro del `.crate` un `.cargo_vcs_info.json`
con el **sha1 del commit desde el que se empaquetó**, así que «de qué árbol salió» es comprobable
**a posteriori**, sin volver a mirar el workflow. Medido: un `.crate` que empaqueta este árbol
limpio registra exactamente su `HEAD`. Ese registro es justo lo que el paso de idempotencia de D8
usa para comparar el árbol publicado con el árbol actual.

Y el `ref` de los dos `checkout` va **cualificado a `refs/tags/…`** precisamente por esto: un `ref`
sin cualificar se resuelve prefiriendo una **rama** del mismo nombre a la etiqueta, así que si
coexistieran una rama `vX.Y.Z` y su etiqueta, los dos jobs resolverían lo mismo —la rama— y la
afirmación `HEAD == commit` compararía la rama consigo misma: pasaría sin cazar nada.

### D8 — Por qué el aviso de la idempotencia NO es un fallo

En la rama del `200` —esa versión ya está en crates.io— el paso descarga el `.crate` publicado, lee
su `.cargo_vcs_info.json` y compara su sha1 con el commit de este árbol; si no coinciden, deja un
`::warning::` y una línea en el resumen del paso, **no** un error. Va contra la intuición de «cuanto
más estricto, mejor», así que van escritas las dos razones por las que aquí la puerta blanda es la
correcta.

La primera es que **no hay nada que reparar**. En esa rama la publicación ya está hecha y es
irreversible: un fallo no arreglaría la versión publicada, que no se puede reemplazar ni borrar
—solo *yank*—, y sí bloquearía lo que queda del release: republicar la imagen y reutilizar el
Release, que es justo el camino que el workflow tiene por diseño (el `workflow_dispatch` con input
`tag` existe para republicar). Un rojo aquí convertiría en fallo la **republicación legítima**.

La segunda es **medida**: el `.crate` publicado de la `0.3.0` registra el commit `bdb76fa…`, que
**no** es el de su etiqueta `v0.3.0` (`d0032a2…`), porque esa versión se publicó **a mano** desde
otra rama. Con una puerta dura, la primera republicación legítima de la `0.3.0` daría rojo por un
motivo histórico que nadie puede arreglar. El aviso deja el hecho a la vista —el resumen del paso lo
dice— sin fingir que es un error.

## Risks / Trade-offs

- [El release tarda más] → el job `crate` instala el toolchain, empaqueta y compila el paquete antes de
  publicar: unos minutos más por release. Se acepta: es el precio de que lo irreversible ocurra sobre
  terreno firme, y el release es raro y deliberado.
- [El `dry-run` del CI añade tiempo a cada PR] → `cargo publish --dry-run` compila el paquete entero
  —una compilación *release* del crate, aparte de las de `puertas`—. Se acepta a cambio de que el
  empaquetado deje de ser el único camino sin puerta; el job `puertas` ya tiene caché de Cargo, que es
  donde se paga el grueso.
- [El paso de publicación depende de la red] → la consulta a crates.io para la idempotencia (D3) puede
  fallar por un corte de red, y daría un rojo que **no es del paquete**. Se acepta: se prefiere un rojo
  ruidoso y revisable a publicar a ciegas. Es el mismo trato que el paso del aviso de GHCR, que también
  habla por HTTP y no falla el release.
- [El release ahora tiene **tres** jobs] → el job `version` va primero, pero es **rápido**: resuelve
  la etiqueta, la versión y la imagen y comprueba el desajuste, y **no instala toolchain**, así que
  cuesta segundos, no minutos. El que sí cuesta es el job `crate`, que publica desde el **árbol de
  la etiqueta** y por eso **no** se beneficia de la caché de Cargo —que se ha quitado a propósito
  del job, para no meter una acción de tercero con tag mutable en el mismo job que maneja el token
  de publicación—: paga su compilación entera. Se acepta: el release es raro y deliberado.

## Migration Plan

1. Comprobar que el secreto `CARGO_REGISTRY_TOKEN` existe en el repositorio (a mano, en los ajustes
   de Actions) y crearlo si no: medido, ya estaba dado de alta el `2026-10-25T06:46:46Z`.
2. Añadir el paso del `dry-run` al job `puertas` de `ci.yml`, y verlo en verde en el PR.
3. Añadir a `release.yml` el job `version` —resolución única de etiqueta, versión e imagen más la
   comprobación de que la versión cuadra—, del que pasan a depender los otros dos, y después el job
   `crate` con `needs: [version, imagen]`, la idempotencia y el `cargo publish --locked` desde el
   árbol de la etiqueta.
4. Ejecutar el guion de tareas: la comprobación de versión en RED con los dos casos, y el `dry-run` en
   verde.
5. Publicar la `0.4.0` con el `workflow_dispatch` que ya existe, sobre la etiqueta ya publicada:
   comprobar que la imagen se republica sin daño, el Release se reutiliza y **el crate aparece en
   crates.io**.
6. Documentar (`AGENTS.md`, `CHANGELOG.md`) y archivar.

## Open Questions

- **¿El `workflow_dispatch` debería ser el camino normal y no la excepción?** Hoy el camino normal es
  el push de la etiqueta y el dispatch existe para republicar. Si republicar se volviera frecuente,
  tendría sentido invertirlo —el dispatch como forma canónica, con el push como atajo—, pero no es el
  caso hoy y el change no lo necesita para su motivo.
- **¿El token debería vivir en un *environment* de GitHub con revisores obligatorios?** Es lo
  correcto para una publicación irreversible: un *environment* con revisores exigiría que alguien
  aprobara el job del crate **antes** de que el token estuviera disponible, de modo que ni un push
  de etiqueta ni un runner comprometido pudieran publicar sin un acto humano. No se hace en este
  change porque convertiría **cada** release en un acto manual, y el release ya es deliberado y
  raro.
- **¿Y la cola de releases?** El `concurrency` del fichero usa la cola **por defecto**
  (`cancel-in-progress: false`), que solo mantiene **una** ejecución pendiente: un push de etiqueta
  que llegue mientras hay una en curso y otra esperando deja la esperando **cancelada y en gris**
  —sin imagen, sin crate y **sin rojo**—, un release a medias que nada señala. Se asume hoy y **no**
  se arregla aquí. La alternativa obvia —un grupo **por etiqueta** (`release-${{ github.ref_name }}`),
  que serializa sin descartar— **no** arregla el problema de fondo: con grupos por etiqueta, dos
  releases de etiquetas distintas correrían **a la vez**, y los dos mueven la misma `latest`, que
  es mutable; eso cambiaría un release perdido en silencio por dos publicaciones pisándose, que es
  peor. Por eso la pregunta sigue abierta y no se resuelve con una línea.
- **¿Y la asimetría entre el crate y la imagen?** El crate sale del árbol de la **etiqueta** (D7);
  la imagen, no: el job `imagen` sigue construyéndose desde la **rama**, como el workflow hace a
  propósito («construye la imagen desde la rama, no desde el árbol de la etiqueta; para una etiqueta
  ya publicada eso es lo que se quiere —republicar—», en `AGENTS.md`). En un `workflow_dispatch`
  sobre una etiqueta antigua, la imagen se reconstruye desde el tip de la rama —y **mueve
  `latest`**— mientras el crate sale del árbol de la etiqueta: los dos artefactos de la misma
  ejecución pueden venir de commits distintos. Se acepta aquí porque cambiarlo **cambia el
  comportamiento documentado de la imagen** y eso sería otro change. La pregunta es si esa
  asimetría debe resolverse algún día —la imagen también desde el árbol de la etiqueta, o dejar
  `latest` quieta en las republicaciones—.
