# Proposal: release-hardening

## Why

El MVP está cerrado: hace lo que promete y está verificado —82 pruebas, tres portales limpios, E2E real contra Traefik y contra la imagen—. Lo que falta no es producto, es **operación y entrega**.

Del lado de la operación, el servicio es hoy una caja opaca detrás del proxy: no comprime, no propaga las cabeceras de correlación con las que una traza atraviesa el proxy, y sus registros son texto libre que ninguna herramienta de agregación parsea con fiabilidad. No hay forma de responder «¿cuántos 503 he servido esta hora?» sin contar líneas de log.

Del lado de la entrega hay tres huecos comprobados: el `README.md` declara licencia **MIT y no existe fichero `LICENSE`**; el `Cargo.toml` no lleva `license`, `authors`, `description` ni `repository`, que su proyecto hermano sí tiene; y no hay pipeline de integración continua, con lo que las tres puertas que hoy pasan a mano no las impone nadie más que quien se acuerde de ejecutarlas.

## What Changes

1. **Compresión gzip cuando el cliente la pide**, con `Content-Encoding: gzip` y `Vary: Accept-Encoding` para que ninguna caché intermedia sirva lo comprimido a quien no lo pidió.
2. **Copia de las cabeceras de correlación** —`X-Request-Id`, `X-Trace-Id`, `X-Correlation-Id` y `X-Amzn-Trace-Id`— de la petición a la respuesta, solo si venían. No se inventan valores.
3. **Formato de registro configurable** con `LOG_FORMAT` (`text`, el actual, por defecto; `json`), para que un colector pueda parsear sin expresiones regulares.
4. **Métricas en `GET /metrics`**, en texto Prometheus: un contador de páginas servidas por código. Es el mínimo que responde a la pregunta que hoy no se puede contestar; servido como endpoint de servicio, fuera de la resolución de páginas de error.
5. **Integridad de la entrega**: fichero `LICENSE` (MIT), metadatos del `Cargo.toml`, pipeline de GitHub Actions que ejecuta las tres puertas, etiqueta `v0.1.0` y `CHANGELOG.md`.

Las cuatro primeras son comportamiento y van con especificación; la quinta es infraestructura y documentación, y no toca ninguna capability.

## Non-goals

- **No hay localización** ni motor de plantillas: siguen fuera, como en el MVP.
- **No hay TLS** ni HTTP/2 en claro: los termina Traefik.
- **No hay registro de peticiones en formato JSON anidado ni campos personalizados**: `json` serializa el registro de traza, y nada más.
- **Las métricas no incluyen histogramas, latencias ni un registro externo**: solo el contador por código, en memoria, expuesto en `/metrics`. Una latencia en un servicio que sirve ficheros no la mira nadie.
- **No hay generador estático** ni pre-renderizado.

## Impact

- `Cargo.toml` / `Cargo.lock` — la feature de compresión de `tower-http` y la de `json` de `tracing-subscriber` (verificadas en la caché local de Cargo antes de fijarlas); metadatos del paquete.
- `src/config.rs` — `LOG_FORMAT` con sus dos valores y su defecto, como error de arranque si el valor no es ninguno de los dos.
- `src/main.rs` — la capa de compresión, la copia de cabeceras de correlación, el router de `/metrics` y el registro de contadores.
- `src/metrics.rs` — **nuevo**: el contador por código y su serialización en texto Prometheus.
- `LICENSE` — **nuevo** (MIT).
- `CHANGELOG.md` — **nuevo**.
- `.github/workflows/ci.yml` — **nuevo**: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` y `cargo test`, en modo `--offline` cuando haga falta.
- `README.md` y `AGENTS.md` — `LOG_FORMAT`, `/metrics` y la compresión.
- tests: gzip con y sin `Accept-Encoding`, copia de las cuatro cabeceras y su ausencia, `/metrics` con su formato y su no-paso por la resolución de páginas de error, y `LOG_FORMAT` válido e inválido.
