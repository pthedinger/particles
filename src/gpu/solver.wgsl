struct Particle {
    position: vec2<f32>, velocity: vec2<f32>, radius: f32, material: u32, id: u32, pad: u32
}
struct Material {
    color: vec4<f32>, density: f32, viscosity: f32, friction: f32, phase: u32
}
struct Params {
    n: u32, side: u32, h: f32, radius: f32, dt: f32, angular: f32, shake: vec2<f32>, cells: u32, blocks: u32, max_radius:f32, min_radius:f32
}
struct Info {
    previous: vec2<f32>, environment: vec2<f32>, buoyancy: vec2<f32>, lambda: f32, load: f32, damping: vec2<f32>, warm_support: vec2<f32>
}
@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> src: array<Particle>;
@group(0) @binding(2) var<storage, read_write> dst: array<Particle>;
@group(0) @binding(3) var<storage, read> materials: array<Material>;
@group(0) @binding(4) var<storage, read_write> info: array<Info>;
@group(0) @binding(5) var<storage, read_write> counts: array<atomic<u32>>;
@group(0) @binding(6) var<storage, read_write> offsets: array<u32>;
var<workgroup> scan: array<u32, 512>;
fn cell_xy(p: vec2<f32>) -> vec2<i32> {
    return clamp(vec2<i32>(floor((p + 1.0)/params.h)) + 1, vec2<i32>(0), vec2<i32>(i32(params.side)-1));
}
fn cell_id(p: vec2<f32>) -> u32 {
    let xy=cell_xy(p);
    return u32(xy.y)*params.side + u32(xy.x);
}
fn cell_start(c:u32) -> u32 {
    if c>=params.cells {
        return params.n;
    }
    return offsets[c]+offsets[params.cells+c/256u];
}
fn volume(p:Particle) -> f32 {
    let r=p.radius/params.radius;
    return r*r;
}
fn mass(p:Particle) -> f32 {
    return materials[p.material].density*volume(p);
}
fn pressure_weight(p:Particle) -> f32 {
    if materials[p.material].phase==2u{
        return 0.0;
    }
    return 1.0/mass(p);
}
// Gas parcels carry their own compressible pressure. Liquid treats their
// volume as an immersed boundary, rather than crushing low-mass samples with
// the liquid incompressibility projection. Buoyancy/drag still couple motion.
fn constraint_neighbor_weight(p:Particle,q:Particle)->f32 {
    if materials[p.material].phase==0u&&materials[q.material].phase==1u{return 0.0;}
    return pressure_weight(q);
}
fn in_support(p:Particle,q:Particle)->bool {
    let d=p.position-q.position;return dot(d,d)<params.h*params.h;
}
fn limited(v:vec2<f32>, maximum:f32)->vec2<f32>{
    return v*min(1.0,maximum/max(length(v),1e-10));
}
fn direction(i:u32,j:u32,d:vec2<f32>)->vec2<f32>{
    let r=length(d);
    if r>1e-7{
        return d/r;
    }
    let a=f32(min(i,j)%997u)*2.399963;
    let v=vec2<f32>(cos(a),sin(a));
    return select(-v,v,i<j);
}
fn contained(p:vec2<f32>,r:f32)->vec2<f32>{
    return limited(p,1.0-r);
}
@compute @workgroup_size(256) fn clear(@builtin(global_invocation_id) gid:vec3<u32>) {
    if gid.x<params.cells {
        atomicStore(&counts[gid.x],0u);
        atomicStore(&counts[params.cells+gid.x],0u);
    }
}
@compute @workgroup_size(256) fn histogram(@builtin(global_invocation_id) gid:vec3<u32>) {
    if gid.x<params.n{atomicAdd(&counts[cell_id(src[gid.x].position)],1u);}
}
@compute @workgroup_size(256) fn scan_cells(@builtin(global_invocation_id) gid:vec3<u32>,@builtin(local_invocation_id) lid:vec3<u32>,@builtin(workgroup_id) group:vec3<u32>) {
    var v=0u;
    if gid.x<params.cells{
        v=atomicLoad(&counts[gid.x]);
    }
    scan[lid.x]=v;
    workgroupBarrier();
    for(var stride=1u;stride<256u;stride*=2u){
        var add=0u;
        if lid.x>=stride{
            add=scan[lid.x-stride];
        }
        workgroupBarrier();
        scan[lid.x]+=add;
        workgroupBarrier();
    }
    if gid.x<params.cells{
        offsets[gid.x]=scan[lid.x]-v;
    }
    if lid.x==255u{
        offsets[params.cells+group.x]=scan[255];
    }
}
// Up to 512 blocks cover all valid 100k recipes, including radius=2.
// Each of the 256 lanes scans two entries; the cell scan uses the first half.
@compute @workgroup_size(256) fn scan_blocks(@builtin(local_invocation_id) lid:vec3<u32>) {
    let a=lid.x;let b=a+256u;var va=0u;var vb=0u;
    if a<params.blocks{va=offsets[params.cells+a];}
    if b<params.blocks{vb=offsets[params.cells+b];}
    scan[a]=va;scan[b]=vb;workgroupBarrier();
    for(var stride=1u;stride<512u;stride*=2u){
        var add_a=0u;var add_b=0u;
        if a>=stride{add_a=scan[a-stride];}
        if b>=stride{add_b=scan[b-stride];}
        workgroupBarrier();scan[a]+=add_a;scan[b]+=add_b;workgroupBarrier();
    }
    if a<params.blocks{offsets[params.cells+a]=scan[a]-va;}
    if b<params.blocks{offsets[params.cells+b]=scan[b]-vb;}
}
@compute @workgroup_size(256) fn scatter(@builtin(global_invocation_id) gid:vec3<u32>) {
    if gid.x<params.n {
        let c=cell_id(src[gid.x].position);
        let k=atomicSub(&counts[c],1u)-1u;
        dst[cell_start(c)+k]=src[gid.x];
    }
}
@compute @workgroup_size(128) fn environment(@builtin(global_invocation_id) gid:vec3<u32>) {
    let i=gid.x;
    if i>=params.n{
        return;
    }
    let p=src[i];
    let a=materials[p.material];
    let xy=cell_xy(p.position);
    var env=vec2<f32>(0.0);
    var touching=false;
    for(var y=max(xy.y-1,0);y<=min(xy.y+1,i32(params.side)-1);y++){
        for(var x=max(xy.x-1,0);x<=min(xy.x+1,i32(params.side)-1);x++){
            let c=u32(y)*params.side+u32(x);
            for(var k=cell_start(c);k<cell_start(c+1u);k++){
                let q=src[k];
                let j=q.id;
                if j==p.id{
                    continue;
                }
                if !in_support(p,q){continue;}
                let b=materials[q.material];
                if a.phase==2u&&b.phase==2u{let d=p.position-q.position;let reach=p.radius+q.radius+0.000001;if dot(d,d)<=reach*reach{touching=true;}}
                if b.phase==0u&&(a.phase!=0u||p.material!=q.material){
                    let w=max(0.0,1.0-distance(p.position,q.position)/params.h);
                    env+=vec2<f32>(b.density,1.0)*w*w;
                }
            }
        }
    }
    var buoyancy=vec2<f32>(0.0);
    if env.y>0.0001{
        let ratio=min(env.x/env.y/a.density,3.0);
        let submerged=min(env.y/0.8,1.0);
        if a.phase==2u{
            buoyancy=vec2<f32>(0.0,1.8)*ratio*submerged;
        }
        else{
            buoyancy=vec2<f32>(0.0,-1.8)*clamp(1.0-ratio,-2.0,1.0)*submerged;
        }
    }
    if a.phase==2u&&!touching{info[p.id].warm_support=vec2<f32>(0.0);}
    info[p.id].environment=env;
    info[p.id].buoyancy=buoyancy;
}
@compute @workgroup_size(128) fn predict(@builtin(global_invocation_id) gid:vec3<u32>) {
    let i=gid.x;
    if i>=params.n{
        return;
    }
    var p=src[i];
    let a=materials[p.material];
    let xy=cell_xy(p.position);
    var acceleration=vec2<f32>(0.0,-1.8)-params.shake+info[p.id].buoyancy;
    if a.phase==0u {
        for(var y=max(xy.y-1,0);y<=min(xy.y+1,i32(params.side)-1);y++){
            for(var x=max(xy.x-1,0);x<=min(xy.x+1,i32(params.side)-1);x++){
                let c=u32(y)*params.side+u32(x);
                for(var k=cell_start(c);k<cell_start(c+1u);k++){
                    let q=src[k];
                    let j=q.id;
                    if j==p.id{
                        continue;
                    }
                if !in_support(p,q){continue;}
                    let b=materials[q.material];
                    let env=info[j].environment;
                    if env.y>0.0001&&(b.phase!=0u||p.material!=q.material){
                        let w=max(0.0,1.0-distance(p.position,q.position)/params.h);
                        acceleration-=info[j].buoyancy*(w*w/env.y)*(mass(q)/mass(p));
                    }
                }
            }
        }
    }
    info[p.id].previous=p.position;
    info[p.id].load=0.0;
    p.velocity+=limited(acceleration,18.0)*params.dt;
    var drag=0.08;
    if a.phase==2u{
        drag=0.3+a.friction*3.0;
    }
    else if a.phase==0u{
        drag=0.12+a.viscosity*0.35;
    }
    let wall_velocity=vec2<f32>(-p.position.y,p.position.x)*params.angular;
    p.velocity+=(wall_velocity-p.velocity)*(1.0-exp(-drag*params.dt));
    // A world-space bound: changing resolution does not secretly slow the clock or flow.
    p.velocity=limited(p.velocity,2.5);
    let free_position=contained(p.position+p.velocity*params.dt,p.radius);
    if a.phase==2u{
        let support=limited(info[p.id].warm_support*0.95,3.6);
        let correction=support*params.dt*params.dt;
        p.position=contained(info[p.id].previous+limited(free_position+correction-info[p.id].previous,2.5*params.dt),p.radius);
        let applied=p.position-free_position;
        info[p.id].warm_support=applied/(params.dt*params.dt);
        info[p.id].load=length(applied);
    }else{p.position=free_position;}
    dst[i]=p;
}
@compute @workgroup_size(128) fn density(@builtin(global_invocation_id) gid:vec3<u32>) {
    let i=gid.x;
    if i>=params.n{
        return;
    }
    let p=src[i];
    let a=materials[p.material];
    if a.phase==2u{
        info[p.id].lambda=0.0;
        return;
    }
    let xy=cell_xy(p.position);
    var rho=volume(p);
    var grad=vec2<f32>(0.0);
    var denominator=0.0;
    for(var y=max(xy.y-1,0);y<=min(xy.y+1,i32(params.side)-1);y++){
        for(var x=max(xy.x-1,0);x<=min(xy.x+1,i32(params.side)-1);x++){
            let c=u32(y)*params.side+u32(x);
            for(var k=cell_start(c);k<cell_start(c+1u);k++){
                let q=src[k];
                let j=q.id;
                if j==p.id{
                    continue;
                }
                if !in_support(p,q){continue;}
                if a.phase==1u&&materials[q.material].phase==0u{continue;}
                let d=p.position-q.position;
                let w=max(0.0,1.0-length(d)/params.h);
                let v=volume(q);
                rho+=v*w*w;
                let g=direction(p.id,j,d)*(-2.0*w/params.h/2.65)*v;
                grad+=g;
                denominator+=constraint_neighbor_weight(p,q)*dot(g,g);
            }
        }
    }
    var stiffness=1.0;
    if a.phase==1u{
        stiffness=0.25;
    }
    info[p.id].lambda=-stiffness*max(rho/2.65-1.0,0.0)/(denominator+pressure_weight(p)*dot(grad,grad)+80.0);
}
@compute @workgroup_size(128) fn project(@builtin(global_invocation_id) gid:vec3<u32>) {
    let i=gid.x;
    if i>=params.n{
        return;
    }
    var p=src[i];
    let a=materials[p.material];
    let xy=cell_xy(p.position);
    var lo=max(xy-vec2<i32>(1),vec2<i32>(0));var hi=min(xy+vec2<i32>(1),vec2<i32>(i32(params.side)-1));
    if a.phase==2u{let reach=p.radius+params.max_radius;lo=cell_xy(p.position-vec2<f32>(reach));hi=cell_xy(p.position+vec2<f32>(reach));}
    var delta=vec2<f32>(0.0);
    var contact=vec2<f32>(0.0);
    var load=0.0;
    for(var y=lo.y;y<=hi.y;y++){
        for(var x=lo.x;x<=hi.x;x++){
            let c=u32(y)*params.side+u32(x);
            for(var k=cell_start(c);k<cell_start(c+1u);k++){
                let q=src[k];
                let j=q.id;
                if j==p.id{
                    continue;
                }
                if !in_support(p,q){continue;}
                let b=materials[q.material];
                let d=p.position-q.position;
                // Grains receive no fluid pressure projection. Only actual grain
                // contacts need the expensive normalization/mass/friction work.
                if a.phase==2u {
                    if b.phase!=2u||dot(d,d)>=(p.radius+q.radius)*(p.radius+q.radius){
                        continue;
                    }
                    let r=length(d);
                    let normal=direction(p.id,j,d);
                    let overlap=p.radius+q.radius-r;
                    let wi=mass(q)/(mass(p)+mass(q));
                    contact+=normal*min(overlap*wi*0.85,p.radius*0.4);
                    load+=min(overlap*wi*0.85,p.radius*0.4);
                    continue;
                }
                let r=length(d);
                let normal=direction(p.id,j,d);
                let w=max(0.0,1.0-r/params.h);
                let wi=mass(q)/(mass(p)+mass(q));
                let g=normal*(-2.0*w/params.h/2.65);
                if !(a.phase==1u&&b.phase==0u) {
                    var neighbour_lambda=info[j].lambda;
                    if a.phase==0u&&b.phase==1u{neighbour_lambda=0.0;}
                    delta+=g*(info[p.id].lambda*volume(q)+neighbour_lambda*volume(p))*pressure_weight(p);
                }
                if p.material==q.material&&a.phase!=2u&&r>p.radius+q.radius{
                    var tension=0.00012;
                    if a.phase==1u{
                        tension=0.0007;
                    }
                    delta-=normal*(w*w*params.radius*tension*120.0)*wi;
                }
                else if a.phase==0u&&b.phase==0u&&p.material!=q.material{
                    delta+=normal*(w*w*params.radius*0.007);
                }
                var contact_distance=0.0;
                var weight=wi;
                if a.phase==2u&&b.phase==2u{
                    contact_distance=p.radius+q.radius;
                }
                else if a.phase!=2u&&b.phase==2u{
                    contact_distance=q.radius;
                    weight=1.0;
                }
                let overlap=contact_distance-r;
                if overlap>0.0{
                    contact+=normal*min(overlap*weight*0.85,p.radius*0.4);
                }
            }
        }
    }
    let before=p.position;
    p.position=contained(p.position+limited(delta,p.radius*0.2)+limited(contact,p.radius*0.4),p.radius);
    if a.phase==2u{info[p.id].warm_support=limited(info[p.id].warm_support+(p.position-before)/(params.dt*params.dt),3.6);}
    dst[i]=p;
    info[p.id].load+=load;
}
// Compute a symmetric bound on each particle's total Jacobi damping weight.
// Sequential pair damping is contractive; an unbounded sum of parallel pair
// updates is not. Normalizing by max(A_i,A_j,1) keeps both endpoints stable.
@compute @workgroup_size(128) fn damping_weights(@builtin(global_invocation_id) gid:vec3<u32>) {
    let i=gid.x;
    if i>=params.n{
        return;
    }
    let p=src[i];
    let a=materials[p.material];
    let xy=cell_xy(p.position);
    var weights=vec2<f32>(0.0);
    for(var y=max(xy.y-1,0);y<=min(xy.y+1,i32(params.side)-1);y++){
        for(var x=max(xy.x-1,0);x<=min(xy.x+1,i32(params.side)-1);x++){
            let c=u32(y)*params.side+u32(x);
            for(var k=cell_start(c);k<cell_start(c+1u);k++){
                let q=src[k];
                if q.id==p.id{
                    continue;
                }
                let b=materials[q.material];
                let r=distance(p.position,q.position);
                let wi=mass(q)/(mass(p)+mass(q));
                if a.phase==2u&&b.phase==2u {
                    if r<=p.radius+q.radius{
                        weights.y+=wi;
                    }
                }
                else {
                    let w=max(0.0,1.0-r/params.h);
                    weights.x+=min(params.dt*(a.viscosity+b.viscosity)*w*2.0,0.08)*wi;
                }
            }
        }
    }
    info[p.id].damping=weights;
}
@compute @workgroup_size(128) fn finish(@builtin(global_invocation_id) gid:vec3<u32>) {
    let i=gid.x;
    if i>=params.n{
        return;
    }
    var p=src[i];
    let a=materials[p.material];
    let xy=cell_xy(p.position);
    var velocity=(p.position-info[p.id].previous)/params.dt;
    var damping=vec2<f32>(0.0);
    for(var y=max(xy.y-1,0);y<=min(xy.y+1,i32(params.side)-1);y++){
        for(var x=max(xy.x-1,0);x<=min(xy.x+1,i32(params.side)-1);x++){
            let c=u32(y)*params.side+u32(x);
            for(var k=cell_start(c);k<cell_start(c+1u);k++){
                let q=src[k];
                let j=q.id;
                if j==p.id{
                    continue;
                }
                if !in_support(p,q){continue;}
                let b=materials[q.material];
                let d=p.position-q.position;
                let r=length(d);
                let v=(q.position-info[j].previous)/params.dt;
                let rel=velocity-v;
                let wi=mass(q)/(mass(p)+mass(q));
                if a.phase==2u&&b.phase==2u {
                    if r>p.radius+q.radius{
                        continue;
                    }
                    let normal=direction(p.id,j,d);
                    let closing=min(dot(rel,normal),0.0);
                    let tangent=rel-normal*dot(rel,normal);
                    let normal_load=(info[p.id].load+info[j].load)/params.dt-closing;
                    let impulse=normal*closing+limited(tangent,sqrt(a.friction*b.friction)*normal_load);
                    damping-=impulse*wi/max(1.0,max(info[p.id].damping.y,info[j].damping.y));
                }
                else {
                    let w=max(0.0,1.0-r/params.h);
                    damping-=rel*min(params.dt*(a.viscosity+b.viscosity)*w*2.0,0.08)*wi/max(1.0,max(info[p.id].damping.x,info[j].damping.x));
                }
            }
        }
    }
    velocity+=damping;
    if length(p.position)>=1.0-p.radius-0.00005{
        let n=normalize(p.position);
        velocity-=n*max(dot(velocity,n),0.0);
        if a.phase==2u{
            let wall_velocity=vec2<f32>(-p.position.y,p.position.x)*params.angular;
            velocity+=(wall_velocity-velocity)*a.friction*0.25;
        }
    }
    p.velocity=limited(velocity,2.5);
    dst[i]=p;
}

// Conservative high-speed guard. Slow contacts retain the mass-weighted
// position/friction solver. Fast grains treat predicted neighbours as swept
// obstacles; this sacrifices exact impact momentum rather than letting a
// shared parallel correction amplify an entire packed bed.
@compute @workgroup_size(128)
fn swept_contacts(@builtin(global_invocation_id) gid:vec3<u32>) {
    let i=gid.x;if i>=params.n{return;}var p=src[i];
    let old=info[p.id].previous;let movement=p.position-old;
    if materials[p.material].phase!=2u||length(movement)<params.min_radius*0.85{dst[i]=p;return;}
    let margin=p.radius+params.max_radius+2.5*params.dt;
    let lo=cell_xy(min(old,p.position)-vec2<f32>(margin));let hi=cell_xy(max(old,p.position)+vec2<f32>(margin));
    var earliest=2.0;var correction=vec2<f32>(0.0);
    for(var y=lo.y;y<=hi.y;y++){for(var x=lo.x;x<=hi.x;x++){
        let c=u32(y)*params.side+u32(x);let reach=bitcast<f32>(atomicLoad(&counts[params.cells+c]));if reach==0.0{continue;}
        let cell_lo=(vec2<f32>(f32(x),f32(y))-1.0)*params.h-1.0;let cell_hi=cell_lo+params.h;
        let sweep_lo=min(old,p.position)-vec2<f32>(p.radius+reach);let sweep_hi=max(old,p.position)+vec2<f32>(p.radius+reach);
        if any(cell_hi<sweep_lo)||any(cell_lo>sweep_hi){continue;}
        for(var k=cell_start(c);k<cell_start(c+1u);k++){
            let q=src[k];if q.id==p.id||materials[q.material].phase!=2u{continue;}
            let q_old=info[q.id].previous;let initial=old-q_old;let relative=movement-(q.position-q_old);let radii=p.radius+q.radius;
            let c0=dot(initial,initial)-radii*radii;let aa=dot(relative,relative);let bb=dot(initial,relative);
            if aa<1e-12||bb>=0.0{continue;}var hit=0.0;
            if c0>0.0{let discriminant=bb*bb-aa*c0;if discriminant<0.0{continue;}hit=(-bb-sqrt(discriminant))/aa;}
            if hit<0.0||hit>1.0||hit>=earliest{continue;}
            let normal=normalize(initial+relative*hit);let inward=min(dot(relative*(1.0-hit),normal),0.0);
            correction=-normal*inward;earliest=hit;
        }
    }}
    p.position=contained(p.position+correction,p.radius);info[p.id].load+=length(correction);dst[i]=p;
}
@compute @workgroup_size(128)
fn motion_bounds(@builtin(global_invocation_id) gid:vec3<u32>){
    if gid.x>=params.n{return;}let p=src[gid.x];if materials[p.material].phase!=2u{return;}
    let old=info[p.id].previous;let c=cell_id(old);atomicMax(&counts[params.cells+c],bitcast<u32>(length(p.position-old)+p.radius));
}
