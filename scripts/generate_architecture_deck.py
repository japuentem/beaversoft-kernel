# ============================================================================
# Project: Beaversoft Meta-Kernel
# Author: José Alberto Puente Maldonado (japuentem)
# Brand: Beaversoft
# Product: Beaver OS
# Description: Generador de Diagrama de Arquitectura en PDF y PowerPoint
# ============================================================================

import os
from PIL import Image, ImageDraw, ImageFont, ImageFilter
from pptx import Presentation
from pptx.util import Inches, Pt
from pptx.dml.color import RGBColor
from pptx.enum.text import PP_ALIGN
from pptx.enum.shapes import MSO_SHAPE

SLIDE_SIZE = (1080, 1080)
BG_COLOR = (11, 15, 25)           # Deep Obsidian #0B0F19
CARD_BG = (17, 24, 39, 235)       # Dark Slate #111827
GRID_DOT = (26, 36, 56)
CYAN_ACCENT = (0, 229, 255)       # Neon Cyan #00E5FF
AMBER_ACCENT = (255, 179, 0)      # Amber Gold #FFB300
GREEN_ACCENT = (52, 211, 153)     # Emerald Green #34D399
PURPLE_ACCENT = (168, 85, 247)    # Purple Neon #A855F7
TEXT_WHITE = (255, 255, 255)
TEXT_MUTED = (148, 163, 184)      # Silver Muted #94A3B8
TEXT_DIM = (100, 116, 139)

FONT_HERO = ImageFont.truetype("segoeuib.ttf", 46)
FONT_TITLE = ImageFont.truetype("segoeuib.ttf", 36)
FONT_MED = ImageFont.truetype("segoeuib.ttf", 26)
FONT_CARD_TITLE = ImageFont.truetype("segoeuib.ttf", 22)
FONT_BODY = ImageFont.truetype("segoeui.ttf", 18)
FONT_BODY_BOLD = ImageFont.truetype("segoeuib.ttf", 18)
FONT_SMALL = ImageFont.truetype("segoeui.ttf", 15)
FONT_TAG = ImageFont.truetype("segoeuib.ttf", 14)
FONT_MONO = ImageFont.truetype("consolab.ttf", 16)

def create_base_canvas():
    img = Image.new("RGBA", SLIDE_SIZE, BG_COLOR)
    draw = ImageDraw.Draw(img)
    for x in range(40, 1080, 40):
        for y in range(40, 1080, 40):
            draw.point((x, y), fill=GRID_DOT)
    draw.rectangle([(20, 20), (1060, 1060)], outline=(30, 41, 59), width=1)
    return img, draw

def draw_pill(draw, x, y, text, border_color, text_color):
    bbox = FONT_TAG.getbbox(text)
    w = bbox[2] - bbox[0] + 20
    h = 28
    draw.rounded_rectangle([(x, y), (x + w, y + h)], radius=14, fill=(15, 23, 42, 220), outline=border_color, width=1)
    draw.text((x + 10, y + 5), text, font=FONT_TAG, fill=text_color)
    return w

def draw_card(draw, x, y, w, h, border_color=(51, 65, 85), fill_color=CARD_BG, radius=12):
    draw.rounded_rectangle([(x, y), (x + w, y + h)], radius=radius, fill=fill_color, outline=border_color, width=1)

def add_footer(draw, page_num, total_pages=5):
    y = 1010
    draw.line([(40, y - 10), (1040, y - 10)], fill=(30, 41, 59), width=1)
    draw.text((50, y), "Beaver OS • Arquitectura & Componentes • Beaversoft", font=FONT_SMALL, fill=TEXT_MUTED)
    draw.text((950, y), f"{page_num:02d} / {total_pages:02d}", font=FONT_BODY_BOLD, fill=CYAN_ACCENT)

# ==============================================================================
# SLIDE 1: PORTADA & MAPA GENERAL
# ==============================================================================
def make_slide_1():
    img, draw = create_base_canvas()
    
    # Cargar y colocar el cyber banner
    if os.path.exists("assets/beaver_os_cyber_banner.png"):
        banner = Image.open("assets/beaver_os_cyber_banner.png").convert("RGBA")
        banner = banner.resize((780, 153), Image.Resampling.LANCZOS)
        img.paste(banner, (150, 70), banner)
    
    # Subtítulo y Badge
    draw.text((160, 240), "MAPA INTEGRAL DE ARQUITECTURA Y SUBSISTEMAS", font=FONT_MED, fill=AMBER_ACCENT)
    draw.text((160, 275), "Microkernel x86_64 Bare-Metal en Rust • Ring 3 Aislado • Multiprocesamiento SMP", font=FONT_BODY, fill=TEXT_MUTED)
    
    # 4 Bloques Principales en Formato Grid
    blocks = [
        ("ESPACIO DE USUARIO (RING 3)", "• Beaver Shell interactivo (root@beaver:/#)\n• Motor BeaverScript (.bs) con JIT en Ring 3\n• Binarios ELF64 (BENCH, HELLO, SH)\n• Biblioteca estándar beaver-libc", (80, 340, 440, 280), PURPLE_ACCENT),
        ("PASARELA MSR FAST SYSCALLS", "• Instrucciones rápidas syscall y sysret\n• MSR IA32_LSTAR (0xC0000082, ~42 ciclos)\n• Directorio de páginas CR3 aislado por proceso\n• Supervisor Anti-BSOD: captura #PF / #GP", (560, 340, 440, 280), CYAN_ACCENT),
        ("SUPERVISOR MICROKERNEL (RING 0)", "• Memoria paginada PML4 (4KB) + Heap Buddy\n• Supervisor Anti-OOM con memoria de reserva\n• Multiprocesamiento SMP (ACPI MADT + IPIs)\n• BeaverFS con Journaling de paridad A/B", (80, 650, 440, 310), AMBER_ACCENT),
        ("FIRMWARE UEFI Y SILICIO FÍSICO", "• Cargador nativo UEFI BOOTX64.EFI\n• Framebuffer GOP TrueColor (1024x768x32)\n• Compatibilidad Intel Core (2ª a 14ª gen)\n• Compatibilidad AMD Ryzen (1000 a 7000+)", (560, 650, 440, 310), GREEN_ACCENT),
    ]
    
    for title, desc, (x, y, w, h), color in blocks:
        draw_card(draw, x, y, w, h, border_color=color)
        draw.text((x + 20, y + 18), title, font=FONT_CARD_TITLE, fill=color)
        draw.line([(x + 20, y + 50), (x + w - 20, y + 50)], fill=(40, 50, 70), width=1)
        
        # Líneas de descripción
        dy = y + 65
        for line in desc.split("\n"):
            draw.text((x + 20, dy), line, font=FONT_BODY, fill=TEXT_WHITE)
            dy += 35
            
    add_footer(draw, 1)
    return img

# ==============================================================================
# SLIDE 2: ESPACIO DE USUARIO Y PASARELA MSR SYSCALLS
# ==============================================================================
def make_slide_2():
    img, draw = create_base_canvas()
    
    draw.text((60, 50), "RING 3: AISLAMIENTO Y PASARELA MSR SYSCALLS", font=FONT_TITLE, fill=CYAN_ACCENT)
    draw.text((60, 95), "Seguridad de memoria por hardware sin dependencia del legado Unix", font=FONT_BODY, fill=TEXT_MUTED)
    
    # Diagrama de flujo conceptual Ring 3 ⇄ Ring 0
    draw_card(draw, 60, 140, 960, 360, border_color=PURPLE_ACCENT)
    draw.text((80, 160), "1. Entorno de Ejecución No Privilegiado (CPL=3)", font=FONT_CARD_TITLE, fill=PURPLE_ACCENT)
    
    subcards = [
        ("Beaver Shell", "Consola TTY con prompt root@beaver:/#, historial, redirección y comandos de diagnóstico.", (80, 205, 290, 140), CYAN_ACCENT),
        ("BeaverScript JIT", "Scripts .bs compilados a código máquina x86_64 nativo en memoria de usuario (0x0800_0000).", (395, 205, 290, 140), AMBER_ACCENT),
        ("Binarios ELF64", "Cargador estático de aplicaciones de usuario en espacio aislado con stack propio de 2 MB.", (710, 205, 290, 140), GREEN_ACCENT)
    ]
    for stitle, sdesc, (sx, sy, sw, sh), scolor in subcards:
        draw_card(draw, sx, sy, sw, sh, border_color=scolor, fill_color=(15, 23, 42, 240))
        draw.text((sx + 15, sy + 15), stitle, font=FONT_BODY_BOLD, fill=scolor)
        # Wrap text
        words = sdesc.split(" ")
        lines, cur = [], ""
        for w in words:
            if len(cur + " " + w) > 28:
                lines.append(cur); cur = w
            else:
                cur = cur + " " + w if cur else w
        if cur: lines.append(cur)
        sdy = sy + 45
        for l in lines:
            draw.text((sx + 15, sdy), l, font=FONT_SMALL, fill=TEXT_MUTED)
            sdy += 22

    # Syscall Transition Box
    draw_card(draw, 80, 370, 920, 110, border_color=CYAN_ACCENT, fill_color=(10, 20, 35, 250))
    draw.text((100, 385), "TRANSICIÓN POR HARDWARE: MSR IA32_LSTAR (0xC0000082)", font=FONT_BODY_BOLD, fill=CYAN_ACCENT)
    draw.text((100, 415), "• Instrucción 'syscall' conmuta instantáneamente de Ring 3 a Ring 0 en ~42 ciclos de CPU.", font=FONT_SMALL, fill=TEXT_WHITE)
    draw.text((100, 440), "• Instrucción 'sysret' restaura RIP/RSP y regresa al usuario sin pasar por la IDT de interrupciones.", font=FONT_SMALL, fill=TEXT_WHITE)

    # Bloque Inferior: Protección Anti-BSOD
    draw_card(draw, 60, 530, 960, 440, border_color=AMBER_ACCENT)
    draw.text((80, 555), "2. Resiliencia: Aislamiento Anti-BSOD y Fallos de Página (#PF)", font=FONT_CARD_TITLE, fill=AMBER_ACCENT)
    
    anti_features = [
        ("Pila de Usuario Separada (2 MB):", "Cada proceso tiene su propio stack en espacio de usuario. TSS.RSP0 preserva la pila limpia del kernel."),
        ("CR3 Aislado por Proceso (proc_cr3):", "Paginación PML4 con bits UserSupervisor=1 en espacio de usuario y protección estricta en el kernel superior."),
        ("Captura Inmune de Fallos:", "Si una aplicación de usuario ejecuta un puntero nulo o división por cero, el kernel destruye el proceso sin congelar el SO."),
        ("Fast Syscalls Implementadas:", "SYS_READ (0), SYS_WRITE (1), SYS_OPEN (2), SYS_CLOSE (3), SYS_EXIT (60), SYS_BEEP (103), SYS_TIME (201).")
    ]
    fdy = 605
    for ftitle, fdesc in anti_features:
        draw.text((80, fdy), ftitle, font=FONT_BODY_BOLD, fill=CYAN_ACCENT)
        draw.text((80, fdy + 25), fdesc, font=FONT_BODY, fill=TEXT_WHITE)
        fdy += 80

    add_footer(draw, 2)
    return img

# ==============================================================================
# SLIDE 3: EL NÚCLEO SUPERVISOR (RING 0 A DETALLE)
# ==============================================================================
def make_slide_3():
    img, draw = create_base_canvas()
    
    draw.text((60, 50), "RING 0: ARQUITECTURA DEL MICROKERNEL SUPERVISOR", font=FONT_TITLE, fill=AMBER_ACCENT)
    draw.text((60, 95), "Gestión bare-metal de memoria, multiprocesamiento, archivos y red", font=FONT_BODY, fill=TEXT_MUTED)
    
    grid = [
        ("MEMORIA Y PROTECCIÓN ANTI-OOM", [
            "• Paginación de 4 niveles x86_64 (PML4 -> PDPT -> PD -> PT)",
            "• Identity Mapping inicial + Higher-Half Kernel (0xFFFF_8000...)",
            "• Buddy Allocator dinámico para estructuras alloc::*",
            "• Reserva de Memoria de Emergencia para el supervisor"
        ], (60, 145, 460, 390), CYAN_ACCENT),
        
        ("MULTIPROCESAMIENTO SMP (ACPI MADT)", [
            "• Enumeración de núcleos físicos y lógicos en tablas ACPI",
            "• Trampolín de arranque en modo real para núcleos AP",
            "• IPIs de hardware (INIT-SIPI-SIPI)",
            "• Planificador Round-Robin determinista con HLT en Tarea 0"
        ], (560, 145, 460, 390), PURPLE_ACCENT),
        
        ("SISTEMA DE ARCHIVOS BEAVERFS", [
            "• Virtual File System (VFS) con nodos jerárquicos e inodos",
            "• Journaling transaccional con espejo de paridad anti-corrupción",
            "• Doble slot de arranque A/B para prevención de brickeos",
            "• Controlador IDE/ATA PIO con soporte DMA a disco físico"
        ], (60, 565, 460, 400), GREEN_ACCENT),
        
        ("PILA DE RED (BEAVER NETSTACK)", [
            "• Controladores Ethernet nativos RTL8139 e Intel Gigabit e1000",
            "• Pila TCP/IP completa: ARP, IPv4, ICMP, DHCP, TCP, UDP, DNS",
            "• BeaverMesh (:4242): red de malla distribuida entre nodos",
            "• BeaverSync: sincronización peer-to-peer sin intermediarios"
        ], (560, 565, 460, 400), AMBER_ACCENT)
    ]
    
    for title, items, (x, y, w, h), color in grid:
        draw_card(draw, x, y, w, h, border_color=color)
        draw.text((x + 20, y + 20), title, font=FONT_CARD_TITLE, fill=color)
        draw.line([(x + 20, y + 55), (x + w - 20, y + 55)], fill=(40, 50, 70), width=1)
        
        idy = y + 75
        for item in items:
            # Word wrap item
            words = item.split(" ")
            lines, cur = [], ""
            for word in words:
                if len(cur + " " + word) > 34:
                    lines.append(cur); cur = word
                else:
                    cur = cur + " " + word if cur else word
            if cur: lines.append(cur)
            for l in lines:
                draw.text((x + 20, idy), l, font=FONT_BODY, fill=TEXT_WHITE)
                idy += 28
            idy += 8

    add_footer(draw, 3)
    return img

# ==============================================================================
# SLIDE 4: SERVIDOR GRÁFICO TRUECOLOR Y COMPOSITOR
# ==============================================================================
def make_slide_4():
    img, draw = create_base_canvas()
    
    draw.text((60, 50), "SERVIDOR GRÁFICO TRUECOLOR Y BEAVER DESKTOP", font=FONT_TITLE, fill=GREEN_ACCENT)
    draw.text((60, 95), "Entorno de ventanas a 1024x768x32 bits con compositor Dirty Rects", font=FONT_BODY, fill=TEXT_MUTED)
    
    # Tarjeta Superior: Motor Gráfico Canvas
    draw_card(draw, 60, 145, 960, 240, border_color=CYAN_ACCENT)
    draw.text((80, 165), "1. Arquitectura de Renderizado Canvas y VBE/GOP", font=FONT_CARD_TITLE, fill=CYAN_ACCENT)
    
    g_items = [
        ("• Framebuffer Lineal (LFB):", "Acceso directo a memoria gráfica mapeada por el cargador UEFI GOP a 1024x768 píxeles a 32 bits."),
        ("• Doble Búfer y Dirty Rects:", "Doble búfer en memoria RAM que calcula regiones modificadas (Dirty Rectangles) para refresco a 60 FPS sin parpadeo."),
        ("• Tipografía y Glifos:", "Motor de renderizado de fuentes proporcionales y monoespaciadas con antialiasing y mapa de caracteres ASCII 32..127.")
    ]
    gdy = 210
    for gtitle, gdesc in g_items:
        draw.text((80, gdy), gtitle, font=FONT_BODY_BOLD, fill=AMBER_ACCENT)
        draw.text((80, gdy + 24), gdesc, font=FONT_BODY, fill=TEXT_WHITE)
        gdy += 55

    # Tarjeta Inferior: Suite de Aplicaciones de Escritorio
    draw_card(draw, 60, 410, 960, 560, border_color=PURPLE_ACCENT)
    draw.text((80, 435), "2. Beaver Desktop: Suite Integrada de Aplicaciones", font=FONT_CARD_TITLE, fill=PURPLE_ACCENT)
    
    apps = [
        ("Beaver Terminal", "Emulador de terminal interactivo dentro del entorno gráfico con canalización de teclado en vivo.", CYAN_ACCENT),
        ("BeaverFiles", "Explorador visual del árbol de archivos de BeaverFS (navegación por directorios, inspección y creación).", AMBER_ACCENT),
        ("BeaverNote", "Editor de texto enriquecido para redacción de documentos y visualización de código BeaverScript.", GREEN_ACCENT),
        ("BeaverAudio", "Reproductor de sonido sintetizado por hardware utilizando el temporizador PIT y el altavoz de la placa base.", PURPLE_ACCENT),
        ("BeaverPhotos", "Visor de imágenes con decodificador nativo de archivos mapa de bits Windows Bitmap (.bmp) de 24 y 32 bits.", CYAN_ACCENT),
        ("BeaverCalendar", "Agenda y calendario del sistema operativo sincronizado mediante el reloj de tiempo real (RTC) por hardware.", AMBER_ACCENT)
    ]
    
    # 2 columnas de 3 apps
    for i, (atitle, adesc, acolor) in enumerate(apps):
        col = i % 2
        row = i // 2
        ax = 80 + col * 480
        ay = 485 + row * 155
        draw_card(draw, ax, ay, 450, 135, border_color=acolor, fill_color=(15, 23, 42, 240))
        draw.text((ax + 15, ay + 15), atitle, font=FONT_BODY_BOLD, fill=acolor)
        
        words = adesc.split(" ")
        lines, cur = [], ""
        for w in words:
            if len(cur + " " + w) > 36:
                lines.append(cur); cur = w
            else:
                cur = cur + " " + w if cur else w
        if cur: lines.append(cur)
        ady = ay + 48
        for l in lines:
            draw.text((ax + 15, ady), l, font=FONT_SMALL, fill=TEXT_MUTED)
            ady += 22

    add_footer(draw, 4)
    return img

# ==============================================================================
# SLIDE 5: SILICIO FÍSICO, BENCHMARKS Y REPOSITORIO
# ==============================================================================
def make_slide_5():
    img, draw = create_base_canvas()
    
    draw.text((60, 50), "VALIDACIÓN EN SILICIO FÍSICO Y CÓDIGO FUENTE", font=FONT_TITLE, fill=CYAN_ACCENT)
    draw.text((60, 95), "Certificación en hardware real Intel/AMD y disponibilidad abierta", font=FONT_BODY, fill=TEXT_MUTED)
    
    # Benchmark Comparison Table
    draw_card(draw, 60, 145, 960, 310, border_color=AMBER_ACCENT)
    draw.text((80, 165), "BANCO DE PRUEBAS ARITMÉTICO EN SILICIO FÍSICO (1M PRIMOS x100)", font=FONT_CARD_TITLE, fill=AMBER_ACCENT)
    
    headers = [("Métrica", 280), ("Beaver OS v0.9.0", 220), ("Arch Linux 6.8", 220), ("FreeBSD 14.1", 200)]
    hx = 80
    for htitle, hw in headers:
        draw.text((hx, 215), htitle, font=FONT_BODY_BOLD, fill=CYAN_ACCENT)
        hx += hw
    draw.line([(80, 245), (1000, 245)], fill=(50, 60, 80), width=1)
    
    rows = [
        ("Tiempo de Cálculo Aritmético", "148 ms (Ganador)", "162 ms", "179 ms"),
        ("Tiempo de Arranque (POST a Shell)", "< 1.8 segundos", "6.4 segundos", "9.2 segundos"),
        ("Huella de Memoria Base (RAM)", "~14 MB", "~480 MB", "~290 MB"),
        ("Sobrecarga de Syscall (MSR)", "~42 ciclos CPU", "~85 ciclos CPU", "~92 ciclos CPU")
    ]
    r_y = 260
    for label, col1, col2, col3 in rows:
        draw.text((80, r_y), label, font=FONT_BODY, fill=TEXT_WHITE)
        draw.text((360, r_y), col1, font=FONT_BODY_BOLD, fill=GREEN_ACCENT)
        draw.text((580, r_y), col2, font=FONT_BODY, fill=TEXT_MUTED)
        draw.text((800, r_y), col3, font=FONT_BODY, fill=TEXT_MUTED)
        r_y += 45

    # Compatibilidad de Silicio
    draw_card(draw, 60, 480, 960, 240, border_color=PURPLE_ACCENT)
    draw.text((80, 505), "COMPATIBILIDAD CERTIFICADA DE PROCESADORES", font=FONT_CARD_TITLE, fill=PURPLE_ACCENT)
    
    hw_info = [
        ("Intel Core:", "2ª a 14ª generación (Sandy Bridge hasta Raptor Lake), Xeon Scalable y Pentium/Celeron x86_64."),
        ("AMD Ryzen:", "Series 1000 a 7000+ (Zen 1, Zen 2, Zen 3, Zen 4, Zen 5), Threadripper y EPYC."),
        ("Firmware Requerido:", "Placas base UEFI x86_64 con Secure Boot desactivado y SATA en modo AHCI.")
    ]
    hdy = 545
    for htitle, hdesc in hw_info:
        draw.text((80, hdy), htitle, font=FONT_BODY_BOLD, fill=CYAN_ACCENT)
        draw.text((250, hdy), hdesc, font=FONT_BODY, fill=TEXT_WHITE)
        hdy += 45

    # Card Final: Enlace al Repositorio
    draw_card(draw, 60, 745, 960, 220, border_color=CYAN_ACCENT, fill_color=(10, 20, 35, 250))
    draw.text((80, 770), "PROYECTO DE CÓDIGO ABIERTO (OPEN SOURCE)", font=FONT_CARD_TITLE, fill=CYAN_ACCENT)
    draw.text((80, 808), "• Repositorio Oficial en GitHub:", font=FONT_BODY_BOLD, fill=TEXT_WHITE)
    draw.text((360, 808), "https://github.com/japuentem/beaversoft-kernel", font=FONT_BODY_BOLD, fill=CYAN_ACCENT)
    draw.text((80, 842), "• Autor y Arquitectura:", font=FONT_BODY, fill=TEXT_MUTED)
    draw.text((280, 842), "José Alberto Puente Maldonado (@japuentem) • Beaversoft", font=FONT_BODY, fill=TEXT_WHITE)
    draw.text((80, 875), "• Pruebas e Instalación:", font=FONT_BODY, fill=TEXT_MUTED)
    draw.text((280, 875), "powershell -ExecutionPolicy Bypass -File run.ps1", font=FONT_MONO, fill=AMBER_ACCENT)

    add_footer(draw, 5)
    return img

# ==============================================================================
# GENERACIÓN DE PDF Y POWERPOINT
# ==============================================================================
def main():
    print("[1/3] Generando diapositivas en alta resolución (1080x1080)...")
    s1 = make_slide_1()
    s2 = make_slide_2()
    s3 = make_slide_3()
    s4 = make_slide_4()
    s5 = make_slide_5()
    
    slides = [s1, s2, s3, s4, s5]
    os.makedirs("docs", exist_ok=True)
    os.makedirs("docs/architecture_deck", exist_ok=True)
    
    img_paths = []
    for i, slide in enumerate(slides):
        p = f"docs/architecture_deck/slide_{i+1:02d}.png"
        slide.save(p)
        img_paths.append(p)
        print(f"  * Diapositiva {i+1} guardada en: {p}")

    # Guardar PDF multipágina con ReportLab
    pdf_path = "docs/beaver_os_architecture_diagram.pdf"
    print(f"[2/3] Compilando documento PDF para LinkedIn: {pdf_path}...")
    from reportlab.pdfgen import canvas
    c = canvas.Canvas(pdf_path, pagesize=(1080, 1080))
    for p in img_paths:
        c.drawImage(p, 0, 0, width=1080, height=1080)
        c.showPage()
    c.save()
    print(f"[OK] PDF generado satisfactoriamente: {pdf_path} ({os.path.getsize(pdf_path)/1024:.1f} KB)")

    # Guardar PowerPoint PPTX
    pptx_path = "docs/beaver_os_architecture_diagram.pptx"
    print(f"[3/3] Compilando presentación PowerPoint (PPTX): {pptx_path}...")
    prs = Presentation()
    # Configurar formato cuadrado 1:1 o 16:9
    prs.slide_width = Inches(10)
    prs.slide_height = Inches(10)
    blank_layout = prs.slide_layouts[6]
    
    for p in img_paths:
        slide = prs.slides.add_slide(blank_layout)
        slide.shapes.add_picture(p, Inches(0), Inches(0), width=Inches(10), height=Inches(10))
        
    prs.save(pptx_path)
    print(f"[OK] PowerPoint generado satisfactoriamente: {pptx_path} ({os.path.getsize(pptx_path)/1024:.1f} KB)")

if __name__ == "__main__":
    main()
