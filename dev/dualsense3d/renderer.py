"""Native GTK/OpenGL 3.3 viewer with PBR materials, shadows and animated controls."""
import ctypes
import math
from pathlib import Path
import sys
import time
import numpy as np
import gi
gi.require_version("Gtk","3.0")
gi.require_version("Gdk","3.0")
from gi.repository import Gtk,Gdk,GdkPixbuf,GLib

HERE=Path(__file__).resolve().parent
sys.path.insert(0,str(HERE/'vendor/python'))
from OpenGL import GL
from OpenGL.raw.GL.VERSION.GL_2_0 import glVertexAttribPointer
from scene import Scene,Animator,targets,translate,rotate,perspective,orthographic,look_at,normalize
from appearance import COLORS,linear_rgb,camouflage_surface


def program(vertex,fragment):
    shaders=[]
    for filename,kind in [(vertex,GL.GL_VERTEX_SHADER),(fragment,GL.GL_FRAGMENT_SHADER)]:
        shader=GL.glCreateShader(kind)
        GL.glShaderSource(shader,(HERE/'shaders'/filename).read_text())
        GL.glCompileShader(shader)
        if not GL.glGetShaderiv(shader,GL.GL_COMPILE_STATUS):raise RuntimeError(GL.glGetShaderInfoLog(shader).decode())
        shaders.append(shader)
    result=GL.glCreateProgram()
    for shader in shaders:GL.glAttachShader(result,shader)
    GL.glLinkProgram(result)
    if not GL.glGetProgramiv(result,GL.GL_LINK_STATUS):raise RuntimeError(GL.glGetProgramInfoLog(result).decode())
    for shader in shaders:GL.glDeleteShader(shader)
    return result


class ControllerView(Gtk.GLArea):
    def __init__(self,owner):
        super().__init__()
        self.owner=owner
        self.set_required_version(3,3)
        self.set_use_es(False)
        self.set_has_depth_buffer(True)
        self.set_has_alpha(True)
        self.set_auto_render(False)
        self.set_can_focus(True)
        self.set_size_request(420,410)
        self.add_events(Gdk.EventMask.BUTTON_PRESS_MASK|Gdk.EventMask.BUTTON_RELEASE_MASK|Gdk.EventMask.POINTER_MOTION_MASK|Gdk.EventMask.SCROLL_MASK|Gdk.EventMask.SMOOTH_SCROLL_MASK|Gdk.EventMask.LEAVE_NOTIFY_MASK|Gdk.EventMask.KEY_PRESS_MASK)
        self.connect('realize',self.realize_gl)
        self.connect('render',self.render_gl)
        self.connect('unrealize',self.release_gl)
        self.connect('button-press-event',self.press)
        self.connect('button-release-event',self.release)
        self.connect('motion-notify-event',self.motion)
        self.connect('scroll-event',self.scroll)
        self.connect('key-press-event',self.key)
        self.connect('focus-out-event',self.release)
        self.connect('leave-notify-event',self.leave)
        self.connect('map',self.mapped)
        self.connect('unmap',self.unmapped)
        self.scene=Scene(HERE/'assets/dualsense.glb')
        self.animation=Animator()
        self.demo=False;self.follow_tilt=False;self.tilt_reference=None
        self.color_scheme='white';self.transparent_background=True
        self.tilt=np.zeros(2,dtype=np.float32)
        self.preview={};self.touch_preview=None;self.hover=None;self.drag=None
        self.failure=None;self.ready=False;self.frame_count=0;self.tick_id=None
        self.last=time.monotonic();self.started=self.last;self.changed=True
        self.transforms={};self.texture_cache={};self.resources=[]
        self.reset_camera()

    def reset_camera(self):
        self.yaw=.29;self.pitch=.30;self.distance=9.0
        self.target=np.array([0,.05,0],dtype=np.float32)
        self.tilt_reference=None;self.tilt[:]=0
        self.changed=True;self.queue_render()

    def front_camera(self):
        self.yaw=0;self.pitch=.04;self.distance=9.0;self.changed=True;self.queue_render()

    def mapped(self,*_):
        self.last=time.monotonic()
        if self.tick_id is None:self.tick_id=self.add_tick_callback(self.tick)

    def unmapped(self,*_):
        if self.tick_id is not None:self.remove_tick_callback(self.tick_id);self.tick_id=None

    def effective_input(self):
        data=self.owner.state
        if self.demo:
            phase=time.monotonic()-self.started
            step=int(phase*1.5)%16
            faces=[16,32,64,128][step] if step<4 else 0
            dpad=[0,2,4,6][step-4] if 4<=step<8 else 8
            shoulder={8:1,9:2,10:16,11:32,15:192}.get(step,0)
            special={12:1,13:4,14:2}.get(step,0)
            state={'sticks':[int(128+100*math.sin(phase*1.2)),int(128+96*math.cos(phase*1.2)),int(128+80*math.sin(phase*.8)),int(128+80*math.cos(phase*.8))],
                   'triggers':[int(127.5*(1+math.sin(phase*.9))),int(127.5*(1+math.cos(phase*.9)))],
                   'buttons':[dpad|faces,shoulder,special],
                   'touch':[[1,int(960+700*math.sin(phase)),int(540+330*math.cos(phase*.7))]],'accel':[0,0,8000]}
            pressed=[0,0,0]
        elif data.get('connected') or not data.get('capture'):
            state=data.get('input',{});pressed=data.get('pressed',[0,0,0])
        else:state={};pressed=[0,0,0]
        values=targets(state,pressed)
        values.update(self.preview)
        touches=self.touch_preview if self.touch_preview is not None else state.get('touch',[])
        return state,values,touches

    def tick(self,*_):
        now=time.monotonic();dt=min(now-self.last,.05);self.last=now
        state,goal,touches=self.effective_input()
        changed=self.animation.step(goal,dt)
        if self.follow_tilt and self.owner.state.get('connected'):
            acceleration=np.asarray(state.get('accel',[0,0,0]),dtype=np.float32)
            if np.linalg.norm(acceleration)>100:
                ax,ay,az=acceleration
                current=np.array([math.atan2(ay,az),math.atan2(ax,math.sqrt(ay*ay+az*az))])
                if self.tilt_reference is None:self.tilt_reference=current
                wanted=np.clip((current-self.tilt_reference+math.pi)%(2*math.pi)-math.pi,-.60,.60)
                self.tilt+=(wanted-self.tilt)*min(1,dt*8);changed=True
        elif np.linalg.norm(self.tilt)>.001:self.tilt*=max(0,1-dt*10);changed=True
        signature=(str(touches),self.hover,self.owner.state.get('connected'),self.demo)
        if signature!=getattr(self,'signature',None):self.signature=signature;changed=True
        if changed or self.changed:self.queue_render();self.changed=False
        return True

    def realize_gl(self,*_):
        self.make_current()
        try:
            if self.get_error():raise RuntimeError(str(self.get_error()))
            self.resources=[];self.texture_cache={};self.failure=None
            self.gl_version=GL.glGetString(GL.GL_VERSION).decode()
            self.gl_renderer=GL.glGetString(GL.GL_RENDERER).decode()
            self.shader=program('model.vert','model.frag')
            self.depth_shader=program('depth.vert','depth.frag')
            self.uniforms={}
            for shader in [self.shader,self.depth_shader]:
                self.uniforms[shader]={name:GL.glGetUniformLocation(shader,name) for name in ['model','view','projection','lightVP','eye','baseColor','metallic','roughness','normalScale','emissive','highlight','hasBase','hasNormal','hasRoughness','floorSurface','baseMap','normalMap','roughnessMap','shadowMap','camouflage','darkDecal','arrowDirection','buttonBounds']}
            for part in self.scene.parts:self.upload(part)
            self.floor={'positions':np.array([[-18,-1.83,-18],[18,-1.83,-18],[18,-1.83,18],[-18,-1.83,18]],np.float32),'normals':np.tile([0,1,0],(4,1)).astype(np.float32),'uv':np.array([[0,0],[1,0],[1,1],[0,1]],np.float32),'indices':np.array([0,2,1,0,3,2],np.uint32)}
            self.upload(self.floor)
            self.marker=self.make_marker()
            self.upload(self.marker)
            self.shadow_fbo=GL.glGenFramebuffers(1);self.shadow_texture=GL.glGenTextures(1)
            previous=int(GL.glGetIntegerv(GL.GL_DRAW_FRAMEBUFFER_BINDING))
            GL.glBindTexture(GL.GL_TEXTURE_2D,self.shadow_texture)
            GL.glTexImage2D(GL.GL_TEXTURE_2D,0,GL.GL_DEPTH_COMPONENT24,1024,1024,0,GL.GL_DEPTH_COMPONENT,GL.GL_FLOAT,None)
            GL.glTexParameteri(GL.GL_TEXTURE_2D,GL.GL_TEXTURE_MIN_FILTER,GL.GL_LINEAR);GL.glTexParameteri(GL.GL_TEXTURE_2D,GL.GL_TEXTURE_MAG_FILTER,GL.GL_LINEAR)
            GL.glTexParameteri(GL.GL_TEXTURE_2D,GL.GL_TEXTURE_WRAP_S,GL.GL_CLAMP_TO_BORDER);GL.glTexParameteri(GL.GL_TEXTURE_2D,GL.GL_TEXTURE_WRAP_T,GL.GL_CLAMP_TO_BORDER)
            GL.glTexParameterfv(GL.GL_TEXTURE_2D,GL.GL_TEXTURE_BORDER_COLOR,np.ones(4,np.float32))
            GL.glBindFramebuffer(GL.GL_FRAMEBUFFER,self.shadow_fbo);GL.glFramebufferTexture2D(GL.GL_FRAMEBUFFER,GL.GL_DEPTH_ATTACHMENT,GL.GL_TEXTURE_2D,self.shadow_texture,0)
            GL.glDrawBuffer(GL.GL_NONE);GL.glReadBuffer(GL.GL_NONE)
            if GL.glCheckFramebufferStatus(GL.GL_FRAMEBUFFER)!=GL.GL_FRAMEBUFFER_COMPLETE:raise RuntimeError('Shadow framebuffer is incomplete')
            GL.glBindFramebuffer(GL.GL_FRAMEBUFFER,previous)
            self.light_vp=orthographic(-4.8,4.8,-4.8,4.8,.1,24)@look_at(np.array([-6,10,10],np.float32),np.array([0,0,0],np.float32))
            self.ready=True
            print('OpenDS 3D renderer:',self.gl_version,'—',self.gl_renderer,flush=True)
            self.queue_render()
        except Exception as error:
            self.failure=str(error);print('OpenDS 3D renderer:',error,file=sys.stderr)

    def upload(self,part):
        vertices=np.ascontiguousarray(np.column_stack((part['positions'],part['normals'],part['uv'])),dtype=np.float32)
        part['vao']=GL.glGenVertexArrays(1);part['vbo']=GL.glGenBuffers(1);part['ebo']=GL.glGenBuffers(1)
        GL.glBindVertexArray(part['vao']);GL.glBindBuffer(GL.GL_ARRAY_BUFFER,part['vbo']);GL.glBufferData(GL.GL_ARRAY_BUFFER,vertices.nbytes,vertices,GL.GL_STATIC_DRAW)
        GL.glBindBuffer(GL.GL_ELEMENT_ARRAY_BUFFER,part['ebo']);GL.glBufferData(GL.GL_ELEMENT_ARRAY_BUFFER,part['indices'].nbytes,part['indices'],GL.GL_STATIC_DRAW)
        for slot,size,offset in [(0,3,0),(1,3,12),(2,2,24)]:
            GL.glEnableVertexAttribArray(slot);glVertexAttribPointer(slot,size,GL.GL_FLOAT,False,32,ctypes.c_void_p(offset))
        GL.glBindVertexArray(0)
        self.resources.append(part)

    @staticmethod
    def make_marker():
        positions=[];normals=[];indices=[]
        for row in range(9):
            theta=row/8*math.pi
            for col in range(17):
                phi=col/16*math.pi*2
                normal=[math.sin(theta)*math.cos(phi),math.sin(theta)*math.sin(phi),math.cos(theta)]
                positions.append(np.asarray(normal)*.043);normals.append(normal)
        for row in range(8):
            for col in range(16):
                a=row*17+col;b=a+17;indices.extend([a,b,a+1,a+1,b,b+1])
        return {'positions':np.array(positions,np.float32),'normals':np.array(normals,np.float32),'uv':np.zeros((len(positions),2),np.float32),'indices':np.array(indices,np.uint32)}

    def texture(self,index,srgb=False):
        source=self.scene.document['textures'][index]['source'];key=(source,srgb)
        if key in self.texture_cache:return self.texture_cache[key]
        loader=GdkPixbuf.PixbufLoader();loader.write(self.scene.image(source));loader.close();pixels=loader.get_pixbuf()
        if not pixels.get_has_alpha():pixels=pixels.add_alpha(False,0,0,0)
        width,height=pixels.get_width(),pixels.get_height()
        data=np.frombuffer(pixels.get_pixels(),np.uint8).reshape(height,pixels.get_rowstride())[:,:width*4].copy()
        result=GL.glGenTextures(1);GL.glBindTexture(GL.GL_TEXTURE_2D,result);GL.glPixelStorei(GL.GL_UNPACK_ALIGNMENT,1)
        GL.glTexImage2D(GL.GL_TEXTURE_2D,0,GL.GL_SRGB8_ALPHA8 if srgb else GL.GL_RGBA8,width,height,0,GL.GL_RGBA,GL.GL_UNSIGNED_BYTE,data)
        GL.glGenerateMipmap(GL.GL_TEXTURE_2D)
        GL.glTexParameteri(GL.GL_TEXTURE_2D,GL.GL_TEXTURE_MIN_FILTER,GL.GL_LINEAR_MIPMAP_LINEAR);GL.glTexParameteri(GL.GL_TEXTURE_2D,GL.GL_TEXTURE_MAG_FILTER,GL.GL_LINEAR)
        GL.glTexParameteri(GL.GL_TEXTURE_2D,GL.GL_TEXTURE_WRAP_S,GL.GL_REPEAT);GL.glTexParameteri(GL.GL_TEXTURE_2D,GL.GL_TEXTURE_WRAP_T,GL.GL_REPEAT)
        self.texture_cache[key]=result;return result

    def uniform(self,name,value,shader=None):
        location=self.uniforms[shader or self.shader].get(name,-1)
        if location<0:return
        if isinstance(value,np.ndarray) and value.shape==(4,4):GL.glUniformMatrix4fv(location,1,GL.GL_TRUE,value.astype(np.float32))
        elif isinstance(value,(list,tuple,np.ndarray)):
            (GL.glUniform4f if len(value)==4 else GL.glUniform3f)(location,*value)
        elif name in {'hasBase','hasNormal','hasRoughness','floorSurface','camouflage','darkDecal','arrowDirection','baseMap','normalMap','roughnessMap','shadowMap'}:GL.glUniform1i(location,int(value))
        else:GL.glUniform1f(location,float(value))

    def material(self,index,highlight=0,marker=False,floor=False,part=None):
        material=self.scene.document['materials'][index] if index is not None else {}
        pbr=material.get('pbrMetallicRoughness',{})
        base=pbr.get('baseColorFactor',[.8,.8,.8,1])
        emission=material.get('emissiveFactor',[0,0,0])
        if marker:base=[.02,.5,1,1];emission=[.06,.5,1]
        if floor:base=[.026,.038,.065,1];emission=[0,0,0]
        clear_cap=part and part['control'] in ('triangle','square','cross','circle') and '__cap__' in part['name']
        rubber=part and part['control'] in ('left-stick','right-stick')
        shell=part and (part['name'].startswith(('back-shell__','white-shell-','touchpad__')))
        _,tint,finish=COLORS[self.color_scheme]
        region=camouflage_surface(part) if self.color_scheme=='grey-camouflage' else None
        self.uniform('camouflage',region=='pattern')
        self.uniform('darkDecal',region=='decal')
        arrow={'up':1,'right':2,'down':3,'left':4}.get(part['control'],0) if region=='dpad-cap' else 0
        self.uniform('arrowDirection',arrow)
        if arrow:self.uniform('buttonBounds',[*part['min'][:2],*part['max'][:2]])
        if shell and tint:base=linear_rgb(tint)+[base[3]]
        if clear_cap:base=[.80,.86,.94,.20]
        if region=='rear':base=linear_rgb('#4a453d')+[1]
        elif region in ('plastic','dpad-cap'):base=[.007,.008,.010,1]
        elif region=='face-cap':base=[.012,.014,.018,.25]
        self.uniform('baseColor',base);self.uniform('emissive',emission)
        metal=finish if shell and finish is not None else 0 if clear_cap or rubber else pbr.get('metallicFactor',1 if index is not None else 0)
        rough=.32 if shell and finish is not None else .17 if clear_cap else .62 if rubber else .95 if floor else pbr.get('roughnessFactor',1 if index is not None else .45)
        if region:
            metal=0
            rough={'rear':.58,'pattern':.55,'plastic':.45,'dpad-cap':.30,'face-cap':.17,'decal':.36}[region]
            if rubber:rough=.68
        self.uniform('metallic',metal);self.uniform('roughness',rough)
        self.uniform('normalScale',material.get('normalTexture',{}).get('scale',1));self.uniform('highlight',highlight);self.uniform('floorSurface',floor)
        for unit,key,flag,sampler,source in [(0,'baseColorTexture','hasBase','baseMap',pbr),(1,'normalTexture','hasNormal','normalMap',material),(2,'metallicRoughnessTexture','hasRoughness','roughnessMap',pbr)]:
            exists=key in source and not marker and not floor
            self.uniform(flag,exists);self.uniform(sampler,unit)
            GL.glActiveTexture(GL.GL_TEXTURE0+unit);GL.glBindTexture(GL.GL_TEXTURE_2D,self.texture(source[key]['index'],unit==0) if exists else 0)

    def matrices(self):
        eye=self.target+self.distance*np.array([math.sin(self.yaw)*math.cos(self.pitch),math.sin(self.pitch),math.cos(self.yaw)*math.cos(self.pitch)],np.float32)
        self.eye=eye;self.view=look_at(eye,self.target)
        self.projection=perspective(35,max(.1,self.get_allocated_width()/max(1,self.get_allocated_height())))
        body=rotate(2,-float(self.tilt[1]))@rotate(0,float(self.tilt[0]))
        self.body=body
        self.transforms={p['name']:body@self.scene.pose(p['control'],self.animation.values) for p in self.scene.parts}

    def draw_part(self,part):
        GL.glBindVertexArray(part['vao']);GL.glDrawElements(GL.GL_TRIANGLES,len(part['indices']),GL.GL_UNSIGNED_INT,None)

    def render_gl(self,*_):
        if not self.ready:return True
        try:
            self.matrices()
            framebuffer=int(GL.glGetIntegerv(GL.GL_DRAW_FRAMEBUFFER_BINDING))
            scale=self.get_scale_factor();width=self.get_allocated_width()*scale;height=self.get_allocated_height()*scale
            GL.glDisable(GL.GL_CULL_FACE);GL.glEnable(GL.GL_DEPTH_TEST);GL.glDepthFunc(GL.GL_LEQUAL)
            GL.glBindFramebuffer(GL.GL_FRAMEBUFFER,self.shadow_fbo);GL.glViewport(0,0,1024,1024);GL.glClear(GL.GL_DEPTH_BUFFER_BIT)
            GL.glUseProgram(self.depth_shader);self.uniform('lightVP',self.light_vp,self.depth_shader)
            GL.glEnable(GL.GL_POLYGON_OFFSET_FILL);GL.glPolygonOffset(2,4)
            for part in self.scene.parts:
                self.uniform('model',self.transforms[part['name']],self.depth_shader);self.draw_part(part)
            GL.glDisable(GL.GL_POLYGON_OFFSET_FILL)
            GL.glBindFramebuffer(GL.GL_FRAMEBUFFER,framebuffer);GL.glViewport(0,0,width,height)
            GL.glClearColor(*( (0,0,0,0) if self.transparent_background else (.042,.062,.10,1) ));GL.glClear(GL.GL_COLOR_BUFFER_BIT|GL.GL_DEPTH_BUFFER_BIT)
            GL.glUseProgram(self.shader)
            self.uniform('view',self.view);self.uniform('projection',self.projection);self.uniform('eye',self.eye);self.uniform('lightVP',self.light_vp)
            self.uniform('shadowMap',3);GL.glActiveTexture(GL.GL_TEXTURE3);GL.glBindTexture(GL.GL_TEXTURE_2D,self.shadow_texture)
            if not self.transparent_background and self.eye[1]>-1.6:
                GL.glEnable(GL.GL_BLEND);GL.glBlendFuncSeparate(GL.GL_SRC_ALPHA,GL.GL_ONE_MINUS_SRC_ALPHA,GL.GL_ONE,GL.GL_ONE_MINUS_SRC_ALPHA)
                self.uniform('model',np.eye(4,dtype=np.float32));self.material(None,floor=True);self.draw_part(self.floor)
                GL.glDisable(GL.GL_BLEND)
            opaque=[];transparent=[]
            for part in self.scene.parts:
                clear_cap=part['control'] in ('triangle','square','cross','circle') and '__cap__' in part['name']
                (transparent if clear_cap or self.scene.document['materials'][part['material']].get('alphaMode')=='BLEND' else opaque).append(part)
            transparent.sort(key=lambda p:float(np.linalg.norm(self.eye-(self.transforms[p['name']]@np.append((p['min']+p['max'])/2,1))[:3])),reverse=True)
            for blend,parts in [(False,opaque),(True,transparent)]:
                if blend:GL.glEnable(GL.GL_BLEND);GL.glBlendFuncSeparate(GL.GL_SRC_ALPHA,GL.GL_ONE_MINUS_SRC_ALPHA,GL.GL_ONE,GL.GL_ONE_MINUS_SRC_ALPHA);GL.glDepthMask(False)
                for part in parts:
                    control=part['control'];value=self.animation.values.get(control,0)
                    highlight=max(value*.16,.32 if control and control==self.hover else 0)
                    self.uniform('model',self.transforms[part['name']]);self.material(part['material'],highlight,part=part);self.draw_part(part)
                if blend:GL.glDepthMask(True);GL.glDisable(GL.GL_BLEND)
            state,values,touches=self.effective_input()
            for active,tx,ty in touches:
                if not active:continue
                x=(tx/1919-.5)*1.88;y=1.53-ty/1079*.90;z=self.scene.touch_height(x,y)
                self.uniform('model',self.body@self.scene.pose('touchpad',self.animation.values)@translate(x,y,z+.035));self.material(None,marker=True);self.draw_part(self.marker)
            GL.glBindVertexArray(0);GL.glUseProgram(0)
            self.frame_count+=1
        except Exception as error:
            self.failure=str(error);self.ready=False
            try:GL.glBindFramebuffer(GL.GL_FRAMEBUFFER,framebuffer)
            except Exception:pass
            print('OpenDS 3D frame:',error,file=sys.stderr)
        return True

    def release_gl(self,*_):
        self.unmapped()
        if not self.ready:return
        self.make_current()
        try:
            for part in self.resources:
                GL.glDeleteVertexArrays(1,[part['vao']]);GL.glDeleteBuffers(1,[part['vbo']]);GL.glDeleteBuffers(1,[part['ebo']])
            GL.glDeleteTextures(list(self.texture_cache.values())+[self.shadow_texture]);GL.glDeleteFramebuffers(1,[self.shadow_fbo])
            GL.glDeleteProgram(self.shader);GL.glDeleteProgram(self.depth_shader)
        except Exception:pass
        self.ready=False

    def ray(self,x,y):
        self.matrices();inverse=np.linalg.inv(self.projection@self.view)
        nx=x/max(1,self.get_allocated_width())*2-1;ny=1-y/max(1,self.get_allocated_height())*2
        near=inverse@np.array([nx,ny,-1,1]);far=inverse@np.array([nx,ny,1,1]);near=near[:3]/near[3];far=far[:3]/far[3]
        return near,normalize(far-near)

    def hit(self,x,y):
        if not self.ready:return None
        origin,direction=self.ray(x,y)
        return self.scene.pick(origin,direction,self.transforms)

    def press(self,widget,event):
        self.grab_focus()
        if event.type==Gdk.EventType.DOUBLE_BUTTON_PRESS:self.reset_camera();return True
        control=self.hit(event.x,event.y) if event.button==1 and not event.state&Gdk.ModifierType.SHIFT_MASK else None
        self.drag={'x':event.x,'y':event.y,'yaw':self.yaw,'pitch':self.pitch,'control':control,'button':event.button}
        if control:
            self.preview[control]=.6 if control in ('l2','r2') else 1
            if control=='touchpad':self.touch_preview=[[1,960,540]]
        self.changed=True;return True

    def motion(self,widget,event):
        if self.drag:
            dx=event.x-self.drag['x'];dy=event.y-self.drag['y'];control=self.drag['control']
            if control in ('left-stick','right-stick'):
                self.preview[control+'-x']=max(-1,min(1,dx/55));self.preview[control+'-y']=max(-1,min(1,dy/55))
                if math.hypot(dx,dy)>4:self.preview[control]=0
            elif control in ('l2','r2'):self.preview[control]=max(0,min(1,.6+dy/100))
            elif control=='touchpad':self.touch_preview=[[1,int(max(0,min(1919,960+dx*12))),int(max(0,min(1079,540+dy*12)))]]
            elif not control:
                self.yaw=self.drag['yaw']-dx*.009;self.pitch=max(-1.10,min(1.25,self.drag['pitch']+dy*.007))
            self.changed=True
        elif time.monotonic()-getattr(self,'last_hover',0)>.06:
            self.last_hover=time.monotonic();self.hover=self.hit(event.x,event.y);self.changed=True
        return True

    def release(self,*_):
        self.drag=None;self.preview.clear();self.touch_preview=None;self.changed=True;return True

    def leave(self,*_):
        self.hover=None
        if not self.drag:self.preview.clear();self.touch_preview=None
        self.changed=True;return False

    def scroll(self,widget,event):
        delta=event.get_scroll_deltas()[2] if event.direction==Gdk.ScrollDirection.SMOOTH else 1 if event.direction==Gdk.ScrollDirection.DOWN else -1
        self.distance=max(6,min(18,self.distance*math.exp(delta*.10)));self.changed=True;return True

    def key(self,widget,event):
        if event.keyval in (Gdk.KEY_r,Gdk.KEY_R,Gdk.KEY_Home):self.reset_camera();return True
        if event.keyval==Gdk.KEY_Escape:self.release();return True
        return False

    def capture_png(self,path):
        self.make_current();self.attach_buffers()
        scale=self.get_scale_factor();width=self.get_allocated_width()*scale;height=self.get_allocated_height()*scale
        GL.glPixelStorei(GL.GL_PACK_ALIGNMENT,1)
        pixels=GL.glReadPixels(0,0,width,height,GL.GL_RGBA,GL.GL_UNSIGNED_BYTE)
        # GL/GTK compositing uses premultiplied alpha; PNG expects straight alpha.
        rgba=np.frombuffer(pixels,np.uint8).reshape(height,width,4).copy()
        alpha=rgba[:,:,3:4].astype(np.float32)
        rgba[:,:,:3]=np.minimum(255,rgba[:,:,:3].astype(np.float32)*255/np.maximum(alpha,1)).astype(np.uint8)
        pixbuf=GdkPixbuf.Pixbuf.new_from_bytes(GLib.Bytes.new(rgba.tobytes()),GdkPixbuf.Colorspace.RGB,True,8,width,height,width*4)
        pixbuf.flip(False).savev(str(path),'png',[],[])
