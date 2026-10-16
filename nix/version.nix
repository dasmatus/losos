# The LosOS release this tree is, spelled like its Git tag. The boot menus,
# the boot pictures, os-release, the console banners and `losos --version`
# show it.
#
# CI keeps it current, so nobody bumps it by hand: .github/workflows/version.yml
# commits the newest `v*` tag here on main whenever a tag is pushed, and on
# every push to main that finds it behind. A tag pushed before that commit is
# built with its own name anyway, because the release and terminal client jobs
# in .github/workflows/ci.yml write the tag over this value first.
"v0.4.0"
