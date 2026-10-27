# Integración con Traefik

Ejemplo **ejecutable**: Traefik v3 + este servicio + un backend que devuelve 4xx/5xx.

```sh
docker compose up -d          # o: podman-compose up -d
./check.sh                    # comprueba los bloques de abajo
docker compose down
```

La imagen **se descarga**, no se construye: el compose usa `ghcr.io/atareao/errserve`. Con
`ERRSERVE_IMAGE` puedes apuntar a otra etiqueta o a una imagen local que hayas construido con
`just build`.

Si el `pull` responde `denied`, el paquete de GHCR sigue privado (nace así aunque el repositorio sea
público); se cambia una sola vez a mano, en los ajustes del paquete.

En **podman rootless** hay que apuntar el socket del motor (sin esto, Traefik no ve los
contenedores y todo responde su propio 404):

```sh
CONTAINER_SOCKET=$XDG_RUNTIME_DIR/podman/podman.sock podman-compose up -d
```

## Quién decide el código HTTP

Es el matiz que hay que entender antes de confiar en el servicio. **Depende de cómo esté montado:**

| Caso | Quién pone el estado | Nuestro código |
|---|---|---|
| Middleware `errors` delante de una aplicación | **Traefik**, que conserva el código del upstream | se ignora |
| Acceso **directo**: router *catch-all*, *default backend* | **nosotros** | es el que llega al cliente |

No es una deducción: está medido. El ejemplo incluye un router deliberadamente divergente
(`diverge.localtest.me`) cuyo middleware pide siempre `/404` aunque el backend devuelva `503`; el
cliente recibe **`503`**, o sea, gana Traefik.

El caso que justifica este proyecto es el segundo: en el *catch-all* no hay upstream cuyo código
preservar, así que `error-pages` devuelve `200` y nosotros el código real. El **cuerpo** y las
cabeceras (`Retry-After`, `X-Robots-Tag`, `Content-Type`, negociación por `Accept`) sí son nuestros
en los dos casos.

## Lo que comprueba `check.sh`

**Ocho bloques**: los cuatro estados de la tabla, las cabeceras de la página servida del `503`, la
página autocontenida con el tema por petición, que el respaldo no publique el endpoint de servicio
`/metrics` y, de este change, cuatro más: que el respaldo no sirva las **páginas de tema crudas**
—con la barra final y la barra codificada incluidas—, que los **dos alias** del código por defecto
sigan sirviéndose —nunca el fichero crudo—, que el respaldo **no se rompa** (`/` y `/503` en el mismo
Host desconocido) y el **`502`** de extremo a extremo, con un upstream que rechaza la conexión.

| Petición | Esperado | Qué demuestra |
|---|---|---|
| `app.localtest.me/` | `404` | El backend devuelve 404; el estado lo conserva Traefik, el cuerpo es nuestro. |
| `app.localtest.me/boom` | `503` | Idem con un 503, más `Retry-After: 120` y sus cabeceras (`X-Robots-Tag`, `Content-Type`). |
| `nada.localtest.me/x` | `404` | **Catch-all sin upstream: el código lo decidimos nosotros.** Es la razón del proyecto. |
| `diverge.localtest.me/boom` | `503` | Backend 503 con el middleware pidiendo `/404`: manda Traefik. |
| `app.localtest.me/boom` con `Accept: text/html` | `503` | **El `503` llega con la página del tema —su `<style>` dentro— y sin ningún `<link>`**, y `X-Theme: terminal` cambia el cuerpo de esa petición. |
| `nada.localtest.me/metrics` | no `200` | El respaldo **no** publica el endpoint de servicio `/metrics`. |
| `nada.localtest.me/minimal.html` (y `/minimal.html/`, `/minimal.html%2F`, `/matrix.html`) | no `200` | El respaldo **no** sirve las páginas de tema crudas: el nombre del tema **es** la ruta de su página, y el fichero crudo lleva los marcadores a la vista. |
| `nada.localtest.me/default.html` (y `/default.htm`) | `404` | Los **dos alias** del código por defecto los intercepta el servicio: código resuelto con `X-Robots-Tag`, **nunca `200`** ni el fichero crudo. |
| `nada.localtest.me/` y `/503` | `404` y `503` | La conjunción **no se lleva por delante el respaldo**: en el mismo Host sigue sirviendo el código real (`/503` con su `Retry-After`). |
| `caido.localtest.me/x` | `502` | **El upstream rechaza la conexión**, Traefik emite `502` y el middleware `errors` lo traduce a la página del servicio: con `X-Robots-Tag`, **sin** `Retry-After` y con el `<style` de la página, no el error en texto del proxy. |

`check.sh` sale con estado 1 si algo no cuadra, así que sirve también en un pipeline.

### La página autocontenida y el tema, por petición

Sobre `app.localtest.me/boom` —el **mismo** `503` del backend, no un caso nuevo—, con tres ayudantes
que afirman y niegan sobre el **cuerpo**:

- `html_body app.localtest.me /boom '<style'` — el `503` del backend llega con la página del tema, que
  trae su `<style>` **dentro**: la página no enlaza ninguna hoja que sirva este servicio.
- `not_html_body app.localtest.me /boom 'rel="stylesheet"'` — y **no** lleva ningún
  `<link rel="stylesheet">`: el enlace a `/css/error.css` **ya no existe**, porque cada página lleva su
  `<style>` dentro. Es el arreglo de un bug **medido**: con el modelo anterior, en el host de la aplicación ese enlace lo
  contestaba el upstream —`404 text/plain`—, el middleware `errors` interceptaba ese `404` y lo volvía
  a pedir como página de error con un `Accept` que **no** negocia HTML, la cascada de formato caía al
  texto plano y el navegador rechazaba la hoja (*«MIME type ('text/plain') is not a supported
  stylesheet MIME type»*). La página salía **sin estilos**, con el `503` y el cuerpo correctos.
- `html_differs app.localtest.me /boom 'X-Theme: terminal'` — el tema se elige **por petición**: pide
  el mismo path dos veces, con y sin esa cabecera, y exige que los cuerpos **difieran**. Es lo que
  demuestra la selección de extremo a extremo, a través de Traefik; `terminal` es uno de los once
  temas que documenta el [`README.md`](../../README.md) raíz.

Los tres mandan `Accept: text/html` **a propósito**. Sin esa pista, un `curl` manda `*/*`, la cascada
de formato no elige HTML —cae al **texto plano** por defecto— y el bloque no probaría nada: no habría
página con su `<style>` que buscar en el cuerpo. Es también el motivo de que `html_differs` compare
**cuerpos** y no busque un color en el `<style>`: un color es un detalle que se puede retocar sin avisar, y
«el cuerpo cambia con la cabecera» es el contrato.

### El respaldo no publica el endpoint de servicio

El bloque de `/metrics` pide ese path en `nada.localtest.me` —un Host sin router propio, el caso del
*catch-all*— y exige que la respuesta **no** sea la del endpoint: ni el estado `200`, ni el
`Content-Type` `text/plain; version=0.0.4` que firma a `/metrics`, ni el contador
`errors_pages_total` en el cuerpo. La ausencia de `X-Robots-Tag` es solo una **guarda**, no un
discriminante —`/metrics` es un endpoint de servicio y nunca lleva esa cabecera, así que se cumple en
los dos estados—: está para que nadie «arregle» la fuga convirtiendo `/metrics` en una página de
error. Todo el montaje —y la medición— está explicado en «Operación y observabilidad» del
[`README.md`](../../README.md) raíz y en «Variante sin socket», abajo.

## La trampa de `HostRegexp`

El router de respaldo necesita una regla que case con cualquier host. **La sintaxis cambió en v3 y
la vieja no falla: simplemente no casa.**

```yaml
# MAL en Traefik v3 — sintaxis v2 (placeholder {nombre:regexp})
traefik.http.routers.errserve.rule: HostRegexp(`{host:.+}`)

# BIEN en Traefik v3 — regexp de Go, con la regla completa del respaldo
traefik.http.routers.errserve.rule: HostRegexp(`.+`) && !Path(`/metrics`)
```

Medido en v3.6: ``HostRegexp(`{host:.+}`)`` **no casa con nada** y ``HostRegexp(`.+`)`` casa. Lo
peligroso es cómo se rompe: el router aparece como `enabled` en el dashboard y Traefik responde **su
propio 404 en texto plano**, sin llegar nunca al servicio. Se parece a que todo funciona, y es
exactamente el fallo que este proyecto existe para corregir.

El bloque va de la **sintaxis**, y sólo de eso: el segundo arreglo —``&& !Path(`/metrics`)``— no tiene
nada que ver con v2/v3, es la exclusión del endpoint de servicio que el respaldo necesita (se explica
en «Variante sin socket», abajo, y en «Operación y observabilidad» del `README.md` raíz). La línea
«BIEN» la lleva puesta a propósito, aunque alargue el ejemplo: es la regla desplegable, y una línea
«BIEN» a medias es el vector por el que `/metrics` se filtró de verdad.

### La trampa del escalar entrecomillado

De la misma familia, y **medida**: la conjunción del leak lleva una barra invertida (`\.html?…`), y en
un escalar YAML **entrecomillado** `\.` no es un escape válido. Escrita
``rule: "… PathRegexp(`\.html?…`) …"``, el *provider* **entero** deja de cargar
—`yaml: found unknown escape character`—, y lo peor es cómo se rompe: el contenedor sigue arriba, la
API responde y no hay **ni un router**, en silencio. Entrecomillada, la barra va **doblada** (`\\.`);
como escalar **plano**, va **simple** —y así los dos `$` de la regla llegan a Traefik verbatim—. Los
cinco sitios de este repositorio la escriben **plana**, y por eso el fragmento de *provider de
fichero* de «Variante sin socket» no va entre comillas.

## El leak de las páginas de tema

Con el modelo actual, **un tema es una página** y el nombre del tema **es** la ruta de su fichero: las
siete (`default.html`, `minimal.html`, …) viven en la **raíz** de `CONTENT_DIR`. El servicio solo
intercepta **`/default.html`** (y `/default.htm`), que son el **alias del código por defecto**
(`DEFAULT_ERROR_PAGE`); las **otras seis** —`/minimal.html`, `/terminal.html`, `/paper.html`,
`/amber.html`, `/compact.html` y `/matrix.html`— se sirven como **assets crudos** si alguien las pide,
con sus marcadores `{{code}}`/`{{reason}}`/`{{description}}` a la vista.

Es un leak **cosmético**, y con las páginas en la raíz de `CONTENT_DIR` es **inevitable**: el servicio
no las oculta, así que quien lo tapa es el **proxy**. La conjunción que lo cierra va **dentro** de la
regla del respaldo, y este repositorio **la entrega** —en los tres labels y en las dos definiciones
de este README—: `tests/traefik_catchall.rs` la guarda en los cinco sitios, porque comprobarlo de
extremo a extremo necesita un motor de contenedores y no corre en el CI. Cubre las **tres** formas de
alcanzar la página cruda, las tres **medidas** abiertas contra Traefik v3.6.25 con una exclusión
anclada al fin de cadena —que dejaba **17** formas sirviendo el fichero con `200`—: el path terminado
en `.html`/`.htm`, el mismo con **barra final** o con un segmento detrás, y el mismo con la barra
**percent-encoded** (`%2F`, en las dos cajas). Los **dos alias** `/default.html` y `/default.htm`
quedan **fuera** de la exclusión **a propósito**: su path se clasifica como el código por defecto, el
servicio lo intercepta y responde el código resuelto —**nunca `200`**—, así que excluirlos quitaría la
única página de tema que no filtra:

```yaml
traefik.http.routers.errserve-catchall.rule: HostRegexp(`.+`) && !Path(`/metrics`) && (!PathRegexp(`\.html?((%2[fF])|/|$)`) || Path(`/default.html`) || Path(`/default.htm`))
```

El precio, escrito y **medido**: la conjunción manda a la `404` de Traefik —su `404 page not found`,
sin pasar por el servicio— todo path cuyo `.html`/`.htm` vaya seguido de fin de cadena, de `/` o de
`%2F`: `GET /404.html`, `GET /503.html`, `GET /sub/x.html`, `GET /x.html/y` y `GET /default.html%2F`.
No rompe el flujo real —el middleware `errors` pide `/404`, **sin** extensión
(`errors.query: /{status}`)—, pero el acceso humano a `/404.html` deja de servirse. Las páginas de
tema del servicio se deciden **dentro** de este repositorio; el bloqueo, también.

Y un límite declarado, que conviene saber porque lo guarda una prueba **textual**: `PathRegexp`
distingue mayúsculas, así que `GET /MINIMAL.HTML` no casa con la exclusión y llega al servicio. No es
un leak: el sistema de ficheros distingue mayúsculas, `ServeDir` no encuentra el fichero y el servicio
responde **su** página de error —`404` con `X-Robots-Tag`—, nunca el fichero crudo.

## El socket

El ejemplo usa **labels de Docker** porque es el mecanismo que emplea la guía oficial de
`error-pages` y por tanto lo que la gente reconoce. El precio es montar el socket del motor dentro
de Traefik, y eso no es gratis: **quien puede escribir labels puede crear contenedores**. Por eso el
ejemplo fija `--providers.docker.exposedByDefault=false`, que limita la exposición a los contenedores
que llevan `traefik.enable=true`.

### Variante sin socket

Si no quieres montar el socket, el mismo montaje se expresa con el **provider de fichero**, que no
necesita socket y se comporta igual en docker y en podman:

```yaml
# traefik.yaml (estático)
providers:
  file:
    filename: /etc/traefik/dynamic.yaml

# dynamic.yaml (dinámico)
http:
  middlewares:
    errpages:
      errors:
        status: "400-599"
        service: errservice
        query: "/{status}"
  services:
    errservice:
      loadBalancer:
        servers: [{url: "http://errserve:8080"}]
  routers:
    catchall:
      rule: HostRegexp(`.+`) && !Path(`/metrics`) && (!PathRegexp(`\.html?((%2[fF])|/|$)`) || Path(`/default.html`) || Path(`/default.htm`))
      entryPoints: [web]
      service: errservice
      priority: 1
```

Las **dos** conjunciones son las mismas que en los labels, y por los mismos motivos. La de
`/metrics`: el respaldo atiende cualquier Host, y `/metrics` no es una página de error sino un
*endpoint de servicio* del binario, así que publicarlo delata el perfil de fallos de la
infraestructura; con la exclusión, una petición a `/metrics` en un Host desconocido la contesta
Traefik con su propio `404` en texto plano y el respaldo sigue sirviendo el código real en todo lo
demás. Y la del **leak**: el nombre de cada tema **es** la ruta de su página, así que sin ella el
respaldo publica las páginas de tema crudas. Está explicado, con la medición, en el apartado
«Operación y observabilidad» del [`README.md`](../../README.md) raíz y en «El leak de las páginas de
tema», arriba.

## Producción

Para un despliegue real, el [`compose.yml`](../../compose.yml) de la raíz trae **solo el servicio**
—imagen, healthcheck, `restart`, variables y los labels de Traefik listos para descomentar— para
encajarlo en la infraestructura que ya tengas. Recuerda que el puerto **interno** es el `8080`.
