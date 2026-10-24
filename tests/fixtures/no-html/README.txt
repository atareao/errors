Fixture del caso «un CONTENT_DIR sin ningún HTML» (auditoría I-2): este
directorio solo tiene un asset, así que ni `418.html` ni `404.html` existen y la
aplicación debe responder el código resuelto con el respaldo de texto, nunca un
`500`.
