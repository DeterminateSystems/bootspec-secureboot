{ nixpkgs ? builtins.fetchTarball {
    url = "https://github.com/NixOS/nixpkgs/archive/20887e4bbfdae3aed6bfa1f53ddf138ee325515e.tar.gz";
    sha256 = "0hc79sv59appb7bynz5bzyqvrapyjdq63s79i649vxl93504kmnv";
  }
, pkgs ? import nixpkgs { }
}:
pkgs.sbsigntool.overrideAttrs ({ patches ? [ ], ... }: {
  pname = "patched-sbattach";
  patches = patches ++ [
    # We want to be able to compare a signed EFI file to an unsigned EFI file,
    # so we must first remove the signature. However, the sbsigntool suite of
    # tools aligns the size of EFI files it writes / modifies to a factor of 2^3
    # (8) and updates the PE32+ checksum accordingly. Removing the signature
    # does not remove the added padding, nor does it undo the change to the
    # PE32+ checksum. This means that files produced the same exact way will not
    # have the same checksum.
    #
    # In order to work around this, the below patch removes an early return when
    # using `sbattach --remove` on a file without a signature. This ensure that
    # the file will then have the same file size and checksum as the EFI file
    # with its signature removed. It is worth noting that this is only used when
    # validating files -- the original files are left untouched.
    #
    # This also patches various Makefiles so that only sbattach is rebuilt --
    # this prevents wasting unnecessary resources on building the entirety of
    # the sbsigntool suite.
    ./pad-files-without-signature.diff
  ];
})
