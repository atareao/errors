# Tasks: security-hardening

## 0. Prerrequisito

- [x] 0.1 Sin dependencias nuevas: los arreglos se hacen con lo que ya hay.

## 1. RED (pruebas que fallan primero)

### 1a. Ningún estado de error del servidor de ficheros se escapa

- [x] 1.1 `src/main.rs`: `GET /` con un segmento de 300 caracteres responde el código resuelto —no un `500` vacío—, con cuerpo y con `X-Robots-Tag`.
- [x] 1.2 `src/main.rs`: `PUT` sobre un asset existente responde el código resuelto —**no** `405`—, con la página de error y `X-Robots-Tag`.
- [x] 1.3 `src/main.rs`: `GET` sobre ese mismo asset sigue respondiendo `200`: lo que se arregla no puede romper el camino bueno.
- [x] 1.4 `src/main.rs`: `Range` múltiple sobre un asset responde el código resuelto —**no** `416`—, con `X-Robots-Tag`.
- [x] 1.5 `src/main.rs`: un asset inexistente con `X-Code: 503` responde `503` con `Retry-After`, no el `404` del servidor de ficheros.
- [x] 1.6 `src/main.rs`: con `DEFAULT_ERROR_PAGE=503`, un asset inexistente responde `503` y no `404`.
- [x] 1.7 `src/main.rs`: el `Retry-After` corresponde al estado servido —lo lleva el `429`/`503` y no lo lleva el resto— sobre una respuesta interceptada.
- [x] 1.8 `src/main.rs`: el cuerpo y el `Content-Type` de una respuesta interceptada siguen la cascada de formato (`Accept: text/html` da HTML).

### 1b. El contador

- [x] 1.9 `src/metrics.rs`: el contador lleva la etiqueta de origen y una página pedida por `X-Code` se cuenta con origen `header`, no `path`.
- [x] 1.10 `src/main.rs`: `/404` con `X-Code: 503` se sirve como `404` y se cuenta con origen `path` (el path gana, y la etiqueta lo dice).

### 1c. Las cabeceras de correlación

- [x] 1.11 `src/main.rs`: `Vary` incluye `Accept-Encoding` y los nombres de las cuatro cabeceras de correlación, con y sin ellas en la petición, también en los endpoints de servicio.

### 1d. Confirmación del RED

- [x] 1.12 Ejecutar `cargo test`: las pruebas nuevas fallan y las anteriores siguen verdes.

## 2. GREEN (implementación mínima)

- [x] 2.1 `src/resolve.rs`: exponer de qué fuente salió el código resuelto (path, cabecera o defecto), sin cambiar el código devuelto.
- [x] 2.2 `src/main.rs`: sustituir por la página de error **cualquier** respuesta del servidor de ficheros con estado de error, con el estado igual al **código resuelto**.
- [x] 2.3 `src/main.rs`: declarar en `Vary` las cuatro cabeceras de correlación, además de `Accept-Encoding`.
- [x] 2.4 `src/metrics.rs`: la etiqueta de origen en el contador y en su serialización Prometheus.
- [x] 2.5 `cargo test` en verde.

## 3. REFACTOR

- [x] 3.1 `cargo fmt --check` y `cargo clippy --all-targets -- -D warnings` limpios.
- [x] 3.2 Sin `unwrap`/`expect`/`panic!` en producción.
- [x] 3.3 La interceptación no puede tragarse un `200`: comprobarlo con un asset real.
- [x] 3.4 Sin `#[allow(dead_code)]`: la cascada tiene un único punto de entrada público.

## 4. Verificación manual (operador)

- [x] 4.1 Repetir las reproducciones de la auditoría y comprobar que ya no ocurren: el segmento largo, el `PUT` sobre un asset y el contador inflado por `X-Code`.
- [x] 4.2 `curl -s localhost:8080/metrics | grep source` y comprobar la etiqueta.
- [x] 4.3 Comprobar con `curl -i` que un asset normal sigue respondiendo `200` con su `Content-Type`.
- [x] 4.4 Levantar el ejemplo de `examples/traefik/` y ejecutar `check.sh`: los casos siguen correctos.

## 5. Cierre

- [x] 5.1 `compose.yml`: `read_only: true`, `cap_drop: [ALL]` y `security_opt: [no-new-privileges:true]`, y comprobar que el contenedor arranca y sirve con eso puesto.
- [x] 5.2 `README.md`: que `/metrics` no se publica en el router de respaldo, y que `/version` se mantiene por paridad.
- [x] 5.3 `AGENTS.md`: lo mismo en las garantías.
- [x] 5.4 `openspec validate security-hardening --strict` y archivar.
