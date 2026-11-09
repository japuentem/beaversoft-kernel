# ============================================================================
# Project: Beaversoft Meta-Kernel
# Author: José Alberto Puente Maldonado (japuentem)
# Brand: Beaversoft
# Product: Beaver OS
# Description: Generador de Carrusel de Experiencia y Posicionamiento de Marca (Apple Style)
# ============================================================================

import os
from PIL import Image, ImageDraw, ImageFont
from pptx import Presentation
from pptx.util import Inches

SLIDE_SIZE = (1080, 1080)

# Paleta de color premium dark mode (estilo Keynote / Apple Dark Luxury)
BG_COLOR = (7, 9, 14)              # Obsidian Black #07090E
CARD_BG = (15, 23, 42, 230)        # Deep Navy Slate #0F172A
CARD_BG_ALT = (20, 30, 50, 240)    # Elevated Card
GRID_DOT = (24, 34, 52)
CYAN_ACCENT = (56, 189, 248)       # Sky Cyan #38BDF8
AMBER_ACCENT = (251, 191, 36)      # Warm Amber Gold #FBBF24
GREEN_ACCENT = (52, 211, 153)      # Emerald Green #34D399
PURPLE_ACCENT = (167, 139, 250)    # Soft Violet #A78BFA
ROSE_ACCENT = (244, 63, 94)        # Vibrant Rose #F43F5E
TEXT_WHITE = (255, 255, 255)
TEXT_MUTED = (148, 163, 184)       # Silver Slate #94A3B8
TEXT_DIM = (100, 116, 139)

# Tipografías nativas de Windows
FONT_BADGE = ImageFont.truetype("segoeuib.ttf", 13)
FONT_HERO = ImageFont.truetype("segoeuib.ttf", 36)
FONT_SUBTITLE = ImageFont.truetype("segoeui.ttf", 17)
FONT_SECTION = ImageFont.truetype("segoeuib.ttf", 22)
FONT_CARD_TITLE = ImageFont.truetype("segoeuib.ttf", 18)
FONT_BODY = ImageFont.truetype("segoeui.ttf", 15)
FONT_BODY_BOLD = ImageFont.truetype("segoeuib.ttf", 15)
FONT_SMALL = ImageFont.truetype("segoeui.ttf", 13)
FONT_SMALL_BOLD = ImageFont.truetype("segoeuib.ttf", 13)
FONT_STAT_NUM = ImageFont.truetype("segoeuib.ttf", 46)
FONT_STAT_LABEL = ImageFont.truetype("segoeuib.ttf", 13)
FONT_URL = ImageFont.truetype("segoeuib.ttf", 30)
FONT_MONO = ImageFont.truetype("consolab.ttf", 15)

TOTAL_SLIDES = 6

def create_base_canvas():
    """Genera el lienzo base con sutil rejilla nanométrica y marco perimetral."""
    img = Image.new("RGBA", SLIDE_SIZE, BG_COLOR)
    draw = ImageDraw.Draw(img)
    
    # Rejilla nanométrica de fondo
    for x in range(40, 1080, 40):
        for y in range(40, 1080, 40):
            draw.point((x, y), fill=GRID_DOT)
            
    # Borde perimetral elegante
    draw.rectangle([(20, 20), (1060, 1060)], outline=(26, 36, 56), width=1)
    return img, draw

def draw_header_badge(draw, text, color=CYAN_ACCENT, y=45):
    """Dibuja una pastilla o badge superior estilo Keynote."""
    bbox = FONT_BADGE.getbbox(text)
    tw = bbox[2] - bbox[0]
    
    x = 55
    pw = tw + 24
    ph = 26
    draw.rounded_rectangle([(x, y), (x + pw, y + ph)], radius=13, fill=(15, 25, 45, 200), outline=color, width=1)
    draw.text((x + 12, y + 4), text, font=FONT_BADGE, fill=color)

def add_footer(draw, page_num, total_pages=TOTAL_SLIDES):
    """Pie de página institucional estandarizado con separación de marca y numeración."""
    y = 1018
    draw.line([(40, y - 10), (1040, y - 10)], fill=(30, 41, 59), width=1)
    draw.text((50, y), "Beaver OS • Una visión de Beaversoft • Creado por José Alberto Puente Maldonado", font=FONT_SMALL, fill=TEXT_MUTED)
    draw.text((950, y), f"{page_num:02d} / {total_pages:02d}", font=FONT_BODY_BOLD, fill=CYAN_ACCENT)

def draw_card(draw, x, y, w, h, border_color=(51, 65, 85), fill_color=CARD_BG, radius=14):
    """Dibuja una tarjeta con esquinas redondeadas y borde de acento."""
    draw.rounded_rectangle([(x, y), (x + w, y + h)], radius=radius, fill=fill_color, outline=border_color, width=1)

def draw_wrapped_text(draw, text, x, y, max_w, font, fill, line_spacing=22):
    """Mide píxel por píxel con soporte nativo de saltos de línea (\n) para evitar cualquier solapamiento."""
    paragraphs = text.split("\n")
    cur_y = y
    for para in paragraphs:
        if not para.strip():
            cur_y += line_spacing // 2
            continue
        words = para.split(" ")
        lines = []
        cur_line = ""
        for w in words:
            test_line = cur_line + " " + w if cur_line else w
            bbox = font.getbbox(test_line)
            line_w = bbox[2] - bbox[0]
            if line_w > max_w:
                if cur_line:
                    lines.append(cur_line)
                cur_line = w
            else:
                cur_line = test_line
        if cur_line:
            lines.append(cur_line)
        for l in lines:
            draw.text((x, cur_y), l, font=font, fill=fill)
            cur_y += line_spacing
    return cur_y

# ==============================================================================
# SLIDE 1: EL MANIFIESTO DE CALMA (APERTURA)
# ==============================================================================
def make_slide_1():
    img, draw = create_base_canvas()
    
    draw_header_badge(draw, "BEAVER OS • LA REINVENCIÓN DE LA COMPUTADORA", CYAN_ACCENT, y=45)
    draw.text((55, 88), "DISEÑADO DESDE CERO PARA DEVOLVERTE LA CALMA.", font=FONT_HERO, fill=TEXT_WHITE)
    
    sub = "La computadora dejó de ser una herramienta silenciosa y se convirtió en una fuente de ansiedad, distracciones y lentitud. Beaver OS nace para que tu tecnología trabaje para ti, no para anunciantes ni algoritmos."
    draw_wrapped_text(draw, sub, 55, 140, 970, FONT_SUBTITLE, TEXT_MUTED, line_spacing=24)
    
    if os.path.exists("assets/beaver_os_cyber_banner.png"):
        banner = Image.open("assets/beaver_os_cyber_banner.png").convert("RGBA")
        bw, bh = 800, 157
        banner = banner.resize((bw, bh), Image.Resampling.LANCZOS)
        bx = (1080 - bw) // 2
        img.paste(banner, (bx, 220), banner)
    
    pillars = [
        ("01", "FLUYE SIN ESPERAS", [
            "Encendido instantáneo en menos de 1.8 segundos.",
            "Solo 14 MB de memoria RAM ocupada por el sistema.",
            "El 99% de tu computadora queda libre para crear."
        ], CYAN_ACCENT, 55, 410, 305, 330),
        
        ("02", "SERENIDAD SIN MIEDO", [
            "Tu trabajo nunca se pierde ni se congela.",
            "Aislamiento Ring 3: si una app falla, solo ella se cierra.",
            "BeaverFS con paridad A/B inmune a apagones."
        ], GREEN_ACCENT, 387, 410, 305, 330),
        
        ("03", "SOBERANÍA Y PRIVACIDAD", [
            "Tus fotos, notas y archivos te pertenecen.",
            "Red de malla BeaverMesh P2P sin nube corporativa.",
            "Cero telemetría, cero rastreo y cero anuncios."
        ], AMBER_ACCENT, 720, 410, 305, 330),
    ]
    
    for num, title, points, color, px, py, pw, ph in pillars:
        draw_card(draw, px, py, pw, ph, border_color=color, fill_color=(13, 20, 36, 235))
        draw.rounded_rectangle([(px + 18, py + 18), (px + 60, py + 46)], radius=6, fill=(20, 32, 56), outline=color, width=1)
        draw.text((px + 27, py + 22), num, font=FONT_BODY_BOLD, fill=color)
        
        draw.text((px + 18, py + 62), title, font=FONT_CARD_TITLE, fill=TEXT_WHITE)
        draw.line([(px + 18, py + 92), (px + pw - 18, py + 92)], fill=(35, 48, 75), width=1)
        
        dy = py + 108
        for pt in points:
            draw.text((px + 18, dy), "•", font=FONT_BODY, fill=color)
            dy = draw_wrapped_text(draw, pt, px + 34, dy, pw - 52, FONT_BODY, TEXT_MUTED, line_spacing=21) + 12

    draw_card(draw, 55, 765, 970, 215, border_color=(45, 60, 90), fill_color=(12, 18, 30, 245))
    draw.text((80, 785), "LA PROMESA DE BEAVER OS: COMPUTACIÓN PURA Y HONESTA", font=FONT_SECTION, fill=CYAN_ACCENT)
    
    quote1 = "No necesitas tolerar sistemas operativos que saturan tu procesador con procesos zombi ni que te traten como un producto comercial. Construido en Rust bare-metal desde la primera línea de código, Beaver OS redefine lo que significa interactuar con una máquina: rapidez transparente, estabilidad matemática y tranquilidad absoluta."
    draw_wrapped_text(draw, quote1, 80, 825, 920, FONT_SUBTITLE, TEXT_WHITE, line_spacing=25)
    draw.text((80, 940), ">> Desliza para descubrir cómo se siente trabajar con un sistema creado para respetarte.", font=FONT_BODY_BOLD, fill=AMBER_ACCENT)
    
    add_footer(draw, 1)
    return img

# ==============================================================================
# SLIDE 2: INMEDIATEZ Y FLUIDEZ (ENCENDER Y CREAR)
# ==============================================================================
def make_slide_2():
    img, draw = create_base_canvas()
    
    draw_header_badge(draw, "EXPERIENCIA • FLUIDEZ & LIGEREZA", CYAN_ACCENT, y=45)
    draw.text((55, 88), "ENCIENDE Y CREA. SIN ESPERAS.", font=FONT_HERO, fill=TEXT_WHITE)
    
    sub = "La mejor tecnología es la que desaparece para dejar que tus ideas fluyan libremente. En Beaver OS, la computadora vuelve a sentirse tan rápida como tu propio pensamiento."
    draw_wrapped_text(draw, sub, 55, 140, 970, FONT_SUBTITLE, TEXT_MUTED, line_spacing=24)
    
    stats = [
        ("< 1.8 s", "ARRANQUE EN FRÍO", "Desde que presionas el botón de encendido hasta que tu escritorio está completamente listo para trabajar.", CYAN_ACCENT, 55, 210, 305, 175),
        ("~14 MB", "HUELLA EN MEMORIA", "El microkernel ocupa un espacio insignificante. Más del 99% de tu hardware queda libre para tus proyectos.", GREEN_ACCENT, 387, 210, 305, 175),
        ("0%", "PUBLICIDAD & RASTREO", "Sin ventanas emergentes comerciales, sin sugerencias molestas y sin algoritmos robándote atención.", AMBER_ACCENT, 720, 210, 305, 175),
    ]
    
    for val, lbl, desc, color, sx, sy, sw, sh in stats:
        draw_card(draw, sx, sy, sw, sh, border_color=color, fill_color=(14, 22, 38, 245))
        draw.text((sx + 20, sy + 15), val, font=FONT_STAT_NUM, fill=color)
        draw.text((sx + 20, sy + 75), lbl, font=FONT_STAT_LABEL, fill=TEXT_WHITE)
        draw_wrapped_text(draw, desc, sx + 20, sy + 102, sw - 40, FONT_SMALL, TEXT_MUTED, line_spacing=18)
    
    cards = [
        ("FOCO TOTAL SIN INTERRUPCIONES", "ADIÓS AL RUIDO MENTAL", 
         "¿Recuerdas cuando encendías tu equipo y comenzaba a actualizarse sin pedir permiso? Beaver OS no interrumpe tu flujo. Trabajas en silencio, concentrado en lo que realmente importa.",
         (55, 410, 465, 250), CYAN_ACCENT),
        
        ("BATERÍA QUE DURA HORAS EXTRA", "EFICIENCIA ENERGÉTICA REAL", 
         "Al desmantelar capas de emuladores pesados y código legado de los años 80, el procesador descansa cuando no lo usas (instrucción HLT). Tu laptop trabaja fresca y la batería dura más.",
         (560, 410, 465, 250), GREEN_ACCENT),
         
        ("RESPUESTA EN NANOSEGUNDOS", "PASARELA MSR ULTRAVELOZ", 
         "Cada clic, tecla o comando se ejecuta casi a la velocidad de la luz. Las llamadas al sistema se resuelven en ~42 ciclos de CPU en hardware puro, sin intermediarios lentos.",
         (55, 680, 465, 290), PURPLE_ACCENT),
         
        ("DISEÑADO PARA DURAR DÉCADAS", "HONESTIDAD CON EL HARDWARE", 
         "No necesitas cambiar de computadora cada 3 años. Beaver OS le devuelve una agilidad asombrosa tanto a silicio moderno Intel Core / AMD Ryzen como a equipos antiguos.",
         (560, 680, 465, 290), AMBER_ACCENT),
    ]
    
    for tag, title, body, (cx, cy, cw, ch), color in cards:
        draw_card(draw, cx, cy, cw, ch, border_color=color, fill_color=(13, 20, 35, 235))
        draw.text((cx + 20, cy + 18), tag, font=FONT_BADGE, fill=color)
        draw.text((cx + 20, cy + 42), title, font=FONT_CARD_TITLE, fill=TEXT_WHITE)
        draw.line([(cx + 20, cy + 72), (cx + cw - 20, cy + 72)], fill=(35, 48, 75), width=1)
        draw_wrapped_text(draw, body, cx + 20, cy + 88, cw - 40, FONT_BODY, TEXT_MUTED, line_spacing=22)

    add_footer(draw, 2)
    return img

# ==============================================================================
# SLIDE 3: SERENIDAD Y PAZ MENTAL (ANTI-BSOD & ANTI-CRASH)
# ==============================================================================
def make_slide_3():
    img, draw = create_base_canvas()
    
    draw_header_badge(draw, "CONFIANZA • RESILIENCIA ABSOLUTA", GREEN_ACCENT, y=45)
    draw.text((55, 88), "TU TRABAJO NUNCA SE PIERDE.", font=FONT_HERO, fill=TEXT_WHITE)
    
    sub = "La tranquilidad de saber que tu computadora jamás se rendirá a mitad de una entrega urgente. Una arquitectura diseñada para soportar fallas externas sin pestañear."
    draw_wrapped_text(draw, sub, 55, 140, 970, FONT_SUBTITLE, TEXT_MUTED, line_spacing=24)
    
    # Lado Izquierdo: La Ansiedad Tradicional
    draw_card(draw, 55, 205, 465, 500, border_color=ROSE_ACCENT, fill_color=(25, 15, 22, 230))
    draw.text((75, 225), "LA ANSIEDAD DE LOS SISTEMAS TRADICIONALES", font=FONT_BADGE, fill=ROSE_ACCENT)
    draw.text((75, 248), "El Miedo Constante a Perder Todo", font=FONT_CARD_TITLE, fill=TEXT_WHITE)
    draw.line([(75, 278), (495, 278)], fill=(70, 30, 40), width=1)
    
    old_pains = [
        ("Pantallas Azules (BSOD) Imprevistas:", "Un fallo en un controlador de video o una app mal programada tumba el sistema entero."),
        ("El Temido Bloqueo ('No Responde'):", "El puntero del ratón gira sin cesar. Tu trabajo de las últimas tres horas se evapora en un instante."),
        ("Actualizaciones Forzadas Inoportunas:", "Justo cuando vas a iniciar una presentación, el sistema decide reiniciarse sin tu consentimiento."),
        ("Pánico a los Cortes Eléctricos:", "Si se va la energía mientras guardabas, el disco se corrompe y tu sistema deja de arrancar.")
    ]
    oy = 295
    for opt, odesc in old_pains:
        draw.text((75, oy), opt, font=FONT_BODY_BOLD, fill=ROSE_ACCENT)
        oy = draw_wrapped_text(draw, odesc, 75, oy + 22, 425, FONT_BODY, TEXT_MUTED, line_spacing=20) + 12

    # Lado Derecho: La Paz con Beaver OS
    draw_card(draw, 560, 205, 465, 500, border_color=GREEN_ACCENT, fill_color=(12, 28, 25, 230))
    draw.text((580, 225), "LA SERENIDAD CON BEAVER OS", font=FONT_BADGE, fill=GREEN_ACCENT)
    draw.text((580, 248), "Paz Mental Blindada por Hardware", font=FONT_CARD_TITLE, fill=TEXT_WHITE)
    draw.line([(580, 278), (1000, 278)], fill=(30, 70, 50), width=1)
    
    new_peace = [
        ("Aislamiento Ring 3 Indestructible:", "Las aplicaciones corren en un espacio estricto de usuario. Si una app colapsa, solo ella se cierra; el kernel y tus demás apps siguen vivas."),
        ("Supervisor Anti-OOM con Reserva:", "Si la memoria física se agota, el supervisor activa memoria de emergencia para salvar tus archivos de forma segura."),
        ("BeaverFS con Paridad A/B Anti-Apagón:", "Doble slot de arranque y registro transaccional. Si la energía se corta súbitamente, el sistema arranca intacto en 1.8 segundos."),
        ("Control Absoluto en tus Manos:", "Ningún proceso se actualiza a tus espaldas. El sistema respeta tu tiempo y jamás se reinicia por sorpresa.")
    ]
    ny = 295
    for npt, ndesc in new_peace:
        draw.text((580, ny), npt, font=FONT_BODY_BOLD, fill=GREEN_ACCENT)
        ny = draw_wrapped_text(draw, ndesc, 580, ny + 22, 425, FONT_BODY, TEXT_WHITE, line_spacing=20) + 12

    draw_card(draw, 55, 730, 970, 245, border_color=CYAN_ACCENT, fill_color=(14, 22, 38, 245))
    draw.text((80, 750), "INGENIERÍA PARA LA TRANQUILIDAD HUMANA", font=FONT_SECTION, fill=CYAN_ACCENT)
    
    quote2 = "En Beaversoft creemos que la verdadera elegancia tecnológica no consiste únicamente en añadir funciones decorativas, sino en erradicar para siempre la frustración. Hemos invertido cientos de horas auditando fallos de página, registros MSR y transacciones de disco para que tú solo tengas que pensar en crear."
    draw_wrapped_text(draw, quote2, 80, 792, 920, FONT_SUBTITLE, TEXT_WHITE, line_spacing=25)
    draw.text((80, 915), ">> Trabaja con la certeza de que tu esfuerzo está protegido en cada milisegundo.", font=FONT_BODY_BOLD, fill=AMBER_ACCENT)

    add_footer(draw, 3)
    return img

# ==============================================================================
# SLIDE 4: SOBERANÍA Y PRIVACIDAD (TUS DATOS SON TUYOS)
# ==============================================================================
def make_slide_4():
    img, draw = create_base_canvas()
    
    draw_header_badge(draw, "PRIVACIDAD • PROPIEDAD REAL", AMBER_ACCENT, y=45)
    draw.text((55, 88), "TUS ARCHIVOS SON TUYOS. DE VERDAD.", font=FONT_HERO, fill=TEXT_WHITE)
    
    sub = "Tus fotos personales, tus documentos confidenciales y tus notas viven en tus dispositivos, no en granjas de servidores corporativos lucrando con tu información."
    draw_wrapped_text(draw, sub, 55, 140, 970, FONT_SUBTITLE, TEXT_MUTED, line_spacing=24)
    
    cards = [
        ("RED DE MALLA BEAVERMESH", "SIN NUBE OBLIGATORIA", 
         "Tus dispositivos se comunican, descubren y sincronizan directamente a través de tu red local en el puerto :4242. Tus archivos viajan a máxima velocidad entre tu laptop, teléfono y pantalla sin salir jamás a internet ni requerir suscripciones mensuales.",
         (55, 215, 465, 335), CYAN_ACCENT),
        
        ("CERO PERFILAMIENTO COMERCIAL", "TÚ NO ERES EL PRODUCTO", 
         "Beaver OS no contiene identificadores de publicidad, telemetría oculta ni recolectores de datos analíticos. No indexamos tus documentos para alimentar inteligencias artificiales de terceros. Tu privacidad no es un ajuste que debas activar: es el cimiento de cada byte.",
         (560, 215, 465, 335), AMBER_ACCENT),
         
        ("TRANSPARENCIA HUMANA TOTAL", "SIN SECRETOS EN MEMORIA", 
         "¿Alguna vez te has preguntado qué hacen realmente los cientos de procesos ocultos en otros sistemas? En Beaver OS, la consola y el monitor del sistema te muestran la verdad: cada hilo, cada puerto y cada página de memoria responde únicamente a ti.",
         (55, 575, 465, 390), GREEN_ACCENT),
         
        ("AUTONOMÍA Y CONTINUIDAD LOCAL", "LIBRE DE OBSOLESCENCIA", 
         "Si te quedas sin conexión a internet en medio de un viaje, tu sistema funciona con la misma plenitud. Tus notas, tu música, tus herramientas de cálculo y tu entorno de desarrollo no dependen de un servidor remoto para abrirse.",
         (560, 575, 465, 390), PURPLE_ACCENT),
    ]
    
    for tag, title, body, (cx, cy, cw, ch), color in cards:
        draw_card(draw, cx, cy, cw, ch, border_color=color, fill_color=(14, 22, 38, 235))
        draw.text((cx + 20, cy + 18), tag, font=FONT_BADGE, fill=color)
        draw.text((cx + 20, cy + 44), title, font=FONT_CARD_TITLE, fill=TEXT_WHITE)
        draw.line([(cx + 20, cy + 74), (cx + cw - 20, cy + 74)], fill=(35, 48, 75), width=1)
        draw_wrapped_text(draw, body, cx + 20, cy + 92, cw - 40, FONT_BODY, TEXT_MUTED, line_spacing=24)

    add_footer(draw, 4)
    return img

# ==============================================================================
# SLIDE 5: EL ECOSISTEMA UBICUO (UNA SOLA MENTE)
# ==============================================================================
def make_slide_5():
    img, draw = create_base_canvas()
    
    draw_header_badge(draw, "ECOSISTEMA • CONTINUIDAD UNIVERSAL", PURPLE_ACCENT, y=45)
    draw.text((55, 88), "TODOS TUS DISPOSITIVOS. UNA SOLA MENTE.", font=FONT_HERO, fill=TEXT_WHITE)
    
    sub = "Una única base de microkernel, compacta y determinista, creada para dar vida e interconectar silenciosamente todo tu universo digital."
    draw_wrapped_text(draw, sub, 55, 140, 970, FONT_SUBTITLE, TEXT_MUTED, line_spacing=24)
    
    quadrants = [
        ("[MÓVILES Y TABLETS]", "Fluidez Táctil y Batería Eterna", 
         "Portabilidad a silicio ARM (aarch64). Huella ultra ligera de ~14 MB de RAM que multiplica la autonomía y ejecuta aplicaciones en espacio Ring 3 seguro.",
         (55, 215, 465, 240), CYAN_ACCENT),
        
        ("[SMART TVs Y PANTALLAS]", "Multimedia Pura Sin Capas Lentas", 
         "Servidor gráfico TrueColor instantáneo. Centro de control domótico y visualización multimedia en tiempo real sin menús pesados ni esperas.",
         (560, 215, 465, 240), PURPLE_ACCENT),
         
        ("[COMPUTADORAS Y LAPTOPS]", "Potencia Creativa Sin Fricción", 
         "Soporte nativo x86_64 multinúcleo (SMP) para Intel y AMD. Entorno Beaver Desktop para desarrollo, edición y productividad fluida.",
         (55, 480, 465, 240), AMBER_ACCENT),
         
        ("[AUTOMOTRIZ Y CRÍTICO (ECUs)]", "Respuesta en Microsegundos", 
         "Determinismo en tiempo real y baja latencia de interrupciones. Tolerancia a fallos donde la vida humana depende de la estabilidad del software.",
         (560, 480, 465, 240), GREEN_ACCENT),
    ]
    
    for tag, title, desc, (qx, qy, qw, qh), color in quadrants:
        draw_card(draw, qx, qy, qw, qh, border_color=color, fill_color=(13, 20, 36, 240))
        draw.text((qx + 20, qy + 18), tag, font=FONT_BODY_BOLD, fill=color)
        draw.text((qx + 20, qy + 44), title, font=FONT_CARD_TITLE, fill=TEXT_WHITE)
        draw.line([(qx + 20, qy + 74), (qx + qw - 20, qy + 74)], fill=(35, 48, 75), width=1)
        draw_wrapped_text(draw, desc, qx + 20, qy + 90, qw - 40, FONT_BODY, TEXT_MUTED, line_spacing=22)

    draw_card(draw, 55, 745, 970, 235, border_color=CYAN_ACCENT, fill_color=(10, 18, 32, 250))
    draw.text((80, 765), "LA PROMESA: CONTINUIDAD MÁGICA Y TEJIDO CONECTIVO TOTAL", font=FONT_SECTION, fill=CYAN_ACCENT)
    
    points = [
        "1. Misma Base Confiable: El mismo microkernel seguro y determinista opera desde el vehículo hasta tu teléfono.",
        "2. Sin Nube Obligatoria: La malla BeaverMesh sincroniza datos en milisegundos directamente en tu red local.",
        "3. Continuidad de Tareas: Comienza un cálculo o documento en tu computadora y proyéctalo o recíbelo en tu tablet al instante."
    ]
    py = 805
    for p in points:
        py = draw_wrapped_text(draw, p, 80, py, 920, FONT_BODY, TEXT_WHITE, line_spacing=22) + 6
        
    draw.text((80, 920), ">> Beaver OS no es solo un SO de escritorio: es la plataforma base para la próxima era de dispositivos inteligentes.", font=FONT_BODY_BOLD, fill=AMBER_ACCENT)

    add_footer(draw, 5)
    return img

# ==============================================================================
# SLIDE 6: EL FUTURO COMIENZA HOY (LLAMADO A LA ACCIÓN & CRÉDITOS)
# ==============================================================================
def make_slide_6():
    img, draw = create_base_canvas()
    
    draw_header_badge(draw, "EL FUTURO • BEAVER OS v0.9.0", CYAN_ACCENT, y=45)
    draw.text((55, 88), "LA COMPUTADORA COMO DEBIÓ HABER SIDO.", font=FONT_HERO, fill=TEXT_WHITE)
    
    sub = "Honesta. Veloz. Segura. Diseñada para empoderar tu creatividad y respetar profundamente tu vida digital."
    draw_wrapped_text(draw, sub, 55, 140, 970, FONT_SUBTITLE, TEXT_MUTED, line_spacing=24)
    
    # Gran Tarjeta Superior de Comunidad y Manifiesto
    draw_card(draw, 55, 200, 970, 485, border_color=CYAN_ACCENT, fill_color=(12, 20, 36, 245))
    
    draw.text((85, 222), "CONSTRUYENDO EL SISTEMA OPERATIVO QUE MERECEMOS", font=FONT_SECTION, fill=CYAN_ACCENT)
    draw.line([(85, 255), (995, 255)], fill=(35, 50, 80), width=1)
    
    p1 = (
        "Beaver OS nació de una convicción inquebrantable: merecemos herramientas limpias, libres de "
        "bloatware corporativo, inmunes a caídas catastróficas y construidas con los más altos estándares "
        "de seguridad moderna en Rust bare-metal."
    )
    y_next = draw_wrapped_text(draw, p1, 85, 275, 910, FONT_SUBTITLE, TEXT_WHITE, line_spacing=25) + 12
    
    p2 = (
        "Hoy el proyecto ha alcanzado la versión v0.9.0, superando auditorías de memoria, multiprocesamiento SMP "
        "y verificación física en silicio real. El código fuente está disponible públicamente para que cualquier "
        "persona pueda estudiarlo, auditarlo y comprobar que la computación honesta es una realidad."
    )
    draw_wrapped_text(draw, p2, 85, y_next, 910, FONT_SUBTITLE, TEXT_WHITE, line_spacing=25)
    
    # Botón / Badge de GitHub Prominente y Elegante
    bx, by, bw, bh = 85, 545, 910, 115
    draw.rounded_rectangle([(bx, by), (bx + bw, by + bh)], radius=12, fill=(18, 30, 55), outline=AMBER_ACCENT, width=2)
    draw.text((bx + 25, by + 16), "REPOSITORIO OFICIAL EN GITHUB", font=FONT_BADGE, fill=AMBER_ACCENT)
    draw.text((bx + 25, by + 42), "https://github.com/japuentem/beaversoft-kernel", font=FONT_URL, fill=TEXT_WHITE)
    draw.text((bx + 25, by + 82), "• Código abierto bajo modelo Source-Available • Explora y deja tu estrella en el proyecto", font=FONT_SMALL, fill=TEXT_MUTED)

    # Tarjeta Inferior de Créditos Institucionales & Marca Registrada
    draw_card(draw, 55, 710, 970, 275, border_color=GREEN_ACCENT, fill_color=(13, 24, 38, 240))
    draw.text((85, 730), "CRÉDITOS INSTITUCIONALES Y TÉRMINOS DE MARCA", font=FONT_CARD_TITLE, fill=GREEN_ACCENT)
    draw.line([(85, 760), (995, 760)], fill=(30, 60, 50), width=1)
    
    legal_bullets = [
        ("• Autor y Creador:", " José Alberto Puente Maldonado (japuentem)."),
        ("• Marca Comercial:", " Beaversoft. Producto y Sistema Operativo: Beaver OS (beaver-kernel)."),
        ("• Términos de Licencia:", " Source-Available con estricta protección de marca. Se permite la auditoría y estudio del código fuente. Queda prohibida la redistribución o forks que utilicen la marca 'BEAVER OS' o 'Beaversoft'; cualquier derivado debe renombrarse por completo."),
    ]
    
    ly = 776
    for b_title, b_desc in legal_bullets:
        bbox_t = FONT_BODY_BOLD.getbbox(b_title)
        tw = bbox_t[2] - bbox_t[0]
        draw.text((85, ly), b_title, font=FONT_BODY_BOLD, fill=GREEN_ACCENT)
        # Dibujamos la descripción continuando o en las siguientes líneas
        ly = draw_wrapped_text(draw, b_desc.strip(), 85 + tw + 8, ly, 910 - tw - 8, FONT_BODY, TEXT_MUTED, line_spacing=22) + 8

    draw.text((85, 950), ">> Si compartes esta visión de un futuro digital libre y sereno, acompáñanos en este viaje.", font=FONT_BODY_BOLD, fill=CYAN_ACCENT)

    add_footer(draw, 6)
    return img

# ==============================================================================
# PROCESO DE COMPILACIÓN (PDF & PPTX)
# ==============================================================================
def main():
    print("=" * 70)
    print("BEAVERSOFT • GENERADOR DE CARRUSEL DE EXPERIENCIA (BEAVER OS)")
    print("=" * 70)
    
    print("\n[1/3] Renderizando diapositivas en formato 1080x1080...")
    s1 = make_slide_1()
    s2 = make_slide_2()
    s3 = make_slide_3()
    s4 = make_slide_4()
    s5 = make_slide_5()
    s6 = make_slide_6()
    
    slides = [s1, s2, s3, s4, s5, s6]
    
    out_dir = "docs/experience_carousel"
    os.makedirs("docs", exist_ok=True)
    os.makedirs(out_dir, exist_ok=True)
    
    img_paths = []
    for idx, slide in enumerate(slides):
        p = f"{out_dir}/slide_{idx+1:02d}.png"
        slide.save(p)
        img_paths.append(p)
        print(f"  * Diapositiva {idx+1}/6 guardada: {p}")
        
    pdf_path = "docs/beaver_os_experience_carousel.pdf"
    print(f"\n[2/3] Compilando documento PDF para carrusel: {pdf_path}...")
    from reportlab.pdfgen import canvas
    c = canvas.Canvas(pdf_path, pagesize=(1080, 1080))
    for p in img_paths:
        c.drawImage(p, 0, 0, width=1080, height=1080)
        c.showPage()
    c.save()
    pdf_size_kb = os.path.getsize(pdf_path) / 1024
    print(f"  [OK] PDF generado con éxito ({pdf_size_kb:.1f} KB)")
    
    pptx_path = "docs/beaver_os_experience_carousel.pptx"
    print(f"\n[3/3] Compilando presentación PowerPoint (PPTX): {pptx_path}...")
    prs = Presentation()
    prs.slide_width = Inches(10)
    prs.slide_height = Inches(10)
    blank_layout = prs.slide_layouts[6]
    
    for p in img_paths:
        slide = prs.slides.add_slide(blank_layout)
        slide.shapes.add_picture(p, Inches(0), Inches(0), width=Inches(10), height=Inches(10))
        
    prs.save(pptx_path)
    pptx_size_kb = os.path.getsize(pptx_path) / 1024
    print(f"  [OK] PowerPoint generado con éxito ({pptx_size_kb:.1f} KB)")
    
    print("\n" + "=" * 70)
    print("PROCESO COMPLETADO SATISFACTORIAMENTE:")
    print(f"  - PDF:  {pdf_path}")
    print(f"  - PPTX: {pptx_path}")
    print(f"  - PNGs: {out_dir}/")
    print("=" * 70)

if __name__ == "__main__":
    main()
