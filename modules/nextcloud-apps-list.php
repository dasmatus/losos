<?php
// The app list behind lososd's `GET /api/cloud/apps`, for
// `losos-nextcloud-apps list` (modules/nextcloud-stack.nix, `appsTool`).
//
// `occ app:list --output=json` says which apps are on and which are off, by
// id and version only. The names and summaries the Apps page shows are in
// each app's appinfo/info.xml, so this reads those straight off the three app
// folders of the webroot instead of booting Nextcloud once per app. It only
// reads files: nothing here calls into Nextcloud, so a Nextcloud update that
// moves an internal class cannot break the list.
//
//   php nextcloud-apps-list.php <webroot>   with app:list's JSON on stdin
//
// Prints one JSON array, an object per app:
//   {"id","name","summary","version","kind","enabled","alwaysEnabled"}
// `kind` is where the app lives: "core" (shipped with Nextcloud, apps/),
// "bundled" (shipped with LosOS, nix-apps/) or "store" (added from the app
// store, store-apps/, the only folder Nextcloud can write or delete in).

if ($argc !== 2) {
    fwrite(STDERR, "usage: nextcloud-apps-list.php <webroot>\n");
    exit(2);
}
$root = rtrim($argv[1], '/');

$state = json_decode(stream_get_contents(STDIN), true);
if (!is_array($state)) {
    fwrite(STDERR, "losos-nextcloud-apps: occ app:list printed no JSON\n");
    exit(1);
}
$enabled = is_array($state['enabled'] ?? null) ? $state['enabled'] : [];

$shipped = json_decode((string) @file_get_contents("$root/core/shipped.json"), true);
$always = is_array($shipped['alwaysEnabled'] ?? null) ? $shipped['alwaysEnabled'] : [];

// The English text of an element that may be repeated per language: the copy
// with no lang attribute or lang="en", else the first one.
function english($nodes): string
{
    $first = null;
    foreach ($nodes as $node) {
        $lang = (string) ($node['lang'] ?? '');
        $text = trim(preg_replace('/\s+/', ' ', (string) $node));
        if ($lang === '' || $lang === 'en') {
            return $text;
        }
        $first ??= $text;
    }
    return $first ?? '';
}

$apps = [];
foreach (['apps' => 'core', 'nix-apps' => 'bundled', 'store-apps' => 'store'] as $dir => $kind) {
    foreach (glob("$root/$dir/*/appinfo/info.xml") ?: [] as $file) {
        $xml = @simplexml_load_file($file);
        if ($xml === false) {
            continue;
        }
        $id = trim((string) $xml->id);
        // Nextcloud takes the first folder that has an app, in this order.
        if ($id === '' || isset($apps[$id])) {
            continue;
        }
        $name = english($xml->name);
        $apps[$id] = [
            'id' => $id,
            'name' => $name === '' ? $id : $name,
            'summary' => english($xml->summary),
            'version' => trim((string) $xml->version),
            'kind' => $kind,
            'enabled' => array_key_exists($id, $enabled),
            'alwaysEnabled' => in_array($id, $always, true),
        ];
    }
}

echo json_encode(array_values($apps), JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE), "\n";
