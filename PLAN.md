# PLAN — temas descubiertos al compilar

Change: `themes-discovered-at-build` · Release objetivo: por determinar

## Objetivo

Sustituir la lista fija de páginas de tema por un descubrimiento en tiempo de compilación: `build.rs` lee `content/*.html`, genera un array `EMBEBIDOS` y el binario embebe lo que haya. Añadir un tema es dejar el fichero y reconstruir.

## Estado

Implementación completa; puertas de código y OpenSpec en verde (fmt, clippy, 213 tests, cargo-deny, openspec); pendiente el build de la imagen tras corregir el `Dockerfile` y el commit.

## Modelo

1. **Compilación**: `build.rs` recorre `content/`, filtra `*.html` con tallo UTF-8 no vacío, ordena por nombre y genera `$OUT_DIR/temas_embebidos.rs`.
2. **Embebido**: `src/embedded.rs` incluye el fichero generado con `include!`.
3. **Registro**: `src/theme.rs` construye la base desde `EMBEBIDOS` y la combina con los temas descubiertos en `CONTENT_DIR` al arrancar. El fichero del directorio gana por nombre.
4. **Páginas opacas**: el servicio no juzga el contenido. Se retiran las guardas de red, movimiento y marcado.
5. **Sin lista fija**: no hay constantes por tema ni listas cerradas de nombres o de páginas sobrias.

## Fases

### Fase 1 — Build System
- `build.rs`: leer `content/`, generar `temas_embebidos.rs`, abortar si falta `content/` o no hay `.html`.
- Emitir `cargo:rerun-if-changed=content`.

### Fase 2 — Refactor
- `src/embedded.rs`: `include!` del fichero generado; sin tests.
- `src/theme.rs`: registro desde `EMBEBIDOS`; eliminar listas fijas.
- `src/main.rs` y `src/render.rs`: adaptar constantes a búsqueda por nombre en `EMBEBIDOS`.

### Fase 3 — Limpieza
- Borrar `tests/content_pages.rs`.
- Borrar las pruebas de contenido en `src/theme.rs`.
- Adaptar pruebas de orden/nombres a `EMBEBIDOS`.

### Fase 4 — OpenSpec
- Change `themes-discovered-at-build` con `proposal.md`, `design.md`, `tasks.md` y deltas en `error-page` y `http-server`.
- `openspec validate --all --strict` en verde.

### Fase 5 — Documentación
- `AGENTS.md` y `README.md` al día: sin recuentos fijos, añadir tema = fichero + reconstruir.

## Verificación final (medida, no afirmada)

- `cargo fmt --check`, `cargo clippy --offline --all-targets -- -D warnings`, `cargo test --offline`, `cargo deny check -D warnings`, `openspec validate --all --strict`.
- Prueba de fuego: `cargo build --offline`, arrancar con `THEME=antique`, `THEME=starwars`, `THEME=terminal` y pedir `/503` con `Accept: text/html`; comprobar que el cuerpo no lleva `{{`.
- Prueba de tema temporal: añadir `content/zzz-temporal.html`, reconstruir, comprobar que se sirve con `THEME=zzz-temporal`, borrarlo y reconstruir.
