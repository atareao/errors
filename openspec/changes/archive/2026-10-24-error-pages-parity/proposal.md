# Proposal: error-pages-parity

## Why

Este repositorio cubre un hueco concreto de `tarampampam/error-pages` cuando quien lo consume es **Traefik**. Ese proyecto responde **siempre HTTP 200** en las páginas de error, porque asume que el proxy reescribe solo el cuerpo y conserva el código original. La suposición se cumple con Nginx o ingress-nginx —hay un upstream cuyo código el proxy conserva—, pero no con el patrón de Traefik que la propia guía oficial documenta: un router de respaldo `HostRegexp(.+)` con prioridad baja. Ahí no hay upstream cuyo código preservar, así que el cliente recibe el `200` que devolvió el servicio de errores y una página «404» llega al navegador como si la petición hubiera ido bien.

La corrección es de una sola dirección: **devolver el código de error real**. Todo lo demás —cómo se resuelve el código por path, cómo se negocia el formato, qué cabeceras acompañan a la respuesta— se mantiene por **paridad** con `error-pages`, para que este servicio sea un reemplazo directo en una configuración de Traefik ya existente: basta cambiar la imagen. El alcance es deliberadamente **supersencillo**: un único binario Rust (axum) que sirve páginas estáticas y genera las respuestas estructuradas, sin motor de plantillas, sin localización y sin los extras que no hacen falta detrás de un proxy.

## What Changes

1. **Nace el servidor HTTP (`src/main.rs`), con axum 0.8 sobre tokio.** Escucha en `0.0.0.0:{PORT}` (por defecto `8080`, el puerto que asume la guía de Traefik). La configuración se lee **solo del entorno**; no se arrastra el `config.yml` que `simpleserver` declara y nunca lee.

2. **El código de error se resuelve en cascada y viaja en el path.** Traefik reenvía con `errors.query: /{status}`, de modo que una página 404 llega como `GET /404`. La resolución sigue el orden de `error-pages`: path → cabecera `X-Code` → defecto configurable (`DEFAULT_ERROR_PAGE`, por defecto `404`).

3. **La respuesta devuelve el código resuelto, no un 200.** Es la razón de existir del proyecto. Toda página de error responde con un `StatusCode` igual al código resuelto; responder `200` se considera un fallo.

4. **El formato se negocia en su propia cascada.** Orden de `error-pages`: extensión del path (`.html`/`.htm`, `.json`, `.xml`, `.txt`) → `Content-Type` de la petición → cabecera `X-Format` → `Accept` → por defecto **texto plano**. Código y formato son ortogonales: `/503.xml` es un `503` en XML.

5. **Cuatro formatos de cuerpo.** HTML desde el fichero `content/<code>.html` —con caída al fichero del código por defecto si el del código pedido no existe—; JSON, XML y texto generados en el proceso a partir del código, su razón canónica y su descripción. Las cabeceras de acompañamiento de `error-pages` se respetan: `Content-Type`, `Content-Length`, `X-Robots-Tag: noindex, nofollow, nosnippet, noarchive` y `Retry-After: 120` para el subconjunto de códigos que lo llevan.

6. **Los assets estáticos y los endpoints de servicio.** Los ficheros de `CONTENT_DIR` (por defecto `content`) se sirven para las rutas que no son páginas de error (p. ej. `/css/404.css`). Los endpoints de salud (`/healthz`, `/health`, `/health/live`, `/live`) responden siempre `200`, y `/version` devuelve `{"version":"..."}` en JSON, como en `error-pages`.

7. **Se reaprovecha la forma de `simpleserver`, corrigiendo sus dos incoherencias.** El proyecto hermano `/data/rust/simpleserver` fija la plantilla de stack (axum + tokio + tower-http + tracing) y de empaquetado (`Dockerfile` multi-stage + `.justfile`), pero aquí: (a) la ruta de estáticos es **una sola** y coherente entre código, `Dockerfile` y `CONTENT_DIR` —no `static` en el código y `content` en el `Dockerfile`—; y (b) la configuración entra por **una sola vía**, el entorno, sin un `config.yml` decorativo.

## Non-goals

- **No hay motor de plantillas.** El HTML sale de ficheros estáticos; no se implanta el sistema de plantillas de `error-pages` ni sus temas.
- **No hay localización.** El texto va en un único idioma.
- **No hay compresión gzip.** `error-pages` la ofrece; aquí queda fuera del MVP.
- **No hay copia de `--proxy-headers`.** No se reenvían `X-Request-Id` ni similares en la respuesta.
- **No hay generador estático (`builder`)** ni pre-renderizado de páginas: solo el servidor.
- **No hay TLS ni HTTP/2 h2c**: el servicio vive detrás de Traefik, que termina el TLS.
- **No hay tabla de descripciones curada para todos los códigos.** La tabla cubre los códigos comunes; el resto usa una descripción genérica.

## Impact

- `Cargo.toml` / `Cargo.lock` — dependencias nuevas: `axum`, `tokio`, `tower-http` (`fs`, `trace`), `tracing`, `tracing-subscriber`, `serde`/`serde_json`; en dev, `tower` (`util`) y `http-body-util`.
- `src/main.rs` — **reescrito**: arranque, router y *fallback*.
- `src/config.rs` — **nuevo**: lectura de la configuración desde el entorno.
- `src/resolve.rs` — **nuevo**: resolución del código y del formato, y tabla de mensajes.
- `src/render.rs` — **nuevo**: construcción del cuerpo por formato y de las cabeceras.
- `content/` — **nuevo**: páginas HTML de error y assets (`css/`).
- `Dockerfile` — **nuevo**: imagen multi-stage, usuario no-root, copia de `content/`.
- `.dockerignore`, `.justfile`, `README.md` — **nuevos**.
- `openspec/specs/http-server/spec.md` y `openspec/specs/error-page/spec.md` — capabilities **nuevas** (`ADDED`); al archivar hay que redactar su `Purpose`.
- tests: unitarios de la resolución (código y formato), del render por formato y del código devuelto, más un test de integración del router con `tower::ServiceExt`.
