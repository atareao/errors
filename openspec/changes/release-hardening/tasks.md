# Tasks: release-hardening

## 0. Prerrequisito: las dependencias

- [ ] 0.1 Verificar en la caché local de `~/.cargo/registry` y añadir lo mínimo: la feature de compresión gzip de `tower-http` (arrastra `async-compression` y `flate2`, ambos ya presentes en la caché) y la feature `json` de `tracing-subscriber`. No fijar versiones de memoria. Dejar escrito en el comentario del `Cargo.toml` por qué.
- [ ] 0.2 `cargo check --offline` limpio con las dependencias puestas.

## 1. RED (pruebas que fallan primero)

- [ ] 1.1 gzip: `GET /404` con `Accept-Encoding: gzip` responde `Content-Encoding: gzip`, y el cuerpo descomprimido es el de siempre.
- [ ] 1.2 gzip: sin `Accept-Encoding`, la respuesta NO SHALL llevar `Content-Encoding`.
- [ ] 1.3 `Vary: Accept-Encoding` presente cuando la respuesta puede comprimirse.
- [ ] 1.4 correlación: con `X-Request-Id` en la petición, la respuesta lo repite con el mismo valor.
- [ ] 1.5 correlación: con las cuatro cabeceras presentes, las cuatro se copian; ausentes, no aparecen en la respuesta (no se inventan).
- [ ] 1.6 `GET /metrics` responde `200` en texto Prometheus y **no** una página de error.
- [ ] 1.7 métricas: tras N peticiones a `/404` y M a `/503`, el contador de cada código es el esperado.
- [ ] 1.8 `LOG_FORMAT`: `json` produce líneas que `serde_json` parsea; `text` es el defecto; un valor distinto es error de arranque que nombra la variable.
- [ ] 1.9 `cargo test`: fallan por no existir el código.

## 2. GREEN (implementación mínima)

- [ ] 2.1 `Cargo.toml`: las features de 0.1 y los metadatos (`license`, `authors`, `description`, `repository`).
- [ ] 2.2 `src/config.rs`: `LOG_FORMAT` con `text` por defecto y error tipado ante un valor desconocido.
- [ ] 2.3 `src/metrics.rs`: el contador por código (atómico, sin cerrojo) y su serialización Prometheus.
- [ ] 2.4 `src/main.rs`: la capa de compresión, la copia de las cabeceras de correlación, el router de `/metrics` y el incremento del contador en el camino de las páginas de error.
- [ ] 2.5 El registro con `tracing-subscriber` según `LOG_FORMAT`.
- [ ] 2.6 `cargo test` en verde.

## 3. REFACTOR

- [ ] 3.1 `cargo fmt --check` y `cargo clippy --all-targets -- -D warnings` limpios.
- [ ] 3.2 Sin `unwrap`/`expect`/`panic!` en producción.
- [ ] 3.3 El contador no puede crecer sin límite: solo cuenta códigos del rango utilizable.

## 4. Verificación manual (operador)

- [ ] 4.1 `curl -i --compressed localhost:8080/404` → `Content-Encoding: gzip` y el cuerpo correcto.
- [ ] 4.2 `curl -i -H 'X-Request-Id: abc123' localhost:8080/404` → la respuesta repite `abc123`.
- [ ] 4.3 `curl -s localhost:8080/metrics` → texto Prometheus con el contador.
- [ ] 4.4 `LOG_FORMAT=json cargo run` y comprobar que las líneas son JSON válido; `LOG_FORMAT=basura` no arranca y nombra la variable.
- [ ] 4.5 Repetir el E2E con Traefik de la verificación anterior y confirmar que la compresión y la correlación atraviesan el proxy.

## 5. Entrega y cierre

- [ ] 5.1 `LICENSE` (MIT) — el `README.md` ya lo declara y hoy no existe.
- [ ] 5.2 `CHANGELOG.md` con la entrada de `v0.1.0`.
- [ ] 5.3 `.github/workflows/ci.yml` con las tres puertas.
- [ ] 5.4 `README.md` y `AGENTS.md`: `LOG_FORMAT`, `/metrics`, compresión y correlación.
- [ ] 5.5 Etiqueta `v0.1.0` y regenerar la imagen local.
- [ ] 5.6 `openspec validate release-hardening --strict` y archivar.
