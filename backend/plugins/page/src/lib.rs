//! The pages lososd's `basic` plugin draws for a browser that runs no
//! JavaScript, in the admin UI's own markup and its built stylesheet, so
//! the two look alike: an HTML builder ([`html`]), the catalogue's text
//! ([`i18n`]), the icons ([`icons`]), the components ([`ui`]) and the
//! frame around a page ([`shell`]).

pub mod html;
pub mod i18n;
pub mod icons;
pub mod shell;
pub mod ui;
