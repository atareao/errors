Esta carpeta se llama `404.html` a propósito: es un **directorio** donde el
render espera la página del código por defecto (`CONTENT_DIR/404.html`).

Un `std::fs::read` de un directorio falla en Linux (`EISDIR`), así que es un
error de lectura **real y reproducible** con el que probar el escenario «la
página del operador existe y no se puede leer»: el estado sigue siendo el código
resuelto y el cuerpo el respaldo de texto, nunca un `500`.

Se usa un directorio —y no un fichero con permisos `000`— porque esos permisos
**no** se pueden probar de forma portable: en CI, que corre como `root`, un
fichero `000` se lee igual y la prueba pasaría por la rama equivocada.

Git no versiona un directorio vacío: este `README.txt` existe solo para que la
carpeta entre en el repositorio.
