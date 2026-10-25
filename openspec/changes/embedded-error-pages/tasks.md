# Tasks: embedded-error-pages

## 0. Prerrequisito

- [ ] 0.1 Sin dependencias nuevas: el embedding lo hace el compilador (`include_str!`/`include_bytes!`),
      así que `Cargo.toml` no cambia y `cargo deny check -D warnings` no ve nada nuevo.
- [ ] 0.2 Comprobar el hecho del que depende D3: el `.crate` que se publica ya lleva `content/`
      dentro (`cargo package --list --offline` lista las once páginas y `content/css/error.css`).
      Sin él, `include_str!` compilaría aquí y fallaría en la máquina de quien instala.
- [ ] 0.3 La imagen de Docker **no** se toca en este change: `Dockerfile` y `.dockerignore` siguen
      copiando `content/`. Es la *Open Question* de `design.md` y no cambia ni los specs ni esta
      tabla.

## 1. RED (pruebas que fallan primero)

### 1a. La página embebida sin ficheros de datos

- [ ] 1.1 `src/render.rs`: `render_body(503, 404, Format::Html, &dir_inexistente)` devuelve `Ok` con
      el cuerpo de la página del `503`; hoy devuelve `RenderError::MissingHtml`. La firma no cambia,
      así que la prueba falla por aserción, no por compilación.
- [ ] 1.2 `src/render.rs`: con un directorio sin ficheros y el defecto `404`, `render_body(418, 404,
      Format::Html, ...)` devuelve la página embebida del `404`, y `render_page` conserva el estado
      `418`.
- [ ] 1.3 `src/main.rs`: `build_router` con un `CONTENT_DIR` inexistente sirve `GET /503.html` con
      estado `503`, `Content-Type` HTML y el cuerpo de la página del `503`.
- [ ] 1.4 `src/main.rs`: sin directorio de contenido y con `Accept: text/html`, los once códigos
      responden su propio estado y un cuerpo que lleva su propio código —y no el respaldo de texto—.
- [ ] 1.5 `src/main.rs`: sin directorio de contenido, `GET /404.html` responde `404` y **no** `200`:
      la página embebida no se sirve como asset.

### 1b. El directorio del operador gana

- [ ] 1.6 Guarda, no RED: `src/render.rs::el_cuerpo_html_sale_del_fichero_del_codigo` ya compara el
      cuerpo con `tests/fixtures/content/404.html`, que **difiere** del de `content/404.html` (271
      bytes frente a 460). Es la prueba que impedirá invertir la precedencia en GREEN.
- [ ] 1.7 `src/main.rs`: con `CONTENT_DIR=tests/fixtures/content` y el defecto `404`, `GET /503.html`
      devuelve el cuerpo de su `404.html` y **no** la página embebida del `503` (decisión D1: la
      cascada del directorio se agota entera antes de entrar en la capa embebida).
- [ ] 1.8 `src/main.rs`: con ese mismo directorio, `GET /404.html` devuelve su `404.html` y no la
      copia embebida del `404`.

### 1c. La hoja embebida

- [ ] 1.9 `src/main.rs`: sin directorio de contenido, `GET /css/error.css` responde `200` con
      `Content-Type` `text/css` y cuerpo no vacío; hoy responde la página de error del defecto. En el
      mismo caso, un path que no esté en la tabla de assets (`GET /nada.css`) sigue dando la página de
      error: el respaldo **solo** responde a sus paths y `404` a todo lo demás, que es lo que el
      middleware necesita para sustituirlo.
- [ ] 1.9b `src/main.rs`: sin directorio de contenido, `HEAD /css/error.css` responde como el `GET`
      —estado `200` y `Content-Type` `text/css`— pero **sin cuerpo**. `ServeDir` copia método, URI y
      cabeceras en la petición del respaldo y `HEAD` está entre los métodos admitidos, así que este
      camino pasa por la tabla; hoy ninguna prueba lo cubre (todas montan `GET` con `Body::empty()`).
- [ ] 1.9c `src/main.rs`: sin directorio de contenido, `GET /css/error.css/` da la página de error y
      **no** un `200 text/css`: `ServeDir` lo trata como `InvalidFilename` y entra al respaldo, así que
      la tabla no puede casarlo —la comparación es exacta, sin prefijos ni barras finales—.
- [ ] 1.10 `src/main.rs`: sin directorio de contenido, la página embebida del `404` referencia
      `css/error.css` y esa ruta resuelve como `200 text/css`, no como página de error.
- [ ] 1.11 Fixture nuevo `tests/fixtures/content/css/error.css` —una hoja propia, distinguible de
      `content/css/error.css`— y, en `src/main.rs`, con `CONTENT_DIR=tests/fixtures/content`,
      `GET /css/error.css` devuelve la del directorio y no la embebida.

### 1d. El respaldo, alcanzable por el par de códigos que le queda

- [ ] 1.12 `src/main.rs`: re-apuntar `sin_fichero_html_el_estado_sigue_siendo_el_codigo_resuelto`
      para arrancar con `DEFAULT_ERROR_PAGE=418` y un `CONTENT_DIR` sin HTML: `GET /418.html`
      responde `418` con el respaldo de texto y nunca `500`. Con el defecto `404` de hoy la prueba
      quedaría verde por la página embebida del `404` y dejaría de fijar el respaldo.
- [ ] 1.13 `src/main.rs`: con el mismo directorio y el defecto `418`, `GET /599.html` responde `599`
      y no `500`.
- [ ] 1.14 `src/render.rs`: `render_body(599, 418, Format::Html, &dir_sin_html)` sigue devolviendo
      `MissingHtml` —el camino no se revoca (D4)—.

### 1e. La paridad de las dos copias

- [ ] 1.15 En los `#[cfg(test)]` del módulo que guarde la tabla: los bytes embebidos de los once
      códigos y de `css/error.css` son idénticos a `content/<código>.html` y
      `content/css/error.css`, leídos por `CARGO_MANIFEST_DIR`; y **todo** `*.html` de `content/`
      tiene entrada en la tabla. En RED **no compila** —la tabla no existe—: es su forma de fallar.
      (No puede vivir en `tests/`: el crate no tiene *lib target* y una prueba de integración no ve
      las constantes del binario.)

### 1f. La suite

- [ ] 1.16 `cargo test`: las nuevas fallan —1.1-1.5 y 1.9-1.13, con 1.9b y 1.9c incluidas, por
      aserción; 1.15 por compilación— y las que ya existían siguen verdes salvo 1.12, que se ha
      re-apuntado.

## 2. GREEN (implementación mínima)

- [ ] 2.1 Módulo con la tabla embebida (propuesta: `src/embedded.rs`): las once páginas por código
      (`include_str!`) y los assets por path (`include_bytes!` con su `Content-Type`), con su
      consulta y docstrings que digan por qué están ahí.
- [ ] 2.2 `src/render.rs`: `html_body` consulta el fichero del código y el del defecto —como hoy— y,
      solo si los dos fallan, la copia embebida del código y la del defecto (D1). `MissingHtml` se
      devuelve cuando no hay nada en ninguna de las dos capas.
- [ ] 2.3 `src/main.rs`: el *fallback* pasa a
      `ServeDir::new(&config.content_dir).append_index_html_on_directories(false).fallback(<servicio de assets embebidos>)`,
      que responde el asset —`200` con su `Content-Type`— si su path está en la tabla y `404` si no
      (D2). **`fallback`, no `not_found_service`**, y conviene saber por qué antes de escribirlo: el
      nombre del segundo es el que sugiere este caso, pero fuerza el estado de la respuesta del
      respaldo a `404` —es literalmente `self.fallback(SetStatus::new(new_fallback, StatusCode::NOT_FOUND))`,
      *tower-http* 0.6.11, `src/services/fs/serve_dir/mod.rs` líneas 240-242, y `SetStatus` escribe el
      estado «regardless of what the inner service returns», `src/set_status.rs` líneas 55 y 134—, así
      que el `200 text/css` de la hoja embebida saldría como un `404` **con cuerpo de CSS** y el
      middleware lo sustituiría por la página de error: D2 en el suelo y 1.9 en rojo. El servicio de
      respaldo, además, tiene que ser **un `Service` de verdad**, no un cierre suelto: `Clone + Send`,
      `poll_ready` que devuelva `Ready` **sin bloquear** —`ServeDir::poll_ready` delega en el suyo
      (`mod.rs`, líneas 401-407)— y **sin ninguna E/S de disco dentro**, porque este camino corre en el
      hilo *async*, a diferencia del render, que va en `spawn_blocking` por la auditoría I-1; los bytes
      embebidos son una tabla en memoria. Y tiene que responder `404` a **todo** path que no esté en su
      tabla, porque con `fallback` nadie le corrige el estado. El resto del router no se toca.
- [ ] 2.4 `cargo test` en verde, con las de 1.1-1.15 —1.9b y 1.9c incluidas— y las que ya existían.

## 3. REFACTOR

- [ ] 3.1 `cargo fmt --check` y `cargo clippy --all-targets -- -D warnings` limpios.
- [ ] 3.2 Sin `unwrap`/`expect`/`panic!` en producción, y sin variantes nuevas en `RenderError`: un
      fichero embebido no puede fallar al leerse.
- [ ] 3.3 La consulta del asset embebido va sobre el path **ya normalizado** —el middleware reescribe
      la URI antes de ceder el paso—, de modo que no hay una segunda forma de llegar al asset.
- [ ] 3.4 La lista de códigos con página embebida y la de `content/` dicen lo mismo (lo fija 1.15) y
      la capa de assets no introduce una tercera lista.
- [ ] 3.5 El mensaje de `RenderError::MissingHtml` deja de hablar solo del directorio de contenido: el
      camino falla cuando no hay página ni en `CONTENT_DIR` ni entre las embebidas, y quien lea ese
      `warn` en el registro tiene que poder deducirlo sin leer el código. Ojo con el otro mensaje: el
      `warn` de `error_response` («sin HTML de la página de error: se responde texto», `src/main.rs`
      línea 542) cubre `MissingHtml` **y** `RenderError::Io`, así que el texto nuevo tiene que servir
      para los dos casos sin mentir en ninguno —una página que existe y no se puede leer también acaba
      ahí, y esa causa no se arregla mirando el directorio—.
- [ ] 3.6 La consulta del asset embebido compara el path **de forma exacta**: sin prefijos, sin
      normalizar barras finales y sin tolerar un directorio, porque el respaldo recibe también
      `InvalidFilename` (un directorio, un path terminado en `/`) y un `200` de más ya no lo corrige
      nadie. Lo fijan las pruebas 1.9c y 1.9.

## 4. Documentación

- [ ] 4.1 `README.md`: `CONTENT_DIR` como *override* de los recursos embebidos —misma variable, mismo
      defecto `content`— y qué sirve hoy un `cargo install errserve`: las páginas de verdad y su
      hoja, sin ficheros de datos.
- [ ] 4.2 `AGENTS.md`: el contrato nuevo en la tabla de configuración y en las garantías —la base
      embebida, el *override* que gana cuando se pueda leer, la paridad de las dos copias—.
- [ ] 4.3 `AGENTS.md`, quirks: `include_str!` exige que el fichero exista al compilar y `content/`
      viaja dentro del `.crate` (comprobable con `cargo package --list`); sin ese hecho, el binario
      compilaría aquí y fallaría en la máquina de quien instala.

## 5. Verificación manual (operador)

- [ ] 5.1 Reproducir la medición que motiva el change: `cargo install --path .` y arrancar el binario
      **desde un directorio vacío**, sin `CONTENT_DIR`. `curl -i /404.html` y `curl -i /404` con
      `Accept: text/html` tienen que devolver `text/html` con la página —no `text/plain` con el
      respaldo—, y `curl -i /css/error.css` tiene que devolver `200 text/css`.
- [ ] 5.2 Comprobar que no cambia nada de lo que ya funcionaba: con el `content/` del repositorio,
      las mismas respuestas que con el binario 0.3.0 publicado, byte a byte.
- [ ] 5.3 Con `CONTENT_DIR` apuntando a un directorio propio con una sola página que se pueda leer: esa
      página gana, también para la hoja.
- [ ] 5.4 La página embebida del `503` sigue llevando `Retry-After` con el valor configurado, con y
      sin `RETRY_AFTER`.
- [ ] 5.5 Sin directorio de contenido y con `DEFAULT_ERROR_PAGE=418`, `GET /418.html` responde `418`
      en texto plano y nunca `500`.

## 6. Cierre

- [ ] 6.1 Las tres puertas en verde: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`.
- [ ] 6.2 `cargo deny check -D warnings` sin cambios (no hay dependencias nuevas).
- [ ] 6.3 `openspec validate embedded-error-pages --strict` limpio, y archivar el change para fusionar
      las deltas en `openspec/specs/`.
