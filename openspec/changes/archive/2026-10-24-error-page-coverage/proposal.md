# Proposal: error-page-coverage

## Why

El servicio ya está verificado de punta a punta contra Traefik, pero su cobertura de páginas tiene un agujero que se nota justo donde duele. `content/` cubre `403`, `404`, `429`, `500`, `502` y `503`: un `504` —uno de los códigos que un proxy emite con más frecuencia, un timeout— sale con el **estado correcto** (eso está garantizado por la cascada y probado) pero con el **cuerpo del 404**, que dice «Not Found» y enlaza la hoja del 404. En un servicio cuyo único producto es la página de error, un timeout que se presenta como «no encontrado» no es un detalle estético: es información equivocada para quien lo lee.

Las seis páginas actuales comparten además una sola hoja de estilos (`css/404.css`), de modo que el problema no es solo de texto.

La verificación E2E con Traefik, ya realizada y en verde, dejó además un hecho medido que la documentación no recoge y que puede inducir a error. Medido, no deducido: **cuando el middleware `errors` intercepta la respuesta de una aplicación, Traefik conserva el código original y el que devolvemos nosotros se ignora** —probado a propósito con una configuración divergente: backend `503`, middleware pidiendo `/404`, y el cliente recibió `503`—. Nuestro código decide en el caso **directo** (router *catch-all*, *default backend*), que es exactamente el caso que justifica el proyecto. Las cabeceras y el cuerpo sí son nuestros en ambos caminos.

## What Changes

1. **Cinco páginas nuevas** —`400`, `401`, `405`, `408` y `504`— que elevan la cobertura a once códigos, la lista de los que un proxy emite con normalidad. Cada página identifica su propio código, de modo que el cuerpo nunca contradiga al estado.
2. **Hoja de estilos por código.** Cada página referencia `css/<código>.css`, y esas hojas existen: hoy las seis apuntan a `css/404.css`.
3. **Dos pruebas que cierran la clase entera de fallo**, no solo los casos de hoy: una recorre la lista de códigos cubiertos y exige página propia que se nombre a sí misma, y otra extrae los `href` locales de cada página y exige que resuelvan a un fichero de `CONTENT_DIR`. Un enlace roto o una página que falte vuelve a fallar.
4. **La documentación recoge el reparto de responsabilidad del código con Traefik**, tal como se midió, para que nadie crea que este servicio arregla el código cuando quien responde es el middleware.

## Non-goals

- **No hay motor de plantillas ni temas.** El HTML sigue saliendo de ficheros estáticos.
- **No se cubren todos los códigos posibles.** La lista es la de los que un proxy emite con normalidad; el respaldo al código por defecto sigue existiendo y no se toca.
- **No se cambia la cascada** de código ni de formato, ni el rango `400..=599`, ni el respaldo cuando falta el HTML.
- **No hay localización.**

## Impact

- `content/400.html`, `content/401.html`, `content/405.html`, `content/408.html`, `content/504.html` — **nuevos**.
- `content/css/<código>.css` — **nuevos**; los `href` de las seis páginas existentes se actualizan.
- `tests/fixtures` — las fixture de prueba siguen valiendo; si alguna prueba fija `css/404.css`, se actualiza al nuevo contrato.
- `README.md` y `AGENTS.md` — la lista de códigos cubiertos y el reparto de responsabilidad del código con Traefik (el hecho medido).
- tests: dos pruebas nuevas que cubren la clase entera (cobertura de la lista y resolución de `href`).
