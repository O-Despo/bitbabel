//! Light or dark. The choice is remembered by name.

/// The two themes. The default is light.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Theme {
    #[default]
    Light,
    Dark,
}

impl Theme {
    /// `light` or `dark`: what is stored, and the value of the page's `data-theme`.
    pub const fn name(self) -> &'static str {
        match self {
            Theme::Light => "light",
            Theme::Dark => "dark",
        }
    }

    /// The theme with this exact name, or `None`.
    pub fn from_name(name: &str) -> Option<Self> {
        [Theme::Light, Theme::Dark]
            .into_iter()
            .find(|theme| theme.name() == name)
    }

    /// The other theme.
    pub const fn toggled(self) -> Self {
        match self {
            Theme::Light => Theme::Dark,
            Theme::Dark => Theme::Light,
        }
    }

    /// The text of the button that switches to the other theme: `dark mode` while light.
    pub const fn switch_label(self) -> &'static str {
        match self {
            Theme::Light => "dark mode",
            Theme::Dark => "light mode",
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn names_round_trip() {
        for theme in [Theme::Light, Theme::Dark] {
            assert_eq!(Theme::from_name(theme.name()), Some(theme));
        }
        assert_eq!(Theme::from_name("Dark"), None);
        assert_eq!(Theme::from_name(""), None);
    }

    #[test]
    fn toggling_twice_is_the_same_theme() {
        assert_eq!(Theme::Light.toggled(), Theme::Dark);
        assert_eq!(Theme::Light.toggled().toggled(), Theme::Light);
    }

    #[test]
    fn the_button_names_the_theme_it_switches_to() {
        assert_eq!(Theme::Light.switch_label(), "dark mode");
        assert_eq!(Theme::Dark.switch_label(), "light mode");
    }
}
