#version 330 core
layout(location=0) in vec3 position;
uniform mat4 model, lightVP;
void main(){gl_Position=lightVP*model*vec4(position,1.0);}
