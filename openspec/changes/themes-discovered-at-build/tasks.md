# Tareas: Descubrimiento de temas al compilar

## 1. Build System

- [x] Implementar `build.rs`: leer `content/`, filtrar `*.html`, generar `$OUT_DIR/temas_embebidos.rs`, emitir `cargo:rerun-if-changed=content`.
- [x] Abortar la compilación si `content/` no existe o no hay ningún `.html`.
- [x] Ordenar por nombre para que el build sea determinista.

## 2. Refactor

- [x] `src/embedded.rs`: sustituir constantes y array por `include!(concat!(env!("OUT_DIR"), "/temas_embebidos.rs"));`. Eliminar tests.
- [x] `src/theme.rs`: construir el registro base desde `EMBEBIDOS`; eliminar `INCORPORATED`, `NOMBRES`, `SOBRIAS`, `default_theme()`.
- [x] `src/theme.rs`: `tema_descubierto` sigue saltándose los tallos de `EMBEBIDOS`; elimina la exclusión por tallo numérico.
- [x] `src/main.rs` y `src/render.rs`: adaptar usos de `EMBEDDED_DEFAULT_PAGE` a búsqueda por nombre en `EMBEBIDOS`.

## 3. Limpieza

- [x] Borrar `tests/content_pages.rs`.
- [x] Borrar las pruebas de contenido en `src/theme.rs` (`cada_pagina_sobria_cubre_el_marcado`, `ninguna_pagina_puede_hacer_una_peticion`, `ninguna_pagina_que_anima_deja_de_poder_apagarse`, helpers).
- [x] Adaptar las pruebas de orden/nombres para leer `EMBEBIDOS`.

## 4. OpenSpec

- [x] Borrar `openspec/changes/ten-themes-and-guard-scope/`.
- [x] Crear `openspec/changes/themes-discovered-at-build/` con `proposal.md`, `design.md`, `tasks.md`.
- [x] Deltas en `specs/error-page/spec.md` y `specs/http-server/spec.md`.

## 5. Documentación

- [x] Actualizar `AGENTS.md` y `README.md`.
- [x] Reescribir `PLAN.md`.

## 6. Verificación

- [x] `cargo fmt --check` (sin salida)
- [x] `cargo clippy --offline --all-targets -- -D warnings` (sin avisos)
- [x] `cargo test --offline` (213 passed)
- [x] `cargo deny check -D warnings` (cargo-deny ok)
- [x] `openspec validate --all --strict` (5 passed, 0 failed)
- [x] Prueba de fuego con `THEME=antique`, `THEME=starwars`, `THEME=terminal` (antique/starwars/terminal).
- [x] Prueba de tema temporal `zzz-temporal` (creado y borrado).
