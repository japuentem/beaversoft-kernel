# ============================================================================
# Project: Beaversoft Meta-Kernel
# Author: José Alberto Puente Maldonado (japuentem)
# Brand: Beaversoft
# Product: Beaver OS
# Description: Generador de Carrusel PDF para LinkedIn (Beaver OS v0.9.5)
#              8 Diapositivas HD (1080x1080) con los Nuevos Hitos de Arquitectura:
#              Demand Paging, Copy-on-Write, Señales POSIX, TLS 1.3, GPT y Audio DMA
# ============================================================================

import os
from PIL import Image, ImageDraw, ImageFont, ImageFilter

SLIDE_SIZE = (1080, 1080)
BG_COLOR = (11, 15, 25)           # #0B0F19 Deep Obsidian
GRID_DOT = (22, 30, 46)           # Subtle grid dot
CARD_BG = (17, 24, 39, 230)       # #111827 Dark Slate
CARD_BORDER = (31, 41, 55)        # Border
CYAN_ACCENT = (0, 229, 255)       # #00E5FF Neon Cyan
AMBER_ACCENT = (255, 179, 0)      # #FFB300 Amber Gold
GREEN_ACCENT = (34, 197, 94)      # #22C55E Emerald Success
TEXT_WHITE = (255, 255, 255)
TEXT_MUTED = (148, 163, 184)      # #94A3B8 Silver Muted
TEXT_DIM = (100, 116, 139)        # #64748B Dim Slate
CARD_CYAN_BORDER = (0, 229, 255, 120)
CARD_AMBER_BORDER = (255, 179, 0, 120)
CARD_GREEN_BORDER = (34, 197, 94, 120)

# Fonts Segoe UI (Standard on Windows, high legibility)
FONT_TITLE_HERO = ImageFont.truetype("segoeuib.ttf", 64)
FONT_TITLE_LARGE = ImageFont.truetype("segoeuib.ttf", 42)
FONT_TITLE_MED = ImageFont.truetype("segoeuib.ttf", 30)
FONT_CARD_TITLE = ImageFont.truetype("segoeuib.ttf", 24)
FONT_BODY = ImageFont.truetype("segoeui.ttf", 19)
FONT_BODY_BOLD = ImageFont.truetype("segoeuib.ttf", 19)
FONT_SMALL = ImageFont.truetype("segoeui.ttf", 16)
FONT_TAG = ImageFont.truetype("segoeuib.ttf", 15)
FONT_MONO = ImageFont.truetype("consolab.ttf", 18)

def get_circular_logo(size=(320, 320)):
    """Carga y procesa el logo oficial con máscara circular y resplandor."""
    raw = Image.open("assets/beaver_os_logo.png").convert("RGBA")
    w, h = raw.size
    mask = Image.new("L", (w, h), 0)
    d = ImageDraw.Draw(mask)
    d.ellipse([(4, 4), (w - 4, h - 4)], fill=255)
    mask = mask.filter(ImageFilter.GaussianBlur(1.5))
    raw.putalpha(mask)
    return raw.resize(size, Image.Resampling.LANCZOS)

def create_base_canvas():
    """Crea el fondo base con cuadrícula sutil y esquinas de circuito."""
    img = Image.new("RGBA", SLIDE_SIZE, BG_COLOR)
    draw = ImageDraw.Draw(img)
    
    for x in range(40, 1080, 40):
        for y in range(40, 1080, 40):
            draw.point((x, y), fill=GRID_DOT)
            
    draw.rectangle([(20, 20), (1060, 1060)], outline=(26, 36, 56), width=1)
    
    corner_len = 24
    draw.line([(20, 20), (20 + corner_len, 20)], fill=CYAN_ACCENT, width=2)
    draw.line([(20, 20), (20, 20 + corner_len)], fill=CYAN_ACCENT, width=2)
    draw.line([(1060 - corner_len, 20), (1060, 20)], fill=CYAN_ACCENT, width=2)
    draw.line([(1060, 20), (1060, 20 + corner_len)], fill=CYAN_ACCENT, width=2)
    draw.line([(20, 1060), (20 + corner_len, 1060)], fill=AMBER_ACCENT, width=2)
    draw.line([(20, 1060 - corner_len), (20, 1060)], fill=AMBER_ACCENT, width=2)
    draw.line([(1060 - corner_len, 1060), (1060, 1060)], fill=AMBER_ACCENT, width=2)
    draw.line([(1060, 1060 - corner_len), (1060, 1060)], fill=AMBER_ACCENT, width=2)
    
    return img

def draw_header(img, draw, topic_tag="SISTEMA OPERATIVO"):
    """Dibuja la barra de encabezado uniforme de Beaversoft."""
    tag_text = f"BEAVERSOFT // {topic_tag.upper()}"
    draw.text((60, 50), tag_text, font=FONT_TAG, fill=CYAN_ACCENT)
    
    brand_sub = "BEAVER OS v0.9.5 // 32 TESTS OK"
    w = draw.textlength(brand_sub, font=FONT_TAG)
    draw.text((1020 - w, 50), brand_sub, font=FONT_TAG, fill=GREEN_ACCENT)
    
    draw.line([(60, 78), (1020, 78)], fill=(30, 41, 59), width=1)

def draw_footer(img, draw, current_slide, total_slides=8):
    """Dibuja la barra de navegación inferior del carrusel."""
    draw.line([(60, 990), (1020, 990)], fill=(30, 41, 59), width=1)
    
    author_text = "Jose Alberto Puente Maldonado (@japuentem) • Beaversoft"
    draw.text((60, 1012), author_text, font=FONT_SMALL, fill=TEXT_MUTED)
    
    counter_text = f"{current_slide:02d} / {total_slides:02d}"
    draw.text((580, 1010), counter_text, font=FONT_TAG, fill=AMBER_ACCENT)
    
    swipe_text = "Desliza para ver más >>" if current_slide < total_slides else "¡Fin del recorrido!"
    sw = draw.textlength(swipe_text, font=FONT_TAG)
    draw.text((1020 - sw, 1010), swipe_text, font=FONT_TAG, fill=CYAN_ACCENT)

def draw_card(draw, bbox, tag, title, body, border_color=CARD_CYAN_BORDER, right_badge=None):
    """Dibuja una tarjeta con estilo HUD de cristal esmerilado."""
    x0, y0, x1, y1 = bbox
    draw.rounded_rectangle([(x0, y0), (x1, y1)], radius=12, fill=CARD_BG, outline=border_color, width=1)
    
    tw = draw.textlength(tag, font=FONT_TAG) + 16
    draw.rounded_rectangle([(x0 + 16, y0 + 16), (x0 + 16 + tw, y0 + 44)], radius=4, fill=(24, 34, 52), outline=border_color, width=1)
    draw.text((x0 + 24, y0 + 21), tag, font=FONT_TAG, fill=CYAN_ACCENT)
    
    draw.text((x0 + 36 + tw, y0 + 17), title, font=FONT_CARD_TITLE, fill=TEXT_WHITE)
    
    if right_badge:
        rw = draw.textlength(right_badge, font=FONT_TAG) + 16
        rx0 = x1 - 16 - rw
        draw.rounded_rectangle([(rx0, y0 + 16), (x1 - 16, y0 + 44)], radius=4, fill=(15, 23, 42), outline=AMBER_ACCENT, width=1)
        draw.text((rx0 + 8, y0 + 21), right_badge, font=FONT_TAG, fill=AMBER_ACCENT)
        
    draw.line([(x0 + 16, y0 + 56), (x1 - 16, y0 + 56)], fill=(30, 41, 59), width=1)
    
    words = body.split(" ")
    lines = []
    cur_line = ""
    max_w = (x1 - x0) - 36
    for word in words:
        test_line = f"{cur_line} {word}".strip()
        if draw.textlength(test_line, font=FONT_BODY) < max_w:
            cur_line = test_line
        else:
            if cur_line:
                lines.append(cur_line)
            cur_line = word
    if cur_line:
        lines.append(cur_line)
        
    line_y = y0 + 68
    for l in lines:
        draw.text((x0 + 20, line_y), l, font=FONT_BODY, fill=TEXT_MUTED)
        line_y += 26

def generate_slide_1():
    """Slide 1: Hero & Portada."""
    img = create_base_canvas()
    draw = ImageDraw.Draw(img)
    draw_header(img, draw, "HITOS DE INGENIERÍA BARE-METAL")
    
    # Badge destacado de estado
    draw.rounded_rectangle([(60, 110), (370, 146)], radius=8, fill=(15, 30, 48), outline=GREEN_ACCENT, width=1)
    draw.text((76, 118), "[ 32 / 32 PRUEBAS PASADAS ]", font=FONT_TAG, fill=GREEN_ACCENT)
    
    draw.text((60, 165), "BEAVER OS", font=FONT_TITLE_HERO, fill=TEXT_WHITE)
    draw.text((475, 165), "v0.9.5", font=FONT_TITLE_HERO, fill=AMBER_ACCENT)
    
    subtitle = "Meta-Kernel Determínico en Rust Bare-Metal (no_std)"
    draw.text((60, 245), subtitle, font=FONT_TITLE_MED, fill=CYAN_ACCENT)
    
    pitch = "Nuevos hitos: Paginación por Demanda, Copy-on-Write, Señales POSIX,\nCriptografía TLS 1.3 en usuario, Tablas UEFI GPT y Audio DMA sobre hardware real."
    draw.text((60, 295), pitch, font=FONT_BODY, fill=TEXT_MUTED)
    
    logo_circ = get_circular_logo((320, 320))
    glow = Image.new("RGBA", (380, 380), (0, 0, 0, 0))
    glow_draw = ImageDraw.Draw(glow)
    glow_draw.ellipse([(20, 20), (360, 360)], fill=(0, 229, 255, 45))
    glow_draw.ellipse([(45, 45), (335, 335)], outline=(0, 229, 255, 180), width=3)
    glow_blurred = glow.filter(ImageFilter.GaussianBlur(10))
    
    img.paste(glow_blurred, (350, 370), glow_blurred)
    img.paste(logo_circ, (380, 400), logo_circ)
        
    chips = [
        ("// MEMORIA", "Demand Paging & CoW (#PF 14)", (60, 790, 500, 860), CYAN_ACCENT),
        ("// POSIX", "Señales kill & sigaction", (520, 790, 1020, 860), AMBER_ACCENT),
        ("// CRYPTO", "TLS 1.3 & ChaCha20-Poly1305", (60, 880, 500, 950), GREEN_ACCENT),
        ("// STORAGE", "Particiones GPT & Protective MBR", (520, 880, 1020, 950), CYAN_ACCENT),
    ]
    for tag, text, (x0, y0, x1, y1), accent in chips:
        draw.rounded_rectangle([(x0, y0), (x1, y1)], radius=10, fill=CARD_BG, outline=accent, width=1)
        tw = draw.textlength(tag, font=FONT_TAG) + 14
        draw.rounded_rectangle([(x0 + 16, y0 + 16), (x0 + 16 + tw, y0 + 42)], radius=4, fill=(24, 34, 52), outline=accent, width=1)
        draw.text((x0 + 23, y0 + 20), tag, font=FONT_TAG, fill=accent)
        draw.text((x0 + 28 + tw, y0 + 17), text, font=FONT_CARD_TITLE, fill=TEXT_WHITE)
        
    draw_footer(img, draw, 1)
    return img

def generate_slide_2():
    """Slide 2: Paginación por Demanda & Copy-on-Write."""
    img = create_base_canvas()
    draw = ImageDraw.Draw(img)
    draw_header(img, draw, "GESTIÓN AVANZADA DE MEMORIA VIRTUAL")
    
    draw.text((60, 110), "Paginación por Demanda & Copy-on-Write (CoW)", font=FONT_TITLE_LARGE, fill=TEXT_WHITE)
    draw.text((60, 168), "Mapeo just-in-time y optimización física sin desperdicio de memoria física en Ring 3:", font=FONT_BODY, fill=TEXT_MUTED)
    
    draw_card(draw, (60, 220, 1020, 420),
              "[ DEMAND PAGING ]",
              "Asignación Just-in-Time (#PF 14 Ausente)",
              "Cuando un proceso de usuario accede a memoria virtual no presente (error_code & 1 == 0), el manejador de página en Ring 0 reserva un marco físico dinámico, lo blanquea con ceros por seguridad y lo mapea al instante. La CPU reintenta la instrucción con éxito transparente.",
              border_color=CARD_CYAN_BORDER, right_badge="Transparente")
              
    draw_card(draw, (60, 440, 1020, 640),
              "[ COPY-ON-WRITE ]",
              "Clonación Eficiente con PTE_COW_BIT (0x200)",
              "Al escribir sobre una página compartida de solo lectura, la MMU intercepta la violación (error_code 0x7). El kernel clona los 4096 bytes en un nuevo marco físico privado, habilita PAGE_WRITABLE e invalida el TLB con invlpg sin afectar a otros procesos.",
              border_color=CARD_AMBER_BORDER, right_badge="Zero-Copy")
              
    draw_card(draw, (60, 660, 1020, 860),
              "[ ANTI-BSOD ]",
              "Aislamiento de Segfaults Ilegales",
              "Intentos ilegales (desreferencias a puntero nulo o invasión al espacio de direcciones del kernel) son rechazados inmediatamente por el supervisor, conteniendo el proceso infractor sin desestabilizar el sistema.",
              border_color=CARD_GREEN_BORDER, right_badge="Inmune")
              
    draw.rounded_rectangle([(60, 880), (1020, 955)], radius=10, fill=(15, 30, 48), outline=CYAN_ACCENT, width=1)
    draw.text((80, 894), ">> IMPACTO TÉCNICO:", font=FONT_BODY_BOLD, fill=AMBER_ACCENT)
    draw.text((80, 920), "Binarios ELF grandes inician en milisegundos consumiendo solo las páginas que realmente tocan.", font=FONT_BODY, fill=TEXT_WHITE)
    
    draw_footer(img, draw, 2)
    return img

def generate_slide_3():
    """Slide 3: Señales Asíncronas POSIX."""
    img = create_base_canvas()
    draw = ImageDraw.Draw(img)
    draw_header(img, draw, "CONTROL DE PROCESOS ESTÁNDAR POSIX")
    
    draw.text((60, 110), "Señales Asíncronas POSIX en Espacio de Usuario", font=FONT_TITLE_LARGE, fill=TEXT_WHITE)
    draw.text((60, 168), "Control de ciclo de vida, captura de excepciones e interrupciones entre procesos:", font=FONT_BODY, fill=TEXT_MUTED)
    
    draw_card(draw, (60, 220, 1020, 420),
              "[ POSIX SIGNALS ]",
              "Semántica y Tabla Estándar Conforme",
              "Soporte completo para SIGHUP (1), SIGINT (2), SIGQUIT (3), SIGKILL (9), SIGSEGV (11), SIGTERM (15) y SIGSTOP (19). Cola de señales pendientes y máscaras de filtrado por hilo de ejecución.",
              border_color=CARD_CYAN_BORDER, right_badge="Estándar")
              
    draw_card(draw, (60, 440, 1020, 640),
              "[ RIGOR POSIX ]",
              "Inmunidad Absoluta de SIGKILL y SIGSTOP",
              "En cumplimiento estricto de la norma POSIX internacional, el kernel rechaza con -EINVAL (-22) cualquier intento de un proceso de capturar o ignorar SIGKILL o SIGSTOP. El administrador conserva control soberano ineludible.",
              border_color=CARD_AMBER_BORDER, right_badge="Ineludible")
              
    draw_card(draw, (60, 660, 1020, 860),
              "[ PASARELA SYSCALL ]",
              "kill(62), sigaction(13) & sigprocmask(14)",
              "Syscalls nativas enlazadas a beaver-libc para consultar existencia de procesos (sig=0), entrega selectiva de señales, manipulación de máscaras atómicas y retorno seguro vía sigreturn.",
              border_color=CARD_GREEN_BORDER, right_badge="beaver-libc")
              
    draw.rounded_rectangle([(60, 880), (1020, 955)], radius=10, fill=(15, 30, 48), outline=AMBER_ACCENT, width=1)
    draw.text((80, 894), ">> RESILIENCIA DE PROCESOS:", font=FONT_BODY_BOLD, fill=CYAN_ACCENT)
    draw.text((80, 920), "Los programas en Ring 3 pueden gestionar Ctrl+C, alarmas y caídas de forma elegante y asíncrona.", font=FONT_BODY, fill=TEXT_WHITE)
    
    draw_footer(img, draw, 3)
    return img

def generate_slide_4():
    """Slide 4: TLS 1.3 & Criptografía."""
    img = create_base_canvas()
    draw = ImageDraw.Draw(img)
    draw_header(img, draw, "CRIPTOGRAFÍA SOBERANA Y SEGURIDAD")
    
    draw.text((60, 110), "Pila Criptográfica TLS 1.3 & Entropía", font=FONT_TITLE_LARGE, fill=TEXT_WHITE)
    draw.text((60, 168), "Comunicaciones de red cifradas y aleatoriedad criptográfica en espacio de usuario:", font=FONT_BODY, fill=TEXT_MUTED)
    
    draw_card(draw, (60, 220, 1020, 420),
              "[ SYS_GETRANDOM ]",
              "Entropía CSPRNG de Hardware (Syscall 318)",
              "Generador criptográficamente seguro (NIST SP 800-90A) que amalgama ciclos de CPU de ultra-alta frecuencia (rdtsc), reloj RTC de hardware y ticks del sistema mediante digestión SHA-256 sin dependencia de librerías externas.",
              border_color=CARD_CYAN_BORDER, right_badge="NIST CSPRNG")
              
    draw_card(draw, (60, 440, 1020, 640),
              "[ RFC 8446 ]",
              "Registro ClientHello TLS 1.3 en Espacio de Usuario",
              "Construcción nativa del paquete ClientHello RFC 8446 con 32 bytes de Client Random, Server Name Indication (SNI) y Supported Versions (0x0304) para negociación directa con servidores web seguros.",
              border_color=CARD_AMBER_BORDER, right_badge="RFC 8446")
              
    draw_card(draw, (60, 660, 1020, 860),
              "[ RFC 8439 ]",
              "Cifrador de Flujo ChaCha20-Poly1305",
              "Cifrado simétrico autenticado (AEAD) operando a máxima velocidad de instrucción en procesadores modernos sobre sockets TCP reales de beaver-libc.",
              border_color=CARD_GREEN_BORDER, right_badge="ChaCha20")
              
    draw.rounded_rectangle([(60, 880), (1020, 955)], radius=10, fill=(15, 30, 48), outline=GREEN_ACCENT, width=1)
    draw.text((80, 894), ">> SEGURIDAD NATIVA:", font=FONT_BODY_BOLD, fill=AMBER_ACCENT)
    draw.text((80, 920), "Conexiones HTTPS verídicas y cifrado soberano sin cajas negras ni certificados opacos.", font=FONT_BODY, fill=TEXT_WHITE)
    
    draw_footer(img, draw, 4)
    return img

def generate_slide_5():
    """Slide 5: Persistencia UEFI GPT y Doble Slot A/B."""
    img = create_base_canvas()
    draw = ImageDraw.Draw(img)
    draw_header(img, draw, "ALMACENAMIENTO MASIVO RESILIENTE")
    
    draw.text((60, 110), "Tablas de Particiones GPT Nativas & A/B", font=FONT_TITLE_LARGE, fill=TEXT_WHITE)
    draw.text((60, 168), "Lectura, validación de integridad CRC32 y formateo físico sector a sector en disco:", font=FONT_BODY, fill=TEXT_MUTED)
    
    draw_card(draw, (60, 220, 1020, 420),
              "[ LBA 0 ]",
              "Protective MBR con Tipo 0xEE",
              "Estructura MBR de protección en sector 0 con firma 0x55AA y tipo 0xEE, impidiendo que utilidades de disco heredadas dañen o sobrescriban el particionado moderno.",
              border_color=CARD_CYAN_BORDER, right_badge="Protección")
              
    draw_card(draw, (60, 440, 1020, 640),
              "[ LBA 1 .. 33 ]",
              "Cabecera GPT Principal e Integridad CRC32",
              "Firma 'EFI PART', validación matemática estricta de checksum IEEE 802.3 CRC32 de la cabecera y del array de 128 particiones. Decodificación verídica de nombres en UTF-16LE.",
              border_color=CARD_AMBER_BORDER, right_badge="CRC32")
              
    draw_card(draw, (60, 660, 1020, 860),
              "[ TOLERANCIA A FALLOS ]",
              "Respaldo A/B Reubicado sin Colisiones",
              "El sector de espejo de arranque BeaverBootRecord se reubicó al sector 20446, erradicando colisiones con las entradas GPT y asegurando arranque dual tolerante a fallos de energía.",
              border_color=CARD_GREEN_BORDER, right_badge="Anti-Brickeo")
              
    draw.rounded_rectangle([(60, 880), (1020, 955)], radius=10, fill=(15, 30, 48), outline=CYAN_ACCENT, width=1)
    draw.text((80, 894), ">> PERSISTENCIA REAL:", font=FONT_BODY_BOLD, fill=AMBER_ACCENT)
    draw.text((80, 920), "BeaverFS y la partición EFI conviven armónicamente con compatibilidad UEFI estándar.", font=FONT_BODY, fill=TEXT_WHITE)
    
    draw_footer(img, draw, 5)
    return img

def generate_slide_6():
    """Slide 6: Multimedia, Audio DMA & Controladores."""
    img = create_base_canvas()
    draw = ImageDraw.Draw(img)
    draw_header(img, draw, "MULTIMEDIA Y CONTROLADORES DE HARDWARE")
    
    draw.text((60, 110), "Audio PCM DMA, xHCI USB 3.0 & Reloj RTC", font=FONT_TITLE_LARGE, fill=TEXT_WHITE)
    draw.text((60, 168), "Control directo sobre el silicio sin capas de abstracción innecesarias ni buffers lentos:", font=FONT_BODY, fill=TEXT_MUTED)
    
    draw_card(draw, (60, 220, 1020, 420),
              "[ AUDIO DMA ]",
              "Reproducción PCM en Ring 3 (Syscalls 130-132)",
              "Pasarela de llamadas de audio para transmitir sonido estéreo de 16 bits directamente a los descriptores DMA de Intel HDA y AC'97, con autodetección de cabeceras RIFF WAVE.",
              border_color=CARD_CYAN_BORDER, right_badge="DMA Lineal")
              
    draw_card(draw, (60, 440, 1020, 640),
              "[ USB 3.0 XHCI ]",
              "Controlador de Host xHCI con Hub y Teclado",
              "Inicialización de registros operacionales xHCI, asignación de ranuras de dispositivos (Device Slots) y recepción de eventos HID desde periféricos USB de alta velocidad.",
              border_color=CARD_AMBER_BORDER, right_badge="xHCI Real")
              
    draw_card(draw, (60, 660, 1020, 860),
              "[ HARDWARE RTC ]",
              "Reloj de Tiempo Real CMOS & PL031",
              "Lectura física atómica de fecha y hora desde chips CMOS en x86_64 y MMIO PL031 en ARM64, con conversión biyectiva exacta a la época Unix (SYS_GETTIMEOFDAY / SYS_CLOCK_GETTIME).",
              border_color=CARD_GREEN_BORDER, right_badge="POSIX Time")
              
    draw.rounded_rectangle([(60, 880), (1020, 955)], radius=10, fill=(15, 30, 48), outline=AMBER_ACCENT, width=1)
    draw.text((80, 894), ">> EXPERIENCIA COMPLETA:", font=FONT_BODY_BOLD, fill=CYAN_ACCENT)
    draw.text((80, 920), "Sonido nativo, entrada USB fluida y sincronización temporal precisa en tiempo real.", font=FONT_BODY, fill=TEXT_WHITE)
    
    draw_footer(img, draw, 6)
    return img

def generate_slide_7():
    """Slide 7: Certificación y 32 Pruebas Automatizadas."""
    img = create_base_canvas()
    draw = ImageDraw.Draw(img)
    draw_header(img, draw, "VERIFICACIÓN CONTINUA Y HONESTIDAD TÉCNICA")
    
    draw.text((60, 110), "32 Pruebas Automatizadas: Cero Simulación", font=FONT_TITLE_LARGE, fill=TEXT_WHITE)
    draw.text((60, 168), "Principio de Veracidad Beaversoft: Cada cambio técnico cuenta con una prueba verificada:", font=FONT_BODY, fill=TEXT_MUTED)
    
    draw_card(draw, (60, 220, 1020, 420),
              "[ REGLA 5 STRICT ]",
              "Prohibición Terminante de Datos Simulados o Mocks",
              "Queda estrictamente prohibido fingir subsistemas. Toda prueba mide registros físicos, tablas de páginas reales, paquetes de red transmitidos y sectores físicos en disco.",
              border_color=CARD_GREEN_BORDER, right_badge="Veracidad")
              
    draw_card(draw, (60, 440, 1020, 640),
              "[ 32 / 32 SUITE OK ]",
              "Batería Completa en QEMU (Salida 0xF4 -> 33)",
              "Desde sincronización SMP y locks reentrantes hasta particionado GPT, Copy-on-Write, sockets TCP y TLS 1.3: el 100% de la batería de pruebas finaliza exitosamente.",
              border_color=CARD_CYAN_BORDER, right_badge="100% Pass")
              
    draw_card(draw, (60, 660, 1020, 860),
              "[ CI DUAL ]",
              "Validación Bi-Arquitectura (x86_64 y ARM64)",
              "Ejecución automatizada mediante 'run.ps1 -Test' en x86_64 y 'run.ps1 -TestArm64' en ARM64 (Cortex-A72 con GICv2, Generic Timer, MMU, VFS y Shell).",
              border_color=CARD_AMBER_BORDER, right_badge="Multi-Arch")
              
    draw.rounded_rectangle([(60, 880), (1020, 955)], radius=10, fill=(15, 30, 48), outline=GREEN_ACCENT, width=1)
    draw.text((80, 894), ">> INGENIERÍA COMPROBABLE:", font=FONT_BODY_BOLD, fill=AMBER_ACCENT)
    draw.text((80, 920), "Cada línea de código es verificada y respaldada por ejecución de hardware real.", font=FONT_BODY, fill=TEXT_WHITE)
    
    draw_footer(img, draw, 7)
    return img

def generate_slide_8():
    """Slide 8: Hoja de Ruta y Expansión Futura."""
    img = create_base_canvas()
    draw = ImageDraw.Draw(img)
    draw_header(img, draw, "HOJA DE RUTA Y EXPANSIÓN FUTURA (pending/)")
    
    draw.text((60, 110), "Hacia Dónde Vamos: Los Próximos Horizontes", font=FONT_TITLE_LARGE, fill=TEXT_WHITE)
    draw.text((60, 168), "Con los fundamentos del meta-kernel concluidos, el foco se expande hacia:", font=FONT_BODY, fill=TEXT_MUTED)
    
    draw_card(draw, (60, 220, 1020, 435),
              "[ PENDING // 01 ]",
              "Aceleración Gráfica 3D por Hardware (VirtIO-GPU / DRM / KMS)",
              "Transición del actual framebuffer lineal a renderizado acelerado por shader con colas de comandos VirtIO-GPU, gestión de memoria compartida DMA-BUF / GEM y sincronización vertical VSync a 60 FPS estables a 4K.",
              border_color=CARD_CYAN_BORDER, right_badge="VirtIO-GPU")
              
    draw_card(draw, (60, 455, 1020, 670),
              "[ PENDING // 02 ]",
              "Portabilidad a Arquitectura RISC-V 64 (RV64GC)",
              "Ampliación del meta-kernel hacia hardware libre: soporte para OpenSBI v0.2+, controlador de interrupciones PLIC, traducción de memoria Sv39/Sv48 y pruebas en placas StarFive / SiFive reales.",
              border_color=CARD_AMBER_BORDER, right_badge="RISC-V 64")
              
    # Tarjeta de cierre / Call to Action
    draw.rounded_rectangle([(60, 695), (1020, 865)], radius=12, fill=(17, 24, 39), outline=GREEN_ACCENT, width=2)
    draw.text((90, 715), "¡UN SISTEMA OPERATIVO CONSTRUIDO DESDE CERO!", font=FONT_CARD_TITLE, fill=GREEN_ACCENT)
    draw.text((90, 755), "Beaver OS demuestra que es posible rediseñar la computación de base con seguridad de", font=FONT_BODY, fill=TEXT_WHITE)
    draw.text((90, 782), "memoria en Rust, resiliencia anti-caídas y transparencia total para el usuario.", font=FONT_BODY, fill=TEXT_WHITE)
    draw.text((90, 815), "Desarrollado con pasión por José Alberto Puente Maldonado (@japuentem) para Beaversoft.", font=FONT_BODY_BOLD, fill=AMBER_ACCENT)
    
    draw.rounded_rectangle([(60, 885), (1020, 955)], radius=10, fill=(15, 30, 48), outline=CYAN_ACCENT, width=1)
    draw.text((80, 898), ">> ÚNETE A LA REVOLUCIÓN:", font=FONT_BODY_BOLD, fill=CYAN_ACCENT)
    draw.text((80, 924), "Dale me gusta, comparte y sigue el desarrollo en GitHub: github.com/japuentem/beaver-kernel", font=FONT_BODY, fill=TEXT_WHITE)
    
    draw_footer(img, draw, 8)
    return img

def main():
    print("[1/3] Generando 8 diapositivas HD (1080x1080) para LinkedIn Carousel...")
    slides = [
        generate_slide_1(),
        generate_slide_2(),
        generate_slide_3(),
        generate_slide_4(),
        generate_slide_5(),
        generate_slide_6(),
        generate_slide_7(),
        generate_slide_8(),
    ]
    
    os.makedirs("docs/carousel_v0.9.5_slides", exist_ok=True)
    
    rgb_slides = []
    for idx, slide in enumerate(slides, start=1):
        slide_path = f"docs/carousel_v0.9.5_slides/slide_{idx:02d}.png"
        slide.save(slide_path, format="PNG")
        print(f"  -> Guardada diapositiva {idx}: {slide_path}")
        
        rgb_slide = Image.new("RGB", slide.size, BG_COLOR)
        rgb_slide.paste(slide, mask=slide.split()[3])
        rgb_slides.append(rgb_slide)
        
    print("[2/3] Ensamblando PDF multipágina de alta calidad para LinkedIn...")
    pdf_path = "docs/beaver_os_v0.9.5_linkedin_carousel.pdf"
    
    rgb_slides[0].save(
        pdf_path,
        "PDF",
        resolution=150.0,
        save_all=True,
        append_images=rgb_slides[1:]
    )
    print(f"[3/3] ¡Carrusel PDF completado con éxito! Archivo: {pdf_path}")
    
    file_size = os.path.getsize(pdf_path)
    print(f"Tamaño total del PDF: {file_size / (1024 * 1024):.2f} MB")

if __name__ == "__main__":
    main()

