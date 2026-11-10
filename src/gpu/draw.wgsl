struct Particle {
    position: vec2<f32>, velocity: vec2<f32>, radius:f32, material:u32, id:u32, pad:u32
}
struct Material {
    color:vec4<f32>, density:f32, viscosity:f32, friction:f32, phase:u32, surface_tension:f32, compression_resistance:f32, mixing_resistance:f32, neighbor_target:f32, thermal_motion:f32, padding:array<f32,3>
}
@group(0) @binding(0) var<storage,read> particles:array<Particle>;
@group(0) @binding(1) var<storage,read> materials:array<Material>;
struct Vertex {
    @builtin(position) clip:vec4<f32>, @location(0) local:vec2<f32>, @location(1) color:vec3<f32>, @location(2) @interpolate(flat) id:u32
}
fn linear_color(c:vec3<f32>)->vec3<f32>{
    return select(c/12.92,pow((c+0.055)/1.055,vec3<f32>(2.4)),c>vec3<f32>(0.04045));
}
fn corner(v:u32)->vec2<f32>{
    var points=array<vec2<f32>,6>(vec2<f32>(-1.0,-1.0),vec2<f32>(1.0,-1.0),vec2<f32>(1.0,1.0),vec2<f32>(-1.0,-1.0),vec2<f32>(1.0,1.0),vec2<f32>(-1.0,1.0));
    return points[v];
}
fn vertex(v:u32,i:u32,solid:bool)->Vertex{
    let p=particles[i];
    let m=materials[p.material];
    let local=corner(v);
    var radius=p.radius*2.4;
    if solid{
        radius=max(p.radius*0.96,0.00098);
    }
    var clip=vec4<f32>(p.position+local*radius,0.0,1.0);
    if (m.phase==2u)!=solid{
        clip=vec4<f32>(2.0,2.0,0.0,1.0);
    }
    return Vertex(clip,local,linear_color(m.color.rgb),p.id);
}
@vertex fn vs_fluid(@builtin(vertex_index) v:u32,@builtin(instance_index)i:u32)->Vertex{
    return vertex(v,i,false);
}
@vertex fn vs_solid(@builtin(vertex_index) v:u32,@builtin(instance_index)i:u32)->Vertex{
    return vertex(v,i,true);
}
@fragment fn fs_fluid(v:Vertex)->@location(0)vec4<f32>{
    let w=max(1.0-dot(v.local,v.local),0.0);
    if w<=0.0{
        discard;
    }
    return vec4<f32>(v.color*w*w,w*w);
}
@fragment fn fs_solid(v:Vertex)->@location(0)vec4<f32>{
    let r=length(v.local);
    if r>1.0{
        discard;
    }
    let variation=0.9+f32(v.id%19u)*0.011;
    let shade=variation*(0.82+0.16*v.local.y);
    let alpha=1.0-smoothstep(0.82,1.0,r);
    return vec4<f32>(v.color*shade,alpha);
}
