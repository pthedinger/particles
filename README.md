Simple Particle Simulator
=========================

Overview
--------
Very simple world simulator with a simple concept of each screen pixel holding a single
particle. Particles have a material and can have a custom color.

How to run
----------

Simply use::
  cargo run

Controls
--------

While the simulator is running the following commands are supported:

 - `a, f, g, o, r, s, w` to set the insertion material to `Air, Fire, Gas, Oil, Rock, Sand, Water` respectively
 - `A, F, G, O, R, S, W` to select a material `Source`
 - `c` to clear all `Sources`
 - `u` to flip the image world upside down
 - `m` to switch between image colors / `Material` view
 - `p` to pause/unpause the Simulation
 - `Enter` to reset the simulation to random materials
 - `1-9` to control the speed of insertion of `Sources` added
 - `Left-Mouse` to insert `Material` / `Sources`
 - `Drag-and-drop` an image to have it loaded into the simulation
 - `[` reduce size of pixels
 - `]` increase size of pixels
