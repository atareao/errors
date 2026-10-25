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

**Seis bloques**: los cuatro estados de la tabla, las cabeceras de la página servida del `503`, la
página autocontenida con el tema por petición, y que el respaldo no publique el endpoint de servicio
`/metrics`.

| Petición | Esperado | Qué demuestra |
|---|---|---|
| `app.localtest.me/` | `404` | El backend devuelve 404; el estado lo conserva Traefik, el cuerpo es nuestro. |
| `app.localtest.me/boom` | `503` | Idem con un 503, más `Retry-After: 120` y sus cabeceras (`X-Robots-Tag`, `Content-Type`). |
| `nada.localtest.me/x` | `404` | **Catch-all sin upstream: el código lo decidimos nosotros.** Es la razón del proyecto. |
| `diverge.localtest.me/boom` | `503` | Backend 503 con el middleware pidiendo `/404`: manda Traefik. |
| `app.localtest.me/boom` con `Accept: text/html` | `503` | **El `503` llega con la hoja incrustada y sin enlazar `/css/error.css`**, y `X-Theme: terminal` cambia el cuerpo de esa petición. |
| `nada.localtest.me/metrics` | no `200` | El respaldo **no** publica el endpoint de servicio `/metrics`. |

`check.sh` sale con estado 1 si algo no cuadra, así que sirve también en un pipeline.

### La página autocontenida y el tema, por petición

Sobre `app.localtest.me/boom` —el **mismo** `503` del backend, no un caso nuevo—, con tres ayudantes
que afirman y niegan sobre el **cuerpo**:

- `html_body app.localtest.me /boom '<style'` — el `503` del backend llega con la hoja **incrustada**:
  la página no pide nada para verse.
- `not_html_body app.localtest.me /boom 'rel="stylesheet"'` — y **no** enlaza `/css/error.css`. Es el
  bug **medido**: en el host de la aplicación ese path lo contestaba el upstream —`404 text/plain`—,
  el middleware `errors` interceptaba ese `404` y lo volvía a pedir como página de error con un
  `Accept` que **no** negocia HTML, la cascada de formato caía al texto plano y el navegador rechazaba
  la hoja (*«MIME type ('text/plain') is not a supported stylesheet MIME type»*). La página salía
  **sin estilos**, con el `503` y el cuerpo correctos.
- `html_differs app.localtest.me /boom 'X-Theme: terminal'` — el tema se elige **por petición**: pide
  el mismo path dos veces, con y sin esa cabecera, y exige que los cuerpos **difieran**. Es lo que
  demuestra la selección de extremo a extremo, a través de Traefik; `terminal` es uno de los seis
  temas que documenta el [`README.md`](../../README.md) raíz.

Los tres mandan `Accept: text/html` **a propósito**. Sin esa pista, un `curl` manda `*/*`, la cascada
de formato no elige HTML —cae al **texto plano** por defecto— y el bloque no probaría nada: no habría
hoja que buscar en el cuerpo. Es también el motivo de que `html_differs` compare **cuerpos** y no
busque un color en la hoja: un color es un detalle que se puede retocar sin avisar, y «el cuerpo
cambia con la cabecera» es el contrato.

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
      rule: "HostRegexp(`.+`) && !Path(`/metrics`)"
      entryPoints: [web]
      service: errservice
      priority: 1
```

La conjunción `&& !Path(`/metrics`)` es la misma que en los labels, y por el mismo motivo: el respaldo
atiende cualquier Host, y `/metrics` no es una página de error sino un *endpoint de servicio* del
binario, así que publicarlo delata el perfil de fallos de la infraestructura. Con la exclusión, una
petición a `/metrics` en un Host desconocido la contesta Traefik con su propio `404` en texto plano y
el respaldo sigue sirviendo el código real en todo lo demás. Está explicado, con la medición, en el
apartado «Operación y observabilidad» del [`README.md`](../../README.md) raíz.

## Producción

Para un despliegue real, el [`compose.yml`](../../compose.yml) de la raíz trae **solo el servicio**
—imagen, healthcheck, `restart`, variables y los labels de Traefik listos para descomentar— para
encajarlo en la infraestructura que ya tengas. Recuerda que el puerto **interno** es el `8080`.
