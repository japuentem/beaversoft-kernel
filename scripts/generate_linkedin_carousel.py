# ============================================================================
# Project: Beaversoft Meta-Kernel
# Author: José Alberto Puente Maldonado (japuentem)
# Brand: Beaversoft
# Product: Beaver OS
# Description: Generador de Carrusel PDF para LinkedIn (Beaver OS v0.9.0)
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
TEXT_WHITE = (255, 255, 255)
TEXT_MUTED = (148, 163, 184)      # #94A3B8 Silver Muted
TEXT_DIM = (100, 116, 139)        # #64748B Dim Slate
CARD_CYAN_BORDER = (0, 229, 255, 120)
CARD_AMBER_BORDER = (255, 179, 0, 120)

# Fonts Segoe UI (Standard on Windows, high legibility)
FONT_TITLE_HERO = ImageFont.truetype("segoeuib.ttf", 66)
FONT_TITLE_LARGE = ImageFont.truetype("segoeuib.ttf", 44)
FONT_TITLE_MED = ImageFont.truetype("segoeuib.ttf", 32)
FONT_CARD_TITLE = ImageFont.truetype("segoeuib.ttf", 25)
FONT_BODY = ImageFont.truetype("segoeui.ttf", 20)
FONT_BODY_BOLD = ImageFont.truetype("segoeuib.ttf", 20)
FONT_SMALL = ImageFont.truetype("segoeui.ttf", 16)
FONT_TAG = ImageFont.truetype("segoeuib.ttf", 15)
FONT_MONO = ImageFont.truetype("consolab.ttf", 18)

def get_circular_logo(size=(340, 340)):
    """Carga y procesa el logo oficial con máscara circular y resplandor."""
    raw = Image.open("assets/beaver_os_logo.png").convert("RGBA")
    w, h = raw.size
    
    # Máscara circular antialiased
    mask = Image.new("L", (w, h), 0)
    d = ImageDraw.Draw(mask)
    d.ellipse([(4, 4), (w - 4, h - 4)], fill=255)
    mask = mask.filter(ImageFilter.GaussianBlur(1.5))
    raw.putalpha(mask)
    
    return raw.resize(size, Image.Resampling.LANCZOS)

def create_base_canvas():
    """Crea el fondo base con cuadrícula sutil y viñeta."""
    img = Image.new("RGBA", SLIDE_SIZE, BG_COLOR)
    draw = ImageDraw.Draw(img)
    
    # Dibujar sutil patrón de puntos de cuadrícula tecnológica (cada 40px)
    for x in range(40, 1080, 40):
        for y in range(40, 1080, 40):
            draw.point((x, y), fill=GRID_DOT)
            
    # Borde exterior de marco sutil
    draw.rectangle([(20, 20), (1060, 1060)], outline=(26, 36, 56), width=1)
    
    # Luces de esquina decorativas
    corner_len = 24
    # Top-Left
    draw.line([(20, 20), (20 + corner_len, 20)], fill=CYAN_ACCENT, width=2)
    draw.line([(20, 20), (20, 20 + corner_len)], fill=CYAN_ACCENT, width=2)
    # Top-Right
    draw.line([(1060 - corner_len, 20), (1060, 20)], fill=CYAN_ACCENT, width=2)
    draw.line([(1060, 20), (1060, 20 + corner_len)], fill=CYAN_ACCENT, width=2)
    # Bottom-Left
    draw.line([(20, 1060), (20 + corner_len, 1060)], fill=AMBER_ACCENT, width=2)
    draw.line([(20, 1060 - corner_len), (20, 1060)], fill=AMBER_ACCENT, width=2)
    # Bottom-Right
    draw.line([(1060 - corner_len, 1060), (1060, 1060)], fill=AMBER_ACCENT, width=2)
    draw.line([(1060, 1060 - corner_len), (1060, 1060)], fill=AMBER_ACCENT, width=2)
    
    return img

def draw_header(img, draw, topic_tag="SISTEMA OPERATIVO"):
    """Dibuja la barra de encabezado uniforme de Beaversoft."""
    tag_text = f"BEAVERSOFT // {topic_tag.upper()}"
    draw.text((60, 52), tag_text, font=FONT_TAG, fill=CYAN_ACCENT)
    
    brand_sub = "BEAVER OS v0.9.0"
    w = draw.textlength(brand_sub, font=FONT_TAG)
    draw.text((1020 - w, 52), brand_sub, font=FONT_TAG, fill=AMBER_ACCENT)
    
    draw.line([(60, 80), (1020, 80)], fill=(30, 41, 59), width=1)

def draw_footer(img, draw, current_slide, total_slides=7):
    """Dibuja la barra de navegación inferior del carrusel con indicador de progreso."""
    draw.line([(60, 990), (1020, 990)], fill=(30, 41, 59), width=1)
    
    author_text = "Jose Alberto Puente Maldonado (@japuentem) • Beaversoft"
    draw.text((60, 1012), author_text, font=FONT_SMALL, fill=TEXT_MUTED)
    
    counter_text = f"{current_slide:02d} / {total_slides:02d}"
    draw.text((580, 1010), counter_text, font=FONT_TAG, fill=AMBER_ACCENT)
    
    # Pildoras de progreso visual
    pill_start_x = 645
    pill_y = 1018
    for i in range(1, total_slides + 1):
        color = CYAN_ACCENT if i == current_slide else (40, 50, 70)
        width = 16 if i == current_slide else 8
        draw.rounded_rectangle([(pill_start_x, pill_y), (pill_start_x + width, pill_y + 4)], radius=2, fill=color)
        pill_start_x += width + 6

    if current_slide < total_slides:
        swipe_text = "Desliza >>"
        draw.text((940, 1010), swipe_text, font=FONT_TAG, fill=CYAN_ACCENT)
    else:
        cta_text = "Guardar y Compartir >>"
        w = draw.textlength(cta_text, font=FONT_TAG)
        draw.text((1020 - w, 1010), cta_text, font=FONT_TAG, fill=CYAN_ACCENT)

def draw_card(draw, box, badge_tag, title, body, border_color=CARD_BORDER, right_badge=None):
    """Dibuja una tarjeta técnica elegante con título y descripción."""
    x0, y0, x1, y1 = box
    draw.rounded_rectangle([x0, y0, x1, y1], radius=14, fill=CARD_BG, outline=border_color, width=1)
    
    # Pill de categoría en esquina izquierda
    pw = draw.textlength(badge_tag, font=FONT_TAG) + 16
    draw.rounded_rectangle([(x0 + 24, y0 + 18), (x0 + 24 + pw, y0 + 44)], radius=5, fill=(24, 34, 52), outline=border_color, width=1)
    draw.text((x0 + 32, y0 + 22), badge_tag, font=FONT_TAG, fill=CYAN_ACCENT if border_color == CARD_CYAN_BORDER else AMBER_ACCENT)
    
    # Título adyacente
    draw.text((x0 + 36 + pw, y0 + 18), title, font=FONT_CARD_TITLE, fill=TEXT_WHITE)
    
    # Badge opcional a la derecha
    if right_badge:
        bw = draw.textlength(right_badge, font=FONT_SMALL) + 16
        bx1 = x1 - 24
        bx0 = bx1 - bw
        draw.rounded_rectangle([(bx0, y0 + 18), (bx1, y0 + 44)], radius=6, fill=(30, 41, 59), outline=CYAN_ACCENT, width=1)
        draw.text((bx0 + 8, y0 + 21), right_badge, font=FONT_SMALL, fill=CYAN_ACCENT)
    
    draw.line([(x0 + 24, y0 + 58), (x1 - 24, y0 + 58)], fill=(30, 41, 59), width=1)
    
    words = body.split(" ")
    lines = []
    cur_line = ""
    max_w = (x1 - x0) - 48
    for word in words:
        test = cur_line + (" " if cur_line else "") + word
        if draw.textlength(test, font=FONT_BODY) <= max_w:
            cur_line = test
        else:
            if cur_line:
                lines.append(cur_line)
            cur_line = word
    if cur_line:
        lines.append(cur_line)
        
    line_y = y0 + 72
    for l in lines:
        draw.text((x0 + 24, line_y), l, font=FONT_BODY, fill=TEXT_MUTED)
        line_y += 28

def generate_slide_1():
    """Slide 1: Portada / Hook."""
    img = create_base_canvas()
    draw = ImageDraw.Draw(img)
    draw_header(img, draw, "LANZAMIENTO OFICIAL")
    
    # Badge destacado
    draw.rounded_rectangle([(60, 115), (320, 150)], radius=8, fill=(15, 30, 48), outline=CYAN_ACCENT, width=1)
    draw.text((76, 122), "[ RC1 ] RELEASE CANDIDATE", font=FONT_TAG, fill=CYAN_ACCENT)
    
    # Título Principal
    draw.text((60, 175), "BEAVER OS", font=FONT_TITLE_HERO, fill=TEXT_WHITE)
    draw.text((475, 175), "v0.9.0", font=FONT_TITLE_HERO, fill=AMBER_ACCENT)
    
    subtitle = "El Micro-Kernel Resiliente en Rust Bare-Metal"
    draw.text((60, 255), subtitle, font=FONT_TITLE_MED, fill=CYAN_ACCENT)
    
    pitch = "Construido desde cero para erradicar las pantallas azules, la memoria opaca\ny los cuelgues catastróficos mediante aislamiento por hardware real."
    draw.text((60, 310), pitch, font=FONT_BODY, fill=TEXT_MUTED)
    
    # Logo circular central con halo
    logo_circ = get_circular_logo((350, 350))
    glow = Image.new("RGBA", (410, 410), (0, 0, 0, 0))
    glow_draw = ImageDraw.Draw(glow)
    glow_draw.ellipse([(20, 20), (390, 390)], fill=(0, 229, 255, 45))
    glow_draw.ellipse([(45, 45), (365, 365)], outline=(0, 229, 255, 180), width=3)
    glow_blurred = glow.filter(ImageFilter.GaussianBlur(10))
    
    img.paste(glow_blurred, (335, 375), glow_blurred)
    img.paste(logo_circ, (365, 405), logo_circ)
        
    # 4 Chips de características clave al pie
    chips = [
        ("// RUST", "100% Rust Bare-Metal", (60, 795, 500, 865), CYAN_ACCENT),
        ("// RING3", "Ring 3 & MSR Syscalls", (520, 795, 1020, 865), AMBER_ACCENT),
        ("// DISK", "BeaverFS A/B Anti-Crash", (60, 885, 500, 955), AMBER_ACCENT),
        ("// NET", "Intel Gigabit TCP/IP", (520, 885, 1020, 955), CYAN_ACCENT),
    ]
    for tag, text, (x0, y0, x1, y1), accent in chips:
        draw.rounded_rectangle([(x0, y0), (x1, y1)], radius=10, fill=CARD_BG, outline=accent, width=1)
        tw = draw.textlength(tag, font=FONT_TAG) + 14
        draw.rounded_rectangle([(x0 + 16, y0 + 18), (x0 + 16 + tw, y0 + 44)], radius=4, fill=(24, 34, 52), outline=accent, width=1)
        draw.text((x0 + 23, y0 + 22), tag, font=FONT_TAG, fill=accent)
        draw.text((x0 + 28 + tw, y0 + 18), text, font=FONT_CARD_TITLE, fill=TEXT_WHITE)
        
    draw_footer(img, draw, 1)
    return img

def generate_slide_2():
    """Slide 2: El Problema de Fondo."""
    img = create_base_canvas()
    draw = ImageDraw.Draw(img)
    draw_header(img, draw, "EL PROBLEMA DE FONDO")
    
    draw.text((60, 115), "¿Por qué construir otro Sistema Operativo?", font=FONT_TITLE_LARGE, fill=TEXT_WHITE)
    draw.text((60, 175), "La arquitectura de los sistemas tradicionales arrastra 40 años de deuda técnica:", font=FONT_BODY, fill=TEXT_MUTED)
    
    draw_card(draw, (60, 230, 1020, 420),
              "[ FALLO ]",
              "Pantallas Azules (BSOD) y Kernel Panics",
              "En los núcleos monolíticos convencionales, un solo error de puntero o división entre cero dentro de un controlador de red, audio o GPU derriba todo el computador y congela la máquina.",
              border_color=(239, 68, 68, 140), right_badge="Fragilidad")
              
    draw_card(draw, (60, 445, 1020, 635),
              "[ OPACIDAD ]",
              "Memoria Opaca & OOM Incontrolable",
              "Los administradores de memoria tradicionales ocultan métricas reales con buffers fantasma. Cuando se agota la RAM, el OOM killer mata procesos sin criterio determinista, dañando la sesión de usuario.",
              border_color=(239, 68, 68, 140), right_badge="Opacidad")
              
    draw_card(draw, (60, 660, 1020, 850),
              "[ CRÍTICO ]",
              "Brickeo por Actualizaciones Fallidas",
              "Un corte de luz o apagado abrupto durante una actualización del sistema o escritura en el sector de arranque inutiliza por completo la instalación física, obligando a reinstalar.",
              border_color=(239, 68, 68, 140), right_badge="Riesgo de Datos")
              
    draw.rounded_rectangle([(60, 875), (1020, 955)], radius=12, fill=(15, 30, 48), outline=CYAN_ACCENT, width=1)
    draw.text((80, 892), ">> LA FILOSOFÍA BEAVERSOFT:", font=FONT_BODY_BOLD, fill=AMBER_ACCENT)
    draw.text((80, 918), "Tolerancia CERO a los fallos en cascada. Aislamiento estricto de privilegios por hardware.", font=FONT_BODY, fill=TEXT_WHITE)
    
    draw_footer(img, draw, 2)
    return img

def generate_slide_3():
    """Slide 3: Aislamiento Ring 3 & MSR Syscalls."""
    img = create_base_canvas()
    draw = ImageDraw.Draw(img)
    draw_header(img, draw, "ARQUITECTURA DE SEGURIDAD")
    
    draw.text((60, 115), "Aislamiento Real: Ring 3 & MSR Syscalls", font=FONT_TITLE_LARGE, fill=TEXT_WHITE)
    draw.text((60, 175), "Separación determinista entre el núcleo supervisor y las aplicaciones de usuario:", font=FONT_BODY, fill=TEXT_MUTED)
    
    draw_card(draw, (60, 230, 1020, 420),
              "[ PRIVILEGIOS ]",
              "Modo No Privilegiado (CPL = 3)",
              "El shell y las aplicaciones de usuario se ejecutan estrictamente en Ring 3. Las instrucciones sensibles de CPU y el acceso directo a puertos de I/O o memoria del kernel quedan bloqueados por hardware.",
              border_color=CARD_CYAN_BORDER, right_badge="CPL=3 Hardware")
              
    draw_card(draw, (60, 445, 1020, 635),
              "[ HARDWARE ]",
              "Transición MSR IA32_LSTAR",
              "Conmutación ultra-rápida mediante la instrucción 'syscall' nativa en hardware. Sin las ralentizaciones de las interrupciones lentas tipo 'int 0x80'. Pasarela compatible con llamadas POSIX.",
              border_color=CARD_CYAN_BORDER, right_badge="0xC0000082")
              
    draw_card(draw, (60, 660, 1020, 830),
              "[ BLINDAJE ]",
              "Inmunidad Absoluta contra Crashes",
              "Si un binario en espacio de usuario genera un General Protection Fault (#GP), desborde o página inválida, el kernel supervisor aísla el hilo y recupera el control sin reiniciar el sistema.",
              border_color=CARD_AMBER_BORDER, right_badge="Anti-Panic")
              
    draw.rounded_rectangle([(60, 855), (1020, 955)], radius=10, fill=(15, 23, 42), outline=(51, 65, 85), width=1)
    draw.text((80, 872), "// Registro del MSR Syscall Gateway en Beaver OS (Rust bare-metal):", font=FONT_SMALL, fill=AMBER_ACCENT)
    draw.text((80, 902), "wrmsr(IA32_LSTAR, syscall_entry as u64);  // Conmutador atómico Ring 3 -> Ring 0", font=FONT_MONO, fill=CYAN_ACCENT)
    
    draw_footer(img, draw, 3)
    return img

def generate_slide_4():
    """Slide 4: BeaverFS & Resiliencia."""
    img = create_base_canvas()
    draw = ImageDraw.Draw(img)
    draw_header(img, draw, "SISTEMA DE ARCHIVOS")
    
    draw.text((60, 115), "Almacenamiento Resiliente: BeaverFS", font=FONT_TITLE_LARGE, fill=TEXT_WHITE)
    draw.text((60, 175), "Diseñado para sobrevivir a desconexiones violentas y asegurar datos intactos:", font=FONT_BODY, fill=TEXT_MUTED)
    
    draw_card(draw, (60, 230, 1020, 435),
              "[ RESILIENCIA ]",
              "Arquitectura Redundante de Doble Slot A/B",
              "El sistema de arranque opera con dos slots de partición espejo (A y B). Cualquier actualización se aplica sobre el slot inactivo; si la verificación de paridad falla, el sistema regresa al slot estable.",
              border_color=CARD_CYAN_BORDER, right_badge="Doble Slot A/B")
              
    draw_card(draw, (60, 460, 1020, 665),
              "[ INTEGRIDAD ]",
              "Algoritmo Zero-Sector Parity Auto-Recovery",
              "Si un corte abrupto de energía interrumpe una transacción en el disco físico, el mecanismo de comprobación de paridad de Sector Cero detecta la inconsistencia y repara la tabla de inodos en el siguiente arranque.",
              border_color=CARD_AMBER_BORDER, right_badge="Auto-Reparación")
              
    draw_card(draw, (60, 690, 1020, 895),
              "[ CONTROLADOR ]",
              "Driver Nativo AHCI SATA con DMA",
              "Cero capas BIOS lentas. Controlador AHCI directo configurado sobre el bus PCI, gestionando transferencias masivas de memoria física mediante listas de descriptores PRDT a velocidad de bus SATA.",
              border_color=CARD_CYAN_BORDER, right_badge="DMA Directo")
              
    draw.rounded_rectangle([(60, 915), (1020, 960)], radius=8, fill=(17, 24, 39), outline=(31, 41, 55), width=1)
    draw.text((80, 926), ">> BeaverFS garantiza que jamás se produzca un estado corrupto irrecuperable en disco.", font=FONT_BODY, fill=TEXT_WHITE)
    
    draw_footer(img, draw, 4)
    return img

def generate_slide_5():
    """Slide 5: Networking Real (BeaverNet)."""
    img = create_base_canvas()
    draw = ImageDraw.Draw(img)
    draw_header(img, draw, "REDES DETERMINISTAS")
    
    draw.text((60, 115), "Conectividad Real: Intel Gigabit & TCP/IP", font=FONT_TITLE_LARGE, fill=TEXT_WHITE)
    draw.text((60, 175), "Pila de comunicaciones escrita en Rust bare-metal sin mocks ni simulaciones:", font=FONT_BODY, fill=TEXT_MUTED)
    
    draw_card(draw, (60, 230, 1020, 435),
              "[ HARDWARE ]",
              "Controlador Bare-Metal Intel 82540EM (e1000)",
              "Controlador Gigabit nativo. Manipulación directa de registros MMIO en bus PCI con anillos circulares de descriptores de transmisión (Tx) y recepción (Rx) sincronizados con interrupciones del procesador.",
              border_color=CARD_CYAN_BORDER, right_badge="Zero Mocks")
              
    draw_card(draw, (60, 460, 1020, 665),
              "[ PROTOCOLO ]",
              "Pila de Red Completa BeaverNet",
              "Implementación determinista de Ethernet II, resolución ARP en broadcast, direccionamiento IPv4, paquetes ICMP Echo (ping) y sockets TCP con negociación de tres vías (SYN -> SYN-ACK -> ACK).",
              border_color=CARD_CYAN_BORDER, right_badge="TCP Handshake")
              
    draw_card(draw, (60, 690, 1020, 895),
              "[ CLIENTE ]",
              "Cliente HTTP/1.1 Nativo y Malla BeaverSync",
              "Utilidad 'curl' integrada para consumir APIs web y payloads JSON sobre sockets TCP, junto al protocolo en malla BeaverSync para intercambio descentralizado de telemetría entre nodos Beaver OS.",
              border_color=CARD_AMBER_BORDER, right_badge="curl + Mesh")
              
    draw.rounded_rectangle([(60, 915), (1020, 960)], radius=8, fill=(17, 24, 39), outline=(31, 41, 55), width=1)
    draw.text((80, 926), "curl http://beaver.lan/info.json  // Petición nativa desde el Beaver Shell", font=FONT_MONO, fill=CYAN_ACCENT)
    
    draw_footer(img, draw, 5)
    return img

def generate_slide_6():
    """Slide 6: Beaver Desktop GUI."""
    img = create_base_canvas()
    draw = ImageDraw.Draw(img)
    draw_header(img, draw, "INTERFAZ GRÁFICA")
    
    draw.text((60, 115), "Beaver Desktop: GUI Nativa TrueColor", font=FONT_TITLE_LARGE, fill=TEXT_WHITE)
    draw.text((60, 175), "Entorno gráfico ligero y acelerado sin dependencias externas pesadas:", font=FONT_BODY, fill=TEXT_MUTED)
    
    draw_card(draw, (60, 230, 1020, 435),
              "[ VIDEO ]",
              "Resolución VBE 1024x768 @ 32bpp TrueColor",
              "Modo de video gráfico de alta definición con profundidad de color de 32 bits (ARGB/RGBA). Gráficos nítidos y vibrantes directamente desde el framebuffer del hardware.",
              border_color=CARD_CYAN_BORDER, right_badge="TrueColor 32bpp")
              
    draw_card(draw, (60, 460, 1020, 665),
              "[ GRÁFICOS ]",
              "Compositor de Doble Búfer Libre de Parpadeo",
              "Gestor de ventanas interactivo con arrastre por barra de título, minimizado, renderizado suavizado y cursor de ratón acelerado sin parpadeo (tear-free rendering).",
              border_color=CARD_CYAN_BORDER, right_badge="Double-Buffered")
              
    draw_card(draw, (60, 690, 1020, 895),
              "[ SUITE ]",
              "Suite Integrada de Aplicaciones Oficiales",
              "Incluye BeaverFiles (explorador de particiones BeaverFS), BeaverNote (editor de notas), BeaverAudio (sintetizador de sonido), BeaverSync (monitor de red), BeaverPhotos y BeaverCalendar.",
              border_color=CARD_AMBER_BORDER, right_badge="6 Apps Nativas")
              
    draw.rounded_rectangle([(60, 915), (1020, 960)], radius=8, fill=(17, 24, 39), outline=(31, 41, 55), width=1)
    draw.text((80, 926), ">> Escribe 'gui' en la consola para conmutar al escritorio gráfico en 1 milisegundo.", font=FONT_BODY, fill=TEXT_WHITE)
    
    draw_footer(img, draw, 6)
    return img

def generate_slide_7():
    """Slide 7: Llamado a la Acción y Descarga."""
    img = create_base_canvas()
    draw = ImageDraw.Draw(img)
    draw_header(img, draw, "DESCARGA Y COMUNIDAD")
    
    draw.text((60, 115), "¡Pruébalo Hoy Mismo!", font=FONT_TITLE_LARGE, fill=TEXT_WHITE)
    draw.text((60, 175), "Beaver OS v0.9.0 está listo para descarga, evaluación y pruebas en hardware real.", font=FONT_BODY, fill=TEXT_MUTED)
    
    # Gran tarjeta central de especificaciones
    draw.rounded_rectangle([(60, 230), (1020, 520)], radius=16, fill=CARD_BG, outline=CARD_CYAN_BORDER, width=2)
    
    draw.text((90, 255), "ESPECIFICACIONES DEL PAQUETE DE LANZAMIENTO", font=FONT_CARD_TITLE, fill=AMBER_ACCENT)
    draw.line([(90, 295), (990, 295)], fill=(30, 41, 59), width=1)
    
    specs = [
        ">>  Imagen ISO Híbrida UEFI / BIOS:  Solo 16.04 MB (arranque universal)",
        ">>  Arranque Ultrarrápido en QEMU:   powershell -ExecutionPolicy Bypass -File run.ps1",
        ">>  Hashes Criptográficos SHA-256:   Verificados e íntegros (ISO & IMG)",
        ">>  Suite de Pruebas Automatizada:   16 de 16 Pruebas Superadas (100% PASS)",
        ">>  Licencia y Marca:                Código Disponible (Nombre BEAVER OS Protegido)"
    ]
    y_sp = 315
    for s in specs:
        draw.text((90, y_sp), s, font=FONT_BODY, fill=TEXT_WHITE)
        y_sp += 36
        
    # Tarjeta de Repositorio & Autor
    draw.rounded_rectangle([(60, 545), (1020, 785)], radius=16, fill=(15, 23, 42), outline=CARD_AMBER_BORDER, width=2)
    
    draw.text((90, 570), "REPOSITORIO OFICIAL Y DOCUMENTACIÓN", font=FONT_CARD_TITLE, fill=TEXT_WHITE)
    draw.text((90, 615), "GitHub:  https://github.com/japuentem/beaversoft-kernel", font=FONT_CARD_TITLE, fill=CYAN_ACCENT)
    
    draw.text((90, 670), "Autor Principal:  José Alberto Puente Maldonado (@japuentem)", font=FONT_BODY_BOLD, fill=TEXT_WHITE)
    draw.text((90, 705), "Firma / Marca:    Beaversoft  •  Meta-Kernel Engineering", font=FONT_BODY, fill=TEXT_MUTED)
    draw.text((90, 740), "Guía Rápida:      Consulta docs/QUICKSTART.md y docs/RELEASE_NOTES_v0.9.0.md", font=FONT_SMALL, fill=AMBER_ACCENT)
    
    # Banner de Engagement LinkedIn
    draw.rounded_rectangle([(60, 810), (1020, 955)], radius=14, fill=(12, 38, 64), outline=CYAN_ACCENT, width=2)
    draw.text((90, 830), "¿Te apasiona la ingeniería de sistemas operativos y Rust bare-metal?", font=FONT_CARD_TITLE, fill=TEXT_WHITE)
    draw.text((90, 870), ">> ¡Dale Like, comenta qué subsistema te interesa más y comparte!", font=FONT_BODY_BOLD, fill=AMBER_ACCENT)
    draw.text((90, 905), ">> Si te parece inspirador este desarrollo, déjanos una estrella en GitHub.", font=FONT_BODY, fill=TEXT_MUTED)
    
    draw_footer(img, draw, 7)
    return img

def main():
    print("[1/3] Generando diapositivas de alta resolución (1080x1080)...")
    slides = [
        generate_slide_1(),
        generate_slide_2(),
        generate_slide_3(),
        generate_slide_4(),
        generate_slide_5(),
        generate_slide_6(),
        generate_slide_7(),
    ]
    
    os.makedirs("docs/carousel_slides", exist_ok=True)
    
    rgb_slides = []
    for idx, slide in enumerate(slides, start=1):
        slide_path = f"docs/carousel_slides/slide_{idx:02d}.png"
        slide.save(slide_path, format="PNG")
        print(f"  -> Guardada diapositiva {idx}: {slide_path}")
        
        rgb_slide = Image.new("RGB", slide.size, BG_COLOR)
        rgb_slide.paste(slide, mask=slide.split()[3])
        rgb_slides.append(rgb_slide)
        
    print("[2/3] Ensamblando PDF multipágina para LinkedIn...")
    pdf_path = "docs/beaver_os_v0.9.0_linkedin_carousel.pdf"
    
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
