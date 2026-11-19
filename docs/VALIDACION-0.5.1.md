# Validación de la versión preliminar 0.5.1

Preparación de publicación: 19 de noviembre de 2026.

## Verificación técnica

- 53 pruebas del perfil normal y 54 del perfil PRUEBA aprobadas.
- Las nueve combinaciones de intervalos, su guardado y restauración, la compatibilidad con configuraciones previas, el límite de dos horas y las pausas están cubiertos por las pruebas automáticas.
- Se verificó la geometría con pantallas simuladas, posiciones negativas, escalas distintas y Dock en varios bordes. También se comprobó la alineación de los pies en las dos animaciones.
- Se verificaron la versión, el perfil, los recursos, la arquitectura arm64 y la firma ad hoc de la aplicación preparada.
- El empaquetado comprueba el inventario del ZIP, lo extrae con una herramienta de macOS y vuelve a verificar la firma, los permisos del ejecutable y las huellas de todos sus archivos.
- Las fuentes se exportan mediante una lista explícita de archivos. Se excluyen preferencias, registros personales, credenciales, cachés y copias de desarrollo.

## Pruebas manuales informadas

En PRUEBA se confirmó que la animación aparece abajo a la derecha de la pantalla del cursor y que permanece allí aunque el cursor cambie de pantalla.

En 0.5.0 se validaron las dos animaciones completas, su primera pose, nitidez, escala y alineación; los íconos de la app y la barra de menú; y la persistencia del sonido con un aviso de agua audible. Una sesión de cuatro horas incluyó al menos dos avisos y un menú que respondió al final, sin problemas observados. Esa medición no se atribuye a 0.5.1.

## Pendiente

- Revisión visual de los menús de 30/60/120 y de su persistencia al reabrir 0.5.1.
- Prueba 8 de SPEC-003: inicio automático después de un cierre de sesión o reinicio real. Sigue pendiente a petición de la usuaria.
- Descargar e instalar esta entrega pública en otra Mac Apple Silicon.
- Validar otras versiones de macOS; 12.0 es el mínimo declarado.
- Firma Developer ID y notarización para distribución de macOS.

Durante la preparación de publicación no se inició la aplicación ni se hicieron capturas.
