//! The Corsair devices Nexus Open knows, and what it does with each. The
//! same on every OS; finding them is per OS (`hidraw::scan` on Linux).

/// Corsair's USB vendor id.
pub const CORSAIR: u16 = 0x1b1c;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Support {
    /// Works now, doing this.
    Supported(&'static str),
    /// Planned, for this; not driven yet.
    Planned(&'static str),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Product {
    pub id: u16,
    pub name: &'static str,
    /// The USB interfaces the app opens. Others, such as a keyboard's
    /// typing interface, are left alone.
    pub interfaces: &'static [u8],
    pub support: Support,
}

pub const PRODUCTS: [Product; 4] = [
    Product {
        id: 0x1b8e,
        name: "iCUE Nexus",
        interfaces: &[0],
        support: Support::Supported("the touch strip"),
    },
    Product {
        id: 0x0c39,
        name: "LCD cap for Elite Capellix coolers",
        interfaces: &[0],
        support: Support::Planned("its screen"),
    },
    Product {
        id: 0x0c32,
        name: "iCUE Commander Core",
        interfaces: &[0, 1],
        support: Support::Planned("cooling: pump, fans and readings"),
    },
    Product {
        id: 0x1b7c,
        name: "K100 RGB keyboard",
        interfaces: &[1, 2],
        support: Support::Planned("lighting, G-keys and the dial"),
    },
];

/// What the app knows of a Corsair product id.
pub fn product(id: u16) -> Option<&'static Product> {
    PRODUCTS.iter().find(|p| p.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_the_keyboards_typing_interface_is_left_alone() {
        for (i, p) in PRODUCTS.iter().enumerate() {
            assert!(PRODUCTS[i + 1..].iter().all(|q| q.id != p.id), "{}", p.name);
            assert!(!p.interfaces.is_empty(), "{}", p.name);
        }
        assert!(!product(0x1b7c).unwrap().interfaces.contains(&0));
        assert_eq!(product(0xffff), None);
    }

    /// The udev rule opens exactly the interfaces listed here.
    #[test]
    fn the_udev_rule_covers_each_product() {
        let rule = include_str!("../../../data/udev/70-nexus.rules").replace("\r\n", "\n");
        for p in PRODUCTS {
            let id = format!("{:04x}", p.id);
            assert!(rule.contains(&id), "the rule has no line for {}", p.name);
        }
        assert!(
            rule.contains(r#"ENV{ID_USB_MODEL_ID}=="1b7c", ENV{ID_USB_INTERFACE_NUM}=="01|02""#)
        );
    }
}
