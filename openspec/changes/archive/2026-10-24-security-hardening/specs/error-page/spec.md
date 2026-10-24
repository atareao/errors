# error-page Specification (delta)

## ADDED Requirements

### Requirement: Ningún estado de error del servidor de ficheros se escapa

Cuando el servidor de ficheros responda con un estado de error —cualquiera de `4xx` o `5xx`, no solo el `404`—, la aplicación SHALL sustituir esa respuesta por la página de error, con cuerpo propio y con sus cabeceras.

El estado de la página SHALL ser **siempre** el código resuelto por la cascada —path, cabecera `X-Code` o defecto— y NO SHALL ser el estado que devolvió el servidor de ficheros. El estado del servidor de ficheros no es una de las fuentes de esa cascada, y conservarlo rompía tres cosas a la vez, todas medidas con el binario delante: obligaba a propagar cabeceras que describen *su* respuesta —`Allow` en el `405`, `Content-Range` en el `416`— so pena de incumplir un `MUST` de RFC 9110; dejaba el estado y el cuerpo en desacuerdo, sirviendo un `404` con `Retry-After: 120` y el cuerpo del `503`; y, con `DEFAULT_ERROR_PAGE=503`, convertía cualquier asset inexistente en un `404`, dejando además sin efecto a `X-Code`, que existe justo para los despliegues que no reescriben el path.

Por eso el estado SHALL derivarse del código resuelto, y el `Retry-After` —que describe el estado que se sirve, no el código de un cuerpo ajeno— SHALL corresponder al estado de la respuesta: `429` y `503` SHALL llevarlo, el resto NO SHALL incluirlo.

El motivo del cambio SHALL quedar escrito: el servidor de ficheros responde `500` —vacío y sin cabeceras— cuando un segmento del path excede el límite de nombre del sistema de ficheros, y permitir fabricar `5xx` con una URL larga es ruido de alertas a coste cero.

#### Scenario: un segmento de path imposible da la página, no un 500 vacío
- **GIVEN** un servicio en marcha
- **WHEN** se pide `GET /` con un segmento de 300 caracteres
- **THEN** el estado SHALL ser el código resuelto —el defecto, `404`—
- **AND** el cuerpo NO SHALL estar vacío
- **AND** la respuesta SHALL llevar `X-Robots-Tag`

#### Scenario: un método no admitido sobre un asset da la página del código resuelto
- **GIVEN** un asset existente en el directorio de contenido
- **WHEN** se pide ese asset con un método que el servidor de ficheros no admite
- **THEN** el estado SHALL ser el código resuelto y NO SHALL ser `405`
- **AND** el cuerpo SHALL ser la página de error
- **AND** la respuesta SHALL llevar `X-Robots-Tag`

#### Scenario: un range no satisfacible da la página del código resuelto
- **GIVEN** un asset existente en el directorio de contenido
- **WHEN** se pide con varios rangos a la vez
- **THEN** el estado SHALL ser el código resuelto y NO SHALL ser `416`
- **AND** la respuesta SHALL llevar `X-Robots-Tag`

#### Scenario: un asset inexistente sigue la cascada y no el estado del servidor de ficheros
- **GIVEN** un servicio en marcha
- **WHEN** se pide un asset que no existe indicando el código con `X-Code: 503`
- **THEN** el estado SHALL ser `503`
- **AND** la respuesta SHALL llevar `Retry-After: 120`

#### Scenario: el defecto configurado manda sobre el not found del servidor de ficheros
- **GIVEN** un servicio arrancado con `DEFAULT_ERROR_PAGE=503`
- **WHEN** se pide un asset que no existe
- **THEN** el estado SHALL ser `503` y NO SHALL ser `404`

#### Scenario: el Retry-After corresponde al estado servido
- **GIVEN** un servicio en marcha
- **WHEN** se sirve una página de error con un código distinto de `429` y `503`
- **THEN** la respuesta NO SHALL incluir `Retry-After`

#### Scenario: el cuerpo y su formato siguen la cascada en una respuesta interceptada
- **GIVEN** un asset existente en el directorio de contenido
- **WHEN** se pide con un método que el servidor de ficheros no admite y con `Accept: text/html`
- **THEN** el `Content-Type` SHALL ser HTML
- **AND** el cuerpo SHALL ser la página HTML del código resuelto

#### Scenario: un asset sigue sirviéndose con 200
- **GIVEN** un asset existente en el directorio de contenido
- **WHEN** se pide con `GET`
- **THEN** el estado SHALL ser `200`
- **AND** NO SHALL ser una página de error
