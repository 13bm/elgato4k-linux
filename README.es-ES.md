# elgato4k-linux

**Utilidad de control no oficial para Linux para tarjetas de captura Elgato 4K**

Una herramienta de línea de comandos para controlar las tarjetas de captura Elgato 4K X y 4K S en Linux. Soporta mapeo de tonos HDR, ajuste del rango de color HDMI, selección de fuente EDID, configuración de EDID personalizada, cambio de velocidad de USB, selección de entrada de audio y control del escalador de vídeo.

> ⚠️ **Descargo de responsabilidad**: Esta es una herramienta no oficial desarrollada por la comunidad. No está afiliada, respaldada ni soportada por Elgato/Corsair.

## Características

- ✅ **Mapeo de tonos HDR** - Activar/desactivar el mapeo de tonos de HDR a SDR
- ✅ **Rango de color HDMI** - Corregir desajustes del rango de color (Auto/Expand/Shrink)
- ✅ **Control de fuente EDID** - Seleccionar modo EDID (Display/Merged/Internal)
- ✅ **EDID Personalizado** - Activar/desactivar preajuste de EDID personalizado (solo 4K X)
- ✅ **Cambio de velocidad USB** - Cambiar entre modos de 5Gbps y 10Gbps (solo 4K X)
- ✅ **Selección de entrada de audio** - Cambiar entre audio HDMI embebido y analógico (solo 4K S)
- ✅ **Escalador de vídeo** - Activar/desactivar el escalado de vídeo (solo 4K S)
- ✅ **Versión de Firmware** - Leer la versión del firmware del dispositivo
- ✅ **Lectura de estado** - Leer la configuración actual tanto de la 4K X como de la 4K S
- ✅ **Auto-detección** - Detecta automáticamente 4K X o 4K S en todos los modos de velocidad USB

## Dispositivos Soportados

| Dispositivo | USB VID:PID | Modo de Velocidad | Protocolo | Estado |
|------------|-------------|------------------|-----------|--------|
| Elgato 4K X | `0fd9:009b` | 10Gbps / SuperSpeed+ (USB 3.2) | UVC Extension Unit | ✅ Soportado |
| Elgato 4K X | `0fd9:009c` | 5Gbps / SuperSpeed (USB 3.1) | UVC Extension Unit | ✅ Totalmente probado |
| Elgato 4K X | `0fd9:009d` | USB 2.0 | UVC Extension Unit | ✅ Soportado |
| Elgato 4K S | `0fd9:00af` | USB 3.0 | HID Output Reports | ✅ Totalmente probado |
| Elgato 4K S | `0fd9:00ae` | USB 2.0 | HID Output Reports | ✅ Soportado |

> **Nota:** La 4K X cambia su PID dependiendo del modo de velocidad USB activo. Si su dispositivo muestra un PID diferente al esperado, es probable que esté en un modo de velocidad distinto. Use `--usb-speed` para cambiar los modos.

## Instalación

### Prrequisitos

- Toolchain de Rust (instalar desde [rustup.rs](https://rustup.rs))
- Archivos de desarrollo de libusb
- Acceso root/sudo (o reglas udev)

**Instalar dependencias:**

```bash
# Ubuntu/Debian
sudo apt-get install libusb-1.0-0-dev

# Fedora/RHEL
sudo dnf install libusb1-devel

# Arch
sudo pacman -S libusb
```

### Compilar desde el código fuente

```bash
# Clonar el repositorio
git clone https://github.com/13bm/elgato4k-linux.git
cd elgato4k-linux

# Compilar binario de release
cargo build --release

# Instalar en el sistema (opcional)
sudo cp target/release/elgato4k-linux /usr/local/bin/
```

### Nota sobre el Modo 10Gbps (PID 009b)

Si su 4K X está en modo 10Gbps (PID `009b`) y su kernel no la reconoce, la solución más sencilla es cambiar al modo 5Gbps:

```bash
sudo elgato4k-linux --usb-speed 5g
```

Esto cambia el dispositivo al PID `009c`, que funciona en todos los kernels. 5Gbps es suficiente para la mayoría de los escenarios de captura.

Si necesita específicamente el modo 10Gbps, puede aplicar el quirk del kernel `USB_QUIRK_NO_BOS` ([enviado upstream](https://lore.kernel.org/linux-usb/20251207090220.14807-1-johannes.bruederl@gmail.com/) por el usuario de Reddit [birdayz](https://reddit.com/u/birdayz)) como parámetro de arranque:

```bash
# Añadir a GRUB_CMDLINE_LINUX en /etc/default/grub:
usbcore.quirks=0fd9:009b:o
```

Luego ejecute `sudo update-grub` y reinicie.

## Uso

### Comandos Básicos

```bash
# Activar mapeo de tonos HDR
sudo elgato4k-linux --hdr-map on

# Establecer rango de color HDMI en Full (expandir rango limitado)
sudo elgato4k-linux --hdmi-range expand

# Establecer fuente EDID en passthrough de pantalla
sudo elgato4k-linux --edid-source display

# Activar preajuste de EDID personalizado (solo 4K X)
sudo elgato4k-linux --custom-edid on

# Cambiar velocidad de USB a 10Gbps (solo 4K X)
sudo elgato4k-linux --usb-speed 10g

# Cambiar entrada de audio a analógica (solo 4K S)
sudo elgato4k-linux --audio-input analog

# Activar escalador de vídeo (solo 4K S)
sudo elgato4k-linux --video-scaler on

# Combinar múltiples ajustes
sudo elgato4k-linux --hdr-map on --hdmi-range expand --edid-source display

# Leer ajustes actuales del dispositivo
sudo elgato4k-linux --status

# Leer versión del firmware
sudo elgato4k-linux --firmware-version
```

### Referencia de Comandos

#### `--hdmi-range <VALUE>` / `--edid-range <VALUE>`
Establece el rango de color HDMI (política de rango EDID):
- `auto` - Respeta el EDID (por defecto/más seguro)
- `expand` - Convierte Rango Limitado (16-235) a Completo (0-255) - corrige colores lavados
- `shrink` - Convierte Rango Completo (0-255) a Limitado (16-235) - corrige negros aplastados

#### `--edid-source <VALUE>`
Selecciona el modo EDID. Esto controla qué información EDID presenta la tarjeta de captura al dispositivo fuente:
- `display` - **Passthrough** del EDID de su monitor al dispositivo fuente. Ideal cuando quiere que la fuente coincida con las capacidades reales de su monitor (resolución, soporte HDR, etc.)
- `merged` - EDID **combinado/negociado** de todas las pantallas conectadas. La tarjeta de captura fusiona las capacidades para encontrar un conjunto común
- `internal` - EDID **integrado** del firmware de la tarjeta de captura. Úselo cuando el dispositivo fuente no funcione bien con el EDID de su monitor, o cuando no haya ningún monitor conectado

#### `--hdr-map <VALUE>`
Controla el mapeo de tonos HDR:
- `on` - Activar mapeo de tonos de HDR a SDR
- `off` - Desactivar mapeo de tonos (passthrough)

#### `--custom-edid <VALUE>` (solo 4K X)
Activa/desactiva un preajuste de EDID personalizado:
- `on` - Activar EDID personalizado (selecciona el índice de preajuste 1)
- `off` - Desactivar EDID personalizado

> **Nota:** Esto alterna un preajuste de EDID preconfigurado almacenado en el dispositivo. La carga de archivos EDID personalizados aún no está soportada en esta herramienta. El preajuste debe configurarse primero usando el software oficial de Elgato en Windows/macOS.

#### `--audio-input <VALUE>` (solo 4K S)
Selecciona la fuente de entrada de audio:
- `embedded` - Audio embebido en HDMI (por defecto)
- `analog` - Entrada de audio analógica/line-in

#### `--video-scaler <VALUE>` (solo 4K S)
Activa/desactiva el escalado de vídeo:
- `on` - Activar escalador de vídeo
- `off` - Desactivar escalador de vídeo

#### `--usb-speed <VALUE>` (solo 4K X)
Cambia el modo de velocidad USB:
- `5g` - 5Gbps (SuperSpeed, el PID cambia a `009c`)
- `10g` - 10Gbps (SuperSpeed+, el PID cambia a `009b`)

> **Advertencia:** El dispositivo se desconectará y volverá a enumerarse con un PID de USB diferente después de cambiar la velocidad. La ruta de su dispositivo USB cambiará y cualquier sesión de captura activa se interrumpirá.

#### `--status`
Lee y muestra los ajustes actuales del dispositivo.
- **4K X**: Versión de firmware, modo de velocidad USB, rango de color HDMI, mapeo de tonos HDR, política de rango EDID y selección de fuente EDID (mediante lecturas de UVC Extension Unit)
- **4K S**: Versión de firmware, mapeo de tonos HDR, rango de color HDMI, modo EDID, entrada de audio y estado del escalador de vídeo (mediante el protocolo HID ReadI2cData, descubierto mediante la decompilación de EGAVDeviceSupport.dll)

#### `--firmware-version`
Lee y muestra la versión del firmware del dispositivo.
- **4K X**: Utiliza el comando AT `0x77` (`AT_Get_Customer_Ver`) para consultar el chip ITE UB700E. Formato de versión: decimal empaquetado YYMMDD (ej. `25.02.10`)
- **4K S**: Utiliza el comando de lectura HID `0x55`/`0x02` para consultar el MCU. Formato de versión: DateThreeBytes BCD (ej. `25.0c.03`)

## Ejecutar sin sudo

Cree una regla udev para permitir que su usuario acceda al dispositivo:

```bash
# Crear archivo de regla udev
sudo nano /etc/udev/rules.d/99-elgato-capture.rules
```

Añada estas líneas:
```
# Elgato 4K X (todos los modos de velocidad)
SUBSYSTEM=="usb", ATTR{idVendor}=="0fd9", ATTR{idProduct}=="009b", MODE="0666", GROUP="plugdev"
SUBSYSTEM=="usb", ATTR{idVendor}=="0fd9", ATTR{idProduct}=="009c", MODE="0666", GROUP="plugdev"
SUBSYSTEM=="usb", ATTR{idVendor}=="0fd9", ATTR{idProduct}=="009d", MODE="0666", GROUP="plugdev"

# Elgato 4K S (todos los modos de velocidad)
SUBSYSTEM=="usb", ATTR{idVendor}=="0fd9", ATTR{idProduct}=="00af", MODE="0666", GROUP="plugdev"
SUBSYSTEM=="usb", ATTR{idVendor}=="0fd9", ATTR{idProduct}=="00ae", MODE="0666", GROUP="plugdev"
```

Recargue las reglas udev:
```bash
sudo udevadm control --reload-rules
sudo udevadm trigger
```

Añada su usuario al grupo plugdev:
```bash
sudo usermod -a -G plugdev $USER
```

Cierre sesión y vuelva a entrar para que los cambios surtan efecto.

## Detalles Técnicos

### Implementación del Protocolo

**Elgato 4K X (Protocolo UVC)**
- Utiliza la UVC Extension Unit #4 con GUID `961073c7-49f7-44f2-ab42-e940405940c2`
- Secuencia de dos paquetes: disparador (selector 0x02) + carga útil (selector 0x01)
- Interfaz 0 (VideoControl)
- Ajustes almacenados en el dispositivo, persisten tras reinicios
- Formateo de comandos AT para funciones avanzadas (cambio de velocidad USB)

**Elgato 4K S (Protocolo HID)**
- Utiliza solicitudes HID SET_REPORT/GET_REPORT en la Interfaz 7
- Paquetes de 255 bytes rellenos de ceros con encabezado `06 06 06 55`
- El byte de comando en el desplazamiento 5 selecciona la función (0x0a=HDR, 0x0b=rango de color, 0x08=audio, 0x12=EDID, 0x19=escalador)
- La mayoría de los ajustes requieren una secuencia de dos paquetes (comando + confirmación `0x13 0x01`)
- La capacidad de lectura utiliza el protocolo ReadI2cData: SET_REPORT `[06, 55, sub_cmd, length]` y luego GET_REPORT para recibir la respuesta
- Sub-comandos de lectura: 0x0a=estado HDR, 0x0b=rango de color, 0x08=entrada de audio, 0x12=modo EDID, 0x19=escalador, 0x02=versión de firmware
- Firmware del MCU: ARM Cortex-M0 (chip "Splitter" de ITE), despacho de comandos en la función del bucle principal

### Ingeniería Inversa

Todos los protocolos fueron revertidos mediante:
- Captura y análisis de paquetes USB (Wireshark/USBPcap)
- Decompilación con Ghidra de DLLs oficiales de Windows (RTK_IO_x64.dll, RTICE_SDK_x64.dll, EGAVDeviceSupport.dll)
- Decompilación con Ghidra del firmware del MCU de la 4K S (FW_4K_S_MCU.bin, ARM Cortex-M0)
- Análisis del protocolo de comandos AT del chip ITE UB700E
- Pruebas y validación manual

Consulte [LOW_CONFIDENCE_COMMANDS.md](LOW_CONFIDENCE_COMMANDS.md) para ver la lista completa de comandos de firmware descubiertos que aún no han sido implementados.

## Casos de Uso Comunes

### Corregir Colores Lavados (PS5/Xbox)
```bash
sudo elgato4k-linux --hdmi-range expand
```
Las consolas de juegos a menudo emiten en rango Limitado, pero las pantallas esperan rango Completo.

### Activar Captura HDR con Mapeo de Tonos
```bash
sudo elgato4k-linux --hdr-map on --hdmi-range auto
```
Convierte la entrada HDR a SDR para su visualización/grabación.

### Passthrough de Capacidades de Pantalla
```bash
sudo elgato4k-linux --edid-source display
```
Permite que el dispositivo fuente vea las capacidades reales de su pantalla.

### Usar Entrada de Audio Analógica (4K S)
```bash
sudo elgato4k-linux --audio-input analog
```
Cambia a la entrada de audio analógica/line-in en lugar del audio embebido en HDMI.

### Cambiar a 10Gbps para Máximo Rendimiento
```bash
sudo elgato4k-linux --usb-speed 10g
```
Habilita el modo SuperSpeed+ de 10Gbps para capturas de mayor ancho de banda. Requiere un puerto USB 3.2 Gen 2 y puede requerir el quirk del kernel (ver sección de instalación).

## Limitaciones Conocidas

- **Carga de EDID personalizado**: Aún no es posible cargar archivos EDID personalizados al dispositivo (solo alternar preajustes preconfigurados)
- **Actualizaciones de firmware**: No soportadas (use el software oficial)
- **Velocidad USB de 4K S**: La 4K S no soporta el cambio de velocidad USB
- **Audio/escalador de 4K S**: Los comandos de entrada de audio y escalador de vídeo fueron descubiertos vía Ghidra y necesitan pruebas de hardware
- **4K Pro/otros modelos**: Solo se soportan 4K X y 4K S
- **Información de señal**: Los tiempos de señal HDMI y los datos de infoframe HDR se pueden leer, pero aún no han sido decodificados (estructuras binarias complejas)

## Solución de Problemas

### Dispositivo no encontrado
```bash
# Verificar si el dispositivo está conectado
lsusb | grep 0fd9

# Ejemplos de salida esperada:
# Bus 002 Device 014: ID 0fd9:009b Elgato Systems GmbH   (4K X, 10Gbps)
# Bus 002 Device 014: ID 0fd9:009c Elgato Systems GmbH   (4K X, 5Gbps)
# Bus 002 Device 015: ID 0fd9:00af Elgato Systems GmbH   (4K S, USB 3.0)
```

### Permiso denegado
```bash
# Ejecutar con sudo
sudo elgato4k-linux --hdr-map on

# O configurar reglas udev (ver sección "Ejecutar sin sudo")
```

### Los ajustes no se aplican
- Asegúrese de que ningún otro software esté usando el dispositivo (OBS, etc.)
- Intente desconectar y volver a conectar el dispositivo
- Verifique que el dispositivo esté completamente inicializado (espere unos segundos después de conectarlo)

### Interrupción del flujo de vídeo
La herramienta desconecta brevemente el controlador del kernel para enviar comandos, lo que puede causar una interrupción momentánea en el software de captura de vídeo. El controlador se vuelve a conectar inmediatamente después de enviar los comandos.

### El modo 10Gbps no funciona
- Solución más fácil: cambiar a 5Gbps con `sudo elgato4k-linux --usb-speed 5g` (suficiente para la mayoría de los casos)
- Si necesita 10Gbps: asegúrese de que su puerto USB soporte USB 3.2 Gen 2
- Aplique el quirk del kernel si su kernel no reconoce el PID `009b` (ver sección de instalación)
- Verifique con `lsusb` que el dispositivo muestre el PID `009b`

## Agradecimientos

- [@RadioFreeKerbin](https://github.com/RadioFreeKerbin) — Descubrió los cambios de PID de la 4K X en los modos de velocidad USB
- [@AndySchroder](https://github.com/AndySchroder) — Señaló la necesidad de una documentación más clara sobre las opciones de EDID
- [@Tatsh2DX](https://www.reddit.com/u/Tatsh2DX) — Identificó la llamada sendATCommand(0x8e) para el cambio de velocidad USB en el binario de macOS.
- [konovalov-nk](https://www.reddit.com/u/konovalov-nk) — por su cómico viaje de sufrimiento habilitando los 10Gbps
- [birdayz](https://reddit.com/u/birdayz) (Johannes Brüderl) — Envió el [parche del kernel `USB_QUIRK_NO_BOS`](https://lore.kernel.org/linux-usb/20251207090220.14807-1-johannes.bruederl@gmail.com/) para el soporte del modo 10Gbps

## Contribuciones

¡Las contribuciones son bienvenidas! Áreas de mejora:
- Soporte para carga de archivos EDID personalizados
- Soporte para otros dispositivos USB de Elgato
- Envoltura (wrapper) GUI
- Ingeniería inversa de funciones adicionales

Dado que ya no tengo mi 4K S, las pruebas y la adición de funciones estarán limitadas. Si tiene una 4K S y puede hacer pruebas, ¡por favor abra un issue con sus resultados!

## Licencia

Este proyecto está bajo la [Licencia Pública General de GNU v3.0](LICENSE).

## Descargo de responsabilidad

Este proyecto no está afiliado, respaldado ni soportado por Elgato Systems GmbH o Corsair Gaming, Inc. Todos los nombres de productos, logotipos y marcas son propiedad de sus respectivos dueños.

Úselo bajo su propio riesgo. Los autores no se hacen responsables de cualquier daño al hardware o a los datos.

## Ver También

- [Sitio Web Oficial de Elgato](https://www.elgato.com)
- [OBS Studio](https://obsproject.com) - Software de streaming/grabación de código abierto
- [v4l2-ctl](https://www.mankier.com/1/v4l2-ctl) - Utilidad de control de Video4Linux
