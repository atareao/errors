# http-server Specification (delta)

## MODIFIED Requirements

### Requirement: La configuración se lee solo del entorno

La aplicación SHALL leer su configuración exclusivamente de variables de entorno: `PORT` (puerto de escucha, por defecto `8080`), `LOG_LEVEL` (filtro de registro, por defecto `info`), `LOG_FORMAT` (formato del registro, `text` o `json`, por defecto `text`), `CONTENT_DIR` (directorio de las páginas y los assets del operador, por defecto `content`; es un *override* de las páginas embebidas en el binario, no la única fuente de páginas), `DEFAULT_ERROR_PAGE` (código por defecto, por defecto `404`; NO SHALL nombrar ningún fichero), `RETRY_AFTER` (segundos de la cabecera `Retry-After` de los códigos que la llevan, por defecto `120`) y `THEME` (tema de las páginas HTML, por defecto `default`: uno de los **siete** incorporados al binario **o un tema descubierto en `CONTENT_DIR`**). NO SHALL leer ningún fichero de configuración como `config.yml`.

Una variable presente pero vacía NO SHALL confundirse con una ausente. Una variable numérica —`PORT`, `DEFAULT_ERROR_PAGE`, `RETRY_AFTER`— presente pero no interpretable SHALL abortar el arranque con un error que nombre la variable. Un `THEME` que no sea uno de los nombres del **registro** —los siete incorporados más los descubiertos en `CONTENT_DIR`—, o la cadena vacía, SHALL abortar el arranque con un error que nombre `THEME` y **enumere los nombres del registro**, en vez de degradar en caliente al tema por defecto. SHALL quedar escrito el **precio medido**: el error enumera los nombres que el registro **tenga en ese arranque**, así que un `THEME` correcto cuyo fichero **todavía** no esté en `CONTENT_DIR` —un volumen que aún no se ha montado— aborta el arranque; es la contrapartida de mantener el aborto, y va escrito para que no sorprenda.

Que el directorio de `CONTENT_DIR` no exista NO SHALL impedir el arranque, ni cambiar el `200` de un asset, ni degradar ninguna página de error a un `500`: las páginas embebidas cubren ese caso.

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

#### Scenario: sin THEME el tema por defecto es default
- **GIVEN** un entorno sin `THEME`
- **WHEN** la aplicación arranca
- **THEN** el tema SHALL ser `default`

#### Scenario: un THEME desconocido no arranca
- **GIVEN** `THEME=neon`
- **WHEN** la aplicación arranca
- **THEN** el arranque SHALL fallar con un error que nombre `THEME` y enumere los nombres del registro

#### Scenario: un THEME descubierto arranca
- **GIVEN** un `CONTENT_DIR` con `neon.html` y `THEME=neon`
- **WHEN** la aplicación arranca
- **THEN** SHALL arrancar
- **AND** el tema SHALL ser `neon`

#### Scenario: un THEME correcto con el volumen sin montar no arranca
- **GIVEN** `THEME=neon` y un `CONTENT_DIR` sin `neon.html`
- **WHEN** la aplicación arranca
- **THEN** SHALL fallar nombrando `THEME` y enumerando los nombres del registro
