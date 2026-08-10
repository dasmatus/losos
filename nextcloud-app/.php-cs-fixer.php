<?php
/**
 * php-cs-fixer config for the losos app. Follows Nextcloud's PSR-12-ish style:
 * short array syntax, no trailing commas in multilines that break old PHP, etc.
 */
declare(strict_types=1);

return (new PhpCsFixer\Config())
	->setRules([
		'@PSR12' => true,
		'array_syntax' => ['syntax' => 'short'],
		'declare_strict_types' => true,
		'no_unused_imports' => true,
		'ordered_imports' => ['sort_algorithm' => 'alpha'],
	])
	->setRiskyAllowed(true)
	->setFinder(
		PhpCsFixer\Finder::create()
			->in(__DIR__ . '/lib')
			->in(__DIR__ . '/tests'),
	);