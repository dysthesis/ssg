{
  vale,
  symlinkJoin,
  makeWrapper,
  ...
}: let
  valeWithStyles = vale.withStyles (
    s:
      with s; [
        readability
        proselint
        write-good
      ]
  );
  valeConfig = ./vale.ini;
in
  symlinkJoin {
    name = "vale";
    paths = [valeWithStyles];
    buildInputs = [makeWrapper];
    postBuild = ''
      wrapProgram $out/bin/vale \
        --add-flags "--config='${valeConfig}'"
    '';
  }
