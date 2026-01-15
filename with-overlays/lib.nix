# Overlay Flake lib as lib.noverby
# and add noverby to lib.maintainers
(final: prev: {
  lib = prev.lib.extend (finalLib: prevLib: {
    noverby = prev.outputs.lib;
    maintainers =
      prevLib.maintainers
      // {
        noverby = {
          name = "Niclas Overby";
          email = "niclas@overby.me";
          github = "noverby";
          githubId = "2422942";
        };
      };
  });
})
