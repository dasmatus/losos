# The published LosOS gateway images a box trusts (backend/access's
# `check::Image`): per image, the PCR 11 its unified kernel image leaves and
# the Authenticode hashes of every program the firmware starts on the way to
# it. `losos-registrar passport measure --uki <file> --name <name> --app
# <kernel>` prints an entry.
#
# Empty until a gateway image boots as a unified kernel image: until then no
# gateway can show a published image, no edge is trusted with trading, and
# boxes share storage through any edge as before.
[ ]
