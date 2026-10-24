# Tasks: error-pages-parity

## 0. Prerrequisito: las dependencias

- [x] 0.1 Resolver, contra la caché local de `~/.cargo/registry`, las versiones exactas de `axum` (con `tokio`), `tokio` (`full`), `tower-http` (`fs`, `trace`), `tracing`, `tracing-subscriber` (`env-filter`), `serde` (`derive`) y `serde_json`; en dev, `tower` (`util`) y `http-body-util`. Escribirlas en `Cargo.toml` sin fijarlas de memoria.
- [x] 0.2 `cargo check` limpio con las dependencias puestas.

## 1. RED (pruebas que fallan primero)

- [x] 1.1 `src/config.rs`: test que fija que sin `PORT` el puerto por defecto es `8080` y que `PORT=9999` lo cambia.
- [x] 1.2 `src/resolve.rs`: test que fija la cascada del código —path, luego `X-Code`, luego `DEFAULT_ERROR_PAGE`— y que el path gana a la cabecera.
- [x] 1.3 `src/resolve.rs`: test que fija la cascada del formato —extensión, `Content-Type`, `X-Format`, `Accept`, defecto texto plano—.
- [x] 1.4 `src/resolve.rs`: test que fija que código y formato son ortogonales en `/503.xml`.
- [x] 1.5 `src/render.rs`: test que fija el cuerpo JSON con las claves `error`, `code`, `message` y `description`, y el cuerpo XML con el elemento `code`.
- [x] 1.6 `src/main.rs`: test de integración con `tower::ServiceExt` que fija que `GET /404` responde `404` —no `200`—, que `GET /` con `X-Code: 503` responde `503`, que `GET /healthz` responde `200` y que `GET /version` responde JSON.
- [x] 1.7 `src/render.rs`: test que fija las cabeceras: `X-Robots-Tag` en toda página, `Retry-After: 120` en `503` y su ausencia en `404`.
- [x] 1.8 `src/main.rs`: test que fija que `GET /css/404.css` sirve el fichero de `CONTENT_DIR` con `Content-Type` `text/css`.
- [x] 1.9 Ejecutar `cargo test`: las pruebas nuevas fallan por no existir el código.

## 2. GREEN (implementación mínima)

- [x] 2.1 `src/config.rs`: lectura de `PORT`, `LOG_LEVEL`, `CONTENT_DIR` y `DEFAULT_ERROR_PAGE` desde el entorno, con sus defectos.
- [x] 2.2 `src/resolve.rs`: la cascada del código, la del formato y la tabla de razón canónica y descripción.
- [x] 2.3 `src/render.rs`: la construcción del cuerpo por formato —HTML desde fichero con caída al defecto; JSON, XML y texto generados— y las cabeceras.
- [x] 2.4 `src/main.rs`: el router —endpoints de servicio, assets estáticos y *fallback* de páginas de error— y el arranque con `tracing`.
- [x] 2.5 `content/`: las páginas HTML de error comunes y `css/404.css`.
- [x] 2.6 `cargo test` en verde.

## 3. REFACTOR

- [x] 3.1 `cargo fmt --all` y `cargo clippy --all-targets -- -D warnings` limpios.
- [x] 3.2 Sin `unwrap()`, ni `expect()`, ni `panic!()` en código de producción; los errores de configuración son errores de arranque tipados.
- [x] 3.3 La ruta de estáticos coincide entre `config.rs`, el `Dockerfile` y `.dockerignore`: una sola, no `static` por un lado y `content` por otro. (Hoy solo existe la vía del código: `CONTENT_DIR` con defecto `content` y ninguna constante `static_dir`; el `Dockerfile` y el `.dockerignore` aún no existen, así que no hay divergencia posible.)
- [x] 3.4 La configuración entra solo por el entorno: no queda ningún `config.yml` ni lectura de fichero de configuración.

## 4. Verificación manual (operador)

- [x] 4.1 `cargo run` y comprobar en el log el arranque y el puerto.
- [x] 4.2 `curl -i localhost:8080/404`: estado `404`, `X-Robots-Tag` presente.
- [x] 4.3 `curl -i localhost:8080/`: estado `404` (defecto) y cuerpo HTML.
- [x] 4.4 `curl -i -H 'X-Code: 503' localhost:8080/`: estado `503` y `Retry-After: 120`.
- [x] 4.5 `curl -s localhost:8080/404.json` y `curl -s -H 'X-Format: application/xml' localhost:8080/404`: cuerpos JSON y XML correctos.
- [x] 4.6 `curl -i localhost:8080/healthz`: `200`.
- [x] 4.7 `curl -i localhost:8080/css/404.css`: `200` con `Content-Type` `text/css`.

## 5. Cierre

- [x] 5.1 `openspec validate error-pages-parity --strict` en verde.
- [x] 5.2 `AGENTS.md`: actualizar la sección «Estado real del repositorio» para reflejar lo que ya exista.
- [x] 5.3 `README.md` con el propósito, el contrato con Traefik y los ejemplos de `curl`.
- [x] 5.4 Redactar el `Purpose` de ambas capabilities nuevas al archivar.
- [x] 5.5 Archivar el change (`openspec archive error-pages-parity`).

## 6. Correcciones de la auditoría

- [x] 6.1 B-1: normalización del path antes de clasificar (percent-decode + `.`/`..`).
- [x] 6.2 B-2: rango `400..=599` en la resolución y validación de `DEFAULT_ERROR_PAGE`.
- [x] 6.3 I-1: render fuera del hilo async (`spawn_blocking`).
- [x] 6.4 I-2: el fallo de render conserva el código resuelto.
- [x] 6.5 I-3: precedencia asset vs. `X-Code` fijada con prueba.
- [x] 6.6 Pruebas de regresión de la auditoría (1–12).
- [x] 6.7 Ruido de log: el `on_failure` del `TraceLayer` baja a `DEBUG`.
- [x] 6.8 `ServeDir` sin `append_index_html_on_directories`.
- [x] 6.9 `debug_assert` del rango del defecto en `resolve_code`.
