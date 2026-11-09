/* The `_losos._tcp` record every LosOS box announces on the LAN
 * (modules/configuration.nix, modules/mdns.nix), shared by the plugin that
 * writes the iOS Info.plist entries and by the browse itself
 * (src/api/discovery.ts). One keeps them in step by hand; there are four
 * spellings between the two consumers and no third one.
 *
 * ESM exports, not module.exports: package.json is "type": "module" now
 * (the Expo project's own package), and a CommonJS .js beside it gets
 * Node's "not defined in ES module scope" when Expo's config loader walks
 * app.config.ts through plugins/.
 */

/** The DNS-SD type the browse walks, no protocol or domain. */
export const serviceType = "_losos";
export const protocol = "tcp";
/** For iOS NSBonjourServices: with and without the domain, since Apple's
 *  and the Bonjour browser guides' spellings both show up in the wild. */
export const bonjourServices = ["_losos._tcp", "_losos._tcp.local."];
