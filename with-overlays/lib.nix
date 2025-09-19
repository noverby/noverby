# Overlay Flake lib as lib.noverby
(final: prev: {lib = prev.lib.extend (_: _: {noverby = prev.outputs.lib;});})
