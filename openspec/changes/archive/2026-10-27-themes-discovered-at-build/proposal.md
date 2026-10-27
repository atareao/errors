# Cambio: Descubrimiento de temas al compilar

## Problema

El modelo actual fija en el código los nombres y el número de las páginas de tema. Añadir un tema exige editar `src/embedded.rs`, añadir una constante y ajustar listas en `src/theme.rs` y en los tests. Es frágil y no escala.

## Solución

Descubrir las páginas al compilar con un `build.rs` que lee `content/*.html` y genera un array `EMBEBIDOS` en `$OUT_DIR/temas_embebidos.rs`. El binario embebe lo que haya, sin lista ni recuento fijo en el código. Añadir un tema pasa a ser dejar el fichero y reconstruir.

## Alcance

- `build.rs` nuevo en la raíz del crate.
- `src/embedded.rs` incluye el fichero generado.
- `src/theme.rs` construye el registro base desde `EMBEBIDOS` y elimina las listas fijas.
- Se borran las pruebas de contenido.
- Se retiran las guardas de contenido (red, movimiento y cobertura de marcado) y la prueba de paridad, porque las páginas son opacas.
- Se actualizan los documentos de OpenSpec.

## Fuera de alcance

- Cambios en los ficheros de `content/`.
- Cambios de comportamiento en el servicio (cascada, formato, cabeceras).
- Publicación o release.
