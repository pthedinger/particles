@group(0) @binding(0) var field:texture_2d<f32>;
@group(0) @binding(1) var linear_sampler:sampler;
@group(0) @binding(2) var boundary_map:texture_2d<f32>;
@group(0) @binding(4) var boundary_sampler:sampler;
@group(0) @binding(3) var<uniform> vessel_pose:vec4<f32>;
struct Vertex{
    @builtin(position) clip:vec4<f32>,@location(0) uv:vec2<f32>
}
@vertex fn vs(@builtin(vertex_index) i:u32)->Vertex{
    let x=f32((i<<1u)&2u);
    let y=f32(i&2u);
    return Vertex(vec4<f32>(x*2.0-1.0,y*2.0-1.0,0.0,1.0),vec2<f32>(x,1.0-y));
}
@fragment fn fs(v:Vertex)->@location(0)vec4<f32>{
    if length(v.uv*2.0-1.0)>1.035{discard;}
    let world=vec2<f32>(v.uv.x*2.0-1.0,1.0-v.uv.y*2.0);
    let wall=vessel_sample(world,vessel_pose.xy,vessel_pose.z==0.0);
    if vessel_pose.z==0.0 {if wall.x<0.0{discard;}}
    else {
        if wall.x < -0.022 {return vec4<f32>(0.012,0.024,0.030,1.0);}
        if wall.x<0.0 {
            let brass=mix(vec3<f32>(0.20,0.16,0.08),vec3<f32>(0.60,0.46,0.23),smoothstep(-0.022,0.0,wall.x));
            return vec4<f32>(brass,1.0);
        }
    }
    let value=textureSample(field,linear_sampler,v.uv);
    let background=vec3<f32>(0.0044,0.0137,0.0212);
    return vec4<f32>(mix(background,value.rgb/max(value.a,0.0001),min(value.a*4.0,1.0)),1.0);
}
