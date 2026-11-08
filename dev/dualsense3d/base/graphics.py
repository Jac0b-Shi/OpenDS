"""Cairo counterparts of the Windows controller, peak envelopes and history view."""
import math
import re
import cairo
import gi
gi.require_version("PangoCairo", "1.0")
from gi.repository import Gtk, Pango, PangoCairo

BLUE = (0.14, 0.45, 0.95)
ORANGE = (0.95, 0.49, 0.16)


def rgb(value, alpha=1):
    if isinstance(value, str):
        value = int(value.lstrip("#"), 16)
    return ((value >> 16 & 255) / 255, (value >> 8 & 255) / 255, (value & 255) / 255, alpha)


def text(cr, x, y, value, size=12, color=(0.18, 0.22, 0.3), bold=False):
    cr.set_source_rgba(*color) if len(color) == 4 else cr.set_source_rgb(*color)
    cr.select_font_face("Sans", cairo.FONT_SLANT_NORMAL, cairo.FONT_WEIGHT_BOLD if bold else cairo.FONT_WEIGHT_NORMAL)
    cr.set_font_size(size)
    if any(ord(character)>127 for character in str(value)):
        layout=PangoCairo.create_layout(cr)
        description=Pango.FontDescription("Sans")
        description.set_absolute_size(size*Pango.SCALE)
        if bold:description.set_weight(Pango.Weight.BOLD)
        layout.set_font_description(description);layout.set_text(str(value),-1)
        cr.move_to(x,y-size);PangoCairo.show_layout(cr,layout)
        return
    cr.move_to(x, y)
    cr.show_text(str(value))


def rect(cr, x, y, w, h, r, color):
    if w <= 0 or h <= 0:
        return
    r = min(r, w / 2, h / 2)
    cr.new_sub_path()
    for cx, cy, start in ((x+w-r, y+r, -math.pi/2), (x+w-r, y+h-r, 0), (x+r, y+h-r, math.pi/2), (x+r, y+r, math.pi)):
        cr.arc(cx, cy, r, start, start+math.pi/2)
    cr.close_path()
    cr.set_source_rgba(*color) if len(color) == 4 else cr.set_source_rgb(*color)
    cr.fill()


def circle(cr, x, y, r, color):
    cr.arc(x, y, r, 0, 2*math.pi)
    cr.set_source_rgb(*color)
    cr.fill()


def controller(cr, x, y, width, state, pressed, edge):
    """Use the same 440-unit paths, positions and button masks as monitor_view.rs."""
    s = state or {}
    sticks = s.get("sticks", [128]*4)
    triggers = s.get("triggers", [0, 0])
    b = [v | p for v, p in zip(s.get("buttons", [8, 0, 0]), pressed or [0, 0, 0])]
    cr.save()
    cr.translate(x, y)
    cr.scale(width / 440, width / 440)
    def path(start, curves, color):
        cr.move_to(*start)
        for points in curves:
            cr.curve_to(*points)
        cr.close_path()
        cr.set_source_rgb(*color)
        cr.fill()
    path((80,80), [(45,85,25,180,42,240),(60,282,92,220,125,200),(155,187,285,187,315,200),(348,220,380,282,398,240),(415,180,395,85,360,80),(340,45,300,35,270,44),(245,48,195,48,170,44),(140,35,100,45,80,80)], (.12,.15,.21))
    path((75,86), [(74,55,135,42,169,52),(198,58,242,58,271,52),(305,42,366,55,365,86),(389,114,400,207,384,228),(375,242,347,195,325,180),(285,159,155,159,115,180),(93,195,65,242,56,228),(40,207,51,114,75,86)], (.89,.92,.96))
    rect(cr,151,64,138,73,12,(.14,.17,.23))
    rect(cr,157,69,126,58,9,BLUE if b[2]&2 else (.21,.25,.32))
    rect(cr,155,61,130,3,1,BLUE)
    for active, tx, ty in s.get("touch", []):
        if active:
            circle(cr,160+min(tx,1919)/1919*120,72+min(ty,1079)/1079*50,5,(.43,.84,1))
    def button(cx, cy, label, on, radius):
        circle(cr,cx,cy+2,radius,(.06,.08,.12))
        circle(cr,cx,cy,radius,BLUE if on else (.20,.24,.30))
        cr.select_font_face("Sans"); cr.set_font_size(12 if radius > 10 else 9)
        tw=cr.text_extents(label).width
        text(cr,cx-tw/2,cy+4,label,12 if radius>10 else 9,(.96,.97,1))
    for cx,cy,label,mask in [(325,91,"△",128),(348,115,"○",64),(325,139,"×",32),(302,115,"□",16)]:
        button(cx,cy,label,b[0]&mask,15)
    dpad = [1,3,2,6,4,12,8,9,0][min(s.get("buttons",[8])[0]&15,8)] | (pressed[0]&15 if pressed else 0)
    for cx,cy,label,mask in [(100,91,"↑",1),(123,115,"→",2),(100,139,"↓",4),(77,115,"←",8)]:
        button(cx,cy,label,dpad&mask,13)
    for cx, sx, sy, mask in [(165,sticks[0],sticks[1],64),(275,sticks[2],sticks[3],128)]:
        circle(cr,cx,170,26,(.08,.10,.15)); circle(cr,cx,170,23,(.23,.27,.34))
        circle(cr,cx+(sx-128)/128*12,170+(sy-128)/128*12,17,BLUE if b[1]&mask else (.12,.15,.21))
    button(220,171,"PS",b[2]&1,13)
    rect(cr,209,196,22,7,3,ORANGE if b[2]&4 else (.32,.38,.45))
    if b[2]&4:
        text(cr,202,221,"Muted",9,ORANGE)
    button(137,83,"C",b[1]&16,9); button(301,75,"≡",b[1]&32,9)
    for cx,label,value,mask in [(66,"L",triggers[0],1),(301,"R",triggers[1],2)]:
        rect(cr,cx,20,72,15,5,BLUE if b[1]&mask else (.21,.25,.32))
        text(cr,cx+26,31,label+"1",10,(1,1,1))
        rect(cr,cx,0,72,13,4,(.8,.83,.88)); rect(cr,cx,0,72*value/255,13,4,BLUE)
        text(cr,cx,-10,f"{label}2 {value*100//255}%",11,(.42,.48,.58))
    if edge:
        for i,(label,mask) in enumerate([("Fn L",16),("Fn R",32),("Back L",64),("Back R",128)]):
            rect(cr,40+i*95,263,88,27,6,BLUE if b[2]&mask else (.8,.83,.88))
            text(cr,47+i*95,281,label,11)
    cr.restore()


class MonitorCanvas(Gtk.DrawingArea):
    def __init__(self, owner):
        super().__init__()
        self.owner = owner
        self.set_size_request(650, 355)
        self.connect("draw", self.draw)

    def draw(self, widget, cr):
        app=self.owner; state=app.state; width=self.get_allocated_width(); height=self.get_allocated_height()
        dark=app.dark; ink=(.88,.91,.96) if dark else (.15,.19,.27); muted=(.57,.63,.72) if dark else (.43,.48,.57)
        left=min(width*.47,470)
        text(cr,14,24,app.T("monitor-view-live-input-buttons-sticks-and-touchpad","Live input · buttons, sticks and touchpad"),12,ink,True)
        pad=state.get("input",{}) if state.get("connected") or not state.get("capture") else {}
        padwidth=min(left,max(220,(height-136)*440/290))
        controller(cr,(left-padwidth)/2,62,padwidth,pad,state.get("pressed",[0,0,0]),state.get("edge",False))
        y=height-52
        sticks=pad.get("sticks",[128]*4)
        text(cr,18,y,f"LS   X {sticks[0]:3}   Y {sticks[1]:3}       RS   X {sticks[2]:3}   Y {sticks[3]:3}",11,muted)
        text(cr,18,y+22,"Gyro " + " / ".join(map(str,pad.get("gyro",[0]*3))) + "    Accel " + " / ".join(map(str,pad.get("accel",[0]*3))),10,muted)
        connected=app.T("monitor-view-connected","Connected") if state.get("connected") else app.T("monitor-view-waiting-for-controller","Waiting for controller")
        text(cr,18,y+44,connected,11,BLUE if state.get("connected") else muted)
        x=left+18; w=width-x-16
        text(cr,x,24,app.T("monitor-view-blue-source-orange-dots-forwarded","Blue: source / orange dots: forwarded"),11,muted)
        waves=state.get("waves",[]); rowheight=(height-46)/3
        titles=[app.T("monitor-view-speaker-front-l-r","Speaker · front L / R"),app.T("monitor-view-haptics-rear-l-r","Haptics · rear L / R"),app.T("monitor-view-microphone-mono","Microphone · mono")]
        states=state.get("wave_states",["Waiting"]*3)
        for group in range(3):
            top=38+group*rowheight; bottom=top+rowheight-12
            rect(cr,x,top,w,rowheight-10,8,(.10,.13,.19) if dark else (.95,.97,.99))
            text(cr,x+10,top+19,titles[group],11,ink,True)
            caption={"Waiting":"No data received yet","NoSignal":"No audio signal selected","Interrupted":"Data interrupted > 500 ms","Silent":"Silent · valid zero amplitude PCM","Active":"Active"}.get(states[group],states[group])
            if group==2 and not state.get("microphone"):
                caption=app.T("monitor-view-microphone-disabled","Microphone disabled")
            text(cr,x+10,bottom-6,caption,9,muted)
            lanes=[0,1] if group==0 else [2,3] if group==1 else [4]
            for lane_index,lane in enumerate(lanes):
                cy=top+38+lane_index*max(14,(rowheight-60)/2)
                amplitude=max(5,(rowheight-70)/(2*len(lanes)))
                cr.set_source_rgb(*muted); cr.set_line_width(.4);cr.move_to(x+31,cy);cr.line_to(x+w-8,cy);cr.stroke()
                text(cr,x+8,cy+3,"M" if group==2 else "LR"[lane_index],8,muted)
                for forwarded,color in [(False,BLUE),(True,ORANGE)]:
                    if not state.get("forwarded" if forwarded else "source",False):continue
                    channel=lane+(5 if forwarded else 0)
                    cr.set_source_rgb(*color);cr.set_line_width(1.2)
                    for index,bucket in enumerate(waves):
                        low,high,valid,gap=bucket
                        if not valid&(1<<channel) or gap&(1<<channel):continue
                        px=x+32+index/max(1,len(waves)-1)*(w-42)
                        if forwarded and index%3==1:continue
                        cr.move_to(px,cy-low[channel]/32768*amplitude);cr.line_to(px,cy-high[channel]/32768*amplitude)
                    cr.stroke()
        text(cr,x,height-2,f"−{app.get('monitor_seconds','60')} s                                               now",9,muted)
        return False


class HistoryCanvas(Gtk.DrawingArea):
    def __init__(self, owner, overlay=False):
        super().__init__()
        self.owner, self.overlay = owner, overlay
        self.set_size_request(320, 150 if not overlay else 100)
        self.connect("draw", self.draw)

    def draw(self, widget, cr):
        app = self.owner
        width, height = self.get_allocated_width(), self.get_allocated_height()
        dark = app.dark if not self.overlay or app.get("command_theme", "0") == "0" else app.get("command_theme") == "2"
        automatic = app.flag("command_auto_colors", True)
        manual_dark = dark and not automatic
        color_prefix = "dark_" if manual_dark else ""
        def color(name, fallback, semantic=False):
            value = app.get("command_" + color_prefix + name, fallback)
            value = int(value.lstrip("#"), 16) if isinstance(value, str) else value
            return (0xffffff-value if value in (0, 0xffffff) and dark and automatic and not semantic else value)
        mode = 1 if manual_dark else 0
        bg = color(f"background_{mode}", "#000000" if manual_dark else "#ffffff")
        fg = color(f"text_{mode}", "#ffffff" if manual_dark else "#000000")
        # Background/text have numbered palettes rather than dark_ keys.
        for name in ("background", "text"):
            value = app.get(f"command_{name}_{mode}", "#ffffff" if (name=="background") != manual_dark else "#000000")
            value = int(value.lstrip("#"),16)
            if dark and automatic and value in (0,0xffffff):value=0xffffff-value
            if name=="background":bg=value
            else:fg=value
        ba = int(app.get("command_bg_alpha", "85"))/100 if self.overlay else 1
        fa = int(app.get("command_fg_alpha", "100"))/100 if self.overlay else 1
        if self.overlay:
            cr.set_operator(cairo.OPERATOR_SOURCE)
            cr.set_source_rgba(0,0,0,0)
            cr.paint()
            cr.set_operator(cairo.OPERATOR_OVER)
        rect(cr,0,0,width,height,0 if self.overlay else 8,rgb(bg,ba))
        scale = int(app.get("command_content_size", "100"))/100
        size, line = 12*scale, 30*scale
        automatic_rows = self.overlay and app.flag("command_overlay_auto_rows", True)
        limit = 64 if automatic_rows else int(app.get("command_overlay_rows" if self.overlay else "command_rows", "8"))
        events = app.state.get("events", [])[:min(limit,64)] if app.flag("command_enabled", True) else []
        if not events:
            text(cr,14,28,app.T("monitor-view-waiting-for-manual-start", "Waiting for manual start"),12,rgb(fg,fa*.7))
        def measure(value):
            layout = PangoCairo.create_layout(cr)
            description = Pango.FontDescription("Sans")
            description.set_absolute_size(size*Pango.SCALE)
            layout.set_font_description(description)
            layout.set_text(value,-1)
            return layout.get_pixel_size()[0]
        y = 0
        for index,event in enumerate(events):
            source = int(app.get("command_source", "2"))
            kind = event.get("kind", "input")
            tokens = []
            if kind != "input":
                key = {"connected":"command-connected","disconnected":"command-disconnected","gap":"command-gap","settings":"command-boundary"}.get(kind, "")
                tokens.append((app.T(key,kind.title()),color("connection","#000000"),None))
            else:
                for token in event.get("combination", "").split(" + "):
                    icon, semantic = None, False
                    if token.startswith(("LS ","RS ")):
                        axis = 0 if token.startswith("LS") else 1
                        # Source labels apply to each stick/d-pad token, as on Windows.
                        strength = event.get("strength",[0]*4)[axis]
                        if strength == 0:ink=color("neutral","#808080")
                        else:
                            semantic=True
                            key=("darkunified" if manual_dark else "unified")+f"_{axis}" if strength==4 else ("darkcolor" if manual_dark else "color")+f"_{axis}_{strength-1}"
                            ink=app.get("command_"+key,"#399adb")
                        if source!=0:token=token.split(" ",1)[1]
                        if source==1:icon=axis
                    elif token.startswith("DP "):
                        ink=color("dpad","#808080")
                        if source!=0:token=token[3:]
                        if source==1:icon=2
                    elif token.startswith(("L2","R2")):
                        axis=2 if token.startswith("L2") else 3
                        strength=event.get("strength",[0]*4)[axis]
                        key=("darkunified" if manual_dark else "unified")+f"_{axis}" if strength==4 else ("darkcolor" if manual_dark else "color")+f"_{axis}_{max(0,strength-1)}"
                        ink=app.get("command_"+key,"#399adb")
                        token=token.split(":")[0]
                    else:ink=color("button","#000000")
                    tokens.append((token,ink,icon))
            # Lay out complete tokens, wrapping combinations rather than clipping them.
            inline_duration = width >= 430*scale
            right = width-(108*scale if inline_duration else 12)
            positions=[]
            x, baseline=12, y+21*scale
            for token,ink,icon in tokens:
                tw=max(30*scale,measure(token)+14*scale+(20*scale if icon is not None else 0))
                if x+tw>right and x>12:x=12;baseline+=line
                positions.append((x,baseline,tw,token,ink,icon))
                x+=tw+5*scale
            rowheight=max(line,baseline-y+10*scale)+(0 if inline_duration else 19*scale)
            if automatic_rows and y+rowheight>height:break
            if y>=height:break
            if index==0:rect(cr,0,y,width,rowheight,0,(*BLUE,.10*fa))
            for x,baseline,tw,token,ink,icon in positions:
                rgba=rgb(ink,fa)
                rect(cr,x,baseline-19*scale,tw,25*scale,4*scale,(*rgba[:3],.10*fa))
                tx=x+7*scale
                if icon is not None:
                    cr.set_source_rgba(*rgba);cr.set_line_width(1.2*scale)
                    cx,cy=tx+7*scale,baseline-5*scale
                    cr.arc(cx,cy,6*scale,0,2*math.pi);cr.stroke()
                    if icon==2:
                        cr.move_to(cx-4*scale,cy);cr.line_to(cx+4*scale,cy);cr.move_to(cx,cy-4*scale);cr.line_to(cx,cy+4*scale);cr.stroke()
                    else:
                        cr.arc(cx+(-2 if icon==0 else 2)*scale,cy-2*scale,2*scale,0,2*math.pi);cr.fill()
                    tx+=20*scale
                text(cr,tx,baseline,token,size,rgba,index==0)
            text(cr,width-98*scale if inline_duration else 14,y+22*scale if inline_duration else y+rowheight-5*scale,event.get("duration",""),10*scale,rgb(fg,fa*.65))
            if app.flag("command_separators",True):
                alpha=int(app.get("command_separator_alpha","55"))/100*fa
                ink=color("separator","#808080")
                style=int(app.get("command_separator_style","1"))
                if style==0:cr.set_source_rgba(*rgb(ink,alpha))
                else:
                    gradient=cairo.LinearGradient(0,0,width,0)
                    for stop,a in [(0,0),(.15,alpha),(.85,alpha),(1,0)]:gradient.add_color_stop_rgba(stop,*rgb(ink,a))
                    cr.set_source(gradient)
                cy=y+rowheight-1
                if style==2:
                    cr.move_to(8,cy);cr.line_to(width*.18,cy-1);cr.line_to(width*.82,cy-1);cr.line_to(width-8,cy);cr.line_to(width*.82,cy+1);cr.line_to(width*.18,cy+1);cr.close_path();cr.fill()
                else:
                    cr.set_line_width(1);cr.move_to(8,cy);cr.line_to(width-8,cy);cr.stroke()
            y+=rowheight
        return False
