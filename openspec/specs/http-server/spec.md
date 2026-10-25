# http-server Specification

## Purpose
El contrato de arranque y servicio: qué variables de entorno se leen y con qué valores por defecto, que el puerto por defecto es el `8080` que asume la guía de Traefik, que los endpoints de salud y de versión responden siempre sin pasar por la resolución de páginas de error, y que los ficheros de `CONTENT_DIR` se sirven como assets estáticos con su `Content-Type`.

## Requirements

### Requirement: La configuración se lee solo del entorno

La aplicación SHALL leer su configuración exclusivamente de variables de entorno: `PORT` (puerto de escucha, por defecto `8080`), `LOG_LEVEL` (filtro de registro, por defecto `info`), `LOG_FORMAT` (formato del registro, `text` o `json`, por defecto `text`), `CONTENT_DIR` (directorio de páginas y assets del operador, por defecto `content`; es un *override* de los recursos embebidos en el binario, no la única fuente de páginas y hojas), `DEFAULT_ERROR_PAGE` (código por defecto, por defecto `404`) y `RETRY_AFTER` (segundos de la cabecera `Retry-After` de los códigos que la llevan, por defecto `120`). NO SHALL leer ningún fichero de configuración como `config.yml`.

Una variable presente pero vacía NO SHALL confundirse con una ausente. Una variable numérica —`PORT`, `DEFAULT_ERROR_PAGE`, `RETRY_AFTER`— presente pero no interpretable SHALL abortar el arranque con un error que nombre la variable.

Que el directorio de `CONTENT_DIR` no exista NO SHALL impedir el arranque, ni cambiar el `200` de un asset embebido, ni degradar ninguna página de error a un `500`: los recursos embebidos cubren ese caso.

#### Scenario: Sin PORT escucha en 8080
- **GIVEN** un entorno sin `PORT`
- **WHEN** la aplicación arranca
- **THEN** SHALL escuchar en `0.0.0.0:8080`

#### Scenario: PORT cambia el puerto
- **GIVEN** `PORT=9999`
- **WHEN** la aplicación arranca
- **THEN** SHALL escuchar en `0.0.0.0:9999`

#### Scenario: RETRY_AFTER cambia el valor de la cabecera
- **GIVEN** `RETRY_AFTER=30`
- **WHEN** la aplicación arranca y sirve una página `503`
- **THEN** la respuesta SHALL incluir `Retry-After: 30`

#### Scenario: un RETRY_AFTER no numérico no arranca
- **GIVEN** `RETRY_AFTER=medio minuto`
- **WHEN** la aplicación arranca
- **THEN** el arranque SHALL fallar con un error que nombre `RETRY_AFTER`

#### Scenario: un RETRY_AFTER vacío no arranca
- **GIVEN** `RETRY_AFTER` presente y vacía
- **WHEN** la aplicación arranca
- **THEN** el arranque SHALL fallar, en vez de usar `120`

#### Scenario: sin CONTENT_DIR el defecto sigue siendo content
- **GIVEN** un entorno sin `CONTENT_DIR`
- **WHEN** la aplicación arranca
- **THEN** el directorio de contenido SHALL ser `content`
- **AND** el arranque NO SHALL fallar aunque ese directorio no exista

### Requirement: Los endpoints de servicio siempre responden 200

Los endpoints `/healthz`, `/health`, `/health/live` y `/live` SHALL responder siempre `200`, sin pasar por la resolución de páginas de error. `GET /version` SHALL devolver `{"version":"..."}` en JSON.

#### Scenario: la salud no es una página de error
- **GIVEN** un servicio en marcha
- **WHEN** se pide `GET /healthz`
- **THEN** SHALL responder `200`
- **AND** el cuerpo NO SHALL ser una página de error

#### Scenario: la versión es JSON
- **GIVEN** un servicio en marcha
- **WHEN** se pide `GET /version`
- **THEN** SHALL responder `200` con `Content-Type` JSON
- **AND** el cuerpo SHALL incluir la clave `version`

### Requirement: Los assets estáticos se sirven desde CONTENT_DIR

Las peticiones que no son páginas de error ni endpoints de servicio SHALL servirse como ficheros estáticos: el fichero de `CONTENT_DIR` cuando exista y se pueda leer, con su `Content-Type` correspondiente, y, si no, el asset **embebido** equivalente si lo hubiera. Un fichero de `CONTENT_DIR` **que se pueda leer** SHALL ganar al asset embebido del mismo path.

Un asset embebido NO SHALL ser una página de error: una página HTML de error se sirve por la vía de la página de error, con el código resuelto, nunca como asset y nunca con un `200`. La hoja `css/error.css` SHALL estar embebida, de modo que el HTML embebido que la referencia se sirva completo.

#### Scenario: un CSS se sirve desde el directorio de contenido
- **GIVEN** el fichero `content/css/404.css`
- **WHEN** se pide `GET /css/404.css`
- **THEN** SHALL responder `200`
- **AND** el `Content-Type` SHALL ser `text/css`

#### Scenario: la hoja embebida se sirve sin directorio de contenido
- **GIVEN** un servicio arrancado sin ningún fichero en `CONTENT_DIR`
- **WHEN** se pide `GET /css/error.css`
- **THEN** SHALL responder `200`
- **AND** el `Content-Type` SHALL ser `text/css`
- **AND** el cuerpo NO SHALL estar vacío
- **AND** NO SHALL ser una página de error

#### Scenario: el fichero del directorio gana al asset embebido
- **GIVEN** un `CONTENT_DIR` con un `css/error.css` propio, distinto del embebido
- **WHEN** se pide `GET /css/error.css`
- **THEN** el cuerpo SHALL ser el de ese fichero
- **AND** NO SHALL ser el asset embebido

#### Scenario: un asset embebido no convierte una página de error en un 200
- **GIVEN** un servicio arrancado sin ningún fichero en `CONTENT_DIR`
- **WHEN** se pide `GET /404.html`
- **THEN** el estado SHALL ser `404`
- **AND** NO SHALL ser `200`
