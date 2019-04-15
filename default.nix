{ lib, fetchFromGitHub, git, runCommand, rustPlatform
, localSrc ? true
}:

let
  inherit (builtins) trace; inherit (lib) traceVal;
  inherit (lib) substring;
  smartPathFilter = basePath: f: let
      relPath = substring (lib.stringLength basePath) (-1); in
    p: let path = relPath (toString p);
      dir = dirOf path; baseName = baseNameOf path; in
    type: !(f path dir baseName type);
  smartPath = args: builtins.path (args // {
    ${if args ? filter then "filter" else null} =
      smartPathFilter (toString args.path) args.filter;
  });
in

rustPlatform.buildRustPackage rec {
  pname = "html2json";
  version = let revDate = if localSrc then
    builtins.readFile (runCommand "${pname}-git-rev-date" {
      src = smartPath {
        path = ./.git; name = "${pname}-local-rev-date-gitdir";
        filter = path: dir: baseName: type:
          path == "/index" ||
          path == "/logs" ||
        false;
      };
      buildInputs = [ git ];
    } ''
      TZ=UTC git --git-dir=$src show -s --format="format:%cd+%h" --date=short-local > "$out"
    '')
  else "2019-04-14+${substring 0 7 src.rev}"; in "0.1.0-${revDate}";

  src = if !localSrc then fetchFromGitHub {
    owner = "bb010g";
    repo = "html2json";
    rev = "v${version}";
    sha256 = "0000000000000000000000000000000000000000000000000000";
  } else smartPath {
    path = ./.; name = "${pname}-local";
    filter = path: dir: baseName: type:
      (type == "symlink" && lib.hasPrefix "/result" path) ||
      path == "/.git" || path == "/target" || lib.hasSuffix ".rs.bak" baseName ||
    false;
  };

  cargoSha256 = "0vn1xfy2zpv3cqb1ys02gm286h0k2fysfzk63205phxgpna5ls3l";

  meta = with lib; {
    description = "Convert HTML losslessly into JSON for easier processing";
    homepage = https://github.com/bb010g/html2json;
    license = with licenses; [ isc asl20 ];
    maintainers = with maintainers; [ bb010g ];
    platforms = platforms.all;
  };
}
