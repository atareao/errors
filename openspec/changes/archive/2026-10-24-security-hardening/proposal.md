# Proposal: security-hardening

## Why

Una auditoría de seguridad dedicada —no una revisión de código— encontró **dos defectos medios y dos bajos**, cada uno con una petición reproducible. El más grave rompe el contrato que justifica el proyecto.

Un segmento de path de más de 255 bytes hace que el servidor de ficheros falle al abrir (el nombre no cabe en el sistema de ficheros) y responda un **`500` con cuerpo vacío**, sin `X-Robots-Tag` y sin que ninguna de las fuentes de resolución lo haya producido. Medido: `GET /<300 caracteres>` → `HTTP/1.1 500`, `content-length: 0`. Los `4xx` propios del servidor de ficheros —el `405` de un método que no sirve estáticos, el `416` de un `Range` múltiple— se escapan igual. El contrato dice que toda página de error lleva sus cabeceras y que el estado es el código resuelto; esto no es ni una cosa ni la otra.

Y `/metrics` está expuesto sin control de acceso con un contador cuyo valor elige quien llama: la cabecera `X-Code` entra en la resolución, así que cincuenta peticiones con `X-Code: 503` fabrican `errors_pages_total{code="503"} 50`. La única señal de observabilidad del servicio se puede inventar, que es justo lo que la hace inútil en una incidencia.

Lo que la auditoría **no** encontró importa igual y queda escrito: unos treinta payloads de *path traversal* sin una sola lectura fuera de `CONTENT_DIR`, sin inyección de cabeceras (hyper rechaza CR/LF), sin bypass del código de estado y sin bomba de compresión. El diseño de la normalización del path aguantó.

### La regla que hubo que enmendar

La primera versión de este change conservaba el estado `4xx` del servidor de ficheros cuando estaba en el rango utilizable, y solo caía al código resuelto con un `5xx`. La revisión de la implementación —con las dos versiones del binario delante— demostró que esa regla era peor que el defecto. Lo medido, con la misma petición antes y después:

| Petición | Conservando el `4xx` | Con el código resuelto |
|---|---|---|
| `PUT /css/error.css` | `405` **sin** `Allow` —el `MUST` de RFC 9110 §15.5.6 se perdía al construir nuestra propia respuesta— | `404` con la página |
| `GET /nada.png` con `X-Code: 503` | `404` con `retry-after: 120` y el cuerpo del `503` | `503` |

Conservar el estado obligaba a propagar cabeceras que describen *su* respuesta (`Allow`, `Content-Range`) so pena de incumplir el RFC, dejaba el estado y el cuerpo en desacuerdo —contradiciendo «el `404` no lleva `Retry-After`»— y, con `DEFAULT_ERROR_PAGE=503`, convertía cualquier asset inexistente en un `404`, dejando sin efecto a `X-Code`, que existe justo para los despliegues que no reescriben el path. El estado del servidor de ficheros no es una de las fuentes de la cascada, y la garantía del proyecto es que el estado de una página de error **es** el código resuelto.

## What Changes

1. **Ningún estado de error del servidor de ficheros se escapa.** Cualquier respuesta suya con estado de error —`4xx` o `5xx`, no solo el `404`— se sustituye por la página de error, con cuerpo y cabeceras, y con el estado igual al **código resuelto** por la cascada, nunca el suyo: una URL larga no puede fabricar un `5xx` del servicio, el `405` de un método no puede tapar el código que manda la configuración, y el `Retry-After` siempre corresponde al estado que se sirve.
2. **El contador distingue el origen del código** —`path`, `header` o `default`—, para que un inflado por cabecera sea visible en el scrape en lugar de indistinguible.
3. **Las cabeceras de correlación se declaran en `Vary`**, para que una caché intermedia no sirva a un usuario el identificador de otro.
4. **Endurecer el contenedor** del `compose.yml`: `read_only`, sin capacidades y sin privilegios nuevos. El binario es estático y no escribe.
5. **Documentar** lo que no se cambia a propósito: `/version` publica la versión por paridad con `error-pages`, y el límite de tasa corresponde a Traefik, no a la aplicación.

## Non-goals

- **No se añade autenticación a `/metrics`.** El sitio correcto para eso es Traefik: el servicio se limita a que su contador sea interpretable, y la documentación dice que `/metrics` no debe publicarse en el router de respaldo.
- **No se añade limitación de tasa ni de tiempo por petición.** La respuesta es un fichero diminuto, no hay estado por petición y el proxy es el lugar correcto para eso.
- **No se deja de publicar `/version`**: es paridad deliberada con `error-pages` y hoy no hay ningún aviso que lo haga explotable.
- **No se cambia la normalización del path.** La auditoría no encontró nada que romper en ella y no se toca lo que aguanta.
- **No se añade registro de peticiones** ni histogramas de latencia.

## Impact

- `src/main.rs` — la interceptación de los estados de error del servidor de ficheros y la declaración de las cabeceras de correlación en `Vary`.
- `src/metrics.rs` — la etiqueta de origen del contador.
- `src/resolve.rs` — exponer de qué fuente salió el código resuelto, con la cascada en un único punto de entrada.
- `compose.yml` — `read_only`, `cap_drop` y `no-new-privileges`.
- `README.md` — que `/metrics` no se publica en el router de respaldo, y por qué `/version` sigue ahí.
- `AGENTS.md` — lo mismo, en las garantías.
- tests: el segmento de path imposible, el método no admitido y el `Range` múltiple sobre un asset, el asset que sigue sirviéndose con `200`, el asset inexistente con `X-Code` y con `DEFAULT_ERROR_PAGE`, el `Retry-After` del estado servido, el cuerpo según el formato en una respuesta interceptada, el path ganando a la cabecera con su etiqueta, la etiqueta de origen del contador y las cabeceras de correlación en `Vary`.
