//! The screens, and how each is written in the address bar (`#/explore`). The page never
//! reloads: a link only changes the part after `#`.

/// One screen of the site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Route {
    #[default]
    Home,
    Explore,
    Bookmarks,
    Encode,
    Decode,
    Library,
    About,
}

impl Route {
    /// The screens in the top bar, in order. Home is the logo.
    pub const NAV: [Route; 6] = [
        Route::Explore,
        Route::Bookmarks,
        Route::Encode,
        Route::Decode,
        Route::Library,
        Route::About,
    ];

    /// The name shown in the top bar and the home menu.
    pub const fn name(self) -> &'static str {
        match self {
            Route::Home => "Home",
            Route::Explore => "Explore",
            Route::Bookmarks => "Bookmarks",
            Route::Encode => "Encode",
            Route::Decode => "Decode",
            Route::Library => "Library",
            Route::About => "About",
        }
    }

    /// The address part after `#/`. Empty for Home.
    const fn path(self) -> &'static str {
        match self {
            Route::Home => "",
            Route::Explore => "explore",
            Route::Bookmarks => "bookmarks",
            Route::Encode => "encode",
            Route::Decode => "decode",
            Route::Library => "library",
            Route::About => "about",
        }
    }

    /// The link that opens this screen: `#/explore`.
    pub fn href(self) -> String {
        format!("#/{}", self.path())
    }

    /// The screen for an address part like `#/explore`. Anything unknown is Home.
    pub fn from_hash(hash: &str) -> Self {
        let path = hash.trim_start_matches('#').trim_start_matches('/');
        [Route::Home]
            .into_iter()
            .chain(Route::NAV)
            .find(|route| route.path() == path)
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn every_screen_round_trips_through_its_link() {
        for route in [Route::Home].into_iter().chain(Route::NAV) {
            assert_eq!(Route::from_hash(&route.href()), route);
        }
    }

    #[test]
    fn links_look_like_this() {
        assert_eq!(Route::Home.href(), "#/");
        assert_eq!(Route::Explore.href(), "#/explore");
    }

    #[test]
    fn an_empty_or_unknown_address_is_home() {
        assert_eq!(Route::from_hash(""), Route::Home);
        assert_eq!(Route::from_hash("#"), Route::Home);
        assert_eq!(Route::from_hash("#/nowhere"), Route::Home);
        assert_eq!(Route::from_hash("#/Explore"), Route::Home);
    }
}
