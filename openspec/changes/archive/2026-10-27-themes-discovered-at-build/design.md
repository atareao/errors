# Diseño: Descubrimiento de temas al compilar

## Contexto

El servicio sirve páginas de error para Traefik. Hasta ahora, las páginas de tema eran un conjunto fijo de ficheros incrustados en el binario mediante constantes en `src/embedded.rs`. El registro de temas se construía combinando esas constantes con el descubrimiento en tiempo de ejecución de `CONTENT_DIR`.

## Decisión

Sustituir las constantes por un `build.rs` que, al compilar, lee `content/*.html`, filtra los ficheros cuyo tallo sea UTF-8 no vacío, los ordena por nombre y genera un array `EMBEBIDOS: [(&str, &str); N]` en `$OUT_DIR/temas_embebidos.rs`. `src/embedded.rs` incluye ese fichero con `include!`. El registro base se construye desde `EMBEBIDOS`.

### Motivos

1. **Determinismo**: el orden alfabético hace que el build sea reproducible.
2. **Simplicidad**: añadir un tema es dejar el fichero y reconstruir, sin editar código.
3. **Robustez**: el binario siempre lleva sus temas; si `content/` falta o está vacío, la compilación aborta con un mensaje claro.
4. **Consistencia**: lo que se embebe es exactamente lo que hay en `content/`, sin listas intermedias que puedan divergir.

### Consecuencias

- El número y los nombres de los temas embebidos NO SHALL fijarse en el código: se derivan de `content/`.
- Las páginas son **opacas**: el servicio no las juzga. Se eliminan las guardas de contenido (red, movimiento, marcado).
- El registro efectivo es `EMBEBIDOS ∪ CONTENT_DIR`; el fichero del directorio gana por nombre.
- Se elimina la exclusión por tallo numérico: cualquier `.html` es un tema.

### Por qué se retiran las guardas de contenido

El servicio **sirve** páginas de error; no las audita. Una guarda textual no puede demostrar que una URL construida en tiempo de ejecución no pide nada, ni que una animación se apaga de verdad en todos los navegadores. Mantener esas guardas obligaba a fijar en el código qué páginas cumplen qué contratos —una lista que ahora cambia con `content/`— y convertía una decisión de aspecto en un requisito de compilación. La consecuencia aceptada y escrita: **las páginas son opacas**. El operador responde de su contenido; el servicio de su disponibilidad.

### Preguntas abiertas

- **Copia de `content/` en runtime**: se mantiene la decisión actual (copia opcional, declarada en la spec de `http-server`). Quitarla haría que la imagen sirviese las páginas embebidas.
