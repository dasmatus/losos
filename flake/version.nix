# The LosOS release this tree is, spelled like its Git tag. The boot menus,
# the boot pictures, os-release and the console banners show it.
#
# Bump it in the commit that gets the tag. The release job in
# .github/workflows/ci.yml writes the tag it builds over this value before
# building, so release media always say their own tag, and warns when the
# two differ, because an installed box builds from the committed tree and
# shows what is here.
"v0.1.8"
