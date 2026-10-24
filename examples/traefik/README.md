# Integración con Traefik

Ejemplo **ejecutable**: Traefik v3 + este servicio + un backend que devuelve 4xx/5xx.

```sh
docker compose up -d          # o: podman-compose up -d
./check.sh                    # comprueba los cuatro casos de abajo
docker compose down
```

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

## Los cuatro casos que comprueba `check.sh`

| Petición | Esperado | Qué demuestra |
|---|---|---|
| `app.localtest.me/` | `404` | El backend devuelve 404; el estado lo conserva Traefik, el cuerpo es nuestro. |
| `app.localtest.me/boom` | `503` | Idem con un 503, más `Retry-After: 120`. |
| `nada.localtest.me/x` | `404` | **Catch-all sin upstream: el código lo decidimos nosotros.** Es la razón del proyecto. |
| `diverge.localtest.me/boom` | `503` | Backend 503 con el middleware pidiendo `/404`: manda Traefik. |

`check.sh` sale con estado 1 si algo no cuadra, así que sirve también en un pipeline.

## La trampa de `HostRegexp`

El router de respaldo necesita una regla que case con cualquier host. **La sintaxis cambió en v3 y
la vieja no falla: simplemente no casa.**

```yaml
# MAL en Traefik v3 — sintaxis v2 (placeholder {nombre:regexp})
traefik.http.routers.errors.rule: HostRegexp(`{host:.+}`)

# BIEN en Traefik v3 — regexp de Go
traefik.http.routers.errors.rule: HostRegexp(`.+`)
```

Medido en v3.6: ``HostRegexp(`{host:.+}`)`` **no casa con nada** y ``HostRegexp(`.+`)`` casa. Lo
peligroso es cómo se rompe: el router aparece como `enabled` en el dashboard y Traefik responde **su
propio 404 en texto plano**, sin llegar nunca al servicio. Se parece a que todo funciona, y es
exactamente el fallo que este proyecto existe para corregir.

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
        servers: [{url: "http://errors:8080"}]
  routers:
    catchall:
      rule: "HostRegexp(`.+`)"
      entryPoints: [web]
      service: errservice
      priority: 1
```

## Producción

Para un despliegue real, el [`compose.yml`](../../compose.yml) de la raíz trae **solo el servicio**
—imagen, healthcheck, `restart`, variables y los labels de Traefik listos para descomentar— para
encajarlo en la infraestructura que ya tengas. Recuerda que el puerto **interno** es el `8080`.
