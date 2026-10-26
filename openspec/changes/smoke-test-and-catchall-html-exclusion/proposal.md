# Proposal: smoke-test-and-catchall-html-exclusion

## Why

Tres promesas que este repositorio hace y que hoy **no vigila ninguna puerta**, y las tres se han medido.

La primera costó un release: la etiqueta `v0.8.0` falló en el job `imagen` con `GET /css/error.css devolvió 404; se esperaba 200` —la aserción del contrato viejo— **antes** de publicar nada. El job `imagen` del CI **solo construye** la imagen; el único smoke test del servicio vive en `release.yml` y corre **solo al etiquetar**. Un cambio que altere el contrato HTTP pasa los tres checks requeridos en verde en su PR y no rompe hasta publicar.

La segunda es un leak cosmético declarado como no-goal en `themes-as-pages`: las seis páginas de tema que no son el alias se sirven **crudas** por el respaldo, con los marcadores a la vista, y la mitigación está escrita en prosa en un solo fichero, sin nada que la exija.

La tercera es el `502`: el código con el que un proxy responde cuando no puede hablar con la aplicación que tiene detrás, que es el escenario que justifica este proyecto —`error-pages` nombra su plantilla por defecto `app-down`—. El servicio le da **razón y descripción propias** (`src/resolve.rs`) y el spec ya exige que todo código del rango tenga su página, pero **ninguna puerta lo nombra**: no está en la lista del smoke test y `examples/traefik/` no tiene ni un caso con el upstream caído. Medido en un banco con Traefik v3.6: con un upstream que **rechaza** la conexión, Traefik emite `502` y el middleware `errors` lo convierte en la página del servicio, con su `X-Robots-Tag`.

## Los hechos, medidos

### 1. El smoke test que solo corre al etiquetar

`ci.yml` tiene un job `imagen` que hace `docker build --tag errserve:ci .` y nada más: **no arranca el contenedor**. Las aserciones sobre el servicio arrancado —estado, cabeceras, cuerpo, `USER` y `HEALTHCHECK` de la imagen— viven dentro del bloque «Probar la imagen con docker (smoke test)» de `release.yml`, que es el único sitio donde se ejercitan. Consecuencia medida en la `v0.8.0`: el PR de release pasó `fmt, clippy y tests`, `cargo-deny` e `imagen` en verde, y el fallo apareció **al etiquetar**, con la etiqueta ya empujada.

Y lo que se rompió no fue una aserción cualquiera: `release.yml` afirmaba `GET /css/error.css -> 200`, el asset que el propio release retira. La aserción vivía en **un solo sitio** y nadie la revisó al cambiar el contrato. La lección no es «añade el smoke test al CI» sino **«hay un solo smoke test»**: si el CI y el release llevasen cada uno su copia, la divergencia volvería.

### 2. El leak que la prosa no exige

`examples/traefik/README.md` tiene la sección «El leak de las páginas de tema»: explica que las seis páginas que no son el alias se sirven crudas y da la conjunción que lo mitiga, con su precio escrito —bloquea también `/404.html` y `/503.html` por el acceso directo, y no rompe el flujo real porque el middleware `errors` pide `/404` **sin** extensión (`errors.query: /{status}`)—. Y declara que «este repositorio **no** entrega un respaldo que la implemente (es un no-goal del change `themes-as-pages`)».

Los **cuatro** sitios donde el repositorio define un respaldo desplegable —los labels del ejemplo (`examples/traefik/compose.yml`), los de la plantilla de producción (`compose.yml`), el fragmento de *provider de fichero* (`examples/traefik/README.md`) y el bloque de labels del `README.md` raíz— llevan **solo** la exclusión de `/metrics`. Es el mismo caso que `catchall-metrics-exclusion` resolvió para `/metrics`: una recomendación en prosa con los desplegables que entrega el repositorio haciendo lo contrario. Aquel change no se conformó con documentarlo y añadió una prueba que lee los ficheros versionados; este es el mismo movimiento para el otro leak.

### 3. El `502`, el caso que justifica el proyecto y que ninguna puerta nombra

El servicio tiene el `502` en su contrato: `src/resolve.rs` le da razón y descripción propias, la lista de «códigos que un proxy emite» de `openspec/specs/error-page/spec.md` lo nombra, y una prueba unitaria recorre esa lista entera. Lo que **no** hay es ninguna puerta que lo ejercite: la lista de aserciones del smoke test no lo incluye, y `examples/traefik/` no tiene ni un caso con el upstream caído —`grep -rn 502 examples/` no devuelve nada—.

Es el escenario por el que existe este proyecto: la aplicación se cae, Traefik no puede hablar con ella y responde `502`; el middleware `errors` lo traduce a la página del servicio. Medido en un banco con Traefik v3.6 y esta imagen: con el upstream **rechazando** la conexión el cliente recibe `502` con la página del servicio y su `X-Robots-Tag`; con un servicio **sin servidores** el código es `503`. La diferencia decide cómo se construye el caso.

## What Changes

1. **Un solo smoke test, y en el CI.** Las aserciones sobre el servicio arrancado se extraen a un script versionado (`.github/scripts/smoke.sh`) que **llaman los dos workflows**: `ci.yml` en cada propuesta de cambio, después de construir la imagen, y `release.yml` antes de publicar. Las aserciones de `release.yml` se retiran: la aserción vive en un sitio, no en dos.
2. **El script sirve para los dos motores.** Toma el motor de contenedores y la referencia de la imagen del entorno —`docker` por defecto—, porque esta máquina de desarrollo no tiene docker y sí `podman`. Arranca el contenedor en loopback, espera a que `/healthz` responda y comprueba el contrato: los estados, las cabeceras, el cuerpo del HTML con `Accept: text/html`, el `USER` y el `HEALTHCHECK`.
3. **El respaldo no publica las páginas de tema crudas.** Los cuatro sitios desplegables llevan la conjunción completa y la prueba de `tests/traefik_catchall.rs` la exige junto a la de `/metrics`, con el mismo método y sus fixtures.
4. **La prosa se pone al día.** El README del ejemplo deja de decir que la mitigación es un no-goal sin implementar; el `README.md` raíz y el `AGENTS.md` explican las dos promesas y qué las vigila.
5. **El `502` entra en una puerta.** El smoke test gana `/502` en su lista, con la aserción **negativa** de que **no** lleva `Retry-After` (solo `429` y `503` lo llevan), y `examples/traefik/check.sh` gana el caso de extremo a extremo —un upstream que rechaza la conexión, Traefik que emite `502`, la página del servicio—, con una prueba que guarda que el caso sigue ahí.

## Capabilities

### New Capabilities

(ninguna)

### Modified Capabilities

- `http-server`: **un requisito modificado** —«La imagen se construye desde el árbol y trae las páginas dentro» gana la obligación de **probar** la imagen arrancada, no solo de construirla— y **tres requisitos nuevos**: «El smoke test del servicio es uno solo», «El respaldo no publica las páginas de tema crudas» y «El código con el que un proxy responde a un upstream caído se ejercita».

## Non-goals

- **No se toca `src/**` ni `content/**`.** Ni una línea del servicio ni de las siete páginas: lo que cambia es qué puertas lo vigilan y qué entrega el respaldo.
- **No se decide la copia de `content/` del stage de runtime del `Dockerfile`.** Sigue abierta; el smoke test comprueba lo que sirva la imagen, sea la copia del disco o la embebida.
- **No se mide el coste por petición ni el tamaño de la imagen.** Son las otras dos preguntas abiertas y no entran aquí.
- **No se emite `Content-Security-Policy`.** Las páginas siguen llevando su `<style>` y su `<script>` en línea: el límite sigue documentado y no se toca.
- **No se cambia el matcher del respaldo ni su prioridad.** La regla es la misma con una conjunción más; el `HostRegexp(.+)` se queda como está.
- **No se añade un job nuevo al CI.** El smoke test entra en el job `imagen` que ya construye la imagen.
- **No se levanta un proxy en el CI.** El `502` de extremo a extremo —y el leak del respaldo— siguen midiéndose en `examples/traefik/check.sh`, que necesita dos contenedores y un proxy: el CI vigila el servicio, el ejemplo vigila la integración.

## Impact

- `.github/scripts/smoke.sh` — **nuevo**; las aserciones, una sola vez.
- `.github/workflows/ci.yml` — el job `imagen` pasa de construir a construir **y probar**.
- `.github/workflows/release.yml` — pierde sus aserciones y llama al script; el comentario de cabecera que afirma que este workflow es «el único sitio donde el healthcheck y el `USER` se ejercitan contra docker de verdad» deja de ser cierto y se reescribe.
- `tests/traefik_catchall.rs` — exige la conjunción del leak, con su caso que la echa en falta.
- `tests/puertas.rs` — **nuevo**: guarda que los workflows no vuelvan a llevar aserciones propias, que los dos invoquen el script y que `check.sh` no pierda el caso del `502`.
- `examples/traefik/compose.yml`, `compose.yml`, `examples/traefik/README.md`, `README.md` — la conjunción y su prosa.
- `examples/traefik/check.sh` — el caso de extremo a extremo del leak y el del upstream caído (`502`); los dos necesitan contenedores y no corren en el CI.
- `.justfile` — una receta `smoke` para correrlo en esta máquina con podman.
- `AGENTS.md` — el quirk del smoke test y el del respaldo.
- `openspec/specs/http-server/spec.md` — vía la delta de este change.
- `src/**`, `content/**`, `Dockerfile`, `Cargo.toml`, `Cargo.lock` — **sin cambios**.

## Release

Nada del servicio publicado cambia: el binario, las páginas y la imagen son idénticos. Lo que cambia es qué puertas lo vigilan y las reglas que el repositorio **entrega** para el respaldo. Aun así se publica —parche, `0.8.1`—, porque la convención de este repositorio es que cada change deja su entrada en el `CHANGELOG` y una etiqueta; la decisión de la versión es del usuario, y la fase 5 de `tasks.md` espera su OK explícito.
