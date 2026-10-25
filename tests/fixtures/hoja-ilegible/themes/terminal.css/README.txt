Esta carpeta se llama `terminal.css` a propósito: es un **directorio** donde el
tema espera un fichero (`CONTENT_DIR/themes/terminal.css`).

Un `std::fs::read` de un directorio falla en Linux (`EISDIR`), así que es un
error de lectura **real y reproducible** con el que probar el escenario «la hoja
existe y no se puede leer» de la delta de `self-contained-themes`.

Se usa un directorio —y no un fichero con permisos `000`— porque esos permisos
**no** se pueden probar de forma portable: en CI, que corre como `root`, un
fichero `000` se lee igual y la prueba pasaría por la rama equivocada.

Git no versiona un directorio vacío: este `README.txt` existe solo para que la
carpeta entre en el repositorio.
