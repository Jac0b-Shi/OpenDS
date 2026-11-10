"""Small glTF scene loader and input-driven part animation, independent of GTK/GL."""
import json
import math
import struct
from pathlib import Path
import numpy as np

CONTROLS = {"left-stick", "right-stick", "l1", "r1", "l2", "r2", "triangle", "circle", "cross", "square", "up", "right", "down", "left", "create", "options", "ps", "mute", "touchpad"}


def translate(x=0, y=0, z=0):
    matrix = np.eye(4, dtype=np.float32)
    matrix[:3, 3] = (x, y, z)
    return matrix


def rotate(axis, angle):
    c, s = math.cos(angle), math.sin(angle)
    result = np.eye(4, dtype=np.float32)
    if axis == 0: result[1:3, 1:3] = ((c, -s), (s, c))
    elif axis == 1: result[np.ix_([0, 2], [0, 2])] = ((c, s), (-s, c))
    else: result[:2, :2] = ((c, -s), (s, c))
    return result


def around(point, matrix):
    return translate(*point) @ matrix @ translate(*(-np.asarray(point)))


def perspective(fov, aspect, near=.1, far=60):
    scale = 1 / math.tan(math.radians(fov) / 2)
    return np.array([[scale/aspect,0,0,0],[0,scale,0,0],[0,0,(far+near)/(near-far),2*far*near/(near-far)],[0,0,-1,0]], dtype=np.float32)


def orthographic(left, right, bottom, top, near, far):
    return np.array([[2/(right-left),0,0,-(right+left)/(right-left)],[0,2/(top-bottom),0,-(top+bottom)/(top-bottom)],[0,0,-2/(far-near),-(far+near)/(far-near)],[0,0,0,1]], dtype=np.float32)


def normalize(value):
    return np.asarray(value, dtype=np.float32) / max(float(np.linalg.norm(value)), 1e-8)


def look_at(eye, target, up=(0,1,0)):
    forward = normalize(np.asarray(target)-eye)
    side = normalize(np.cross(forward, up))
    vertical = np.cross(side, forward)
    result = np.eye(4, dtype=np.float32)
    result[:3, :3] = (side, vertical, -forward)
    result[:3, 3] = -result[:3, :3] @ eye
    return result


class Scene:
    def __init__(self, path):
        raw = Path(path).read_bytes()
        magic, version, length = struct.unpack_from("<4sII", raw)
        if magic != b"glTF" or version != 2 or length != len(raw):
            raise ValueError("Expected a complete glTF 2 binary")
        size, kind = struct.unpack_from("<II", raw, 12)
        if kind != 0x4e4f534a: raise ValueError("Missing glTF JSON chunk")
        self.document = json.loads(raw[20:20+size])
        binary_size, binary_kind = struct.unpack_from("<II", raw, 20+size)
        if binary_kind != 0x004e4942: raise ValueError("Missing glTF binary chunk")
        self.binary = raw[28+size:28+size+binary_size]
        self.parts = []
        self.controls = {}
        for node in self.document["nodes"]:
            if "mesh" not in node: continue
            name = node.get("name", "mesh")
            group = name.split("__")[0]
            control = group if group in CONTROLS else None
            for primitive in self.document["meshes"][node["mesh"]]["primitives"]:
                attrs = primitive["attributes"]
                positions = self.accessor(attrs["POSITION"]).astype(np.float32)
                normals = self.accessor(attrs["NORMAL"]).astype(np.float32)
                uv = self.accessor(attrs["TEXCOORD_0"]).astype(np.float32) if "TEXCOORD_0" in attrs else np.zeros((len(positions),2),np.float32)
                indices = self.accessor(primitive["indices"]).astype(np.uint32).ravel()
                part = {"name":name,"control":control,"positions":positions,"normals":normals,"uv":uv,"indices":indices,"material":primitive.get("material",0),"min":positions.min(axis=0),"max":positions.max(axis=0)}
                part["triangles"] = positions[indices.reshape(-1,3)]
                self.parts.append(part)
                if control: self.controls.setdefault(control, []).append(part)
        self.pivots = {}
        for control, parts in self.controls.items():
            low = np.min([p["min"] for p in parts],axis=0)
            high = np.max([p["max"] for p in parts],axis=0)
            pivot = (low+high)/2
            if "stick" in control: pivot[2] = low[2]+.045
            if control in ("l2","r2"): pivot[1],pivot[2] = low[1]+.07,high[2]-.025
            self.pivots[control] = pivot
        self.minimum = np.min([p["min"] for p in self.parts],axis=0)
        self.maximum = np.max([p["max"] for p in self.parts],axis=0)

    def accessor(self, index):
        spec = self.document["accessors"][index]
        view = self.document["bufferViews"][spec["bufferView"]]
        dtype = np.dtype({5120:"i1",5121:"u1",5122:"<i2",5123:"<u2",5125:"<u4",5126:"<f4"}[spec["componentType"]])
        columns = {"SCALAR":1,"VEC2":2,"VEC3":3,"VEC4":4,"MAT4":16}[spec["type"]]
        offset = view.get("byteOffset",0)+spec.get("byteOffset",0)
        result = np.ndarray((spec["count"],columns),dtype=dtype,buffer=self.binary,offset=offset,strides=(view.get("byteStride",dtype.itemsize*columns),dtype.itemsize)).copy()
        if spec.get("normalized") and dtype.kind in "ui":
            result = result.astype(np.float32)/np.iinfo(dtype).max
            if dtype.kind == "i": result = np.maximum(result,-1)
        return result

    def image(self, index):
        image = self.document["images"][index]
        view = self.document["bufferViews"][image["bufferView"]]
        offset = view.get("byteOffset",0)
        return self.binary[offset:offset+view["byteLength"]]

    def pose(self, control, values):
        if not control: return np.eye(4,dtype=np.float32)
        amount = values.get(control,0)
        if "stick" in control:
            turn = rotate(1,values.get(control+"-x",0)*.28) @ rotate(0,values.get(control+"-y",0)*.28)
            return translate(z=-amount*.045) @ around(self.pivots[control],turn)
        if control in ("l2","r2"):
            return around(self.pivots[control],rotate(0,-amount*.48))
        if control in ("l1","r1"): return translate(y=-amount*.028,z=-amount*.045)
        return translate(z=-amount*(.026 if control in ("touchpad","mute") else .055))

    def touch_height(self, x, y):
        surface=next(p for p in self.controls['touchpad'] if '__surface__' in p['name'])
        triangles=surface['triangles'];a=triangles[:,0];e1=triangles[:,1]-a;e2=triangles[:,2]-a
        point=np.array([x,y])-a[:,:2]
        determinant=e1[:,0]*e2[:,1]-e1[:,1]*e2[:,0]
        valid=np.abs(determinant)>1e-9
        inverse=np.divide(1,determinant,out=np.zeros_like(determinant),where=valid)
        u=(point[:,0]*e2[:,1]-point[:,1]*e2[:,0])*inverse
        v=(e1[:,0]*point[:,1]-e1[:,1]*point[:,0])*inverse
        z=(a[:,2]+u*e1[:,2]+v*e2[:,2])[valid & (u>=0) & (v>=0) & (u+v<=1)]
        return float(z.max()) if len(z) else .79

    def pick(self, origin, direction, transforms):
        nearest = None
        distance = float("inf")
        for part in self.parts:
            inverse = np.linalg.inv(transforms[part["name"]])
            start = (inverse @ np.append(origin,1))[:3]
            ray = normalize((inverse @ np.append(direction,0))[:3])
            safe = np.where(np.abs(ray)<1e-8,1e-8,ray)
            first,last = (part["min"]-start)/safe,(part["max"]-start)/safe
            if np.max(np.minimum(first,last))>np.min(np.maximum(first,last)): continue
            triangles=part["triangles"]
            e1,e2=triangles[:,1]-triangles[:,0],triangles[:,2]-triangles[:,0]
            p=np.cross(ray,e2);det=np.einsum("ij,ij->i",e1,p)
            valid=np.abs(det)>1e-8; inv=np.divide(1,det,out=np.zeros_like(det),where=valid)
            t=start-triangles[:,0];u=np.einsum("ij,ij->i",t,p)*inv
            q=np.cross(t,e1);v=q @ ray * inv
            depth=np.einsum("ij,ij->i",e2,q)*inv
            hits=depth[valid & (u>=0) & (v>=0) & (u+v<=1) & (depth>0)]
            if len(hits) and hits.min()<distance: nearest,distance=part["control"],float(hits.min())
        return nearest


def targets(state, pressed=(0,0,0)):
    sticks=state.get("sticks",[128]*4);triggers=state.get("triggers",[0,0]);buttons=state.get("buttons",[8,0,0])
    b=[int(v)|int(p) for v,p in zip(buttons,pressed)]
    result={name:0.0 for name in CONTROLS}
    for index,name in enumerate(("left-stick","right-stick")):
        for axis in range(2):
            value=int(sticks[index*2+axis])-128
            result[name+("-x" if axis==0 else "-y")]=max(-1,min(1,value/(128 if value<0 else 127)))
    for name,mask in [("square",16),("cross",32),("circle",64),("triangle",128)]:result[name]=float(bool(b[0]&mask))
    dpad=[1,3,2,6,4,12,8,9,0][min(buttons[0]&15,8)] | (pressed[0]&15)
    for name,mask in [("up",1),("right",2),("down",4),("left",8)]:result[name]=float(bool(dpad&mask))
    for name,mask in [("l1",1),("r1",2),("create",16),("options",32),("left-stick",64),("right-stick",128)]:result[name]=float(bool(b[1]&mask))
    for name,mask in [("ps",1),("touchpad",2),("mute",4)]:result[name]=float(bool(b[2]&mask))
    result["l2"],result["r2"]=triggers[0]/255,triggers[1]/255
    return result


class Animator:
    def __init__(self):
        self.values=targets({});self.velocity={name:0.0 for name in self.values}

    def step(self, target, dt):
        dt=min(max(dt,0),.025)
        changed=False
        for name in self.values:
            goal=target.get(name,0)
            stiffness,damping=(440,38) if "stick" not in name and name not in ("l2","r2") else (250,30)
            self.velocity[name]+=(stiffness*(goal-self.values[name])-damping*self.velocity[name])*dt
            delta=self.velocity[name]*dt
            self.values[name]+=delta
            changed |= abs(delta)>.00001
        return changed
