# Profiles

Each person on LosOS Login has one profile: a picture, a banner, a colour,
their pronouns and a line about themselves. lososd keeps it
(`backend/src/profiles.rs`) and every page that shows a person reads it from
there, so a picture changed once shows everywhere. This note is the contract
for the box's own apps.

## What a profile is

```json
{
  "name": "eva",
  "displayName": "Eva Novak",
  "pronouns": "she/her",
  "about": "Keeps the family photos in order.",
  "accent": "rose",
  "accentChosen": true,
  "picture": "/api/profiles/eva/picture?v=3",
  "banner": "/api/profiles/eva/banner?v=4"
}
```

- `name` is the sign-in name, the key everywhere. Show it beside the picture
  (`@eva`), so a picture never stands in for who someone is.
- `displayName` comes from the people file; it falls back to `name`.
- `accent` is one of `teal`, `blue`, `violet`, `rose`, `amber`, `green`.
  When the person has not picked one (`accentChosen: false`) it is the colour
  their name gives them, the same on every page. Draw it with the tokens
  `--person-<accent>` (initials, rings) and `--person-<accent>-wash`
  (initials' ground, a missing banner) from `tokens.css`, which has light
  and dark values for each.
- `picture` and `banner` are URLs on the box's own origin, or `null`. A new
  picture is a new `?v=`, so the bytes can be cached for good.
- The email address is never part of a profile.

With no picture, draw the initials (first letter of the first and last word
of `displayName`) on the wash in the strong colour. With no banner, fill the
banner's place with the wash. `admin-ui/app/src/components/person.tsx` does
both and is the reference.

## Reading

| Route | Who | Answer |
|---|---|---|
| `GET /api/profiles` | anyone signed in | `{"people": [profile, ...]}`, everyone LosOS Login lets in |
| `GET /api/profiles/{name}` | anyone signed in | one profile, 404 for nobody |
| `GET /api/profiles/{name}/picture?v=N` | anyone signed in | the picture's bytes |
| `GET /api/profiles/{name}/banner?v=N` | anyone signed in | the banner's bytes |
| `GET /api/me` | the person | carries `picture` and `accent` too |

A browser page on the box's origin uses the URLs as they are: nginx asks
LosOS Login with the session cookie, as for every `/api/` call. A backend
on the box asks lososd on `127.0.0.1:8082` with the admin token and
`X-Losos-As: <name>` of the person it acts for, the way
`/api/access/call` replays a call.

Pictures are 512 by 512 at most, banners 1500 by 500 at most (a smaller
upload keeps its size). Each is a JPEG, or a PNG when it has transparent
pixels.

## Changing

Only the person changes their own profile, and only an administrator takes
someone else's picture down.

| Route | Body | Answer |
|---|---|---|
| `POST /api/account/profile` | `{"pronouns", "about", "accent"}`, each optional; `"accent": null` goes back to the name's colour | the profile |
| `PUT /api/account/picture` | the picture's bytes (PNG, JPEG, GIF or WebP, up to 8 MiB) | the profile |
| `DELETE /api/account/picture` | | the profile |
| `PUT /api/account/banner` | the banner's bytes | the profile |
| `DELETE /api/account/banner` | | the profile |
| `POST /api/people/profile/clear` | `{"name", "kind": "picture" or "banner"}`, administrators only | `{"name", "kind", "cleared": true}` |

A refused change is a 400 with the reason in `error`, in words a page can
show as they are: a file that is not a picture, a picture larger than 8192
pixels on a side, pronouns over 40 characters, a line over 160.

lososd never keeps an upload as it arrived. It decodes it, turns it upright
by its orientation tag, crops the middle to the shape, scales it down and
encodes it again. The camera's tags, the place a photo was taken among
them, do not survive.

A person taken off the box loses their profile with their account.
