@group(0) @binding(0) var field:texture_2d<f32>;
@group(0) @binding(1) var linear_sampler:sampler;
struct Vertex{
    @builtin(position) clip:vec4<f32>,@location(0) uv:vec2<f32>
}
@vertex fn vs(@builtin(vertex_index) i:u32)->Vertex{
    let x=f32((i<<1u)&2u);
    let y=f32(i&2u);
    return Vertex(vec4<f32>(x*2.0-1.0,y*2.0-1.0,0.0,1.0),vec2<f32>(x,1.0-y));
}
@fragment fn fs(v:Vertex)->@location(0)vec4<f32>{
    if length(v.uv*2.0-1.0)>1.0{
        discard;
    }
    let value=textureSample(field,linear_sampler,v.uv);
    let background=vec3<f32>(0.0044,0.0137,0.0212);
    return vec4<f32>(mix(background,value.rgb/max(value.a,0.0001),min(value.a*4.0,1.0)),1.0);
}
