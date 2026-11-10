#version 330 core
layout(location=0) in vec3 position;
layout(location=1) in vec3 normal;
layout(location=2) in vec2 texcoord;
uniform mat4 model, view, projection, lightVP;
out vec3 worldPosition;
out vec3 worldNormal;
out vec3 modelPosition;
out vec2 uv;
out vec4 shadowPosition;
void main() {
    vec4 world=model*vec4(position,1.0);
    worldPosition=world.xyz;
    modelPosition=position;
    worldNormal=normalize(transpose(inverse(mat3(model)))*normal);
    uv=texcoord;
    shadowPosition=lightVP*world;
    gl_Position=projection*view*world;
}
