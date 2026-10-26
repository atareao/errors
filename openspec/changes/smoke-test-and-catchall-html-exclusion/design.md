# Design: smoke-test-and-catchall-html-exclusion

## El hecho que abre el change

La `v0.8.0` se publicó al segundo intento. El primer fallo **con la etiqueta ya empujada**, en el job
`imagen` de `release.yml`, con `GET /css/error.css devolvió 404; se esperaba 200`. Ninguna de las tres
puertas requeridas del PR lo vio: `fmt, clippy y tests`, `cargo-deny` e `imagen` pasaron en verde, y
la tercera **solo construye** la imagen. Y no salvó nada que la aserción fuese una sola línea: la
lección del incidente es que la aserción vivía en **un solo sitio** y el cambio del contrato no pasó
por ahí.

De ahí salen las tres mitades de este change: **una puerta que no miraba** (`/css/error.css`), **una
promesa que nadie exigía** (el leak del respaldo) y **un código que ninguna puerta nombraba** (el
`502`, el caso por el que existe el proyecto).

## D1 — Un script, y no un job nuevo ni una receta

Tres formas posibles de que el CI pruebe la imagen:

1. **Copiar las aserciones a `ci.yml`.** Es lo barato y es lo que hay que **no** hacer: crea dos
   copias de la misma lista, y dos copias divergen. La `v0.8.0` es la prueba de que divergen.
2. **Una receta del `.justfile`.** Tentador —el repositorio ya tiene `build`, `run`, `dev`— pero el
   CI tendría que instalar `just`, que es una dependencia nueva en el runner. El `.justfile` se queda
   como lo que es: la comodidad de esta máquina.
3. **Un script versionado** que los dos workflows invocan como `bash .github/scripts/smoke.sh`. Sin
   dependencias, sin nada que instalar, y con una sola copia de la lista. **Es la elegida.**

La receta `smoke` del `.justfile` envuelve el script para esta máquina; no lo sustituye.

## D2 — El motor y la imagen vienen del entorno

El script **no** hardcodea el binario del motor. Esta máquina no tiene `docker` y sí `podman`, y esa
diferencia ya está parametrizada en el `.justfile` (`engine`, `build_format`). El script la respeta:
`ENGINE` (por defecto `docker`) e `IMAGE` (obligatoria) por entorno, más un `PORT` opcional para el
puerto de loopback.

El precio, escrito: el script es un punto de entrada con contrato propio, y quien lo corra a mano
tiene que saber que `IMAGE` es obligatoria. No se adivina la imagen a propósito —adivinar la etiqueta
es cómo un smoke test acaba probando la imagen anterior a la que acabas de construir—.

## D3 — Qué comprueba el smoke test, y qué no puede

Comprueba **el contrato del servicio arrancado**, que es lo que la imagen promete:

- el estado de `GET /404`, `/503`, `/502`, `/404.json`, `/healthz` y `/metrics`;
- las cabeceras `X-Robots-Tag` en el `404` y `Retry-After: 120` en el `503`, y la **ausencia** de
  `Retry-After` en el `502` —solo `429` y `503` la llevan, y el `502` es el vecino que lo demuestra por
  contraste—;
- el **cuerpo** del `/404` con `Accept: text/html`, que lleva su `<style>` y **no** enlaza ninguna
  hoja —con el `Accept` explícito a propósito, porque sin él `curl` cae en texto plano por defecto y
  la comprobación mediría el formato equivocado—;
- `/minimal.html` a `200` crudo y `/default.html` a `404`, que son las dos caras de la garantía 4;
- el `USER` con el que corre el contenedor y su `HEALTHCHECK`.

**No** comprueba el contrato con Traefik: el leak del respaldo y la exclusión de `/metrics` necesitan
dos contenedores y un proxy delante, y eso vive en `examples/traefik/check.sh`, que se corre a mano y
**no** corre en el CI. Es un límite declarado, no una promesa: el CI vigila el servicio, el ejemplo
vigila la integración.

## D4 — El `HEALTHCHECK`, y los dos formatos de imagen

`podman` construye en formato **OCI** por defecto, y en OCI el `HEALTHCHECK` del `Dockerfile`
**no existe** en la config de la imagen: la instrucción se descarta en silencio. La receta del
`.justfile` construye con `--format docker` justo por eso, y así el healthcheck se pudo ejercitar en
la `v0.8.0` con podman. El CI usa `docker build` nativo, donde el healthcheck es nativo.

Decisión: el script comprueba el `HEALTHCHECK` **cuando la imagen lo declara**, y cuando no lo declara
lo dice en voz alta sin contarlo como fallo. La alternativa —exigirlo siempre— pondría en rojo a quien
construya con podman sin `--format docker`, que es un hecho sobre **su** construcción, no sobre el
servicio. El límite queda escrito en el requisito.

## D5 — La guarda de los workflows: qué puede demostrar una prueba textual

El corazón de la primera mitad no es «añadir el smoke test al CI» sino que **no haya dos listas**.
Eso es una propiedad sobre ficheros versionados, igual que la del respaldo, así que se guarda como
las de la casa: una prueba que lee `.github/workflows/*.yml` y falla si:

- algún workflow define o llama a los ayudantes de aserción del smoke test (`comprobar(`, `cabecera(`,
  `fallo `) fuera del script, o lleva su propio bucle de arranque contra el contenedor;
- **alguno de los dos** workflows no invoca `.github/scripts/smoke.sh`.

Un `curl` suelto **no** es incumplimiento: el workflow del release usa `curl` contra la API de GHCR
para comprobar la visibilidad del paquete, y eso no es una aserción sobre el servicio. La lista es
**cerrada** y se declara, para no confundir la higiene con la propiedad.

La guarda hereda de `traefik_catchall.rs` el criterio de leer el **valor** de la regla como lo lee
YAML, y la trampa medida más abajo (D6) añade un paso: un valor **entrecomillado** se lee con la barra
desdoblada (`\\.` → `\.`), porque es lo que hace el parser; uno plano, no. Sin ese paso, quien
escriba la regla entre comillas —una forma legítima— se llevaría un rojo que dice que falta una
subcadena que está delante de sus ojos. Y los dos límites, escritos: no es un parser de YAML —quita el
comentario de cola, las comillas y desdobla la barra del entrecomillado, y nada más—, así que **no**
valida que el entrecomillado sea YAML válido; de esa trampa avisa el README del ejemplo y la caza un
despliegue.

Lo que la guarda **no puede** demostrar, escrito en lugar de escondido: que el script compruebe
*todas* las aserciones del contrato. Eso no lo mide ninguna prueba textual —lo mide la revisión, y lo
mide correr el script contra una imagen de verdad—. La guarda protege contra la divergencia, que es
el fallo que ocurrió.

## D6 — La conjunción del leak: qué se excluye, y por qué los dos alias se quedan

La regla queda así, y es la misma en los cuatro sitios:

```
traefik.http.routers.errserve-catchall.rule: HostRegexp(`.+`) && !Path(`/metrics`) && (!PathRegexp(`\.html?$`) || Path(`/default.html`) || Path(`/default.htm`))
```

Tres decisiones dentro de esa línea:

1. **El matcher es `PathRegexp`, y hay que medirlo.** Este repositorio ya se quemó con una sintaxis de
   Traefik v2 —``HostRegexp(`{host:.+}`)``, que no casa con nada y deja el router `enabled` sirviendo
   `404` de Traefik— y por eso `examples/traefik/README.md` lleva su sección «La trampa de
   `HostRegexp`». Nadie ha medido `PathRegexp` aquí: la fase 0 lo mide contra Traefik **v3.6** (la
   versión que fija el ejemplo) antes de escribirlo en ningún sitio, y si no casa con nada, el
   matcher cambia y esta decisión se reescribe.
2. **Se cubren `.html` y `.htm`.** La regla recomendada en el README del ejemplo excluía solo `.html`
   y dejaba `/default.htm` fuera de su excepción, lo que haría de los dos alias un caso asimétrico:
   uno alcanzable por el respaldo y el otro no. Con `\.html?$` y los **dos** `Path`, los dos alias se
   comportan igual. El precio de la regla larga se paga a gusto por la simetría.
3. **Los alias se quedan fuera de la exclusión, a propósito.** `/default.html` y `/default.htm` **no**
   son un leak: su path se clasifica como el código por defecto, el servicio lo intercepta y responde
   el código resuelto, **nunca `200`**. Excluirlos quitaría la única página de tema que no filtra y
   rompería un rasgo documentado. La alternativa —bloquear `*.html` entero, sin excepciones— es más
   corta y también cierra el leak, pero deja los dos alias solo para el acceso directo; se descarta
   porque la conjunción ya está escrita y revisada en prosa y el coste de mantener la excepción es una
   matcher más.

El precio, escrito y **medido** en el banco de la fase 0: la conjunción bloquea **también**
`GET /404.html` y `GET /503.html` por el respaldo, y los contesta Traefik con su `404 page not found`.
No rompe el flujo real, porque el middleware `errors` pide `/404`, **sin** extensión
(`errors.query: /{status}`); y en el Host del respaldo `/`, `/503`, `/404` y `/abc` siguen recibiendo
la página de su código con sus cabeceras.

Límite declarado y **medido**: `PathRegexp` es sensible a mayúsculas, así que `GET /MINIMAL.HTML` no
casa con la exclusión y llega al servicio. No es un leak, y eso no es una suposición: medido, el
servicio responde **su página de error** —con su `X-Robots-Tag`—, porque el sistema de ficheros de la
imagen distingue mayúsculas, `ServeDir` no encuentra el fichero y contesta `404`, que la garantía 1
sustituye. Es una dependencia del sistema de ficheros, y queda escrita como tal.

**Y una trampa nueva, medida, de la misma familia que la de `HostRegexp`.** La conjunción lleva una
barra invertida (`\.html?$`), y en un escalar YAML **entrecomillado** `\.` no es un escape válido:

| así escrito | qué pasa |
|---|---|
| ``rule: "… PathRegexp(`\.html?$`) …"`` | `ERR Cannot start the provider *file.Provider error="yaml: line 4: found unknown escape character"` — **el provider entero no carga** |
| ``rule: "… PathRegexp(`\\.html?$`) …"`` | carga, y Traefik lee `\.html?$` |
| ``rule: … PathRegexp(`\.html?$`) …`` (escalar plano) | carga, y Traefik lee `\.html?$` |

La primera fila es peor de lo que parece: el contenedor **sigue arriba**, la API responde y no hay **ni
un router** —el provider no se cargó—. Falla en silencio, con el servicio aparentemente vivo. Por eso
los **cuatro** sitios del repositorio escriben la regla como **escalar plano** —los tres labels ya lo
eran, y el fragmento de *provider de fichero* del README del ejemplo deja de estar entrecomillado—, y
por eso el README del ejemplo gana esta fila en su sección de trampas: entrecomillar la regla obliga a
doblar la barra.

Y el `$` con el que termina la regex **no** lo toca la interpolación de compose: medido, el label que
recibe el motor es exactamente la regla pretendida (`podman inspect` sobre el contenedor que
`podman-compose up` crea). El `$$` que se ve en la salida de `docker compose config` es el escape de
**salida**, no del valor: la ida y vuelta lo deja igual.

## D7 — Lo que la prueba de los ficheros puede guardar, y lo que no

La prueba de `tests/traefik_catchall.rs` ya sabe descubrir un respaldo por su nombre (`catchall`, en
forma de label o de *provider de fichero*), quedarse con el **valor** de la regla —ignorando un `#` de
cola, que en YAML abre un comentario— y exigir sus conjunciones. Este change **no** inventa un método
nuevo: añade la conjunción del leak a la lista que ya se exige, y añade el caso que la echa en falta
(un respaldo con `/metrics` y sin ella incumple) más el caso simétrico del comentario de cola, porque
la conjunción del leak es más larga y lleva `#` a mano.

Lo que la prueba **no** puede guardar: que la regla **case** con las peticiones que debe bloquear. Eso
depende del motor de Traefik y lo mide `check.sh` con el ejemplo levantado. Está declarado en el
requisito y en D6.

## D8 — El `502`: dónde se puede medir el upstream caído

El `502` ya está en el contrato del servicio y la capacidad `error-page` ya exige que los códigos que un
proxy emite tengan página. Lo que este change añade es una puerta, y son dos sitios distintos porque el
escenario tiene dos partes:

1. **El código servido**, en el smoke test. `GET /502` contra la imagen arrancada: `502`, con su
   `X-Robots-Tag` y **sin** `Retry-After`. Es la aserción negativa que hoy solo vive en una prueba
   unitaria, y aquí se mide en la imagen de verdad.
2. **El camino de extremo a extremo**, en `examples/traefik/check.sh`. El smoke test no tiene proxy
   delante —no puede: son un proxy y dos contenedores, y el CI no levanta contenedores—. El ejemplo sí:
   un upstream que rechaza la conexión, Traefik que emite `502`, el middleware `errors` que pide `/502`
   y nuestra página en la respuesta del cliente. Una prueba guarda que ese caso sigue escrito en el
   script; que **corra** no lo puede demostrar (D5, el mismo límite).

**Cómo se construye el caso, y por qué así.** Medido en el banco de la fase 0, con Traefik v3.6:

| situación | código que emite Traefik |
|---|---|
| el servicio tiene un servidor declarado y la conexión se **rechaza** | `502` |
| el servicio **no tiene servidores** disponibles (contenedor parado, `servers: []`) | `503` |

Así que el caso **no** puede ser «paro el contenedor del backend»: eso mediría `503`, no `502`, y
pasaría en verde midiendo otro código. El caso apunta a un puerto donde nada escucha —un servidor
declarado que rechaza—, que es lo que un upstream caído mira desde Traefik.

## Preguntas abiertas

- **¿El `smoke` del `.justfile` debe construir la imagen o exigirla ya construida?** La decisión de
  arranque es que la construye (`just smoke` hace `build` y luego el script), pero eso hace de la
  receta un atajo cómodo y no una puerta: la puerta es el script. Se resuelve al implementarlo.
- **¿Se publica?** El binario, las páginas y la imagen no cambian; lo que cambia son las puertas y las
  reglas que el repositorio entrega. La convención de la casa es publicar cada change (`0.8.1`), pero
  la decisión es del usuario y la fase 5 espera su OK.
- **¿Qué pasa con `examples/traefik/README.md` y su no-goal?** Deja de ser un no-goal y pasa a ser un
  hecho entregado y guardado por una prueba. La sección se reescribe, no se borra: el hecho del leak
  sigue siendo cierto y sigue siendo la razón de la conjunción.
- **Un code span mal formado en el requisito de `/metrics`, de antes.** El requisito «El respaldo no
  publica el endpoint de servicio `/metrics`», ya fusionado en `openspec/specs/http-server/spec.md`,
  escribe la regla dentro de un *code span* de **un** backtick con backticks dentro, así que el
  markdown se cierra en el primero y la regla se renderiza partida: quien la copie de ahí, la copia
  mal. Es deuda **ajena a este change** —la escribió `catchall-metrics-exclusion`— y no se arregla
  aquí, porque un `MODIFIED` solo para eso obligaría a reproducir el requisito entero y haría parecer
  que este change lo cambia. El requisito nuevo de este change escribe la suya con **doble** backtick.
  No afecta a ninguna guarda: las pruebas leen el **valor de la regla** en los ficheros desplegables,
  no el markdown del spec.

## Lo medido en la fase 0, y cómo

Banco: `/tmp/opencode/phase0`, **fuera del repositorio** —esta imagen (`v0.8.0`) y **Traefik v3.6.25**,
la versión que fija el ejemplo, en una red de podman con *file provider*—.

- **`PathRegexp` existe, casa y discrimina**: con la conjunción el router sale `enabled`, la API publica
  el valor exacto de la regla, y las **seis** páginas de tema dejan de llegar al servicio (Traefik
  responde `404 page not found`, cero marcadores). El **control** con la regla vieja da `200` con los
  marcadores a la vista: el leak era real y el instrumento lo distingue.
- **Los dos alias** siguen dando **nuestro** `404` con `X-Robots-Tag`, nunca `200`.
- **El respaldo no se rompe**: `/`, `/503`, `/404` y `/abc` en un Host desconocido siguen recibiendo la
  página de su código. `/metrics` sigue excluido.
- **El precio**, confirmado: `/404.html` y `/503.html` → `404` de Traefik.
- **El límite de mayúsculas**, confirmado: `/MINIMAL.HTML` llega al servicio y recibe **su página de
  error**, no un fichero crudo.
- **El `502` y el `503`**, separados por el hecho que los distingue (la tabla de D8).
- **El contrato del servicio**, medido directo al contenedor y sin proxy: `/502` → `502` con
  `X-Robots-Tag` y **sin** `Retry-After`; `/503` y `/429` → `Retry-After: 120`; `/404` y `/500` → sin
  ella; `/healthz` y `/metrics` → `200` sin `X-Robots-Tag`; `/404` con `Accept: text/html` →
  `text/html` con `<style` y sin `rel="stylesheet"` ni marcadores crudos. Es la lista de D3, ejercitada
  a mano antes de escribirla.
- **El escapado de la barra y el `$` de compose** (la tabla de D6).

## Lo que sigue sin medirse

- Que la conjunción se comporte igual **desde los labels del ejemplo y con `podman-compose`**: el banco
  usó *file provider*. Lo mide la tarea 3.2, con el ejemplo levantado.
- El coste de arrancar un contenedor en cada propuesta de cambio: la puerta `imagen` del CI pasa de
  construir a construir y arrancar. Se medirá al implementarlo y se escribirá el número.
- Que la guarda de los workflows demuestre que el script lo comprueba **todo**: no lo puede demostrar
  ninguna prueba textual (D5).
