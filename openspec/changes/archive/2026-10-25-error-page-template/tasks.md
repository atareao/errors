# Tasks: error-page-template

> La página del proyecto deja de ser once ficheros que repiten lo que el servidor ya sabe, y pasa a ser
> **una plantilla** rellenada al servir. Con eso el mismo error no puede decir dos cosas distintas
> según el `Accept`, y ningún código sirve el cuerpo de otro.

## 0. Prerrequisito

- [x] 0.1 Dejar constancia de la medición: cinco de los once códigos divergen entre el HTML y el JSON
      (`400`, `401`, `405`, `408`, `504`), y **189** códigos de `400..=599` sirven un cuerpo que dice
      `404`, porque la cascada les presta el fichero del código por defecto.
- [x] 0.2 Dejar constancia de que `embedded::PAGE_CODES` no lo usa nadie fuera de `src/embedded.rs`, y
      de que `embedded::page` se llama dos veces, ambas desde `src/render.rs`.
- [x] 0.3 Dejar constancia de la medición del nombre del fichero: `GET /404.html` → `404` con la
      página (la cascada del código lo intercepta) y `GET /css/error.css` → `200 text/css` (el asset
      gana). Un nombre neutro se serviría crudo.

## 1. RED

- [x] 1.1 Unitarias de la sustitución en `src/render.rs`: `{{code}}`, `{{reason}}` y `{{description}}`
      se rellenan con el código y con lo que devuelven `canonical_reason`/`description`; se sustituye
      **cada** aparición, no solo la primera; y una página **sin** marcadores se sirve **byte a byte**.
- [x] 1.2 El escape: una razón o una descripción con `&`, `<` o `>` sale escapada y el HTML sigue
      siendo válido.
- [x] 1.3 **El caso que da nombre al change**: un código sin fichero propio (`451`) recibe su propio
      cuerpo —su código y su razón canónica— y **no** el del `404`.
- [x] 1.4 La razón y la descripción del HTML son **las mismas** que las del JSON para el mismo código.
- [x] 1.5 La paridad byte a byte entre la página embebida y la de `content/`.
- [x] 1.6 De router, en `src/main.rs`: `GET /451` en HTML devuelve estado `451` y un cuerpo que nombra
      `451`.
- [x] 1.7 Confirmar que las pruebas previas siguen en verde y que las que fallan son **solo** las
      nuevas que se espera que fallen.

## 2. GREEN

- [x] 2.1 La sustitución de los marcadores y el escape para HTML, en `src/render.rs`, con las reglas
      del marcador de la hoja. Sin dependencias nuevas: escaneo de bytes, sin `regex` y sin parser.
- [x] 2.2 La página única, con sus marcadores, y los diez ficheros por código retirados.
- [x] 2.3 `src/embedded.rs`: de once páginas a una; `PAGES` y `PAGE_CODES` retirados.
- [x] 2.4 `src/resolve.rs`: la redacción del HTML en los cinco códigos que divergen.
- [x] 2.5 `RenderError::MissingHtml` retirado —la plantilla siempre está—, con el respaldo de texto
      reservado al fallo de E/S.
- [x] 2.6 `tests/content_pages.rs` y los fixtures, que hoy iteran once ficheros.
- [x] 2.7 Todas las pruebas en verde, y `git diff content/` con **una** página y diez borrados, y
      **ninguna regla** de las seis hojas tocada.

## 3. Verificación

- [x] 3.1 `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --offline` y
      `cargo deny check -D warnings`.
- [x] 3.2 `cargo package --list --offline --allow-dirty`: la página y las seis hojas siguen viajando.
- [x] 3.3 **Con el binario real**: `GET /451`, `GET /418` y `GET /400` en los **cuatro formatos**, y
      que la razón y la descripción coincidan entre el HTML y el JSON.
- [x] 3.4 Un incumplimiento se nombra: si la página y el spec discrepan, el mensaje dice cuál y dónde.

## 4. Documentación

- [x] 4.1 `AGENTS.md`: la página única, los marcadores, el escape, qué gana el operador y que el
      fichero conserva el nombre del código por defecto (con el motivo medido).
- [x] 4.2 `README.md`: lo mismo, en la sección que describe `content/`.
- [x] 4.3 `src/theme.rs` y `src/embedded.rs`: la documentación de módulo que habla de once páginas.
- [x] 4.4 Las **seis** hojas de `content/`: dicen «la hoja de las once páginas de error (400, 401, …)»
      y pasan a ser la hoja **de la página**. Sin tocar ni una regla.
- [x] 4.5 Ningún sitio puede seguir diciendo «once páginas» mintiendo: son 19 los que lo dicen.

## 5. Cierre

- [x] 5.1 `openspec validate error-page-template --strict` limpio.
- [x] 5.2 `openspec archive error-page-template`.
- [x] 5.3 Borrar `PLAN.md`.
      > Sin commit ni PR: el trabajo se queda en el árbol hasta que el usuario lo decida.
