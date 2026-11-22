#version 330 core
in vec3 worldPosition, worldNormal;
in vec3 modelPosition;
in vec2 uv;
in vec4 shadowPosition;
out vec4 outputColor;
uniform vec3 eye, emissive;
uniform vec4 baseColor;
uniform float metallic, roughness, normalScale, highlight;
uniform bool hasBase, hasNormal, hasRoughness, floorSurface, camouflage, darkDecal;
uniform int arrowDirection;
uniform vec4 buttonBounds;
uniform sampler2D baseMap, normalMap, roughnessMap, shadowMap;
const float PI=3.14159265;

float camouflageHash(vec3 p) {
    p=fract(p*.1031);p+=dot(p,p.yzx+33.33);
    return fract((p.x+p.y)*p.z);
}
float camouflageNoise(vec3 p) {
    vec3 i=floor(p),f=fract(p);f=f*f*(3.0-2.0*f);
    return mix(mix(mix(camouflageHash(i),camouflageHash(i+vec3(1,0,0)),f.x),
                   mix(camouflageHash(i+vec3(0,1,0)),camouflageHash(i+vec3(1,1,0)),f.x),f.y),
               mix(mix(camouflageHash(i+vec3(0,0,1)),camouflageHash(i+vec3(1,0,1)),f.x),
                   mix(camouflageHash(i+vec3(0,1,1)),camouflageHash(i+vec3(1,1,1)),f.x),f.y),f.z);
}
float camouflageMicroShapes(vec2 position) {
    vec2 tiled=position*36.0,cell=floor(tiled),q=fract(tiled)-.5;
    float kind=mod(cell.x+cell.y*2.0,4.0),d;
    if (kind<1.0) d=abs(length(q)-.26);
    else if (kind<2.0) d=abs(max(abs(q.x),abs(q.y))-.24);
    else if (kind<3.0) d=abs(max(abs(q.x)*.866+q.y*.5,-q.y)-.20);
    else d=max(min(abs(q.x-q.y),abs(q.x+q.y))*.707,max(abs(q.x),abs(q.y))-.27);
    float aa=max(fwidth(d),.02);
    float ink=1.0-smoothstep(.03-aa,.03+aa,d);
    return ink*(1.0-smoothstep(.40,.85,max(fwidth(tiled).x,fwidth(tiled).y)));
}
vec3 camouflageColor(vec3 position) {
    // A subdued, broad front-panel print, with tiny geometric motifs only in
    // its lighter islands. Original mesh coordinates keep it attached in motion.
    vec2 p=position.xy*2.5;
    vec2 warp=vec2(camouflageNoise(vec3(p*.65,7.1)),camouflageNoise(vec3(p*.65,21.8)));
    p+=warp*1.8;
    float a=camouflageNoise(vec3(p,2.3))+.22*camouflageNoise(vec3(p*2.7,11.0));
    float b=camouflageNoise(vec3(p*.85+8.0,9.1));
    float edge=max(fwidth(a)*.65,.002);
    vec3 color=mix(vec3(.22,.20,.20),vec3(.34,.31,.30),smoothstep(.46-edge,.46+edge,a));
    color=mix(color,vec3(.29,.30,.29),smoothstep(.59-edge,.59+edge,b));
    float pale=smoothstep(.68-edge,.68+edge,a);
    color=mix(color,vec3(.43,.40,.39)-camouflageMicroShapes(position.xy)*.055,pale);
    return pow(color,vec3(2.2));
}
float dpadArrow() {
    vec2 q=(modelPosition.xy-(buttonBounds.xy+buttonBounds.zw)*.5)/(buttonBounds.zw-buttonBounds.xy);
    if (arrowDirection==2) q=vec2(-q.y,q.x);
    if (arrowDirection==3) q=-q;
    if (arrowDirection==4) q=vec2(q.y,-q.x);
    float d=min(q.y+.07,.09-q.y-abs(q.x)*1.3);
    return smoothstep(0.0,max(fwidth(d),.004),d)*smoothstep(.4,.7,worldNormal.z);
}

vec3 surfaceNormal() {
    vec3 n=normalize(worldNormal);
    if (!gl_FrontFacing) n=-n;
    if (!hasNormal) return n;
    vec3 sampleNormal=texture(normalMap,uv).xyz*2.0-1.0;
    sampleNormal.xy*=normalScale;
    vec3 p1=dFdx(worldPosition),p2=dFdy(worldPosition);
    vec2 t1=dFdx(uv),t2=dFdy(uv);
    vec3 q1=cross(p2,n),q2=cross(n,p1);
    vec3 tangent=q1*t1.x+q2*t2.x;
    vec3 bitangent=q1*t1.y+q2*t2.y;
    float length2=max(dot(tangent,tangent),dot(bitangent,bitangent));
    if (length2<1e-10) return n;
    mat3 frame=mat3(tangent*inversesqrt(length2),bitangent*inversesqrt(length2),n);
    return normalize(frame*sampleNormal);
}
float shadowVisibility(vec3 n,vec3 l) {
    vec3 coordinate=shadowPosition.xyz/shadowPosition.w*.5+.5;
    if (coordinate.z>1.0||coordinate.z<0.0||any(lessThan(coordinate.xy,vec2(0)))||any(greaterThan(coordinate.xy,vec2(1)))) return 1.0;
    float bias=max(.0012*(1.0-dot(n,l)),.00035);
    float result=0.0;
    vec2 texel=1.0/vec2(textureSize(shadowMap,0));
    for (int x=-1;x<=1;x++) for (int y=-1;y<=1;y++)
        result+=(coordinate.z-bias<=texture(shadowMap,coordinate.xy+vec2(x,y)*texel*3.0).r)?1.0:0.0;
    return result/9.0;
}
vec3 illuminate(vec3 n,vec3 v,vec3 l,vec3 radiance,vec3 color,float metal,float rough) {
    vec3 h=normalize(v+l);
    float nv=max(dot(n,v),.001),nl=max(dot(n,l),0.0),nh=max(dot(n,h),0.0),hv=max(dot(h,v),0.0);
    float a=rough*rough,a2=a*a;
    float distribution=a2/(PI*pow(nh*nh*(a2-1.0)+1.0,2.0)+.00001);
    float k=pow(rough+1.0,2.0)/8.0;
    float geometry=nv/(nv*(1.0-k)+k)*nl/(nl*(1.0-k)+k);
    vec3 fresnel=mix(vec3(.04),color,metal)+(1.0-mix(vec3(.04),color,metal))*pow(1.0-hv,5.0);
    vec3 specular=distribution*geometry*fresnel/max(4.0*nv*max(nl,.001),.001);
    vec3 diffuse=(1.0-fresnel)*(1.0-metal)*color/PI;
    return (diffuse+specular)*radiance*nl;
}
vec3 filmic(vec3 x) {return clamp((x*(2.51*x+.03))/(x*(2.43*x+.59)+.14),0.0,1.0);}
void main() {
    vec4 albedo=baseColor;
    if (hasBase) albedo*=texture(baseMap,uv);
    if (darkDecal) {
        vec3 ink=hasBase ? texture(baseMap,uv).rgb : albedo.rgb;
        float glyph=clamp((.72-dot(ink,vec3(.2126,.7152,.0722)))/.40,0.0,1.0);
        albedo.rgb=mix(vec3(.004,.005,.007),vec3(.105,.11,.12),glyph);
    }
    if (camouflage) albedo.rgb=camouflageColor(modelPosition);
    if (arrowDirection!=0) albedo.rgb=mix(albedo.rgb,vec3(.10,.11,.12),dpadArrow());
    if (albedo.a<.04) discard;
    float metal=metallic,rough=roughness;
    if (hasRoughness) {vec4 mr=texture(roughnessMap,uv);rough*=mr.g;metal*=mr.b;}
    rough=clamp(rough,.14,1.0);
    vec3 n=surfaceNormal(),v=normalize(eye-worldPosition);
    vec3 key=normalize(vec3(-3.5,6.5,7.0));
    float visible=shadowVisibility(n,key);
    vec3 color=illuminate(n,v,key,vec3(4.8,4.65,4.4)*visible,albedo.rgb,metal,rough);
    color+=illuminate(n,v,normalize(vec3(5.5,1.5,4.0)),vec3(1.3,1.6,2.1),albedo.rgb,metal,rough);
    color+=illuminate(n,v,normalize(vec3(0.0,3.0,-6.0)),vec3(2.0,2.3,3.0),albedo.rgb,metal,rough);
    vec3 ambient=mix(vec3(.10,.12,.17),vec3(.38,.43,.54),n.y*.5+.5);
    color+=albedo.rgb*ambient*(1.0-metal*.45);
    vec3 reflected=reflect(-v,n);
    float softbox=pow(max(dot(reflected,normalize(vec3(-.4,.7,.9))),0.0),mix(70.0,4.0,rough));
    color+=mix(vec3(.04),albedo.rgb,metal)*softbox*(1.0-rough)*1.8;
    color+=emissive*1.1+vec3(.03,.20,.75)*highlight*.35;
    float alpha=floorSurface ? 1.0-smoothstep(4.0,15.0,length(worldPosition.xz)) : albedo.a;
    outputColor=vec4(pow(filmic(color),vec3(1.0/2.2)),alpha);
}
