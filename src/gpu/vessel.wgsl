// boundary_map stores signed free-space clearance and inward gradient in local space.
fn vessel_sample(p:vec2<f32>,pose:vec2<f32>,circle:bool)->vec3<f32>{
    if circle {return vec3<f32>(1.0-length(p),-p/max(length(p),0.000001));}
    let local=vec2<f32>(pose.x*p.x+pose.y*p.y,-pose.y*p.x+pose.x*p.y);
    let dimensions=vec2<f32>(textureDimensions(boundary_map));
    let uv=(clamp(local/2.2+0.5,vec2<f32>(0.0),vec2<f32>(1.0))*(dimensions-1.0)+0.5)/dimensions;
    let value=textureSampleLevel(boundary_map,boundary_sampler,uv,0.0).xyz;
    var n=value.yz;let norm=length(n);
    if norm<0.000001 {n=vec2<f32>(0.0,1.0);}else{n/=norm;}
    return vec3<f32>(value.x,vec2<f32>(pose.x*n.x-pose.y*n.y,pose.y*n.x+pose.x*n.y));
}
