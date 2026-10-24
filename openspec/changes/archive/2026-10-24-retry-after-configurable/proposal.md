# Proposal: retry-after-configurable

## Why

La cabecera `Retry-After` que acompaña al `429` y al `503` está fijada a `120` segundos en `src/resolve.rs`, igual que en `tarampampam/error-pages`. Pero ese número no es un detalle de implementación: es lo que se le promete al cliente sobre cuándo volver, y el valor correcto depende del servicio que está detrás, no de nuestra elección. Si el backend tarda cinco minutos en volver, `120` es una mentira que el cliente obedece.

Es el único valor de la respuesta que un operador querría ajustar sin recompilar, y hoy no tiene variable de entorno.

De paso se cierra un desfase de la documentación: el servicio **ya emite** el registro de peticiones —método, ruta, latencia y estado— y solo está apagado, pero ni el `README` ni `AGENTS.md` dicen cómo encenderlo.

## What Changes

1. **`RETRY_AFTER`: los segundos de la cabecera `Retry-After`, configurables por entorno**, con `120` por defecto —el valor actual, y el de `error-pages`—. La cabecera sigue emitiéndose **solo** en `429` y `503`: qué códigos la llevan no se configura.
2. **Documentar el registro de peticiones que ya existe** y cómo acotar su ruido: encenderlo con `LOG_LEVEL` y excluir el evento de fallo.

No hay más cambios de comportamiento que el valor de la cabecera. En particular, **no se añade código de registro**: el registro de traza ya lo emite `TraceLayer`, y lo único que faltaba era decirlo.

## Capabilities

### New Capabilities

(ninguna)

### Modified Capabilities

- `error-page`: el requisito «Las cabeceras de acompañamiento» deja de fijar `Retry-After: 120` y pasa a fijar la cabecera con el valor configurado, `120` por defecto.
- `http-server`: el requisito «La configuración se lee solo del entorno» enumera las variables y está incompleto: le falta `LOG_FORMAT`, que existe desde el change `release-hardening`, y le falta `RETRY_AFTER`.

## Non-goals

- **No se configura qué códigos llevan `Retry-After`.** Siguen siendo `429` y `503`, decidido por el código y cubierto por la spec; una variable para eso sería una forma de romper la paridad sin querer.
- **No se admite una fecha HTTP como valor.** RFC 9110 permite `HTTP-date` o `delay-seconds`; el servicio emite la forma numérica, y aceptar texto libre abriría la puerta a escribir cualquier cosa en una cabecera.
- **No se acota `RETRY_AFTER` con un techo arbitrario.** Un valor enorme no rompe ningún contrato —el cliente reintentará más tarde—, a diferencia de `DEFAULT_ERROR_PAGE`, que sí se acota porque un código fuera de rango haría responder `200` a toda petición sin pistas.
- **No se añade un middleware de registro propio** ni campos nuevos al evento (la IP del cliente, el `X-Code` recibido): el registro de traza ya existe, y el `X-Code` ya es visible en `/metrics` con su etiqueta de origen.
- **No se toca la normalización del path, la resolución del código ni la del formato.**

## Impact

- `src/config.rs` — el campo `retry_after` en `Config`, la variante `InvalidRetryAfter` en `ConfigError` y su validación.
- `src/resolve.rs` — `retry_after_secs` pasa a recibir el valor configurado; sigue decidiendo qué códigos lo llevan.
- `src/render.rs` — `render_page` lo lee de la `Config` que ya recibe; `render_text_fallback` y `companion_headers` ganan el parámetro.
- `src/main.rs` — las dos llamadas de producción a `render_text_fallback`.
- `README.md` — la variable nueva en su tabla, y cómo encender y acotar el registro de peticiones.
- `AGENTS.md` — lo mismo en la tabla de configuración y en los quirks.
- `openspec/specs/` — `error-page` y `http-server`, vía las deltas de este change.
- tests — el valor por defecto sigue siendo `120`, así que ninguna aserción existente cambia de valor esperado; se añaden el valor configurado, el error de arranque y la coherencia entre el cuerpo y la cabecera.
