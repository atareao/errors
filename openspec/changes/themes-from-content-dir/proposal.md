# Proposal: themes-from-content-dir

## Why

Hoy la lista de temas es una **tabla cerrada en el binario**: `THEMES` es
`static [Theme; 7]` en `src/theme.rs`, `THEME_NAMES` es una constante de texto, las siete páginas van
embebidas con `include_str!` y `Config.theme` es `&'static Theme`. Esa tabla protege dos cosas: un
`cargo install` —un binario y ningún fichero de datos— sirve los siete diseños, y `THEME` se valida
**al arrancar** enumerando los nombres. El precio es que un fichero de más **no es un tema**:
`content/neon.html` se sirve **crudo** con `200` y los marcadores a la vista, `THEME=neon` aborta el
arranque y dos guardas de conteo exigen exactamente siete (`src/embedded.rs` y
`tests/content_pages.rs`). El usuario pide lo contrario: **añadir tantos temas como quiera con solo
añadir el fichero HTML**.

## La decisión que se somete a aprobación

**El directorio `CONTENT_DIR` añade temas al registro**: los siete incorporados se quedan como base y un `*.html` de más es un tema nuevo.

1. **Los siete incorporados se quedan.** Siguen siendo `default`, `minimal`, `terminal`, `paper`,
   `amber`, `compact` y `matrix`, con `default` por defecto y sus páginas embebidas. Un binario
   instalado sin ficheros de datos sigue sirviendo los siete diseños. Se **rechazó** la alternativa
   «el directorio es la **única** fuente»: dejaría a quien instale el crate sin ficheros de datos
   sirviendo el respaldo de texto.
2. **El barrido del directorio es al arrancar.** El servicio construye el **registro efectivo** —los
   siete incorporados **más** los descubiertos en la raíz de `CONTENT_DIR`— **una vez**, al arrancar.
   Añadir un tema exige **reiniciar**; los **bytes** de la página se siguen leyendo **al servir**, así
   que editar un tema se ve sin reiniciar.
3. **`THEME` sigue abortando el arranque** si su valor no es uno de los nombres del registro —o la
   cadena vacía—, con un error que enumera los nombres **del registro**. Un `X-Theme` desconocido
   sigue cayendo al configurado sin error. Esa variable la escribe el **operador**, no un cliente.
4. **La página del operador NO se inspecciona.** Los tres contratos —no pedir nada por la red, animar
   solo si puede apagarse, cubrir el marcado— siguen cubriendo **solo** las páginas incorporadas al
   binario. Se **rechazó** añadir un `warn!` al arrancar: movería el escáner de `#[cfg(test)]` al
   binario.

## What Changes

1. **`src/theme.rs`** — el **registro** y el **descubrimiento**: `THEMES` deja de ser la única fuente
   de temas y pasa a ser el registro **incorporado**; una función nueva construye el registro efectivo
   a partir de un `CONTENT_DIR` (raíz, extensión exacta `html`, el tallo como nombre, un nombre de
   código fuera). El tipo deja de ser `&'static`: `Theme` pasa a tener el nombre y —si lo hay— su
   copia embebida, y un tema descubierto **no** la tiene. `resolve()` y `theme_by_name()` operan sobre
   el registro, no sobre la tabla estática.
2. **`src/config.rs`** — `THEME` se valida contra el **registro** (`ConfigError::UnknownTheme`), y el
   mensaje enumera los nombres **en ejecución**, que son los que el registro tenga en ese arranque.
3. **`src/render.rs` y `src/main.rs`** — el tema viaja como **referencia del registro** en vez de
   `&'static`: `render_page` y `error_response` reciben la referencia que sale del registro. La ruta de
   la página **nunca** se compone con el valor de `X-Theme`.
4. **`src/embedded.rs`** — **sin cambios**: las **siete** páginas y su prueba de paridad byte a byte,
   que siguen cubriendo los temas incorporados.
5. **Las dos guardas de conteo** — `src/embedded.rs` (`el_directorio_entrega_exactamente_las_siete_paginas_de_tema`)
   y `tests/content_pages.rs` (`cada_tema_tiene_su_pagina_y_el_tallo_coincide`): siguen exigiendo que
   el `content/` **del repositorio** entregue exactamente siete páginas. El descubrimiento es para el
   `CONTENT_DIR` **del operador**, no para el `content/` del repositorio.
6. **`tests/`** — las pruebas de descubrimiento (un fichero de más es un tema; el nombre de un
   incorporado no añade un tema; un nombre de código no es un tema; solo la raíz; la extensión; sin
   directorio; un directorio ilegible; añadir un tema en marcha no lo convierte en tema) y un
   directorio de fixtures **propio** (`tests/fixtures/content/` ya existe y no añade ningún tema).
7. **`README.md`** y **`AGENTS.md`** — el modelo nuevo: la tabla de temas ya no es cerrada, el barrido
   es al arrancar, `THEME` valida contra el registro y la página del operador no se inspecciona.
8. **A decidir y reportar** — un bloque del smoke test `.github/scripts/smoke.sh` que arranque el
   contenedor con un `CONTENT_DIR` montado y un tema de más, para probar el descubrimiento contra la
   **imagen arrancada**. Va en `tasks.md` como decisión de la fase 3, no como algo ya hecho.

## Capabilities

### New Capabilities

(ninguna)

### Modified Capabilities

- `error-page`: **modificado** «El tema del HTML elige una página» —el registro efectivo son los siete
  incorporados **más** los descubiertos; el nombre de un tema descubierto es el tallo de su fichero;
  `THEME` valida contra el registro y el error enumera sus nombres; un tema descubierto no tiene copia
  embebida y su fichero desaparecido responde el respaldo dejando constancia; el nombre de `X-Theme`
  **nunca** compone una ruta— y «Las siete páginas del proyecto» —el repositorio entrega exactamente
  las siete de sus temas incorporados, y el descubrimiento es para el `CONTENT_DIR` del operador—.
  **Añadido** «El directorio añade temas al registro»: las reglas del barrido —raíz, extensión exacta,
  el tallo como nombre, un nombre de código fuera, enlaces simbólicos, el directorio inexistente o
  ilegible—, que el barrido es al arrancar y que los tres contratos no aplican a un tema descubierto.
- `http-server`: **modificado** «La configuración se lee solo del entorno» —`THEME` pasa a ser «uno de
  los siete incorporados **o un tema descubierto en `CONTENT_DIR`», y el aborto enumera los nombres
  **del registro**, con el precio medido de la trampa del montaje escrito—.

## Non-goals

- **La página del operador no se inspecciona.** Los tres contratos (red, movimiento, cobertura de
  marcado) siguen midiendo **solo** las páginas incorporadas al binario (decisión 4). Añadir un
  `warn!` al arrancar movería el escáner de `#[cfg(test)]` al binario, y el servicio no valida la
  página al servir.
- **No se descubre en caliente.** El barrido es al arrancar (decisión 2). El precio, escrito: añadir un
  tema exige **reiniciar**. Un fichero añadido con el servicio en marcha sigue sirviéndose como **asset
  crudo** con `200` hasta el siguiente arranque. Un barrido por petición pondría un `read_dir` en el
  camino de cada página de error, justo cuando más se piden, y la validación de `THEME` solo podría ser
  perezosa.
- **No se toca el leak.** `GET /neon.html` sigue siendo un `200` **crudo** con los marcadores a la
  vista: la garantía «un asset existente gana a la página de error» no se toca. El respaldo de Traefik
  que el repositorio entrega ya lo bloquea **por forma** —`!PathRegexp(\.html?((%2[fF])|/|$))`—, sin
  lista de nombres que mantener: un `content/neon.html` nuevo queda protegido **gratis**.
- **No se toca el `Dockerfile` ni el empaquetado.** `content/` sigue viajando en el crate y en la
  imagen, y las siete páginas siguen embebiéndose con `include_str!`.
- **No se toca la rotación de temas** (`--rotation-mode` de `error-pages`): sigue siendo un no-goal
  declarado del proyecto.
- **No se implementa el release.** Etiqueta, imagen, crate y Release quedan tras el OK explícito del
  usuario (`tasks.md`, fase 5).

## Impact

- `src/theme.rs` — `THEMES` pasa a ser el registro **incorporado**; una función nueva construye el
  registro efectivo desde `CONTENT_DIR`; `Theme` deja de ser `&'static` y un tema descubierto **no**
  lleva copia embebida; `theme_by_name()`, `default_theme()` y `resolve()` operan sobre el registro.
- `src/config.rs` — `Config.theme` deja de ser `&'static Theme` y pasa a referenciar el registro (que
  la `Config` posee); `THEME` valida contra él; `ConfigError::UnknownTheme` enumera los nombres en
  ejecución.
- `src/render.rs` y `src/main.rs` — `render_page` y `error_response` reciben el tema como referencia
  del registro; `resolve()` deja de devolver `&'static Theme`; el valor de `X-Theme` no compone rutas.
- `src/embedded.rs` — sin cambios funcionales; las siete páginas y su paridad byte a byte siguen.
- `tests/` — pruebas de descubrimiento nuevas y un directorio de fixtures propio; las pruebas que
  asumían `&'static` se retocan.
- `README.md`, `AGENTS.md` — la tabla de temas deja de ser cerrada; el barrido al arrancar; la trampa
  del montaje; la página del operador no se inspecciona.
- `openspec/specs/error-page/spec.md` y `openspec/specs/http-server/spec.md` — vía las deltas de este
  change.
