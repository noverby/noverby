# Overlay Flake lib as lib.noverby
# and add noverby to lib.maintainers
(_: prev: {
  lib = prev.lib.extend (_: prevLib: {
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
