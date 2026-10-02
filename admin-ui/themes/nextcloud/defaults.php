<?php
/* LosOS cloud: the name, the words and the links Nextcloud shows as its own.
 *
 * A theme folder's defaults.php is read by OC_Defaults on every request
 * (lib/private/legacy/OC_Defaults.php), and the theming app takes its own
 * defaults from there, so whatever is not set here falls through to
 * Nextcloud's: "Nextcloud", "a safe home for all your data", nextcloud.com.
 * Every method below replaces one of those. The theming app still wins over
 * all of them once an admin enters a value under Administration → Theming,
 * which is how it should be: the owner's word beats the appliance's.
 *
 * The logo and favicons are not set here. They are files beside this one,
 * under core/img/, which Nextcloud looks up in the theme folder before its
 * own (admin-ui/themes/default.nix renders them from brand/).
 */

class OC_Theme {
	/* The name in the page title, on the login button ("Log in to …"), in
	 * the footer and in every e-mail. */
	public function getName(): string {
		return 'LosOS cloud';
	}

	public function getHTMLName(): string {
		return 'LosOS cloud';
	}

	public function getTitle(): string {
		return 'LosOS cloud';
	}

	public function getEntity(): string {
		return 'LosOS cloud';
	}

	public function getProductName(): string {
		return 'LosOS cloud';
	}

	/* Nextcloud's tagline ("a safe home for all your data") is its
	 * marketing, so there is none, except on the login page: there the footer
	 * under the login card carries one line from the books every Slovak pupil
	 * reads at school, in Slovak, a different one on each load. The theming
	 * app prints the slogan as "<name> – <slogan>" on every guest page and in
	 * e-mail footers, hence the check on the path: a quote in a password-reset
	 * mail would only confuse. */
	public function getSlogan(?string $lang = null): string {
		$path = parse_url($_SERVER['REQUEST_URI'] ?? '', PHP_URL_PATH) ?: '';
		if (!preg_match('#/login/?$#', $path)) {
			return '';
		}
		$quote = self::QUOTES[random_int(0, count(self::QUOTES) - 1)];
		return '„' . $quote[0] . '“ (' . $quote[1] . ', ' . $quote[2] . ')';
	}

	/* [quote, author, book], in Slovak. Short attributed lines only. The
	 * wording should match the Slovak editions read in school; add or correct
	 * lines here and nowhere else. */
	private const QUOTES = [
		['Vojna je mier. Sloboda je otroctvo. Nevedomosť je sila.', 'George Orwell', '1984'],
		['Veľký brat ťa sleduje.', 'George Orwell', '1984'],
		['Všetky zvieratá sú si rovné, ale niektoré zvieratá sú si rovnejšie.', 'George Orwell', 'Zvieracia farma'],
		['Štyri nohy dobré, dve nohy zlé.', 'George Orwell', 'Zvieracia farma'],
		['Na západe nič nové.', 'Erich Maria Remarque', 'Na západe nič nové'],
	];

	/* An empty URL makes the footer's name plain text instead of a link out
	 * to nextcloud.com. */
	public function getBaseUrl(): string {
		return '';
	}

	/* The colour e-mails are headed with, and the one the theming app starts
	 * from: tokens.css's light --accent. */
	public function getColorPrimary(): string {
		return '#0e6e7d';
	}

	public function getMailHeaderColor(): string {
		return '#0e6e7d';
	}
}
