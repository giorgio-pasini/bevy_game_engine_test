shader_type canvas_item;
uniform float brightness = 0.0;
void fragment() {
	vec4 pixel = texture(TEXTURE, UV);
	COLOR = vec4(clamp(pixel.rgb + vec3(brightness), vec3(0.0), vec3(1.0)), pixel.a);
}
