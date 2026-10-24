# Design: retry-after-configurable

## Context

Ver `proposal.md` — Why. Lo que condiciona el diseño:

- La configuración entra **solo** por entorno, con una regla ya establecida: una variable presente pero vacía **no** es una variable ausente, es un error de arranque tipado (`ConfigError`).
- `render_page` ya recibe `&Config` (`src/render.rs`), así que tiene acceso al valor sin cambiar de firma; `render_text_fallback(code)` y `companion_headers(code)` no reciben configuración alguna.
- `resolve::retry_after_secs(code)` mezcla hoy dos decisiones en una: **qué códigos** llevan la cabecera y **qué valor** llevan.
- El estado compartido del proyecto vive en el estado del router, nunca en un `static`: así lo decidió el contador de métricas, para que dos routers no se contaminen entre sí y las pruebas sean deterministas.

## Goals / Non-Goals

**Goals**

- Que el operador pueda fijar el valor de `Retry-After` sin recompilar, y sin cambiar el comportamiento por defecto.
- Separar, en el código, «qué códigos lo llevan» de «cuántos segundos».

**Non-Goals**

- No se rediseña la resolución de cabeceras: `RenderedPage` sigue siendo la respuesta completa y lista para servir.
- No se añade otra vía de configuración (fichero, argumentos): sigue siendo el entorno.

## Decisions

**D1. Una sola variable para los dos códigos, `RETRY_AFTER`, con `120` por defecto.** El defecto es el valor actual y el de `error-pages`, así que no cambia nada para quien no configure. *Alternativa descartada:* `RETRY_AFTER_429` y `RETRY_AFTER_503` — dos variables para un caso que nadie ha pedido, y dos formas de desincronizar dos respuestas que hoy dicen lo mismo.

**D2. Tipo `u32` y `0` admitido.** `0` significa «reintenta ya» y es un `delay-seconds` válido en RFC 9110. No se pone techo: un valor absurdo no rompe ningún contrato, mientras que el techo de `DEFAULT_ERROR_PAGE` sí está justificado porque un código fuera de rango haría responder `200` a toda petición sin pistas. *Alternativa descartada:* acotar a `86400` (un día); inventar un límite que el RFC no pone obliga a justificarlo y a mantenerlo.

**D3. Una variable presente pero vacía o no numérica es un error de arranque**, con `ConfigError::InvalidRetryAfter`. Es la regla que ya siguen `PORT` y `DEFAULT_ERROR_PAGE`. *Alternativa descartada:* caer al defecto en silencio —quien escribe `RETRY_AFTER` cree que está configurando algo—.

**D4. Solo `delay-seconds`, nunca una fecha HTTP.** El servicio emite la forma numérica, como `error-pages`. Aceptar y reemitir texto libre convertiría una variable de entorno en un canal para escribir cabeceras arbitrarias.

**D5. El valor viaja por parámetro, desde `Config`.** `render_page` lo lee de la `Config` que ya recibe; `render_text_fallback` y `companion_headers` ganan un `retry_after: u32`. *Alternativa descartada:* un `static` o un `OnceLock` con la configuración —el proyecto ya evitó el estado global en el contador justo para no contaminar routers ni pruebas; aquí además impediría probar dos valores en el mismo proceso—.

**D6. `retry_after_secs(code, configured) -> Option<u32>`.** La función sigue viviendo en `resolve` y sigue diciendo **qué códigos** lo llevan (`429` y `503`); el valor lo aporta la configuración. Mantiene la frontera: `resolve` decide, `render` compone. *Alternativa descartada:* renombrarla a `carries_retry_after(code) -> bool` y que `render` ponga el valor — más explícito, pero renombra la función y sus pruebas sin cambiar nada de lo que se observa desde fuera—.

## Risks / Trade-offs

- [Los escenarios de la spec citan `Retry-After: 120` literal] → el defecto se mantiene en `120`, así que solo se reescribe el escenario del `503` para que hable del valor configurado, y se añaden los del valor distinto.
- [Duplicar la decisión «qué códigos llevan la cabecera» en dos sitios] → se queda en `resolve`, en un único `match`, y la prueba del `404` sin `Retry-After` lo sigue fijando.
- [Cambiar la firma de `render_text_fallback` toca cinco pruebas] → es el precio de no usar estado global; el cambio es mecánico y las aserciones no se debilitan, porque el defecto sigue dando `120`.

## Open Questions

(ninguna: la única duda viva —si se admite `0` y si se acota— la cierra D2, y dejarla abierta cambiaría la spec.)
