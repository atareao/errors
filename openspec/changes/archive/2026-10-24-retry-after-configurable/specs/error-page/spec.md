# error-page Specification (delta)

## MODIFIED Requirements

### Requirement: Las cabeceras de acompañamiento

Toda página de error SHALL incluir `Content-Type` según el formato y `Content-Length` del cuerpo, y SHALL incluir `X-Robots-Tag: noindex, nofollow, nosnippet, noarchive`. Los códigos `429` y `503` SHALL incluir además `Retry-After` con el valor configurado en `RETRY_AFTER` —`120` por defecto, el mismo que `error-pages`—; el resto NO SHALL incluirlo.

Qué códigos llevan la cabecera NO SHALL ser configurable: lo decide el código, no el entorno.

El valor SHALL ser un número entero de segundos (`delay-seconds` de RFC 9110), nunca una fecha HTTP.

#### Scenario: toda página lleva X-Robots-Tag
- **GIVEN** una petición a `GET /404`
- **WHEN** se responde
- **THEN** la respuesta SHALL incluir `X-Robots-Tag: noindex, nofollow, nosnippet, noarchive`

#### Scenario: el 503 lleva Retry-After
- **GIVEN** un servicio arrancado sin `RETRY_AFTER`
- **WHEN** se responde una página `503`
- **THEN** la respuesta SHALL incluir `Retry-After: 120`

#### Scenario: el 503 lleva el valor configurado
- **GIVEN** un servicio arrancado con `RETRY_AFTER=30`
- **WHEN** se responde una página `503`
- **THEN** la respuesta SHALL incluir `Retry-After: 30`

#### Scenario: el 429 también lo lleva
- **GIVEN** un servicio arrancado con `RETRY_AFTER=45`
- **WHEN** se responde una página `429`
- **THEN** la respuesta SHALL incluir `Retry-After: 45`

#### Scenario: el 404 no lleva Retry-After
- **GIVEN** un servicio arrancado con `RETRY_AFTER=30`
- **WHEN** se responde una página `404`
- **THEN** la respuesta NO SHALL incluir `Retry-After`

#### Scenario: el valor no depende del formato ni del origen del código
- **GIVEN** un servicio arrancado con `RETRY_AFTER=30`
- **WHEN** se pide un `503` en JSON y un asset inexistente con `X-Code: 503`
- **THEN** las dos respuestas SHALL incluir `Retry-After: 30`
