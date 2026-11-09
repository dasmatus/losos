/* The `_losos._tcp` record every LosOS box announces on the LAN
 * (modules/configuration.nix, modules/mdns.nix), shared by the plugin that
 * writes the iOS Info.plist entries and by the browse itself
 * (src/api/discovery.ts). One keeps them in step by hand; there are four
 * spellings between the two consumers and no third one.
 */

module.exports = {
  /** The DNS-SD type the companion browses, no protocol or domain. */
  serviceType: "_losos",
  protocol: "tcp",
  /** For iOS NSBonjourServices: with and without the domain, since Apple's
   *  and the Bonjour browser guides' spellings both show up in the wild. */
  bonjourServices: ["_losos._tcp", "_losos._tcp.local."],
};
