# Tasks: retry-after-configurable

## 0. Prerrequisito

- [x] 0.1 Sin dependencias nuevas: el valor entra por la configuración que ya existe.

## 1. RED (pruebas que fallan primero)

### 1a. La configuración

- [x] 1.1 `src/config.rs`: sin `RETRY_AFTER`, el defecto es `120`.
- [x] 1.2 `src/config.rs`: `RETRY_AFTER=30` se respeta.
- [x] 1.3 `src/config.rs`: `RETRY_AFTER` vacía o no numérica (`""`, `"medio minuto"`, `"-1"`) es error de arranque y el mensaje **nombra la variable**.
- [x] 1.4 `src/config.rs`: `RETRY_AFTER=0` es válido: es un `delay-seconds` legal («reintenta ya»).

### 1b. La cabecera

- [x] 1.5 `src/render.rs`: con `RETRY_AFTER=30` la página `503` lleva `Retry-After: 30`, y el respaldo de texto también.
- [x] 1.6 `src/render.rs`: el `429` lleva el valor configurado.
- [x] 1.7 `src/render.rs`: el `404` NO lleva `Retry-After` aunque haya `RETRY_AFTER` configurado.
- [x] 1.8 `src/main.rs`: con `RETRY_AFTER=30`, un asset inexistente con `X-Code: 503` lleva `Retry-After: 30` — el valor no depende del origen del código.
- [x] 1.9 Ejecutar `cargo test`: las nuevas fallan y las anteriores siguen verdes (el defecto sigue siendo `120`, así que ninguna aserción existente cambia de valor esperado).

## 2. GREEN (implementación mínima)

- [x] 2.1 `src/config.rs`: el campo `retry_after` en `Config`, la variante `InvalidRetryAfter` en `ConfigError` con su mensaje, y el parseo.
- [x] 2.2 `src/resolve.rs`: `retry_after_secs(code, configured)` — sigue diciendo qué códigos lo llevan.
- [x] 2.3 `src/render.rs`: `companion_headers(code, retry_after)` y `render_text_fallback(code, retry_after)`; `render_page` lee `config.retry_after`.
- [x] 2.4 `src/main.rs`: las dos llamadas de producción pasan el valor de la configuración.
- [x] 2.5 `cargo test` en verde.

## 3. REFACTOR

- [x] 3.1 `cargo fmt --check` y `cargo clippy --all-targets -- -D warnings` limpios.
- [x] 3.2 Sin `unwrap`/`expect`/`panic!` en producción y sin `#[allow(dead_code)]`.
- [x] 3.3 La decisión de «qué códigos llevan la cabecera» vive en un solo sitio.

## 4. Documentación del registro de peticiones

- [x] 4.1 `README.md`: `RETRY_AFTER` en la tabla de variables, con su defecto y su límite (solo `429` y `503`).
- [x] 4.2 `README.md`: cómo encender el registro de peticiones (`LOG_LEVEL`) y cómo acotar el evento de fallo que marca todo `5xx` como fallo, con el filtro medido.
- [x] 4.3 `AGENTS.md`: la variable en la tabla de configuración y el registro de peticiones en los quirks.
- [x] 4.4 Comprobar contra el binario real que el filtro documentado deja el registro limpio: una línea por petición, sin el falso «response failed» de cada `503`.

## 5. Verificación manual (operador)

- [x] 5.1 Arrancar con `RETRY_AFTER=30` y comprobar con `curl -i` que el `503` lleva `Retry-After: 30` y que el `404` no lleva la cabecera.
- [x] 5.2 Arrancar sin `RETRY_AFTER` y comprobar que sigue siendo `120`.
- [x] 5.3 `RETRY_AFTER=basura` no arranca, y el mensaje nombra la variable.

## 6. Cierre

- [x] 6.1 `openspec validate retry-after-configurable --strict` y archivar.
