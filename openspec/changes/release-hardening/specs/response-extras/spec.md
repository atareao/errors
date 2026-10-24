# response-extras Specification (delta)

## ADDED Requirements

### Requirement: La respuesta se comprime cuando el cliente la pide

Cuando la petición traiga `Accept-Encoding: gzip`, la respuesta SHALL ir comprimida con `Content-Encoding: gzip` y su `Content-Length` SHALL ser el del cuerpo comprimido. Sin esa cabecera, la respuesta NO SHALL ir comprimida.

Toda respuesta que pueda comprimirse SHALL declarar `Vary: Accept-Encoding`, para que ninguna caché intermedia entregue a un cliente un cuerpo comprimido que no pidió.

#### Scenario: con gzip pedido, la respuesta va comprimida
- **GIVEN** un servicio en marcha
- **WHEN** se pide `GET /404` con `Accept-Encoding: gzip`
- **THEN** la respuesta SHALL llevar `Content-Encoding: gzip`
- **AND** el estado SHALL seguir siendo `404`

#### Scenario: sin gzip pedido, la respuesta va en claro
- **GIVEN** un servicio en marcha
- **WHEN** se pide `GET /404` sin `Accept-Encoding`
- **THEN** la respuesta NO SHALL llevar `Content-Encoding`
- **AND** el cuerpo SHALL ser el de siempre

#### Scenario: la respuesta declara que varía por codificación
- **GIVEN** una respuesta que puede comprimirse
- **WHEN** se inspeccionan sus cabeceras
- **THEN** SHALL llevar `Vary: Accept-Encoding`

### Requirement: Las cabeceras de correlación se copian

Las cabeceras `X-Request-Id`, `X-Trace-Id`, `X-Correlation-Id` y `X-Amzn-Trace-Id` presentes en la petición SHALL copiarse a la respuesta con el mismo valor, en la página de error y en los endpoints de servicio.

La aplicación NO SHALL inventar un valor cuando la cabecera no venía, ni copiar ninguna otra cabecera de la petición.

#### Scenario: el identificador de petición se propaga
- **GIVEN** una petición con `X-Request-Id: abc123`
- **WHEN** se responde la página de error
- **THEN** la respuesta SHALL llevar `X-Request-Id: abc123`

#### Scenario: las cuatro cabeceras se copian juntas
- **GIVEN** una petición con las cuatro cabeceras de correlación
- **WHEN** se responde
- **THEN** las cuatro SHALL aparecer en la respuesta con sus valores

#### Scenario: una cabecera ausente no se inventa
- **GIVEN** una petición sin `X-Trace-Id`
- **WHEN** se responde
- **THEN** la respuesta NO SHALL llevar `X-Trace-Id`
